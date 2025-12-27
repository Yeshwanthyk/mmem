use crate::model::{MessageContext, MessageHit, SessionHit};
use rusqlite::{Connection, params};

const FIND_SESSIONS_SQL: &str = r#"
SELECT s.path,
       s.title,
       s.agent,
       s.workspace,
       s.repo_root,
       s.repo_name,
       s.branch,
       s.last_message_at,
       s.snippet,
       bm25(sessions_fts) AS score
FROM sessions_fts
JOIN sessions s ON s.path = sessions_fts.path
WHERE sessions_fts MATCH ?1
  AND (?2 IS NULL OR s.agent = ?2)
  AND (?3 IS NULL OR s.workspace = ?3)
  AND (?4 IS NULL OR s.repo_name = ?4 OR s.repo_root = ?4)
  AND (?5 IS NULL OR s.branch = ?5)
  AND (?6 IS NULL OR s.last_message_at >= ?6)
  AND (?7 IS NULL OR s.last_message_at <= ?7)
ORDER BY score ASC, s.last_message_at DESC
LIMIT ?8;
"#;

const FIND_MESSAGES_SQL: &str = r#"
SELECT m.session_path,
       m.turn_index,
       m.role,
       m.timestamp,
       m.text,
       s.title,
       s.agent,
       s.workspace,
       s.repo_root,
       s.repo_name,
       s.branch,
       bm25(messages_fts) AS score
FROM messages_fts
JOIN messages m ON m.id = messages_fts.message_id
JOIN sessions s ON s.path = m.session_path
WHERE messages_fts MATCH ?1
  AND (?2 IS NULL OR s.agent = ?2)
  AND (?3 IS NULL OR s.workspace = ?3)
  AND (?4 IS NULL OR s.repo_name = ?4 OR s.repo_root = ?4)
  AND (?5 IS NULL OR s.branch = ?5)
  AND (?6 IS NULL OR m.role = ?6)
  AND (?7 IS NULL OR COALESCE(m.timestamp, s.last_message_at) >= ?7)
  AND (?8 IS NULL OR COALESCE(m.timestamp, s.last_message_at) <= ?8)
ORDER BY score ASC, COALESCE(m.timestamp, s.last_message_at) DESC
LIMIT ?9;
"#;

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("query is empty")]
    EmptyQuery,
    #[error("sqlite error: {source}")]
    Sqlite { source: rusqlite::Error },
}

impl From<rusqlite::Error> for QueryError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Sqlite { source }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindScope {
    Session,
    Message,
}

impl Default for FindScope {
    fn default() -> Self {
        Self::Message
    }
}

#[derive(Debug, Default)]
pub struct FindFilters {
    pub agent: Option<String>,
    pub workspace: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub role: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: usize,
    pub around: usize,
    pub scope: FindScope,
}

pub fn find_sessions(
    conn: &Connection,
    query: &str,
    filters: &FindFilters,
) -> Result<Vec<SessionHit>, QueryError> {
    let query = normalize_query(query)?;
    let limit = normalize_limit(filters.limit);

    let mut stmt = conn.prepare(FIND_SESSIONS_SQL)?;
    let rows = stmt.query_map(
        params![
            query,
            &filters.agent,
            &filters.workspace,
            &filters.repo,
            &filters.branch,
            &filters.after,
            &filters.before,
            limit,
        ],
        |row| {
            Ok(SessionHit {
                path: row.get(0)?,
                title: row.get(1)?,
                agent: row.get(2)?,
                workspace: row.get(3)?,
                repo_root: row.get(4)?,
                repo_name: row.get(5)?,
                branch: row.get(6)?,
                last_message_at: row.get(7)?,
                snippet: row.get(8)?,
                score: row.get(9)?,
            })
        },
    )?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }

    Ok(results)
}

pub fn find_messages(
    conn: &Connection,
    query: &str,
    filters: &FindFilters,
) -> Result<Vec<MessageHit>, QueryError> {
    let query = normalize_query(query)?;
    let limit = normalize_limit(filters.limit);

    let mut stmt = conn.prepare(FIND_MESSAGES_SQL)?;
    let rows = stmt.query_map(
        params![
            query,
            &filters.agent,
            &filters.workspace,
            &filters.repo,
            &filters.branch,
            &filters.role,
            &filters.after,
            &filters.before,
            limit,
        ],
        |row| {
            Ok(MessageHit {
                path: row.get(0)?,
                turn_index: row.get(1)?,
                role: row.get(2)?,
                timestamp: row.get(3)?,
                text: row.get(4)?,
                title: row.get(5)?,
                agent: row.get(6)?,
                workspace: row.get(7)?,
                repo_root: row.get(8)?,
                repo_name: row.get(9)?,
                branch: row.get(10)?,
                score: row.get(11)?,
                context: None,
            })
        },
    )?;

    let mut results = Vec::new();
    for row in rows {
        let mut hit = row?;
        if filters.around > 0 {
            hit.context = Some(load_context(
                conn,
                &hit.path,
                hit.turn_index,
                filters.around,
            )?);
        }
        results.push(hit);
    }

    Ok(results)
}

fn load_context(
    conn: &Connection,
    session_path: &str,
    turn_index: i64,
    around: usize,
) -> Result<Vec<MessageContext>, QueryError> {
    let around = around as i64;
    let start = turn_index.saturating_sub(around);
    let end = turn_index.saturating_add(around);

    let mut stmt = conn.prepare(
        "SELECT turn_index, role, timestamp, text
         FROM messages
         WHERE session_path = ?1 AND turn_index BETWEEN ?2 AND ?3
         ORDER BY turn_index ASC",
    )?;
    let rows = stmt.query_map(params![session_path, start, end], |row| {
        Ok(MessageContext {
            turn_index: row.get(0)?,
            role: row.get(1)?,
            timestamp: row.get(2)?,
            text: row.get(3)?,
        })
    })?;

    let mut context = Vec::new();
    for row in rows {
        context.push(row?);
    }

    Ok(context)
}

fn normalize_query(query: &str) -> Result<&str, QueryError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(QueryError::EmptyQuery);
    }
    Ok(query)
}

fn normalize_limit(limit: usize) -> i64 {
    (if limit == 0 { 5 } else { limit }) as i64
}
