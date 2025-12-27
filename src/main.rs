mod cli;

use clap::Parser;
use mmem::doctor::run_doctor;
use mmem::index::{configure_connection, init_schema};
use mmem::model::{MessageContext, MessageHit, SessionHit};
use mmem::query::{FindFilters, FindScope, find_messages, find_sessions};
use mmem::scan::index_root;
use mmem::session::{
    SessionEntry, ToolCallMatch, extract_tool_calls, load_entry_by_line, load_entry_by_turn,
    scan_tool_calls,
};
use mmem::stats::load_stats;
use rusqlite::Connection;
use serde_json::{Map, Value};
use std::collections::HashSet;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

const MAX_OUTPUT_LEN: usize = 160;

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
        cli::Command::Find(args) => handle_find(*args),
        cli::Command::Show(args) => handle_show(args),
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

    let scope = match args.scope {
        cli::FindScopeArg::Session => FindScope::Session,
        cli::FindScopeArg::Message => FindScope::Message,
    };

    let role = normalize_role_filter(args.role.as_deref(), args.include_assistant);
    let fields_specified = args.fields.is_some();
    let field_set = build_field_set(args.fields.as_deref(), scope);
    let include_context = args.around > 0 && (!fields_specified || field_set.contains("context"));
    let around = if args.json || args.jsonl {
        if include_context { args.around } else { 0 }
    } else {
        args.around
    };

    let mut filters = FindFilters {
        agent: args.agent.clone(),
        workspace: args.workspace.clone(),
        repo: args.repo.clone(),
        branch: args.branch.clone(),
        role,
        after: args.after.clone(),
        before: args.before.clone(),
        limit: args.limit,
        around,
        scope,
    };

    if filters.after.is_none()
        && let Some(days) = args.days
    {
        let cutoff = OffsetDateTime::now_utc() - Duration::days(days as i64);
        filters.after = Some(cutoff.format(&Rfc3339)?);
    }

    match scope {
        FindScope::Session => {
            let results = find_sessions(&conn, &args.query, &filters)?;
            if args.json || args.jsonl {
                emit_sessions_json(&results, &field_set, args.jsonl)?;
            } else {
                emit_sessions_text(&results, args.snippet);
            }
        }
        FindScope::Message => {
            let results = find_messages(&conn, &args.query, &filters)?;
            if args.json || args.jsonl {
                emit_messages_json(&results, &field_set, include_context, args.jsonl)?;
            } else {
                emit_messages_text(&results, args.snippet, around);
            }
        }
    }

    Ok(())
}

fn handle_show(args: cli::ShowArgs) -> Result<(), Box<dyn std::error::Error>> {
    let tool_filter = if args.turn.is_none() && args.line.is_none() && args.tool.is_none() {
        Some("read")
    } else {
        args.tool.as_deref()
    };

    if let Some(turn) = args.turn {
        let entry = load_entry_by_turn(&args.path, turn)?;
        return emit_show_entry(&entry, tool_filter, args.extract, args.json);
    }

    if let Some(line) = args.line {
        let entry = load_entry_by_line(&args.path, line)?;
        return emit_show_entry(&entry, tool_filter, args.extract, args.json);
    }

    let matches = scan_tool_calls(&args.path, tool_filter)?;
    if args.json {
        let values: Vec<Value> = matches.into_iter().map(tool_match_to_json).collect();
        println!("{}", serde_json::to_string_pretty(&values)?);
        return Ok(());
    }

    if matches.is_empty() {
        println!("no tool calls found");
        return Ok(());
    }

    for item in matches {
        let turn = item
            .message_index
            .map(|idx| format!("turn {}", idx))
            .unwrap_or_else(|| "turn ?".to_string());
        println!("line {} ({}) tool={}", item.line, turn, item.tool.name);
        println!("{}", format_tool_args(&item.tool.arguments));
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

fn normalize_role_filter(role: Option<&str>, include_assistant: bool) -> Option<String> {
    if include_assistant {
        return role
            .map(|value| value.trim().to_lowercase())
            .filter(|v| !v.is_empty());
    }

    let value = role
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "user".to_string());
    Some(value)
}

fn build_field_set(fields: Option<&[String]>, scope: FindScope) -> HashSet<String> {
    let defaults: &[&str] = match scope {
        FindScope::Session => &["path", "title", "last_message_at", "score"],
        FindScope::Message => &["path", "title", "timestamp", "role", "turn_index", "score"],
    };

    let mut set = HashSet::new();
    match fields {
        Some(fields) => {
            for field in fields {
                let field = field.trim().to_lowercase();
                if !field.is_empty() {
                    set.insert(field);
                }
            }
        }
        None => {
            for field in defaults {
                set.insert((*field).to_string());
            }
        }
    }

    set
}

fn emit_sessions_text(results: &[SessionHit], show_snippet: bool) {
    for hit in results {
        let title = hit
            .title
            .clone()
            .unwrap_or_else(|| "(untitled)".to_string());
        let when = hit
            .last_message_at
            .clone()
            .unwrap_or_else(|| "(unknown)".to_string());
        println!("{} | {}", when, title);
        println!("{}", hit.path);
        if show_snippet && let Some(snippet) = hit.snippet.as_deref() {
            let snippet = trim_output(snippet);
            if !snippet.is_empty() {
                println!("{}", snippet);
            }
        }
        println!();
    }
}

fn emit_messages_text(results: &[MessageHit], show_snippet: bool, around: usize) {
    for hit in results {
        let title = hit
            .title
            .clone()
            .unwrap_or_else(|| "(untitled)".to_string());
        let when = hit
            .timestamp
            .clone()
            .unwrap_or_else(|| "(unknown)".to_string());
        println!("{} | {}", when, title);
        println!("{}#{}", hit.path, hit.turn_index);
        if show_snippet {
            let snippet = trim_output(&hit.text);
            if !snippet.is_empty() {
                println!("{}", snippet);
            }
        }
        if around > 0
            && let Some(context) = hit.context.as_deref()
        {
            emit_context_lines(context);
        }
        println!();
    }
}

fn emit_show_entry(
    entry: &SessionEntry,
    tool_filter: Option<&str>,
    extract: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tools = extract_tool_calls(&entry.value);
    if let Some(filter) = tool_filter {
        tools.retain(|tool| tool.name.eq_ignore_ascii_case(filter));
    }

    if extract {
        let mut extracted = false;
        for tool in tools {
            if !tool.name.eq_ignore_ascii_case("read") {
                continue;
            }
            if let Some(read_args) = parse_read_args(&tool.arguments) {
                emit_read_extract(&read_args)?;
                extracted = true;
            }
        }

        if !extracted {
            println!("no readable tool calls found");
        }
        return Ok(());
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entry_to_json(entry, &tools))?
        );
        return Ok(());
    }

    if tools.is_empty() {
        println!("no tool calls found");
        return Ok(());
    }

    let turn = entry
        .message_index
        .map(|idx| format!("turn {}", idx))
        .unwrap_or_else(|| "turn ?".to_string());
    let role = entry.role.as_deref().unwrap_or("unknown");
    println!("line {} ({}, role {})", entry.line, turn, role);
    if let Some(timestamp) = entry.timestamp.as_deref() {
        println!("timestamp {}", timestamp);
    }

    for tool in tools {
        println!("tool={}", tool.name);
        println!("{}", format_tool_args(&tool.arguments));
        println!();
    }

    Ok(())
}

fn entry_to_json(entry: &SessionEntry, tools: &[mmem::session::ToolCall]) -> Value {
    let mut map = Map::new();
    map.insert("line".to_string(), Value::from(entry.line as i64));
    if let Some(turn) = entry.message_index {
        map.insert("turn".to_string(), Value::from(turn as i64));
    }
    if let Some(role) = entry.role.as_deref() {
        map.insert("role".to_string(), Value::String(role.to_string()));
    }
    if let Some(timestamp) = entry.timestamp.as_deref() {
        map.insert(
            "timestamp".to_string(),
            Value::String(timestamp.to_string()),
        );
    }

    let tool_values: Vec<Value> = tools.iter().map(tool_to_json).collect();
    map.insert("tools".to_string(), Value::Array(tool_values));

    Value::Object(map)
}

fn tool_match_to_json(item: ToolCallMatch) -> Value {
    let mut map = Map::new();
    map.insert("line".to_string(), Value::from(item.line as i64));
    if let Some(turn) = item.message_index {
        map.insert("turn".to_string(), Value::from(turn as i64));
    }
    map.insert("tool".to_string(), tool_to_json(&item.tool));
    Value::Object(map)
}

fn tool_to_json(tool: &mmem::session::ToolCall) -> Value {
    let mut map = Map::new();
    map.insert("name".to_string(), Value::String(tool.name.clone()));
    map.insert("arguments".to_string(), tool.arguments.clone());
    Value::Object(map)
}

fn format_tool_args(arguments: &Value) -> String {
    match normalize_arguments(arguments) {
        Some(Value::Object(map)) if map.contains_key("path") => {
            if let Some(read_args) = parse_read_args(arguments) {
                return format!(
                    "path={} offset={} limit={}",
                    read_args.path, read_args.offset, read_args.limit
                );
            }
            trim_output(&serde_json::to_string(arguments).unwrap_or_default())
        }
        Some(value) => trim_output(&serde_json::to_string(&value).unwrap_or_default()),
        None => "(no arguments)".to_string(),
    }
}

fn emit_read_extract(read_args: &ReadArgs) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(&read_args.path)?;
    let lines: Vec<&str> = content.lines().collect();
    let start = read_args.offset.saturating_sub(1);
    let end = std::cmp::min(lines.len(), start.saturating_add(read_args.limit));

    println!(
        ">>> {}:{} (limit {})",
        read_args.path, read_args.offset, read_args.limit
    );
    for (idx, line) in lines[start..end].iter().enumerate() {
        let line_no = read_args.offset + idx;
        println!("{:>4} {}", line_no, line);
    }
    println!();
    Ok(())
}

#[derive(Debug)]
struct ReadArgs {
    path: String,
    offset: usize,
    limit: usize,
}

fn parse_read_args(arguments: &Value) -> Option<ReadArgs> {
    let args = normalize_arguments(arguments)?;
    let obj = args.as_object()?;
    let path = obj.get("path").and_then(|v| v.as_str())?.to_string();
    let offset = obj.get("offset").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let limit = obj.get("limit").and_then(|v| v.as_u64()).unwrap_or(200) as usize;
    Some(ReadArgs {
        path,
        offset,
        limit,
    })
}

fn normalize_arguments(arguments: &Value) -> Option<Value> {
    if arguments.is_object() {
        return Some(arguments.clone());
    }
    let raw = arguments.as_str()?;
    serde_json::from_str(raw).ok()
}

fn emit_context_lines(context: &[MessageContext]) {
    for message in context {
        let role = message.role.as_deref().unwrap_or("unknown");
        let text = trim_output(&message.text);
        if text.is_empty() {
            continue;
        }
        println!("  {}:{} {}", message.turn_index, role, text);
    }
}

fn emit_sessions_json(
    results: &[SessionHit],
    fields: &HashSet<String>,
    jsonl: bool,
) -> Result<(), serde_json::Error> {
    if jsonl {
        for hit in results {
            let value = session_to_json(hit, fields);
            println!("{}", serde_json::to_string(&value)?);
        }
        return Ok(());
    }

    let values: Vec<Value> = results
        .iter()
        .map(|hit| session_to_json(hit, fields))
        .collect();
    println!("{}", serde_json::to_string_pretty(&values)?);
    Ok(())
}

fn emit_messages_json(
    results: &[MessageHit],
    fields: &HashSet<String>,
    include_context: bool,
    jsonl: bool,
) -> Result<(), serde_json::Error> {
    if jsonl {
        for hit in results {
            let value = message_to_json(hit, fields, include_context);
            println!("{}", serde_json::to_string(&value)?);
        }
        return Ok(());
    }

    let values: Vec<Value> = results
        .iter()
        .map(|hit| message_to_json(hit, fields, include_context))
        .collect();
    println!("{}", serde_json::to_string_pretty(&values)?);
    Ok(())
}

fn session_to_json(hit: &SessionHit, fields: &HashSet<String>) -> Value {
    let mut map = Map::new();
    insert_field(&mut map, "path", &hit.path, fields);
    insert_opt_field(&mut map, "title", hit.title.as_deref(), fields);
    insert_opt_field(&mut map, "agent", hit.agent.as_deref(), fields);
    insert_opt_field(&mut map, "workspace", hit.workspace.as_deref(), fields);
    insert_opt_field(&mut map, "repo_root", hit.repo_root.as_deref(), fields);
    insert_opt_field(&mut map, "repo_name", hit.repo_name.as_deref(), fields);
    insert_opt_field(&mut map, "branch", hit.branch.as_deref(), fields);
    insert_opt_field(
        &mut map,
        "last_message_at",
        hit.last_message_at.as_deref(),
        fields,
    );
    if fields.contains("snippet")
        && let Some(snippet) = hit.snippet.as_deref()
    {
        map.insert("snippet".to_string(), Value::String(trim_output(snippet)));
    }
    if fields.contains("score") {
        map.insert("score".to_string(), Value::from(hit.score));
    }
    Value::Object(map)
}

fn message_to_json(hit: &MessageHit, fields: &HashSet<String>, include_context: bool) -> Value {
    let mut map = Map::new();
    insert_field(&mut map, "path", &hit.path, fields);
    insert_opt_field(&mut map, "title", hit.title.as_deref(), fields);
    insert_opt_field(&mut map, "agent", hit.agent.as_deref(), fields);
    insert_opt_field(&mut map, "workspace", hit.workspace.as_deref(), fields);
    insert_opt_field(&mut map, "repo_root", hit.repo_root.as_deref(), fields);
    insert_opt_field(&mut map, "repo_name", hit.repo_name.as_deref(), fields);
    insert_opt_field(&mut map, "branch", hit.branch.as_deref(), fields);
    if fields.contains("turn_index") {
        map.insert("turn_index".to_string(), Value::from(hit.turn_index));
    }
    if fields.contains("role") {
        insert_opt_field(&mut map, "role", hit.role.as_deref(), fields);
    }
    insert_opt_field(&mut map, "timestamp", hit.timestamp.as_deref(), fields);
    if fields.contains("text") {
        map.insert("text".to_string(), Value::String(trim_output(&hit.text)));
    }
    if fields.contains("score") {
        map.insert("score".to_string(), Value::from(hit.score));
    }
    if include_context
        && fields.contains("context")
        && let Some(context) = hit.context.as_deref()
    {
        let values: Vec<Value> = context.iter().map(message_context_to_json).collect();
        map.insert("context".to_string(), Value::Array(values));
    }

    Value::Object(map)
}

fn message_context_to_json(context: &MessageContext) -> Value {
    let mut map = Map::new();
    map.insert("turn_index".to_string(), Value::from(context.turn_index));
    if let Some(role) = context.role.as_deref() {
        map.insert("role".to_string(), Value::String(role.to_string()));
    }
    if let Some(timestamp) = context.timestamp.as_deref() {
        map.insert(
            "timestamp".to_string(),
            Value::String(timestamp.to_string()),
        );
    }
    map.insert(
        "text".to_string(),
        Value::String(trim_output(&context.text)),
    );
    Value::Object(map)
}

fn insert_field(map: &mut Map<String, Value>, key: &str, value: &str, fields: &HashSet<String>) {
    if fields.contains(key) {
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn insert_opt_field(
    map: &mut Map<String, Value>,
    key: &str,
    value: Option<&str>,
    fields: &HashSet<String>,
) {
    if fields.contains(key)
        && let Some(value) = value
    {
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn trim_output(text: &str) -> String {
    let compacted = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compacted.chars().count() <= MAX_OUTPUT_LEN {
        return compacted;
    }
    compacted.chars().take(MAX_OUTPUT_LEN).collect()
}
