use crate::model::{ParsedMessage, ParsedSession};
use serde_json::Value;

const MAX_SNIPPET_LEN: usize = 240;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid json: {source}")]
    InvalidJson { source: serde_json::Error },
    #[error("invalid jsonl at line {line}: {source}")]
    InvalidJsonl {
        line: usize,
        source: serde_json::Error,
    },
}

#[derive(Debug, Default)]
struct Meta {
    created_at: Option<String>,
    last_message_at: Option<String>,
    agent: Option<String>,
    workspace: Option<String>,
}

pub fn extract_message(value: &Value) -> Option<ParsedMessage> {
    if let Some(message) = format_session_entry(value) {
        return Some(message);
    }

    if has_tool_call(value) {
        return Some(ParsedMessage {
            role: extract_role(value),
            text: String::new(),
            timestamp: extract_timestamp(value),
        });
    }

    None
}

fn has_tool_call(value: &Value) -> bool {
    let Some(content) = extract_content_array(value) else {
        return false;
    };

    content.iter().any(|item| {
        item.get("type")
            .and_then(|t| t.as_str())
            .map(|t| t == "toolCall")
            .unwrap_or(false)
    })
}

fn extract_content_array(value: &Value) -> Option<&Vec<Value>> {
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

fn extract_role(value: &Value) -> Option<String> {
    value
        .get("message")
        .and_then(|m| m.get("role"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            value
                .get("payload")
                .and_then(|p| p.get("role"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| value.get("role").and_then(|v| v.as_str()))
        .map(normalize_role)
}

pub fn parse_jsonl(input: &str) -> Result<ParsedSession, ParseError> {
    let mut meta = Meta::default();
    let mut messages = Vec::new();

    for (idx, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(line).map_err(|e| ParseError::InvalidJsonl {
            line: idx + 1,
            source: e,
        })?;

        update_meta_from_value(&mut meta, &value);
        if let Some(message) = format_session_entry(&value) {
            messages.push(message);
        }
    }

    Ok(build_parsed_session(messages, meta))
}

pub fn parse_json(input: &str) -> Result<ParsedSession, ParseError> {
    let root: Value =
        serde_json::from_str(input).map_err(|e| ParseError::InvalidJson { source: e })?;

    let mut meta = Meta::default();
    update_meta_from_value(&mut meta, &root);

    let entries: Vec<&Value> = match &root {
        Value::Array(items) => items.iter().collect(),
        Value::Object(map) => {
            if let Some(Value::Array(messages)) = map.get("messages") {
                messages.iter().collect()
            } else if let Some(Value::Array(events)) = map.get("events") {
                events.iter().collect()
            } else {
                vec![&root]
            }
        }
        _ => Vec::new(),
    };

    let mut messages = Vec::new();
    for entry in entries {
        update_meta_from_value(&mut meta, entry);
        if let Some(message) = format_session_entry(entry) {
            messages.push(message);
        }
    }

    Ok(build_parsed_session(messages, meta))
}

pub fn parse_markdown(input: &str) -> ParsedSession {
    let mut messages = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (role, text) = match split_role_prefix(line) {
            Some((role, text)) => (Some(role), text),
            None => (None, line.to_string()),
        };

        if text.trim().is_empty() {
            continue;
        }

        messages.push(ParsedMessage {
            role,
            text,
            timestamp: None,
        });
    }

    build_parsed_session(messages, Meta::default())
}

fn build_parsed_session(messages: Vec<ParsedMessage>, mut meta: Meta) -> ParsedSession {
    if meta.created_at.is_none() {
        meta.created_at = messages.first().and_then(|m| m.timestamp.clone());
    }
    if meta.last_message_at.is_none() {
        meta.last_message_at = messages.last().and_then(|m| m.timestamp.clone());
    }

    let content_lines: Vec<String> = messages
        .iter()
        .map(|message| format_message_line(&message.role, &message.text))
        .collect();
    let content = content_lines.join("\n");

    let title =
        first_user_title(&messages).or_else(|| messages.first().map(|m| m.text.trim().to_string()));

    ParsedSession {
        created_at: meta.created_at,
        last_message_at: meta.last_message_at,
        agent: meta.agent,
        workspace: meta.workspace,
        title,
        message_count: messages.len(),
        snippet: make_snippet(&content),
        content,
        messages,
    }
}

fn format_session_entry(value: &Value) -> Option<ParsedMessage> {
    if value
        .get("type")
        .and_then(|v| v.as_str())
        .map(|v| v == "session_meta")
        .unwrap_or(false)
    {
        return None;
    }

    if value
        .get("type")
        .and_then(|v| v.as_str())
        .map(|v| v == "response_item")
        .unwrap_or(false)
        && let Some(payload) = value.get("payload")
        && let Some(mut message) = message_from_object(payload)
    {
        if message.timestamp.is_none() {
            message.timestamp = extract_timestamp(value);
        }
        return Some(message);
    }

    if let Some(message_value) = value.get("message") {
        if message_value.is_object() {
            if let Some(mut message) = message_from_object(message_value) {
                if message.role.is_none() {
                    message.role = value
                        .get("role")
                        .and_then(|v| v.as_str())
                        .map(normalize_role);
                }
                if message.timestamp.is_none() {
                    message.timestamp = extract_timestamp(value);
                }
                return Some(message);
            }
        } else if let Some(text) = coerce_content(message_value) {
            return Some(ParsedMessage {
                role: value
                    .get("role")
                    .and_then(|v| v.as_str())
                    .map(normalize_role),
                text: text.trim().to_string(),
                timestamp: extract_timestamp(value),
            });
        }
    }

    if let Some(mut message) = message_from_object(value) {
        if message.timestamp.is_none() {
            message.timestamp = extract_timestamp(value);
        }
        return Some(message);
    }

    None
}

fn message_from_object(value: &Value) -> Option<ParsedMessage> {
    let role = value
        .get("role")
        .and_then(|v| v.as_str())
        .map(normalize_role);

    let content = value
        .get("content")
        .and_then(coerce_content)
        .or_else(|| value.get("text").and_then(coerce_content))
        .or_else(|| value.get("message").and_then(coerce_content));

    let text = content?.trim().to_string();
    if text.is_empty() {
        return None;
    }

    Some(ParsedMessage {
        role,
        text,
        timestamp: extract_timestamp(value),
    })
}

fn coerce_content(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }

    if let Some(array) = value.as_array() {
        let parts: Vec<String> = array.iter().filter_map(coerce_content).collect();
        return if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        };
    }

    if value
        .get("type")
        .and_then(|v| v.as_str())
        .map(|v| v == "input_text")
        .unwrap_or(false)
        && let Some(text) = value.get("text").and_then(|v| v.as_str())
    {
        let trimmed = text.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }

    if let Some(content) = value.get("content") {
        return coerce_content(content);
    }

    if let Some(text) = value.get("text") {
        return coerce_content(text);
    }

    None
}

fn extract_timestamp(value: &Value) -> Option<String> {
    extract_string_field(value, "created_at")
        .or_else(|| extract_string_field(value, "timestamp"))
        .or_else(|| extract_string_field(value, "time"))
        .or_else(|| extract_string_field(value, "ts"))
}

fn extract_string_field(value: &Value, key: &str) -> Option<String> {
    let field = value.get(key)?;
    if let Some(text) = field.as_str() {
        let trimmed = text.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    if let Some(num) = field.as_i64() {
        return Some(num.to_string());
    }
    if let Some(num) = field.as_u64() {
        return Some(num.to_string());
    }
    if let Some(num) = field.as_f64() {
        return Some(num.to_string());
    }
    None
}

fn update_meta_from_value(meta: &mut Meta, value: &Value) {
    let Some(object) = value.as_object() else {
        return;
    };

    maybe_set(
        &mut meta.agent,
        object
            .get("agent")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
    );
    maybe_set(
        &mut meta.workspace,
        object
            .get("workspace")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
    );

    if meta.created_at.is_none() {
        meta.created_at = extract_string_field(value, "created_at");
    }

    if meta.last_message_at.is_none() {
        meta.last_message_at = extract_string_field(value, "last_message_at");
    } else if let Some(value) = extract_string_field(value, "last_message_at") {
        meta.last_message_at = Some(value);
    }
}

fn maybe_set(target: &mut Option<String>, value: Option<String>) {
    if target.is_none() {
        *target = value;
    }
}

fn format_message_line(role: &Option<String>, text: &str) -> String {
    match role {
        Some(role) => format!("[{}] {}", role, text),
        None => text.to_string(),
    }
}

fn normalize_role(role: &str) -> String {
    role.trim().to_lowercase()
}

fn first_user_title(messages: &[ParsedMessage]) -> Option<String> {
    messages.iter().find_map(|message| {
        if message.role.as_deref() == Some("user") {
            let title = message.text.trim();
            if title.is_empty() {
                None
            } else {
                Some(title.to_string())
            }
        } else {
            None
        }
    })
}

fn make_snippet(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let snippet: String = trimmed.chars().take(MAX_SNIPPET_LEN).collect();
    snippet
}

fn split_role_prefix(line: &str) -> Option<(String, String)> {
    let (role, text) = line.split_once(':')?;
    let role = role.trim();
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    if matches_role(role) {
        Some((normalize_role(role), text.to_string()))
    } else {
        None
    }
}

fn matches_role(role: &str) -> bool {
    matches!(
        role.to_lowercase().as_str(),
        "user" | "assistant" | "system" | "developer" | "tool"
    )
}
