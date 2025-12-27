use clap::{Args, Parser, Subcommand};
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
    Find(FindArgs),
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
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
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
