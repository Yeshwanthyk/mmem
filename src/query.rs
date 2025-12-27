use crate::model::SessionHit;
use rusqlite::{Connection, params};

const FIND_SQL: &str = r#"
SELECT s.path,
       s.title,
       s.agent,
       s.workspace,
       s.last_message_at,
       s.snippet,
       bm25(sessions_fts) AS score
FROM sessions_fts
JOIN sessions s ON s.path = sessions_fts.path
WHERE sessions_fts MATCH ?1
  AND (?2 IS NULL OR s.agent = ?2)
  AND (?3 IS NULL OR s.workspace = ?3)
  AND (?4 IS NULL OR s.last_message_at >= ?4)
  AND (?5 IS NULL OR s.last_message_at <= ?5)
ORDER BY score ASC, s.last_message_at DESC
LIMIT ?6;
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

#[derive(Debug, Default)]
pub struct FindFilters {
    pub agent: Option<String>,
    pub workspace: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub limit: usize,
}

pub fn find_sessions(
    conn: &Connection,
    query: &str,
    filters: &FindFilters,
) -> Result<Vec<SessionHit>, QueryError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(QueryError::EmptyQuery);
    }

    let limit = if filters.limit == 0 {
        10
    } else {
        filters.limit
    } as i64;

    let mut stmt = conn.prepare(FIND_SQL)?;
    let rows = stmt.query_map(
        params![
            query,
            &filters.agent,
            &filters.workspace,
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
                last_message_at: row.get(4)?,
                snippet: row.get(5)?,
                score: row.get(6)?,
            })
        },
    )?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }

    Ok(results)
}
