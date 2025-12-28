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
    Stats(StatsArgs),
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    #[arg(long)]
    pub full: bool,
    #[arg(long)]
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
    #[arg(long)]
    pub days: Option<u32>,
    #[arg(long)]
    pub before: Option<String>,
    #[arg(long)]
    pub after: Option<String>,
    #[arg(long)]
    pub agent: Option<String>,
    #[arg(long)]
    pub workspace: Option<String>,
    #[arg(long, alias = "project")]
    pub repo: Option<String>,
    #[arg(long)]
    pub branch: Option<String>,
    #[arg(long)]
    pub role: Option<String>,
    #[arg(long)]
    pub include_assistant: bool,
    #[arg(long, default_value_t = 0)]
    pub around: usize,
    #[arg(long, value_enum, default_value_t = FindScopeArg::Message)]
    pub scope: FindScopeArg,
    #[arg(long, default_value_t = 5)]
    pub limit: usize,
    #[arg(long, help = "Use raw FTS5 query syntax (advanced)")]
    pub fts: bool,
    #[arg(long, conflicts_with = "jsonl", help = "JSON array output (machine-friendly)")]
    pub json: bool,
    #[arg(long, conflicts_with = "json", help = "JSON Lines output (machine-friendly)")]
    pub jsonl: bool,
    #[arg(long, value_delimiter = ',')]
    pub fields: Option<Vec<String>>,
    #[arg(long)]
    pub snippet: bool,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(value_name = "PATH|SESSION_ID")]
    pub target: String,
    #[arg(long, conflicts_with = "line")]
    pub turn: Option<usize>,
    #[arg(long, conflicts_with = "turn")]
    pub line: Option<usize>,
    #[arg(long)]
    pub tool: Option<String>,
    #[arg(long)]
    pub limit: Option<usize>,
    #[arg(long, help = "Extract and show file contents from read tool calls") ]
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
