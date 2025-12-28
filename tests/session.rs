use mmem::session::{SessionError, extract_tool_calls, load_entry_by_turn, resolve_session_path, scan_tool_calls};
use std::path::Path;
use tempfile::tempdir;

#[test]
fn loads_turn_and_extracts_tool_calls() {
    let path = Path::new("tests/fixtures/session_tools.jsonl");
    let entry = load_entry_by_turn(path, 0).expect("turn 0");

    assert_eq!(entry.line, 2);
    assert_eq!(entry.message_index, Some(0));

    let tools = extract_tool_calls(&entry.value);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "read");
}

#[test]
fn scans_tool_calls_with_filter() {
    let path = Path::new("tests/fixtures/session_tools.jsonl");
    let matches = scan_tool_calls(path, Some("read"), None).expect("scan tool calls");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].line, 2);
    assert_eq!(matches[0].message_index, Some(0));
    assert_eq!(matches[0].tool.name, "read");
}


#[test]
fn resolves_session_path_by_prefix() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    let file = root.join("1766632198584_test.jsonl");
    std::fs::write(&file, "{}\n").expect("write file");

    let resolved = resolve_session_path("1766632198584", root).expect("resolve");
    assert_eq!(resolved, file);
}

#[test]
fn resolve_session_path_reports_ambiguous_prefix() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("1766632198584_a.jsonl"), "{}\n").expect("write a");
    std::fs::write(root.join("1766632198584_b.jsonl"), "{}\n").expect("write b");

    let err = resolve_session_path("1766632198584", root).expect_err("ambiguous");
    assert!(matches!(err, SessionError::Ambiguous { .. }));
}

#[test]
fn resolve_session_path_reports_missing_prefix() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let err = resolve_session_path("nope", root).expect_err("missing");
    assert!(matches!(err, SessionError::NotFound { .. }));
}
