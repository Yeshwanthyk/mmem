use mmem::index::{init_schema, remove_session, upsert_session};
use mmem::model::SessionRecord;
use rusqlite::{Connection, params};

fn sample_record() -> SessionRecord {
    SessionRecord {
        path: "/tmp/session.jsonl".to_string(),
        mtime: 1700000000,
        size: 1234,
        hash: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        last_message_at: Some("2024-01-01T00:00:02Z".to_string()),
        agent: Some("gpt-4".to_string()),
        workspace: Some("ws-test".to_string()),
        title: Some("hello".to_string()),
        message_count: 2,
        snippet: "hello".to_string(),
        content: "[user] hello\n[assistant] hi".to_string(),
    }
}

#[test]
fn indexes_and_removes_sessions() {
    let mut conn = Connection::open_in_memory().expect("open memory db");
    init_schema(&conn).expect("schema");

    let record = sample_record();
    upsert_session(&mut conn, &record).expect("insert");

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("sessions count");
    assert_eq!(count, 1);

    let fts_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions_fts", [], |row| row.get(0))
        .expect("fts count");
    assert_eq!(fts_count, 1);

    let snippet: String = conn
        .query_row(
            "SELECT snippet FROM sessions WHERE path = ?1",
            params![&record.path],
            |row| row.get(0),
        )
        .expect("snippet");
    assert_eq!(snippet, "hello");

    let mut updated = record.clone();
    updated.snippet = "updated".to_string();
    updated.content = "updated content".to_string();
    upsert_session(&mut conn, &updated).expect("update");

    let updated_snippet: String = conn
        .query_row(
            "SELECT snippet FROM sessions WHERE path = ?1",
            params![&record.path],
            |row| row.get(0),
        )
        .expect("updated snippet");
    assert_eq!(updated_snippet, "updated");

    let fts_content: String = conn
        .query_row(
            "SELECT content FROM sessions_fts WHERE path = ?1",
            params![&record.path],
            |row| row.get(0),
        )
        .expect("fts content");
    assert_eq!(fts_content, "updated content");

    remove_session(&mut conn, &record.path).expect("remove");

    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("remaining count");
    assert_eq!(remaining, 0);
}
