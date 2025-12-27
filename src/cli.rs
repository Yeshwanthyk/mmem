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
    Find(Box<FindArgs>),
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
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FindScopeArg {
    Session,
    Message,
}

#[derive(Debug, Args)]
pub struct FindArgs {
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
    #[arg(long, conflicts_with = "jsonl")]
    pub json: bool,
    #[arg(long, conflicts_with = "json")]
    pub jsonl: bool,
    #[arg(long, value_delimiter = ',')]
    pub fields: Option<Vec<String>>,
    #[arg(long)]
    pub snippet: bool,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    pub path: PathBuf,
    #[arg(long, conflicts_with = "line")]
    pub turn: Option<usize>,
    #[arg(long, conflicts_with = "turn")]
    pub line: Option<usize>,
    #[arg(long)]
    pub tool: Option<String>,
    #[arg(long)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub extract: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long)]
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
