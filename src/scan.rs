use crate::index::{
    load_indexed_sessions, remove_session_tx, replace_messages_tx, upsert_session_tx,
};
use crate::model::{MessageRecord, ParsedSession};
use crate::parse::{parse_json, parse_jsonl, parse_markdown};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[derive(Debug, Default, serde::Serialize)]
pub struct ScanStats {
    pub scanned: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub removed: usize,
    pub parse_errors: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("io error: {source}")]
    Io { source: std::io::Error },
    #[error("walk error: {source}")]
    Walk { source: walkdir::Error },
    #[error("system time error for {path}: {source}")]
    Time {
        path: PathBuf,
        source: std::time::SystemTimeError,
    },
    #[error("index error: {source}")]
    Index { source: crate::index::IndexError },
}

impl From<std::io::Error> for ScanError {
    fn from(source: std::io::Error) -> Self {
        Self::Io { source }
    }
}

impl From<walkdir::Error> for ScanError {
    fn from(source: walkdir::Error) -> Self {
        Self::Walk { source }
    }
}

impl From<crate::index::IndexError> for ScanError {
    fn from(source: crate::index::IndexError) -> Self {
        Self::Index { source }
    }
}

impl From<rusqlite::Error> for ScanError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Index {
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct RepoInfo {
    repo_root: Option<String>,
    repo_name: Option<String>,
    branch: Option<String>,
}

pub fn index_root(conn: &mut Connection, root: &Path, full: bool) -> Result<ScanStats, ScanError> {
    let mut stats = ScanStats::default();

    let existing = load_indexed_sessions(conn)?;
    let mut existing_map = HashMap::new();
    for entry in existing {
        existing_map.insert(entry.path, (entry.mtime, entry.size));
    }

    let mut seen = HashSet::new();
    let mut repo_cache: HashMap<PathBuf, RepoInfo> = HashMap::new();
    let tx = conn.transaction()?;

    for entry in WalkDir::new(root) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let Some(ext) = entry.path().extension().and_then(|ext| ext.to_str()) else {
            continue;
        };

        let ext = ext.to_ascii_lowercase();
        if !matches!(ext.as_str(), "jsonl" | "json" | "md") {
            continue;
        }

        stats.scanned += 1;

        let path = entry.path().to_path_buf();
        let path_str = path.to_string_lossy().to_string();
        seen.insert(path_str.clone());

        let metadata = entry.metadata()?;
        let mtime = modified_to_unix(&path, &metadata)?;
        let size = metadata.len() as i64;

        if !full
            && let Some((prev_mtime, prev_size)) = existing_map.get(&path_str)
            && *prev_mtime == mtime
            && *prev_size == size
        {
            stats.skipped += 1;
            continue;
        }

        let contents = std::fs::read_to_string(&path)?;
        let parsed = match parse_by_extension(&ext, &contents) {
            Ok(parsed) => parsed,
            Err(_) => {
                stats.parse_errors += 1;
                continue;
            }
        };

        let (mut record, messages) = parsed.into_parts(path_str, mtime, size, None);
        let workspace_path = workspace_path_from_meta(record.workspace.as_deref())
            .or_else(|| decode_workspace_from_session_path(&path));
        let repo_info = infer_repo_info(workspace_path.as_deref(), &mut repo_cache);
        record.repo_root = repo_info.repo_root;
        record.repo_name = repo_info.repo_name;
        record.branch = repo_info.branch;

        let message_records: Vec<MessageRecord> = messages
            .into_iter()
            .enumerate()
            .map(|(idx, message)| MessageRecord {
                turn_index: idx as i64,
                role: message.role,
                timestamp: message.timestamp,
                text: message.text,
            })
            .collect();

        upsert_session_tx(&tx, &record)?;
        replace_messages_tx(&tx, &record.path, &message_records)?;
        stats.indexed += 1;
    }

    for (path, _) in existing_map {
        if !seen.contains(&path) {
            remove_session_tx(&tx, &path)?;
            stats.removed += 1;
        }
    }

    tx.commit()?;
    Ok(stats)
}

fn parse_by_extension(
    ext: &str,
    contents: &str,
) -> Result<ParsedSession, crate::parse::ParseError> {
    match ext {
        "jsonl" => parse_jsonl(contents),
        "json" => parse_json(contents),
        "md" => Ok(parse_markdown(contents)),
        _ => Ok(ParsedSession::empty()),
    }
}

fn modified_to_unix(path: &Path, metadata: &std::fs::Metadata) -> Result<i64, ScanError> {
    let modified = metadata.modified()?;
    let duration = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|source| ScanError::Time {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(duration.as_secs() as i64)
}

fn workspace_path_from_meta(workspace: Option<&str>) -> Option<PathBuf> {
    let workspace = workspace?;
    let expanded = expand_home(workspace)?;
    if expanded.is_dir() {
        Some(expanded)
    } else {
        None
    }
}

fn expand_home(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        let home = std::env::var_os("HOME")?;
        return Some(PathBuf::from(home).join(stripped));
    }
    Some(PathBuf::from(path))
}

fn decode_workspace_from_session_path(session_path: &Path) -> Option<PathBuf> {
    let parent = session_path.parent()?;
    let component = parent.file_name()?.to_str()?;
    decode_workspace_component(component)
}

fn decode_workspace_component(component: &str) -> Option<PathBuf> {
    if !component.contains("--") {
        return None;
    }

    let mut decoded = component.replace("--", "/");
    while decoded.contains("//") {
        decoded = decoded.replace("//", "/");
    }
    if !decoded.starts_with('/') {
        decoded.insert(0, '/');
    }

    let path = PathBuf::from(decoded);
    if path.is_dir() { Some(path) } else { None }
}

fn infer_repo_info(workspace: Option<&Path>, cache: &mut HashMap<PathBuf, RepoInfo>) -> RepoInfo {
    let Some(workspace) = workspace else {
        return RepoInfo::default();
    };

    if let Some(info) = cache.get(workspace) {
        return info.clone();
    }

    let repo_root = git_output(workspace, &["rev-parse", "--show-toplevel"])
        .and_then(|root| PathBuf::from(root).canonicalize().ok())
        .filter(|path| path.is_dir());

    let branch = repo_root
        .as_ref()
        .and_then(|root| git_output(root, &["rev-parse", "--abbrev-ref", "HEAD"]))
        .filter(|name| name != "HEAD");

    let repo_name = repo_root
        .as_ref()
        .and_then(|root| root.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.to_string());

    let info = RepoInfo {
        repo_root: repo_root
            .as_ref()
            .map(|root| root.to_string_lossy().to_string()),
        repo_name,
        branch,
    };

    cache.insert(workspace.to_path_buf(), info.clone());
    info
}

fn git_output(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}
