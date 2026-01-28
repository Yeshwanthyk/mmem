//! SQLite schema management and CRUD operations.
//!
//! This module defines the database schema and provides functions for
//! upserting/removing sessions and messages, with FTS5 index maintenance.
//!
//! # Schema
//!
//! - `sessions`: Session metadata (path, agent, workspace, timestamps, etc.)
//! - `sessions_fts`: FTS5 index of session content
//! - `messages`: Individual messages with turn indices
//! - `messages_fts`: FTS5 index of message text
//!
//! # Key Functions
//!
//! - [`init_schema`]: Create tables and indexes
//! - [`configure_connection`]: Set WAL mode, busy timeout, etc.
//! - [`upsert_session`] / [`upsert_session_tx`]: Insert or update a session
//! - [`replace_messages_tx`]: Replace all messages for a session
//! - [`remove_session`] / [`remove_session_tx`]: Delete a session and its messages
//!
//! # Transaction Pattern
//!
//! Functions with `_tx` suffix operate within an existing transaction.
//! Non-`_tx` variants create their own transaction.

use crate::model::{MessageRecord, SessionRecord};
use rusqlite::{Connection, Transaction, params};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
  path TEXT PRIMARY KEY,
  mtime INTEGER NOT NULL,
  size INTEGER NOT NULL,
  hash TEXT,
  created_at TEXT,
  last_message_at TEXT,
  agent TEXT,
  workspace TEXT,
  title TEXT,
  message_count INTEGER,
  snippet TEXT,
  repo_root TEXT,
  repo_name TEXT,
  branch TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
  content,
  path UNINDEXED
);

CREATE TABLE IF NOT EXISTS messages (
  id INTEGER PRIMARY KEY,
  session_path TEXT NOT NULL,
  turn_index INTEGER NOT NULL,
  role TEXT,
  timestamp TEXT,
  text TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
  text,
  message_id UNINDEXED,
  session_path UNINDEXED,
  role UNINDEXED
);

CREATE INDEX IF NOT EXISTS idx_sessions_last_message_at ON sessions(last_message_at);
CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent);
CREATE INDEX IF NOT EXISTS idx_sessions_workspace ON sessions(workspace);
CREATE INDEX IF NOT EXISTS idx_sessions_repo_name ON sessions(repo_name);
CREATE INDEX IF NOT EXISTS idx_sessions_branch ON sessions(branch);
CREATE INDEX IF NOT EXISTS idx_messages_session_turn ON messages(session_path, turn_index);
"#;

#[derive(Debug, Clone)]
pub struct IndexedSession {
    pub path: String,
    pub mtime: i64,
    pub size: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("sqlite error: {source}")]
    Sqlite { source: rusqlite::Error },
}

impl From<rusqlite::Error> for IndexError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Sqlite { source }
    }
}

pub fn init_schema(conn: &Connection) -> Result<(), IndexError> {
    conn.execute_batch(SCHEMA)?;
    ensure_column(conn, "sessions", "repo_root", "TEXT")?;
    ensure_column(conn, "sessions", "repo_name", "TEXT")?;
    ensure_column(conn, "sessions", "branch", "TEXT")?;
    Ok(())
}

/// Configure SQLite connection for optimal performance and concurrency.
///
/// Settings:
/// - WAL mode: allows concurrent reads during writes
/// - synchronous=NORMAL: good durability/speed tradeoff
/// - busy_timeout=5000ms: retry on lock contention instead of immediate failure
pub fn configure_connection(conn: &Connection) -> Result<(), IndexError> {
    conn.pragma_update(None, "busy_timeout", 5000)?;
    let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

pub fn load_indexed_sessions(conn: &Connection) -> Result<Vec<IndexedSession>, IndexError> {
    let mut stmt = conn.prepare("SELECT path, mtime, size FROM sessions")?;
    let rows = stmt.query_map([], |row| {
        Ok(IndexedSession {
            path: row.get(0)?,
            mtime: row.get(1)?,
            size: row.get(2)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }

    Ok(entries)
}

pub fn upsert_session(conn: &mut Connection, record: &SessionRecord) -> Result<(), IndexError> {
    let tx = conn.transaction()?;
    upsert_session_tx(&tx, record)?;
    tx.commit()?;
    Ok(())
}

pub fn upsert_session_tx(tx: &Transaction<'_>, record: &SessionRecord) -> Result<(), IndexError> {
    tx.execute(
        "INSERT INTO sessions (
            path,
            mtime,
            size,
            hash,
            created_at,
            last_message_at,
            agent,
            workspace,
            title,
            message_count,
            snippet,
            repo_root,
            repo_name,
            branch
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        ON CONFLICT(path) DO UPDATE SET
            mtime = excluded.mtime,
            size = excluded.size,
            hash = excluded.hash,
            created_at = excluded.created_at,
            last_message_at = excluded.last_message_at,
            agent = excluded.agent,
            workspace = excluded.workspace,
            title = excluded.title,
            message_count = excluded.message_count,
            snippet = excluded.snippet,
            repo_root = excluded.repo_root,
            repo_name = excluded.repo_name,
            branch = excluded.branch",
        params![
            &record.path,
            record.mtime,
            record.size,
            &record.hash,
            &record.created_at,
            &record.last_message_at,
            &record.agent,
            &record.workspace,
            &record.title,
            record.message_count,
            &record.snippet,
            &record.repo_root,
            &record.repo_name,
            &record.branch,
        ],
    )?;

    tx.execute(
        "DELETE FROM sessions_fts WHERE path = ?1",
        params![&record.path],
    )?;
    tx.execute(
        "INSERT INTO sessions_fts (content, path) VALUES (?1, ?2)",
        params![&record.content, &record.path],
    )?;

    Ok(())
}

pub fn replace_messages_tx(
    tx: &Transaction<'_>,
    session_path: &str,
    messages: &[MessageRecord],
) -> Result<(), IndexError> {
    tx.execute(
        "DELETE FROM messages_fts WHERE session_path = ?1",
        params![session_path],
    )?;
    tx.execute(
        "DELETE FROM messages WHERE session_path = ?1",
        params![session_path],
    )?;

    let mut insert_message = tx.prepare(
        "INSERT INTO messages (session_path, turn_index, role, timestamp, text)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut insert_fts = tx.prepare(
        "INSERT INTO messages_fts (text, message_id, session_path, role)
         VALUES (?1, ?2, ?3, ?4)",
    )?;

    for message in messages {
        insert_message.execute(params![
            session_path,
            message.turn_index,
            &message.role,
            &message.timestamp,
            &message.text,
        ])?;
        let message_id = tx.last_insert_rowid();
        insert_fts.execute(params![
            &message.text,
            message_id,
            session_path,
            &message.role,
        ])?;
    }

    Ok(())
}

pub fn remove_session(conn: &mut Connection, path: &str) -> Result<(), IndexError> {
    let tx = conn.transaction()?;
    remove_session_tx(&tx, path)?;
    tx.commit()?;
    Ok(())
}

pub fn remove_session_tx(tx: &Transaction<'_>, path: &str) -> Result<(), IndexError> {
    tx.execute(
        "DELETE FROM messages_fts WHERE session_path = ?1",
        params![path],
    )?;
    tx.execute(
        "DELETE FROM messages WHERE session_path = ?1",
        params![path],
    )?;
    tx.execute("DELETE FROM sessions_fts WHERE path = ?1", params![path])?;
    tx.execute("DELETE FROM sessions WHERE path = ?1", params![path])?;

    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    col_type: &str,
) -> Result<(), IndexError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(());
        }
    }

    conn.execute(
        &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, col_type),
        [],
    )?;
    Ok(())
}
