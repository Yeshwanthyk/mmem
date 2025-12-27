#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSession {
    pub created_at: Option<String>,
    pub last_message_at: Option<String>,
    pub agent: Option<String>,
    pub workspace: Option<String>,
    pub title: Option<String>,
    pub message_count: usize,
    pub snippet: String,
    pub content: String,
}

impl ParsedSession {
    pub fn empty() -> Self {
        Self {
            created_at: None,
            last_message_at: None,
            agent: None,
            workspace: None,
            title: None,
            message_count: 0,
            snippet: String::new(),
            content: String::new(),
        }
    }
}
