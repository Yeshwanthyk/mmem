use mmem::parse::{parse_json, parse_jsonl, parse_markdown};

#[test]
fn parses_jsonl_sessions() {
    let input = include_str!("fixtures/session.jsonl");
    let parsed = parse_jsonl(input).expect("jsonl parse");

    assert_eq!(parsed.message_count, 2);
    assert_eq!(parsed.title.as_deref(), Some("hello"));
    assert_eq!(parsed.agent.as_deref(), Some("gpt-4"));
    assert_eq!(parsed.workspace.as_deref(), Some("ws-a"));
    assert_eq!(parsed.created_at.as_deref(), Some("2024-01-01T00:00:00Z"));
    assert_eq!(
        parsed.last_message_at.as_deref(),
        Some("2024-01-01T00:00:02Z")
    );
    assert!(parsed.content.contains("[user] hello"));
    assert!(parsed.content.contains("[assistant] hi there"));
}

#[test]
fn parses_json_sessions() {
    let input = include_str!("fixtures/session.json");
    let parsed = parse_json(input).expect("json parse");

    assert_eq!(parsed.message_count, 2);
    assert_eq!(parsed.title.as_deref(), Some("first question"));
    assert_eq!(parsed.agent.as_deref(), Some("gpt-4"));
    assert_eq!(parsed.workspace.as_deref(), Some("ws-b"));
    assert_eq!(parsed.created_at.as_deref(), Some("2024-02-01T00:00:00Z"));
    assert_eq!(
        parsed.last_message_at.as_deref(),
        Some("2024-02-01T00:00:02Z")
    );
    assert!(parsed.content.contains("[user] first question"));
    assert!(parsed.content.contains("[assistant] first answer"));
}

#[test]
fn parses_markdown_sessions() {
    let input = include_str!("fixtures/session.md");
    let parsed = parse_markdown(input);

    assert_eq!(parsed.message_count, 2);
    assert_eq!(parsed.title.as_deref(), Some("hello from md"));
    assert!(parsed.content.contains("[user] hello from md"));
    assert!(parsed.content.contains("[assistant] hi from md"));
}
