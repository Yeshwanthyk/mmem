use mmem::parse::{parse_json, parse_jsonl, parse_markdown};

#[test]
fn handles_empty_jsonl_file() {
    let parsed = parse_jsonl("").expect("empty parse");
    assert_eq!(parsed.message_count, 0);
    assert!(parsed.messages.is_empty());
    assert!(parsed.title.is_none());
}

#[test]
fn handles_whitespace_only_jsonl_file() {
    let parsed = parse_jsonl("   \n\n   \n").expect("whitespace parse");
    assert_eq!(parsed.message_count, 0);
    assert!(parsed.messages.is_empty());
}

#[test]
fn parses_jsonl_sessions() {
    let input = include_str!("fixtures/session.jsonl");
    let parsed = parse_jsonl(input).expect("jsonl parse");

    assert_eq!(parsed.message_count, 2);
    assert_eq!(parsed.messages.len(), 2);
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
    assert_eq!(parsed.messages[0].role.as_deref(), Some("user"));
    assert_eq!(parsed.messages[1].role.as_deref(), Some("assistant"));
}

#[test]
fn parses_json_sessions() {
    let input = include_str!("fixtures/session.json");
    let parsed = parse_json(input).expect("json parse");

    assert_eq!(parsed.message_count, 2);
    assert_eq!(parsed.messages.len(), 2);
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
    assert_eq!(parsed.messages[0].role.as_deref(), Some("user"));
    assert_eq!(parsed.messages[1].role.as_deref(), Some("assistant"));
}

#[test]
fn parses_markdown_sessions() {
    let input = include_str!("fixtures/session.md");
    let parsed = parse_markdown(input);

    assert_eq!(parsed.message_count, 2);
    assert_eq!(parsed.messages.len(), 2);
    assert_eq!(parsed.title.as_deref(), Some("hello from md"));
    assert!(parsed.content.contains("[user] hello from md"));
    assert!(parsed.content.contains("[assistant] hi from md"));
    assert_eq!(parsed.messages[0].role.as_deref(), Some("user"));
    assert_eq!(parsed.messages[1].role.as_deref(), Some("assistant"));
}

#[test]
fn includes_toolcall_only_entries_in_message_count() {
    let input = include_str!("fixtures/session_toolcall_only.jsonl");
    let parsed = parse_jsonl(input).expect("jsonl parse");

    // Should have 2 messages: toolCall-only assistant + user "thanks"
    assert_eq!(parsed.message_count, 2);
    assert_eq!(parsed.messages.len(), 2);

    // First message is toolCall-only (empty text)
    assert_eq!(parsed.messages[0].role.as_deref(), Some("assistant"));
    assert!(parsed.messages[0].text.is_empty());

    // Second message has text
    assert_eq!(parsed.messages[1].role.as_deref(), Some("user"));
    assert_eq!(parsed.messages[1].text, "thanks");
}
