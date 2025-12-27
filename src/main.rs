mod cli;

use clap::Parser;
use mmem::index::init_schema;
use mmem::query::{FindFilters, find_sessions};
use rusqlite::Connection;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();
    match cli.command {
        cli::Command::Find(args) => handle_find(args),
    }
}

fn handle_find(args: cli::FindArgs) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = cli::default_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;
    init_schema(&conn)?;

    let mut filters = FindFilters {
        agent: args.agent.clone(),
        workspace: args.workspace.clone(),
        after: args.after.clone(),
        before: args.before.clone(),
        limit: args.limit,
    };

    if filters.after.is_none()
        && let Some(days) = args.days
    {
        let cutoff = OffsetDateTime::now_utc() - Duration::days(days as i64);
        filters.after = Some(cutoff.format(&Rfc3339)?);
    }

    let results = find_sessions(&conn, &args.query, &filters)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    for hit in results {
        let title = hit.title.unwrap_or_else(|| "(untitled)".to_string());
        let when = hit
            .last_message_at
            .unwrap_or_else(|| "(unknown)".to_string());
        println!("{} | {}", when, title);
        println!("{}", hit.path);
        if let Some(snippet) = hit.snippet
            && !snippet.is_empty()
        {
            println!("{}", snippet);
        }
        println!();
    }

    Ok(())
}
