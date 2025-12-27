mod cli;

use clap::Parser;
use mmem::doctor::run_doctor;
use mmem::index::{configure_connection, init_schema};
use mmem::query::{FindFilters, find_sessions};
use mmem::scan::index_root;
use mmem::stats::load_stats;
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
        cli::Command::Index(args) => handle_index(args),
        cli::Command::Find(args) => handle_find(args),
        cli::Command::Stats(args) => handle_stats(args),
        cli::Command::Doctor(args) => handle_doctor(args),
    }
}

fn open_db() -> Result<Connection, Box<dyn std::error::Error>> {
    let db_path = cli::default_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;
    configure_connection(&conn)?;
    Ok(conn)
}

fn handle_index(args: cli::IndexArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = open_db()?;
    init_schema(&conn)?;

    let root = args.root.unwrap_or_else(cli::default_sessions_root);
    let stats = index_root(&mut conn, &root, args.full)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

    println!("scanned: {}", stats.scanned);
    println!("indexed: {}", stats.indexed);
    println!("skipped: {}", stats.skipped);
    println!("removed: {}", stats.removed);
    println!("parse_errors: {}", stats.parse_errors);

    Ok(())
}

fn handle_find(args: cli::FindArgs) -> Result<(), Box<dyn std::error::Error>> {
    let conn = open_db()?;
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

fn handle_stats(args: cli::StatsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let conn = open_db()?;
    init_schema(&conn)?;

    let stats = load_stats(&conn)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

    println!("sessions: {}", stats.session_count);
    println!(
        "oldest: {}",
        stats
            .oldest_message_at
            .unwrap_or_else(|| "(unknown)".to_string())
    );
    println!(
        "newest: {}",
        stats
            .newest_message_at
            .unwrap_or_else(|| "(unknown)".to_string())
    );
    match stats.parse_failures {
        Some(count) => println!("parse_failures: {}", count),
        None => println!("parse_failures: unknown"),
    }

    Ok(())
}

fn handle_doctor(args: cli::DoctorArgs) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = cli::default_db_path();
    let root = cli::default_sessions_root();

    let report = run_doctor(&db_path, &root);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("root: {}", report.root.display());
    println!("root_exists: {}", report.root_exists);
    println!("db_path: {}", report.db_path.display());
    println!("db_exists: {}", report.db_exists);
    println!("schema_ok: {}", report.schema_ok);
    if let Some(error) = report.schema_error {
        println!("schema_error: {}", error);
    }
    println!("fts5_available: {}", report.fts5_available);
    println!("indexed_sessions: {}", report.indexed_sessions);
    println!(
        "newest_message_at: {}",
        report
            .newest_message_at
            .unwrap_or_else(|| "(unknown)".to_string())
    );

    Ok(())
}
