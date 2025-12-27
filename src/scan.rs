use crate::index::{load_indexed_sessions, remove_session, upsert_session};
use crate::model::ParsedSession;
use crate::parse::{parse_json, parse_jsonl, parse_markdown};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
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

pub fn index_root(conn: &mut Connection, root: &Path, full: bool) -> Result<ScanStats, ScanError> {
    let mut stats = ScanStats::default();

    let existing = load_indexed_sessions(conn)?;
    let mut existing_map = HashMap::new();
    for entry in existing {
        existing_map.insert(entry.path, (entry.mtime, entry.size));
    }

    let mut seen = HashSet::new();

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

        let record = parsed.into_record(path_str, mtime, size, None);
        upsert_session(conn, &record)?;
        stats.indexed += 1;
    }

    for (path, _) in existing_map {
        if !seen.contains(&path) {
            remove_session(conn, &path)?;
            stats.removed += 1;
        }
    }

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
