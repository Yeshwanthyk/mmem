//! Core data structures for session indexing and search.
//!
//! # Parse-Time Types
//!
//! - [`ParsedMessage`]: A message extracted during parsing
//! - [`ParsedSession`]: A fully parsed session before database insertion
//!
//! # Database Types
//!
//! - [`SessionRecord`]: Session data for database storage
//! - [`MessageRecord`]: Message data for database storage
//!
//! # Query Result Types
//!
//! - [`SessionHit`]: Search result for session-scope queries
//! - [`MessageHit`]: Search result for message-scope queries
//! - [`MessageContext`]: Surrounding messages for context display

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMessage {
    pub role: Option<String>,
    pub text: String,
    pub timestamp: Option<String>,
}

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
    pub messages: Vec<ParsedMessage>,
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
            messages: Vec::new(),
        }
    }

    pub fn into_parts(
        self,
        path: String,
        mtime: i64,
        size: i64,
        hash: Option<String>,
    ) -> (SessionRecord, Vec<ParsedMessage>) {
        let record = SessionRecord {
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
            repo_root: None,
            repo_name: None,
            branch: None,
        };

        (record, self.messages)
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
    pub repo_root: Option<String>,
    pub repo_name: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRecord {
    pub turn_index: i64,
    pub role: Option<String>,
    pub timestamp: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionHit {
    pub path: String,
    pub title: Option<String>,
    pub agent: Option<String>,
    pub workspace: Option<String>,
    pub repo_root: Option<String>,
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub last_message_at: Option<String>,
    pub snippet: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageContext {
    pub turn_index: i64,
    pub role: Option<String>,
    pub timestamp: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageHit {
    pub path: String,
    pub title: Option<String>,
    pub agent: Option<String>,
    pub workspace: Option<String>,
    pub repo_root: Option<String>,
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub turn_index: i64,
    pub role: Option<String>,
    pub timestamp: Option<String>,
    pub text: String,
    pub score: f64,
    pub context: Option<Vec<MessageContext>>,
}
