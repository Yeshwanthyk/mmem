use mmem::index::{init_schema, replace_messages_tx, upsert_session_tx};
use mmem::model::{MessageRecord, SessionRecord};
use mmem::query::{FindFilters, FindScope, QueryError, QueryMode, find_messages};
use rusqlite::Connection;

fn record(path: &str, agent: &str, workspace: &str, last_message_at: &str) -> SessionRecord {
    SessionRecord {
        path: path.to_string(),
        mtime: 1700000000,
        size: 1234,
        hash: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        last_message_at: Some(last_message_at.to_string()),
        agent: Some(agent.to_string()),
        workspace: Some(workspace.to_string()),
        title: Some("title".to_string()),
        message_count: 2,
        snippet: "snippet".to_string(),
        content: "alpha beta".to_string(),
        repo_root: None,
        repo_name: None,
        branch: None,
    }
}

fn insert_session(conn: &mut Connection, record: &SessionRecord, messages: &[MessageRecord]) {
    let tx = conn.transaction().expect("tx");
    upsert_session_tx(&tx, record).expect("session insert");
    replace_messages_tx(&tx, &record.path, messages).expect("message insert");
    tx.commit().expect("commit");
}

#[test]
fn finds_messages_with_filters() {
    let mut conn = Connection::open_in_memory().expect("db");
    init_schema(&conn).expect("schema");

    let rec_a = record("/tmp/a.jsonl", "gpt-4", "ws-a", "2024-01-01T00:00:01Z");
    let rec_b = record("/tmp/b.jsonl", "gpt-3", "ws-b", "2024-01-02T00:00:01Z");

    insert_session(
        &mut conn,
        &rec_a,
        &[MessageRecord {
            turn_index: 0,
            role: Some("user".to_string()),
            timestamp: Some("2024-01-01T00:00:01Z".to_string()),
            text: "alpha".to_string(),
        }],
    );
    insert_session(
        &mut conn,
        &rec_b,
        &[MessageRecord {
            turn_index: 0,
            role: Some("user".to_string()),
            timestamp: Some("2024-01-02T00:00:01Z".to_string()),
            text: "alpha".to_string(),
        }],
    );

    let mut filters = FindFilters {
        agent: Some("gpt-4".to_string()),
        role: Some("user".to_string()),
        limit: 10,
        scope: FindScope::Message,
        ..Default::default()
    };

    let results = find_messages(&conn, "alpha", &filters).expect("query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "/tmp/a.jsonl");

    filters.agent = None;
    filters.workspace = Some("ws-b".to_string());
    let results = find_messages(&conn, "alpha", &filters).expect("query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "/tmp/b.jsonl");

    filters.workspace = None;
    filters.after = Some("2024-01-02T00:00:00Z".to_string());
    let results = find_messages(&conn, "alpha", &filters).expect("query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "/tmp/b.jsonl");
}

#[test]
fn fts_syntax_error_produces_helpful_message() {
    let conn = Connection::open_in_memory().expect("db");
    init_schema(&conn).expect("schema");

    // Deliberately invalid FTS5 query
    let filters = FindFilters {
        query_mode: QueryMode::Fts,
        limit: 5,
        ..Default::default()
    };

    let result = find_messages(&conn, "AND AND", &filters);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, QueryError::InvalidFtsSyntax { .. }),
        "Expected InvalidFtsSyntax, got: {:?}",
        err
    );
}

#[test]
fn returns_empty_for_no_matches() {
    let conn = Connection::open_in_memory().expect("db");
    init_schema(&conn).expect("schema");

    let filters = FindFilters {
        limit: 10,
        ..Default::default()
    };

    let results = find_messages(&conn, "nonexistent query term xyz", &filters).expect("query");
    assert!(results.is_empty());
}
