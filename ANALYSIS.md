# mmem Deep Analysis Report

## Executive Summary

**Overall Grade: B+** — Well-structured Rust CLI with good error handling patterns.

### Fixed This Session ✅
1. Parse failures now remove stale indexed data
2. Code duplication (`message_content` ≡ `extract_content_array`) eliminated
3. Missing database indexes added (`workspace`, `repo_name`, `branch`)
4. SQLite `busy_timeout` added (5s)
5. Lint configuration added (`clippy.toml` + lib.rs)
6. Code duplication (`expand_home_path` x3) → extracted to `util::expand_home`

### Remaining Issues
1. **Semantic inconsistency**: `turn_index` means different things in DB vs `show` command
2. **N+1 query**: `find_messages` with `--around` does separate query per result

---

## 1. Remaining Critical Issues

### 1.1 Inconsistent `turn_index` Semantics 🔴

**Problem**: Two different definitions of "turn":

| Context | Definition | Source |
|---------|------------|--------|
| DB `messages.turn_index` | Index of messages with extractable **text** | `format_session_entry()` in parse.rs |
| `show --turn` | Index of all message events (incl **toolCall-only**) | `extract_message()` in parse.rs |

**Root cause**: 
- `extract_message()` returns `Some(ParsedMessage)` for toolCall-only entries (with empty text)
- `format_session_entry()` returns `None` for toolCall-only entries
- DB indexing uses `format_session_entry()`, but `show --turn` uses `extract_message()`

**Impact**: `mmem find` returns `path#turn_index` that may not match `mmem show <path> --turn <n>`.

**Fix options**:
1. **Align both to include toolCall-only** (recommended) — change `parse_jsonl`/`parse_json` to use `extract_message` instead of `format_session_entry`
2. **Align both to exclude toolCall-only** — change `load_entry_by_turn` to only count text messages
3. **Document the difference** — add clear docs that DB turn != show turn

```rust
// Option 1: In parse.rs, change parse_jsonl to use extract_message
if let Some(message) = extract_message(&value) {
    messages.push(message);
}
```

---

### 1.2 ~~Triple Duplication of `expand_home_path`~~ ✅ FIXED

Extracted to `src/util.rs` with:
- Shared `expand_home()` function
- Unit tests for edge cases
- Doc comments with examples

All three call sites (`main.rs`, `session.rs`, `scan.rs`) now use `crate::util::expand_home`.

---

## 2. Performance Issues

### 2.1 N+1 Query in Context Loading 🟡

**Location**: `src/query.rs:145-162`

```rust
for row in rows {
    let mut hit = row?;
    if filters.around > 0 {
        hit.context = Some(load_context(...)?);  // Query per result!
    }
}
```

**Impact**: With `--limit 100 --around 3`, executes 101 queries.

**Practical impact**: Low — typical usage is `--limit 5-20`. SQLite is fast for small queries.

**Fix options** (if needed):
1. Batch context load with `WHERE (path, turn) IN (...)`
2. Use window functions in main query
3. Accept as-is for CLI tool (recommended unless benchmarks show issues)

---

### 2.2 Long-Running Transaction During Scan 🟢

**Location**: `src/scan.rs:63-126`

Single transaction wraps entire filesystem walk.

**Actual impact**: Low — WAL mode (already enabled) allows concurrent readers. Write lock only blocks other writers.

**When to fix**: Only if scanning 10k+ files causes issues. Consider batch commits every 500 files.

---

## 3. Code Quality Issues

### 3.1 Stringly-Typed Magic Constants 🟡

JSON type discriminators scattered throughout parse.rs and session.rs:

```rust
// Repeated ~10 times across files
.get("type").and_then(|v| v.as_str()).map(|v| v == "session_meta")
.get("type").and_then(|v| v.as_str()).map(|v| v == "response_item")
.get("type").and_then(|v| v.as_str()).map(|v| v == "toolCall")
```

**Fix**: Extract to module constants + helper:

```rust
// src/parse.rs (at top)
mod json_types {
    pub const SESSION_META: &str = "session_meta";
    pub const RESPONSE_ITEM: &str = "response_item";
    pub const MESSAGE: &str = "message";
    pub const INPUT_TEXT: &str = "input_text";
    pub const TOOL_CALL: &str = "toolCall";
}

fn type_is(value: &Value, expected: &str) -> bool {
    value.get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t == expected)
}

// Usage:
if type_is(value, json_types::SESSION_META) { ... }
```

---

### 3.2 FTS5 Syntax Errors Not User-Friendly 🟡

**Location**: `src/query.rs`

When `--fts` mode is used, malformed queries (`AND AND`, unbalanced quotes) produce cryptic SQLite errors like:
```
sqlite error: fts5: syntax error near "AND"
```

**Fix**: Add a new error variant and catch FTS errors:

```rust
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("query is empty")]
    EmptyQuery,
    #[error("invalid FTS5 syntax in query: {query}")]
    InvalidFtsSyntax { query: String },
    #[error("sqlite error: {source}")]
    Sqlite { source: rusqlite::Error },
}

// In find_sessions/find_messages, wrap the query execution:
let rows = stmt.query_map(...).map_err(|e| {
    if e.to_string().contains("fts5: syntax error") {
        QueryError::InvalidFtsSyntax { query: query.to_string() }
    } else {
        QueryError::Sqlite { source: e }
    }
})?;
```

---

### 3.3 Silent Error Masking in Tool Extraction 🟢

**Location**: `src/session.rs:147-149`

```rust
let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
```

**Actual impact**: Low — malformed tool calls are rare, and "unknown" is a reasonable fallback for display.

**When to fix**: Only if debugging tool extraction issues becomes common.

---

## 4. Architecture Notes

### 4.1 main.rs Organization

At ~716 lines, `main.rs` is acceptable for a CLI. The output formatting functions (`emit_*`, `*_to_json`) could be extracted, but this is polish, not a problem.

**Recommendation**: Keep as-is unless adding significant new commands.

---

### 4.2 Error Type Design

Current: Per-module error enums + `Box<dyn Error>` in main.rs

**Assessment**: Fine for a CLI tool. The per-module errors are well-designed with `thiserror`.

**When to change**: Only if you need:
- Typed exit codes (e.g., 1=db error, 2=query error)
- Library API stability guarantees

---

### 4.3 Schema Evolution

Current approach (`ensure_column`) only handles additive column changes.

**Assessment**: Acceptable for a personal tool. The current migrations (`repo_root`, `repo_name`, `branch`) work.

**When to improve**: If you need:
- Column type changes
- Index rebuilds
- FTS schema changes

Then implement versioned migrations:
```sql
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY);
```

---

## 5. Linting Configuration (Implemented ✅)

### Current Setup

**clippy.toml**:
```toml
msrv = "1.85"
cognitive-complexity-threshold = 25
too-many-arguments-threshold = 7
too-many-lines-threshold = 100
```

**src/lib.rs**:
```rust
#![deny(unsafe_code)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]
#![warn(clippy::cognitive_complexity)]
#![warn(clippy::too_many_arguments)]
#![warn(clippy::too_many_lines)]
```

### Additional Recommendations for AI Development

Add to pre-commit or CI:

```bash
#!/bin/bash
set -e

# Format check
cargo fmt --check -p mmem

# Lint with strict warnings
cargo clippy -p mmem -- \
    -D warnings \
    -D clippy::dbg_macro \
    -D clippy::print_stdout \
    -D clippy::print_stderr \
    -W clippy::nursery

# Test
cargo test -p mmem
```

Consider also:
- `clippy::pedantic` (with targeted allows) for stricter checks
- `cargo-deny` for dependency auditing
- `cargo-semver-checks` if publishing as library

---

## 6. Documentation Gaps

### 6.1 Module Documentation

Each module should have `//!` docs. Priority order:

| Module | Purpose | Status |
|--------|---------|--------|
| parse.rs | Session file parsing (JSONL/JSON/MD) | Needs docs |
| session.rs | Tool call extraction, turn/line lookup | Needs docs |
| query.rs | FTS5 search, filter application | Needs docs |
| scan.rs | Filesystem walk, incremental indexing | Needs docs |
| index.rs | SQLite schema, CRUD operations | Has partial docs |

### 6.2 Function Documentation Priority

Non-obvious functions that need `///` docs:

| Function | Why |
|----------|-----|
| `normalize_role_filter()` | Complex interaction between `role` and `include_assistant` |
| `extract_message()` vs `format_session_entry()` | Different semantics (the turn_index bug source) |
| `build_field_set()` | Scope-dependent defaults |
| `infer_repo_info()` | Git subprocess + caching behavior |

---

## 7. Test Coverage

### 7.1 Current Coverage

| File | Tests | Coverage |
|------|-------|----------|
| parse.rs | 3 (in tests/parse.rs) | Basic happy paths |
| query.rs | 3 (2 unit + 1 integration) | Literal/FTS modes, filters |
| session.rs | 5 | Turn lookup, tool extraction, path resolution |
| scan.rs | 1 | Index/skip/remove cycle |
| index.rs | 1 | CRUD operations |
| stats.rs | 1 | Count and bounds |
| doctor.rs | 1 | Missing DB case |
| main.rs | 0 | No unit tests for helpers |

### 7.2 Missing Test Cases

**High priority**:
- `parse.rs`: Empty files, malformed JSON, toolCall-only entries
- `query.rs`: FTS syntax errors, empty results, `--around` context
- `scan.rs`: Parse failure → stale data removal (new behavior)

**Medium priority**:
- `main.rs`: `normalize_role_filter`, `trim_output`, `expand_home_path`

**Example test for new stale data behavior**:
```rust
#[test]
fn removes_stale_data_on_parse_failure() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.jsonl");
    
    // First: valid content
    std::fs::write(&file, r#"{"role":"user","content":"hello"}"#).unwrap();
    let mut conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    
    let stats = index_root(&mut conn, dir.path(), false).unwrap();
    assert_eq!(stats.indexed, 1);
    
    // Second: corrupt the file
    std::fs::write(&file, "not valid json {{{").unwrap();
    let stats = index_root(&mut conn, dir.path(), false).unwrap();
    
    assert_eq!(stats.parse_errors, 1);
    assert_eq!(stats.removed, 1);  // Should remove stale entry
    
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0);
}
```

---

## 8. Implementation Priority

### Immediate — All Done ✅
1. ✅ Fix parse failure stale data bug
2. ✅ Extract duplicate `message_content`
3. ✅ Add missing indexes
4. ✅ Add busy_timeout
5. ✅ Add lint configuration
6. ✅ Extract `expand_home_path` to util module

### This Week
7. 🔲 Decide and fix `turn_index` semantics — 30 min
8. 🔲 Add test for stale data removal — 15 min
9. 🔲 Extract JSON type constants — 20 min

### Later (As Needed)
10. 🔲 Improve FTS5 error messages
11. 🔲 Add module documentation
12. 🔲 Add main.rs unit tests

---

## Appendix: File Health Summary

| File | Lines | Status | Notes |
|------|-------|--------|-------|
| src/lib.rs | 30 | ✅ Good | Has lint config |
| src/util.rs | 64 | ✅ Good | New: shared expand_home |
| src/model.rs | 131 | ✅ Good | Clean data structures |
| src/stats.rs | 64 | ✅ Good | Simple, correct |
| src/doctor.rs | 62 | ✅ Good | Could add more checks |
| src/cli.rs | 144 | ✅ Good | Clean clap setup |
| src/index.rs | 263 | ✅ Good | Fixed: indexes, busy_timeout |
| src/query.rs | 275 | 🟡 OK | N+1 (acceptable), FTS errors |
| src/scan.rs | 285 | ✅ Good | Fixed: stale data, uses util |
| src/session.rs | 300 | ✅ Good | Uses shared util |
| src/parse.rs | 466 | 🟡 OK | Has magic strings |
| src/main.rs | 702 | ✅ Good | Cleaner after util extraction |
