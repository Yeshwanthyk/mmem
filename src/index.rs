use crate::model::SessionRecord;
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
  snippet TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
  content,
  path UNINDEXED
);

CREATE INDEX IF NOT EXISTS idx_sessions_last_message_at ON sessions(last_message_at);
CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent);
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
    Ok(())
}

pub fn configure_connection(conn: &Connection) -> Result<(), IndexError> {
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
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
            snippet
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
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
            snippet = excluded.snippet",
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

pub fn remove_session(conn: &mut Connection, path: &str) -> Result<(), IndexError> {
    let tx = conn.transaction()?;
    remove_session_tx(&tx, path)?;
    tx.commit()?;
    Ok(())
}

pub fn remove_session_tx(tx: &Transaction<'_>, path: &str) -> Result<(), IndexError> {
    tx.execute("DELETE FROM sessions_fts WHERE path = ?1", params![path])?;
    tx.execute("DELETE FROM sessions WHERE path = ?1", params![path])?;

    Ok(())
}
