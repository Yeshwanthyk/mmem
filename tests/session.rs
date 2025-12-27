use mmem::session::{extract_tool_calls, load_entry_by_turn, scan_tool_calls};
use std::path::Path;

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
    let matches = scan_tool_calls(path, Some("read")).expect("scan tool calls");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].line, 2);
    assert_eq!(matches[0].message_index, Some(0));
    assert_eq!(matches[0].tool.name, "read");
}
