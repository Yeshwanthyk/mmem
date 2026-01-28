//! Health check diagnostics for mmem configuration.
//!
//! The doctor command checks:
//! - Sessions root directory exists
//! - Database file exists and is readable
//! - Schema is valid and queryable
//! - FTS5 extension is available
//!
//! # Key Functions
//!
//! - [`run_doctor`]: Generate a diagnostic report

use crate::index::init_schema;
use crate::stats::load_stats;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

#[derive(Debug, serde::Serialize)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub root_exists: bool,
    pub db_path: PathBuf,
    pub db_exists: bool,
    pub schema_ok: bool,
    pub schema_error: Option<String>,
    pub fts5_available: bool,
    pub indexed_sessions: i64,
    pub newest_message_at: Option<String>,
}

pub fn run_doctor(db_path: &Path, root: &Path) -> DoctorReport {
    let root_exists = root.is_dir();
    let db_exists = db_path.exists();

    let fts5_available = Connection::open_in_memory()
        .ok()
        .and_then(|conn| init_schema(&conn).ok())
        .is_some();

    let mut schema_ok = false;
    let mut schema_error = None;
    let mut indexed_sessions = 0;
    let mut newest_message_at = None;

    if db_exists {
        match Connection::open(db_path) {
            Ok(conn) => match load_stats(&conn) {
                Ok(stats) => {
                    schema_ok = true;
                    indexed_sessions = stats.session_count;
                    newest_message_at = stats.newest_message_at;
                }
                Err(err) => {
                    schema_error = Some(err.to_string());
                }
            },
            Err(err) => {
                schema_error = Some(err.to_string());
            }
        }
    }

    DoctorReport {
        root: root.to_path_buf(),
        root_exists,
        db_path: db_path.to_path_buf(),
        db_exists,
        schema_ok,
        schema_error,
        fts5_available,
        indexed_sessions,
        newest_message_at,
    }
}
