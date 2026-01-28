//! Command-line argument parsing with clap.
//!
//! Defines the CLI structure for mmem commands:
//! - `index`: Index sessions from disk
//! - `find`: Search sessions and messages
//! - `show`: Inspect tool calls in a session
//! - `stats`: Show index statistics
//! - `agents`: List unique agents
//! - `doctor`: Check index health

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "mmem")]
#[command(about = "Marvin session memory search", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Index sessions from disk into SQLite")]
    Index(IndexArgs),
    #[command(
        about = "Search sessions and messages",
        long_about = "Search session content. Default search is literal (safe for dates and punctuation). Use --fts for raw FTS5 syntax.",
        after_help = r#"Examples:
  mmem find "quickdiff 2025-12-27"
  mmem find "quickdiff 2025-12-27" --jsonl --fields path,title,turn_index,text
  mmem find "title:rust AND async" --fts
  mmem find "error handling" --days 7 --repo my-project"#,
    )]
    Find(Box<FindArgs>),
    #[command(
        about = "Inspect tool calls in a session JSONL",
        long_about = "Show tool calls for a session. Accepts a JSONL path or a session id prefix (the numeric prefix of the filename). Default tool filter is read.",
        after_help = r#"Examples:
  mmem show 1766632198584
  mmem show 1766632198584 --tool write
  mmem show 1766632198584 --json
  mmem show ~/.config/marvin/sessions/path/session.jsonl --extract"#,
    )]
    Show(ShowArgs),
    #[command(about = "Show index statistics")]
    Stats(StatsArgs),
    #[command(about = "List unique agents in the index")]
    Agents(AgentsArgs),
    #[command(about = "Check index health and configuration")]
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
pub struct AgentsArgs {
    #[arg(long, help = "JSON output (machine-friendly)")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    #[arg(long, help = "Full reindex (ignore mtime/size cache)")]
    pub full: bool,
    #[arg(long, help = "Sessions root directory")]
    pub root: Option<PathBuf>,
    #[arg(long, help = "JSON output (machine-friendly)")]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FindScopeArg {
    Session,
    Message,
}

#[derive(Debug, Args)]
pub struct FindArgs {
    #[arg(value_name = "QUERY", help = "Search query (literal by default)")]
    pub query: String,
    #[arg(long, help = "Filter to last N days")]
    pub days: Option<u32>,
    #[arg(long, help = "Filter messages before date (ISO8601)")]
    pub before: Option<String>,
    #[arg(long, help = "Filter messages after date (ISO8601)")]
    pub after: Option<String>,
    #[arg(long, help = "Filter by agent name")]
    pub agent: Option<String>,
    #[arg(long, help = "Filter by workspace path")]
    pub workspace: Option<String>,
    #[arg(long, alias = "project", help = "Filter by repo name or path")]
    pub repo: Option<String>,
    #[arg(long, help = "Filter by git branch")]
    pub branch: Option<String>,
    #[arg(long, help = "Filter by message role (user/assistant)")]
    pub role: Option<String>,
    #[arg(long, help = "Include assistant messages (default: user only)")]
    pub include_assistant: bool,
    #[arg(long, default_value_t = 0, help = "Context messages around match")]
    pub around: usize,
    #[arg(long, value_enum, default_value_t = FindScopeArg::Message, help = "Search scope")]
    pub scope: FindScopeArg,
    #[arg(long, default_value_t = 5, help = "Max results to return")]
    pub limit: usize,
    #[arg(long, help = "Use raw FTS5 query syntax (advanced)")]
    pub fts: bool,
    #[arg(long, conflicts_with = "jsonl", help = "JSON array output (machine-friendly)")]
    pub json: bool,
    #[arg(long, conflicts_with = "json", help = "JSON Lines output (machine-friendly)")]
    pub jsonl: bool,
    #[arg(long, value_delimiter = ',', help = "Output fields (comma-separated)")]
    pub fields: Option<Vec<String>>,
    #[arg(long, help = "Show text snippet in results")]
    pub snippet: bool,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(value_name = "PATH|SESSION_ID", help = "Session file path or ID prefix")]
    pub target: String,
    #[arg(long, conflicts_with = "line", help = "Show specific turn by index")]
    pub turn: Option<usize>,
    #[arg(long, conflicts_with = "turn", help = "Show specific line number")]
    pub line: Option<usize>,
    #[arg(long, help = "Filter by tool name")]
    pub tool: Option<String>,
    #[arg(long, help = "Max tool calls to show")]
    pub limit: Option<usize>,
    #[arg(long, help = "Extract and show file contents from read tool calls")]
    pub extract: bool,
    #[arg(long, help = "JSON output (machine-friendly)")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long, help = "JSON output (machine-friendly)")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long, help = "JSON output (machine-friendly)")]
    pub json: bool,
}

pub fn default_db_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".config/marvin/mmem.sqlite")
}

pub fn default_sessions_root() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".config/marvin/sessions")
}
