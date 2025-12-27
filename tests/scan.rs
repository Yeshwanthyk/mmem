use mmem::index::init_schema;
use mmem::scan::index_root;
use rusqlite::Connection;

#[test]
fn indexes_skips_and_removes_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let jsonl_path = dir.path().join("a.jsonl");
    let md_path = dir.path().join("b.md");

    std::fs::write(
        &jsonl_path,
        "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":\"hello\"}}\n",
    )
    .expect("write jsonl");
    std::fs::write(&md_path, "User: hi\nAssistant: hey\n").expect("write md");

    let mut conn = Connection::open_in_memory().expect("db");
    init_schema(&conn).expect("schema");

    let stats = index_root(&mut conn, dir.path(), false).expect("index");
    assert_eq!(stats.indexed, 2);
    assert_eq!(stats.skipped, 0);
    assert_eq!(stats.removed, 0);
    assert_eq!(stats.parse_errors, 0);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 2);

    let stats = index_root(&mut conn, dir.path(), false).expect("reindex");
    assert_eq!(stats.indexed, 0);
    assert_eq!(stats.skipped, 2);

    std::fs::remove_file(&md_path).expect("remove md");
    let stats = index_root(&mut conn, dir.path(), false).expect("remove index");
    assert_eq!(stats.removed, 1);
}
