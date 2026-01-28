# mmem Analysis Fixes Implementation Plan

## Plan Metadata
- Created: 2025-01-28
- Status: complete
- Owner: yesh
- Assumptions:
  - Rust 2024 edition features (let-chains) available
  - All existing tests should continue to pass
  - No breaking changes to CLI interface

## Progress Tracking
- [x] Phase 1: Fix turn_index Semantic Inconsistency
- [x] Phase 2: Extract JSON Type Constants
- [x] Phase 3: Improve FTS5 Error Messages
- [x] Phase 4: Add Module Documentation
- [x] Phase 5: Add Missing Tests

## Overview

This plan addresses all remaining issues identified in ANALYSIS.md after the initial session fixes. The goal is to improve correctness, code quality, and maintainability while adding comprehensive test coverage.

## Current State

### Key Discoveries
- **turn_index bug** (`src/parse.rs:117,149`): DB indexing uses `format_session_entry()` which excludes toolCall-only entries, but `show --turn` uses `extract_message()` which includes them
- **Magic strings** (`src/parse.rs:49,70,76,219,228,321`): JSON type discriminators like `"session_meta"`, `"response_item"`, `"toolCall"` repeated throughout
- **FTS5 errors** (`src/query.rs:55-67`): `QueryError` has no variant for FTS syntax errors
- **Missing docs**: No `//!` module docs on parse.rs, session.rs, query.rs, scan.rs
- **Test gaps**: No tests for parse edge cases, FTS errors, stale data removal

### Pattern to Follow
```rust
// Existing error pattern in query.rs
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("query is empty")]
    EmptyQuery,
    #[error("sqlite error: {source}")]
    Sqlite { source: rusqlite::Error },
}
```

## Desired End State

1. `mmem find` turn_index matches `mmem show --turn` index
2. All JSON type strings defined as constants
3. FTS5 syntax errors produce user-friendly messages
4. All modules have `//!` documentation
5. Test coverage for edge cases and new behaviors

### Verification
```bash
# All tests pass
cargo test -p mmem

# Clippy clean
cargo clippy -p mmem -- -D warnings

# Turn index consistency (manual)
# 1. Create session with toolCall-only entry
# 2. Index it
# 3. Search for it
# 4. Verify show --turn matches find result
```

## Out of Scope
- N+1 query optimization (acceptable for typical usage)
- main.rs reorganization (not needed)
- Schema versioning (overkill for personal tool)
- Batch transaction commits (only needed for 10k+ files)

## Breaking Changes
None. All changes are internal implementation details.

## Dependency and Configuration Changes
None required.

## Error Handling Strategy
- FTS5 syntax errors: Catch SQLite errors containing "fts5: syntax error", wrap in new `QueryError::InvalidFtsSyntax` variant
- Parse errors: Already handled with stale data removal (fixed in previous session)

## Implementation Approach

**Why this order:**
1. Phase 1 (turn_index) is a correctness bug affecting user-visible behavior
2. Phase 2 (constants) is a refactor that makes Phase 3 cleaner
3. Phase 3 (FTS errors) improves UX
4. Phase 4 (docs) captures knowledge before it's lost
5. Phase 5 (tests) locks in correct behavior

**Alternative considered:** Could do Phase 5 first (TDD style), but the turn_index semantics need to be decided first to know what to test.

## Phase Dependencies and Parallelization
- Dependencies: Phase 2 and 3 can run after Phase 1; Phase 4 and 5 independent
- Parallelizable: Phase 4 (docs) can run in parallel with Phase 2/3
- Suggested @agents:
  - Main agent: Phases 1, 2, 3
  - Doc agent: Phase 4 (can be done in parallel)

---

## Phase 1: Fix turn_index Semantic Inconsistency

### Overview
Align DB `messages.turn_index` with `show --turn` by including toolCall-only entries in parsing. This ensures `mmem find` results can be directly used with `mmem show --turn`.

### Prerequisites
- [ ] Understand current behavior via test

### Change Checklist
- [ ] Change `parse_jsonl` to use `extract_message` instead of `format_session_entry`
- [ ] Change `parse_json` to use `extract_message` instead of `format_session_entry`
- [ ] Add test fixture with toolCall-only entry
- [ ] Add test verifying turn_index consistency

### Changes

#### 1. Update parse_jsonl to use extract_message
**File**: `src/parse.rs`
**Location**: lines 115-119

**Before**:
```rust
        update_meta_from_value(&mut meta, &value);
        if let Some(message) = format_session_entry(&value) {
            messages.push(message);
        }
```

**After**:
```rust
        update_meta_from_value(&mut meta, &value);
        if let Some(message) = extract_message(&value) {
            messages.push(message);
        }
```

**Why**: `extract_message` includes toolCall-only entries (with empty text), aligning DB turn_index with `show --turn` semantics.

#### 2. Update parse_json to use extract_message
**File**: `src/parse.rs`
**Location**: lines 147-151

**Before**:
```rust
    for entry in entries {
        update_meta_from_value(&mut meta, entry);
        if let Some(message) = format_session_entry(entry) {
            messages.push(message);
        }
    }
```

**After**:
```rust
    for entry in entries {
        update_meta_from_value(&mut meta, entry);
        if let Some(message) = extract_message(entry) {
            messages.push(message);
        }
    }
```

**Why**: Same as above, consistency between JSONL and JSON parsing.

#### 3. Add test fixture with toolCall-only entry
**File**: `tests/fixtures/session_toolcall_only.jsonl` (new file)

**Add**:
```json
{"type":"session_meta","created_at":"2024-01-01T00:00:00Z"}
{"type":"message","timestamp":"2024-01-01T00:00:01Z","message":{"role":"assistant","content":[{"type":"toolCall","name":"read","arguments":{"path":"/tmp/test.txt"}}]}}
{"type":"message","timestamp":"2024-01-01T00:00:02Z","message":{"role":"user","content":"thanks"}}
```

**Why**: Entry at turn 0 has no text content, only a toolCall. This tests the semantic alignment.

#### 4. Add test for turn_index consistency
**File**: `tests/parse.rs`

**Add** (at end of file):
```rust
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
```

**Why**: Verifies that toolCall-only entries are now counted as messages, aligning with `show --turn` behavior.

### Edge Cases to Handle
- [x] Empty text for toolCall-only: Already handled by `extract_message` returning `ParsedMessage { text: String::new(), ... }`
- [x] Mixed content (text + toolCall): `format_session_entry` extracts text, `extract_message` falls back to it

### Success Criteria

**Automated**:
```bash
cargo test -p mmem -- parse::includes_toolcall  # New test passes
cargo test -p mmem                               # All existing tests pass
cargo clippy -p mmem -- -D warnings             # No warnings
```

**Manual**:
- [ ] Index a session with toolCall-only entries
- [ ] Run `mmem find` to get turn_index
- [ ] Run `mmem show <path> --turn <index>` and verify it matches

### Rollback
```bash
git restore -- src/parse.rs tests/parse.rs
rm tests/fixtures/session_toolcall_only.jsonl
```

### Notes
Space for implementer discoveries.

---

## Phase 2: Extract JSON Type Constants

### Overview
Replace scattered string literals with named constants and add a `type_is` helper function for cleaner, more maintainable code.

### Prerequisites
- [ ] Phase 1 complete (or can be done in parallel with care)

### Change Checklist
- [ ] Add `json_types` module with constants
- [ ] Add `type_is` helper function
- [ ] Replace all magic strings in parse.rs
- [ ] Add lint to catch future magic strings

### Changes

#### 1. Add json_types constants module
**File**: `src/parse.rs`
**Location**: After imports (around line 5)

**Add**:
```rust
/// JSON type discriminator constants used in session formats.
mod json_types {
    pub const SESSION_META: &str = "session_meta";
    pub const RESPONSE_ITEM: &str = "response_item";
    pub const MESSAGE: &str = "message";
    pub const INPUT_TEXT: &str = "input_text";
    pub const TOOL_CALL: &str = "toolCall";
}

/// Check if a JSON value has a specific "type" field value.
fn type_is(value: &Value, expected: &str) -> bool {
    value
        .get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t == expected)
}
```

**Why**: Centralizes magic strings, enables typo detection at compile time, and reduces repetition.

#### 2. Replace magic strings in has_tool_call
**File**: `src/parse.rs`
**Location**: lines 47-50

**Before**:
```rust
    content.iter().any(|item| {
        item.get("type")
            .and_then(|t| t.as_str())
            .map(|t| t == "toolCall")
            .unwrap_or(false)
    })
```

**After**:
```rust
    content.iter().any(|item| type_is(item, json_types::TOOL_CALL))
```

#### 3. Replace magic strings in extract_content_array
**File**: `src/parse.rs`
**Location**: lines 68-78

**Before**:
```rust
    if value
        .get("type")
        .and_then(|v| v.as_str())
        .map(|v| v == "response_item")
        .unwrap_or(false)
        && let Some(payload) = value.get("payload")
        && payload
            .get("type")
            .and_then(|v| v.as_str())
            .map(|v| v == "message")
            .unwrap_or(false)
```

**After**:
```rust
    if type_is(value, json_types::RESPONSE_ITEM)
        && let Some(payload) = value.get("payload")
        && type_is(payload, json_types::MESSAGE)
```

#### 4. Replace magic strings in format_session_entry
**File**: `src/parse.rs`
**Location**: lines 217-230

**Before**:
```rust
fn format_session_entry(value: &Value) -> Option<ParsedMessage> {
    if value
        .get("type")
        .and_then(|v| v.as_str())
        .map(|v| v == "session_meta")
        .unwrap_or(false)
    {
        return None;
    }

    if value
        .get("type")
        .and_then(|v| v.as_str())
        .map(|v| v == "response_item")
        .unwrap_or(false)
```

**After**:
```rust
fn format_session_entry(value: &Value) -> Option<ParsedMessage> {
    if type_is(value, json_types::SESSION_META) {
        return None;
    }

    if type_is(value, json_types::RESPONSE_ITEM)
```

#### 5. Replace magic strings in coerce_content
**File**: `src/parse.rs`
**Location**: lines 319-322

**Before**:
```rust
    if value
        .get("type")
        .and_then(|v| v.as_str())
        .map(|v| v == "input_text")
        .unwrap_or(false)
```

**After**:
```rust
    if type_is(value, json_types::INPUT_TEXT)
```

### Success Criteria

**Automated**:
```bash
cargo test -p mmem                   # All tests pass
cargo clippy -p mmem -- -D warnings  # No warnings
```

**Manual**:
- [ ] `rg '"session_meta"|"response_item"|"toolCall"|"input_text"' src/parse.rs` returns only the constants definition

### Rollback
```bash
git restore -- src/parse.rs
```

---

## Phase 3: Improve FTS5 Error Messages

### Overview
Add a dedicated error variant for FTS5 syntax errors so users get helpful messages instead of raw SQLite errors.

### Prerequisites
- [ ] Phase 1 complete

### Change Checklist
- [ ] Add `InvalidFtsSyntax` variant to `QueryError`
- [ ] Detect FTS5 errors in find_sessions
- [ ] Detect FTS5 errors in find_messages
- [ ] Add test for FTS5 error handling

### Changes

#### 1. Add InvalidFtsSyntax error variant
**File**: `src/query.rs`
**Location**: lines 55-62

**Before**:
```rust
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("query is empty")]
    EmptyQuery,
    #[error("sqlite error: {source}")]
    Sqlite { source: rusqlite::Error },
}

impl From<rusqlite::Error> for QueryError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Sqlite { source }
    }
}
```

**After**:
```rust
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("query is empty")]
    EmptyQuery,
    #[error("invalid FTS5 syntax: {query}")]
    InvalidFtsSyntax { query: String },
    #[error("sqlite error: {source}")]
    Sqlite { source: rusqlite::Error },
}

/// Convert rusqlite errors, detecting FTS5 syntax errors.
fn convert_sqlite_error(e: rusqlite::Error, query: &str) -> QueryError {
    let msg = e.to_string();
    if msg.contains("fts5: syntax error") || msg.contains("fts5: parse error") {
        QueryError::InvalidFtsSyntax { query: query.to_string() }
    } else {
        QueryError::Sqlite { source: e }
    }
}
```

**Remove** the `From` impl (we'll use explicit conversion to pass query context).

#### 2. Update find_sessions to detect FTS5 errors
**File**: `src/query.rs`
**Location**: in `find_sessions` function, around line 108

**Before**:
```rust
    let mut stmt = conn.prepare(FIND_SESSIONS_SQL)?;
    let rows = stmt.query_map(
```

**After**:
```rust
    let mut stmt = conn.prepare(FIND_SESSIONS_SQL).map_err(|e| convert_sqlite_error(e, &query))?;
    let rows = stmt.query_map(
        // ... params unchanged
    ).map_err(|e| convert_sqlite_error(e, &query))?;
```

Also update the row iteration:
**Before**:
```rust
    for row in rows {
        results.push(row?);
    }
```

**After**:
```rust
    for row in rows {
        results.push(row.map_err(|e| convert_sqlite_error(e, &query))?);
    }
```

#### 3. Update find_messages to detect FTS5 errors
**File**: `src/query.rs`
**Location**: in `find_messages` function

Apply the same pattern as find_sessions:
- Wrap `conn.prepare()` with `map_err(|e| convert_sqlite_error(e, &query))`
- Wrap `stmt.query_map()` with `map_err(|e| convert_sqlite_error(e, &query))`
- Wrap `row?` with `row.map_err(|e| convert_sqlite_error(e, &query))?`

#### 4. Update load_context error handling
**File**: `src/query.rs`
**Location**: in `load_context` function

This function doesn't use FTS, so keep the `?` operator (uses `From` impl).

Wait, we removed the From impl. We need to keep it for non-FTS queries:

**Add back** (after `convert_sqlite_error`):
```rust
impl From<rusqlite::Error> for QueryError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Sqlite { source }
    }
}
```

This keeps `load_context` working with `?`.

#### 5. Add test for FTS5 error handling
**File**: `tests/query.rs`

**Add** (at end of file):
```rust
#[test]
fn fts_syntax_error_produces_helpful_message() {
    let conn = Connection::open_in_memory().expect("db");
    init_schema(&conn).expect("schema");

    // Deliberately invalid FTS5 query
    let filters = FindFilters {
        query_mode: QueryMode::Fts,
        limit: 5,
        ..Default::default()
    };

    let result = find_messages(&conn, "AND AND", &filters);
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, QueryError::InvalidFtsSyntax { .. }),
        "Expected InvalidFtsSyntax, got: {:?}",
        err
    );
}
```

**Add import** at top of file:
```rust
use mmem::query::QueryMode;
```

### Success Criteria

**Automated**:
```bash
cargo test -p mmem -- fts_syntax  # New test passes
cargo test -p mmem               # All tests pass
```

**Manual**:
```bash
# Should show "invalid FTS5 syntax" not raw SQLite error
mmem find "AND AND" --fts
```

### Rollback
```bash
git restore -- src/query.rs tests/query.rs
```

---

## Phase 4: Add Module Documentation

### Overview
Add `//!` module-level documentation to all source files explaining their purpose, key types, and usage patterns.

### Prerequisites
- [ ] None (can run in parallel with other phases)

### Change Checklist
- [ ] Add docs to parse.rs
- [ ] Add docs to session.rs
- [ ] Add docs to query.rs
- [ ] Add docs to scan.rs
- [ ] Add docs to index.rs
- [ ] Add docs to model.rs
- [ ] Add docs to doctor.rs
- [ ] Add docs to stats.rs
- [ ] Add docs to cli.rs

### Changes

#### 1. parse.rs module docs
**File**: `src/parse.rs`
**Location**: line 1

**Add**:
```rust
//! Session file parsing for JSONL, JSON, and Markdown formats.
//!
//! This module extracts messages, metadata, and content from AI session files
//! for FTS5 indexing. It handles multiple session formats from different AI agents.
//!
//! # Key Functions
//!
//! - [`parse_jsonl`]: Parse newline-delimited JSON session files
//! - [`parse_json`]: Parse single JSON session files
//! - [`parse_markdown`]: Parse markdown conversation logs
//! - [`extract_message`]: Extract a single message from a JSON value
//!
//! # Turn Index Semantics
//!
//! Messages are indexed including toolCall-only entries (entries with no text content
//! but containing tool invocations). This ensures consistency between:
//! - Database `messages.turn_index`
//! - `mmem show --turn` command
//!
//! A toolCall-only message will have `text: ""` but still count as a turn.

```

#### 2. session.rs module docs
**File**: `src/session.rs`
**Location**: line 1

**Add**:
```rust
//! Session file inspection and tool call extraction.
//!
//! This module provides runtime inspection of JSONL session files without
//! going through the database index. Used by the `mmem show` command.
//!
//! # Key Functions
//!
//! - [`load_entry_by_turn`]: Load a specific message by turn index
//! - [`load_entry_by_line`]: Load a specific line from a session file
//! - [`scan_tool_calls`]: Find all tool calls in a session
//! - [`extract_tool_calls`]: Extract tool calls from a JSON message
//! - [`resolve_session_path`]: Resolve a session ID prefix to a file path
//!
//! # Turn Index Semantics
//!
//! Turn indices match the database `messages.turn_index` and include all message
//! events, including toolCall-only entries with no text content.

```

#### 3. query.rs module docs
**File**: `src/query.rs`
**Location**: line 1

**Add**:
```rust
//! FTS5 full-text search and filter application.
//!
//! This module implements search queries against the SQLite FTS5 index,
//! with support for various filters (agent, workspace, repo, date range, etc.).
//!
//! # Query Modes
//!
//! - **Literal** (default): Each word is quoted for exact matching. Safe for
//!   dates and punctuation like `"2025-01-28"`.
//! - **FTS**: Raw FTS5 syntax for advanced queries like `title:rust AND async`.
//!
//! # Key Functions
//!
//! - [`find_sessions`]: Search session-level content
//! - [`find_messages`]: Search individual messages with optional context
//!
//! # Error Handling
//!
//! FTS5 syntax errors (in `--fts` mode) produce [`QueryError::InvalidFtsSyntax`]
//! with the original query for debugging.

```

#### 4. scan.rs module docs
**File**: `src/scan.rs`
**Location**: line 1

**Add**:
```rust
//! Filesystem scanning and incremental indexing.
//!
//! This module walks the sessions directory, parses session files, and
//! updates the SQLite index. It supports incremental indexing by comparing
//! file mtime/size against cached values.
//!
//! # Key Functions
//!
//! - [`index_root`]: Main entry point for indexing a sessions directory
//!
//! # Incremental Indexing
//!
//! Files are re-indexed only when mtime or size changes. Use `--full` to
//! force a complete reindex.
//!
//! # Parse Failure Handling
//!
//! If a previously-indexed file fails to parse, its stale data is removed
//! from the index to prevent returning outdated results.
//!
//! # Git Integration
//!
//! Extracts `repo_root`, `repo_name`, and `branch` from the workspace directory
//! using git commands. Results are cached per-workspace during a scan.

```

#### 5. index.rs module docs
**File**: `src/index.rs`
**Location**: line 1

**Add**:
```rust
//! SQLite schema management and CRUD operations.
//!
//! This module defines the database schema and provides functions for
//! upserting/removing sessions and messages, with FTS5 index maintenance.
//!
//! # Schema
//!
//! - `sessions`: Session metadata (path, agent, workspace, timestamps, etc.)
//! - `sessions_fts`: FTS5 index of session content
//! - `messages`: Individual messages with turn indices
//! - `messages_fts`: FTS5 index of message text
//!
//! # Key Functions
//!
//! - [`init_schema`]: Create tables and indexes
//! - [`configure_connection`]: Set WAL mode, busy timeout, etc.
//! - [`upsert_session`] / [`upsert_session_tx`]: Insert or update a session
//! - [`replace_messages_tx`]: Replace all messages for a session
//! - [`remove_session`] / [`remove_session_tx`]: Delete a session and its messages
//!
//! # Transaction Pattern
//!
//! Functions with `_tx` suffix operate within an existing transaction.
//! Non-`_tx` variants create their own transaction.

```

#### 6. model.rs module docs
**File**: `src/model.rs`
**Location**: line 1

**Add**:
```rust
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

```

#### 7. doctor.rs module docs
**File**: `src/doctor.rs`
**Location**: line 1

**Add**:
```rust
//! Health check diagnostics for mmem configuration.
//!
//! The doctor command checks:
//! - Sessions root directory exists
//! - Database file exists and is readable
//! - Schema is valid and queryable
//! - FTS5 extension is available
//!
//! # Key Functions
//!
//! - [`run_doctor`]: Generate a diagnostic report

```

#### 8. stats.rs module docs
**File**: `src/stats.rs`
**Location**: line 1

**Add**:
```rust
//! Index statistics and agent listing.
//!
//! # Key Functions
//!
//! - [`load_stats`]: Get session count and date bounds
//! - [`load_agents`]: List unique agents with session counts

```

#### 9. cli.rs module docs
**File**: `src/cli.rs`
**Location**: line 1

**Add**:
```rust
//! Command-line argument parsing with clap.
//!
//! Defines the CLI structure for mmem commands:
//! - `index`: Index sessions from disk
//! - `find`: Search sessions and messages
//! - `show`: Inspect tool calls in a session
//! - `stats`: Show index statistics
//! - `agents`: List unique agents
//! - `doctor`: Check index health

```

### Success Criteria

**Automated**:
```bash
cargo doc -p mmem --no-deps  # Generates docs without warnings
cargo test -p mmem           # All tests still pass
```

**Manual**:
- [ ] `cargo doc -p mmem --open` shows documentation for all modules

### Rollback
```bash
git restore -- src/*.rs
```

---

## Phase 5: Add Missing Tests

### Overview
Add test coverage for edge cases, new behaviors, and critical functions that lack tests.

### Prerequisites
- [ ] Phase 1 complete (turn_index fix)
- [ ] Phase 3 complete (FTS error handling)

### Change Checklist
- [ ] Add test for stale data removal on parse failure
- [ ] Add test for empty file parsing
- [ ] Add test for FTS empty results
- [ ] Add tests for normalize_role_filter in main.rs
- [ ] Add tests for trim_output in main.rs

### Changes

#### 1. Test for stale data removal
**File**: `tests/scan.rs`

**Add** (at end of file):
```rust
#[test]
fn removes_stale_data_on_parse_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("test.jsonl");

    // First: valid content
    std::fs::write(
        &file,
        r#"{"type":"response_item","payload":{"type":"message","role":"user","content":"hello"}}"#,
    )
    .expect("write valid");

    let mut conn = Connection::open_in_memory().expect("db");
    init_schema(&conn).expect("schema");

    let stats = index_root(&mut conn, dir.path(), false).expect("index valid");
    assert_eq!(stats.indexed, 1);
    assert_eq!(stats.parse_errors, 0);

    // Second: corrupt the file
    std::fs::write(&file, "not valid json {{{").expect("write corrupt");

    let stats = index_root(&mut conn, dir.path(), false).expect("index corrupt");
    assert_eq!(stats.parse_errors, 1);
    assert_eq!(stats.removed, 1);

    // Verify session was removed
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0);
}
```

**Add imports** at top:
```rust
use rusqlite::Connection;
```

#### 2. Test for empty file parsing
**File**: `tests/parse.rs`

**Add**:
```rust
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
```

#### 3. Test for FTS empty results
**File**: `tests/query.rs`

**Add**:
```rust
#[test]
fn returns_empty_for_no_matches() {
    let conn = Connection::open_in_memory().expect("db");
    init_schema(&conn).expect("schema");

    let filters = FindFilters {
        limit: 10,
        ..Default::default()
    };

    let results = find_messages(&conn, "nonexistent query term xyz", &filters).expect("query");
    assert!(results.is_empty());
}
```

#### 4. Unit tests for main.rs helpers
**File**: `src/main.rs`
**Location**: At end of file

**Add**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    mod normalize_role_filter {
        use super::*;

        #[test]
        fn defaults_to_user_without_include_assistant() {
            let result = normalize_role_filter(None, false);
            assert_eq!(result, Some("user".to_string()));
        }

        #[test]
        fn returns_none_with_include_assistant_and_no_role() {
            let result = normalize_role_filter(None, true);
            assert_eq!(result, None);
        }

        #[test]
        fn respects_explicit_role() {
            let result = normalize_role_filter(Some("assistant"), false);
            assert_eq!(result, Some("assistant".to_string()));
        }

        #[test]
        fn normalizes_role_to_lowercase() {
            let result = normalize_role_filter(Some("USER"), false);
            assert_eq!(result, Some("user".to_string()));
        }

        #[test]
        fn trims_whitespace() {
            let result = normalize_role_filter(Some("  user  "), false);
            assert_eq!(result, Some("user".to_string()));
        }
    }

    mod trim_output {
        use super::*;

        #[test]
        fn preserves_short_text() {
            let result = trim_output("short text");
            assert_eq!(result, "short text");
        }

        #[test]
        fn truncates_long_text() {
            let long = "a".repeat(200);
            let result = trim_output(&long);
            assert_eq!(result.len(), MAX_OUTPUT_LEN);
        }

        #[test]
        fn collapses_whitespace() {
            let result = trim_output("hello   world\n\ntest");
            assert_eq!(result, "hello world test");
        }
    }
}
```

### Success Criteria

**Automated**:
```bash
cargo test -p mmem  # All tests pass including new ones
```

**Coverage check**:
```bash
cargo test -p mmem 2>&1 | grep -E "^test .* ok$" | wc -l
# Should be significantly higher than before
```

### Rollback
```bash
git restore -- src/main.rs tests/*.rs
```

---

## Testing Strategy

### Unit Tests Added
- `parse.rs`: Empty files, whitespace-only, toolCall-only entries
- `query.rs`: FTS syntax errors, empty results
- `scan.rs`: Stale data removal on parse failure
- `main.rs`: `normalize_role_filter`, `trim_output`

### Integration Tests
- Turn index consistency (via parse test with fixture)
- FTS error handling (via query test)

### Manual Testing Checklist
1. [ ] Index a session directory with various file types
2. [ ] Search with `--fts` mode using invalid syntax
3. [ ] Verify turn index matches between `find` and `show --turn`
4. [ ] Corrupt a previously-indexed file and verify reindex removes it

## Anti-Patterns to Avoid
- **Don't add new magic strings**: Use `json_types::*` constants
- **Don't use `?` for FTS queries**: Use `map_err(|e| convert_sqlite_error(e, &query))`
- **Don't forget `_tx` suffix**: When adding new DB functions within transactions

## Open Questions
None - all decisions made in ANALYSIS.md.

## References
- Analysis: `ANALYSIS.md`
- Existing parse tests: `tests/parse.rs`
- Existing query tests: `tests/query.rs`
- FTS5 syntax: https://www.sqlite.org/fts5.html
