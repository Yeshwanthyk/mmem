use mmem::index::{init_schema, upsert_session};
use mmem::model::SessionRecord;
use mmem::stats::load_stats;
use rusqlite::Connection;

fn record(path: &str, last_message_at: &str) -> SessionRecord {
    SessionRecord {
        path: path.to_string(),
        mtime: 1700000000,
        size: 1234,
        hash: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        last_message_at: Some(last_message_at.to_string()),
        agent: Some("gpt-4".to_string()),
        workspace: Some("ws".to_string()),
        title: Some("title".to_string()),
        message_count: 2,
        snippet: "snippet".to_string(),
        content: "alpha".to_string(),
    }
}

#[test]
fn stats_report_counts_and_bounds() {
    let mut conn = Connection::open_in_memory().expect("db");
    init_schema(&conn).expect("schema");

    let rec_a = record("/tmp/a.jsonl", "2024-01-01T00:00:01Z");
    let rec_b = record("/tmp/b.jsonl", "2024-01-03T00:00:01Z");

    upsert_session(&mut conn, &rec_a).expect("insert a");
    upsert_session(&mut conn, &rec_b).expect("insert b");

    let stats = load_stats(&conn).expect("stats");
    assert_eq!(stats.session_count, 2);
    assert_eq!(
        stats.oldest_message_at.as_deref(),
        Some("2024-01-01T00:00:01Z")
    );
    assert_eq!(
        stats.newest_message_at.as_deref(),
        Some("2024-01-03T00:00:01Z")
    );
    assert!(stats.parse_failures.is_none());
}
