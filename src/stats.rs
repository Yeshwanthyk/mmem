//! Index statistics and agent listing.
//!
//! # Key Functions
//!
//! - [`load_stats`]: Get session count and date bounds
//! - [`load_agents`]: List unique agents with session counts

use rusqlite::Connection;

#[derive(Debug, serde::Serialize)]
pub struct StatsReport {
    pub session_count: i64,
    pub oldest_message_at: Option<String>,
    pub newest_message_at: Option<String>,
    pub parse_failures: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("sqlite error: {source}")]
    Sqlite { source: rusqlite::Error },
}

impl From<rusqlite::Error> for StatsError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Sqlite { source }
    }
}

pub fn load_stats(conn: &Connection) -> Result<StatsReport, StatsError> {
    let (count, oldest, newest): (i64, Option<String>, Option<String>) = conn.query_row(
        "SELECT COUNT(*), MIN(last_message_at), MAX(last_message_at) FROM sessions",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    Ok(StatsReport {
        session_count: count,
        oldest_message_at: oldest,
        newest_message_at: newest,
        parse_failures: None,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct AgentInfo {
    pub name: String,
    pub session_count: i64,
}

pub fn load_agents(conn: &Connection) -> Result<Vec<AgentInfo>, StatsError> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(agent, '(unknown)') as name, COUNT(*) as count 
         FROM sessions 
         GROUP BY agent 
         ORDER BY count DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(AgentInfo {
            name: row.get(0)?,
            session_count: row.get(1)?,
        })
    })?;

    let mut agents = Vec::new();
    for row in rows {
        agents.push(row?);
    }
    Ok(agents)
}
