use serde::Serialize;

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

    pub fn into_record(
        self,
        path: String,
        mtime: i64,
        size: i64,
        hash: Option<String>,
    ) -> SessionRecord {
        SessionRecord {
            path,
            mtime,
            size,
            hash,
            created_at: self.created_at,
            last_message_at: self.last_message_at,
            agent: self.agent,
            workspace: self.workspace,
            title: self.title,
            message_count: self.message_count as i64,
            snippet: self.snippet,
            content: self.content,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub path: String,
    pub mtime: i64,
    pub size: i64,
    pub hash: Option<String>,
    pub created_at: Option<String>,
    pub last_message_at: Option<String>,
    pub agent: Option<String>,
    pub workspace: Option<String>,
    pub title: Option<String>,
    pub message_count: i64,
    pub snippet: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionHit {
    pub path: String,
    pub title: Option<String>,
    pub agent: Option<String>,
    pub workspace: Option<String>,
    pub last_message_at: Option<String>,
    pub snippet: Option<String>,
    pub score: f64,
}
