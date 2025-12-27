use crate::model::ParsedMessage;
use crate::parse::extract_message;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub line: usize,
    pub message_index: Option<usize>,
    pub role: Option<String>,
    pub timestamp: Option<String>,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone)]
pub struct ToolCallMatch {
    pub line: usize,
    pub message_index: Option<usize>,
    pub tool: ToolCall,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("io error: {source}")]
    Io { source: std::io::Error },
    #[error("invalid json at line {line}: {source}")]
    InvalidJson {
        line: usize,
        source: serde_json::Error,
    },
    #[error("unsupported session format: {path}")]
    UnsupportedFormat { path: PathBuf },
    #[error("turn {turn} out of range (messages: {available})")]
    TurnOutOfRange { turn: usize, available: usize },
    #[error("line {line} out of range")]
    LineOutOfRange { line: usize },
}

impl From<std::io::Error> for SessionError {
    fn from(source: std::io::Error) -> Self {
        Self::Io { source }
    }
}

pub fn load_entry_by_turn(path: &Path, turn: usize) -> Result<SessionEntry, SessionError> {
    ensure_jsonl(path)?;

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut message_index = 0usize;

    for (line_idx, line) in reader.lines().enumerate() {
        let line_no = line_idx + 1;
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(line).map_err(|err| SessionError::InvalidJson {
            line: line_no,
            source: err,
        })?;

        if let Some(parsed) = extract_message(&value) {
            if message_index == turn {
                return Ok(build_entry(
                    value,
                    line_no,
                    Some(message_index),
                    Some(parsed),
                ));
            }
            message_index += 1;
        }
    }

    Err(SessionError::TurnOutOfRange {
        turn,
        available: message_index,
    })
}

pub fn load_entry_by_line(path: &Path, line: usize) -> Result<SessionEntry, SessionError> {
    ensure_jsonl(path)?;

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    for (line_idx, line_value) in reader.lines().enumerate() {
        let line_no = line_idx + 1;
        if line_no != line {
            continue;
        }
        let line_value = line_value?;
        let trimmed = line_value.trim();
        if trimmed.is_empty() {
            return Err(SessionError::LineOutOfRange { line });
        }

        let value: Value =
            serde_json::from_str(trimmed).map_err(|err| SessionError::InvalidJson {
                line: line_no,
                source: err,
            })?;
        let parsed = extract_message(&value);
        let message_index = None;
        return Ok(build_entry(value, line_no, message_index, parsed));
    }

    Err(SessionError::LineOutOfRange { line })
}

pub fn scan_tool_calls(
    path: &Path,
    tool: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<ToolCallMatch>, SessionError> {
    ensure_jsonl(path)?;

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut message_index = 0usize;
    let mut matches = Vec::new();
    let max_matches = limit.unwrap_or(usize::MAX);

    for (line_idx, line) in reader.lines().enumerate() {
        let line_no = line_idx + 1;
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(line).map_err(|err| SessionError::InvalidJson {
            line: line_no,
            source: err,
        })?;

        let parsed = extract_message(&value);
        let message_index_opt = parsed.as_ref().map(|_| message_index);
        let tool_calls = extract_tool_calls(&value);
        for tool_call in tool_calls {
            if let Some(filter) = tool
                && !tool_call.name.eq_ignore_ascii_case(filter)
            {
                continue;
            }
            matches.push(ToolCallMatch {
                line: line_no,
                message_index: message_index_opt,
                tool: tool_call,
            });
            if matches.len() >= max_matches {
                return Ok(matches);
            }
        }

        if parsed.is_some() {
            message_index += 1;
        }
    }

    Ok(matches)
}

pub fn extract_tool_calls(value: &Value) -> Vec<ToolCall> {
    let Some(content) = message_content(value) else {
        return Vec::new();
    };

    let mut tools = Vec::new();
    for item in content {
        let Some(item_type) = item.get("type").and_then(|t| t.as_str()) else {
            continue;
        };
        if item_type != "toolCall" {
            continue;
        }

        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let arguments = item.get("arguments").cloned().unwrap_or(Value::Null);
        tools.push(ToolCall { name, arguments });
    }

    tools
}

fn build_entry(
    value: Value,
    line: usize,
    message_index: Option<usize>,
    parsed: Option<ParsedMessage>,
) -> SessionEntry {
    let (role, timestamp) = parsed
        .map(|message| (message.role, message.timestamp))
        .unwrap_or((None, None));

    SessionEntry {
        line,
        message_index,
        role,
        timestamp,
        value,
    }
}

fn message_content(value: &Value) -> Option<&Vec<Value>> {
    if let Some(message) = value.get("message")
        && let Some(content) = message.get("content").and_then(|v| v.as_array())
    {
        return Some(content);
    }

    if value
        .get("type")
        .and_then(|v| v.as_str())
        .map(|v| v == "response_item")
        .unwrap_or(false)
        && let Some(payload) = value.get("payload")
        && payload
            .get("type")
            .and_then(|v| v.as_str())
            .map(|v| v == "message")
            .unwrap_or(false)
        && let Some(content) = payload.get("content").and_then(|v| v.as_array())
    {
        return Some(content);
    }

    value.get("content").and_then(|v| v.as_array())
}

fn ensure_jsonl(path: &Path) -> Result<(), SessionError> {
    if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
        return Ok(());
    }

    Err(SessionError::UnsupportedFormat {
        path: path.to_path_buf(),
    })
}
