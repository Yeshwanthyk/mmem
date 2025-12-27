# mmem (Marvin Memory) - Plan

## 0. Intent
mmem indexes Marvin session files under `~/.config/marvin/sessions/` and exposes a fast, offline search CLI.
Keep it simple: scan, parse, index, query.

## 1. Goals / Non-goals
Goals
- Fast lookup of old sessions by keyword + filters
- Incremental indexing (no full reparse on every search)
- Minimal dependencies, local-only, no network required
- Robust parsing across JSONL / JSON / Markdown

Non-goals
- Full semantic embedding search (later optional)
- Rewriting session data
- UI beyond CLI

## 2. Sources and Format Assumptions
Primary root
- `~/.config/marvin/sessions/`

Supported file types (best-effort)
- `.jsonl` line-delimited session events
- `.json` session arrays or objects with `messages`
- `.md` plain text transcripts

Marvin/Codex-style entries to handle
- `{ type: "response_item", payload: { type: "message", role, content: [...] } }`
- `{ type: "session_meta", ... }` (skip)
- `{ role, content }` / `{ text }` / `{ message }`

## 3. Architecture (Minimal Pipeline)

ASCII flow

[filesystem] -> [scanner] -> [parser] -> [indexer] -> [sqlite+fts5] -> [query]

Scanner
- Walk dir with `walkdir`
- Detect changes via `(mtime, size)` and optional `blake3` hash
- Remove index entries for missing files

Parser
- Best-effort JSONL / JSON / Markdown
- Normalize message content into `"[role] text"` lines
- Extract `created_at`, `last_message_at`, `agent`, `message_count`, `title`, `snippet`

Indexer
- SQLite table for metadata
- FTS5 virtual table for full-text
- Upsert by `path` (primary key)

Query
- `bm25` rank from FTS
- Secondary sort by `last_message_at` (older/newer)
- Filters: agent, workspace, date range

## 4. Data Model
SessionRecord (logical)
- path: absolute file path
- mtime: unix seconds
- size: bytes
- hash: blake3 (optional)
- created_at: ISO8601 string
- last_message_at: ISO8601 string
- agent: string
- workspace: string (optional)
- title: first user line (trimmed)
- message_count: int
- snippet: short excerpt (<= 240 chars)
- content: full normalized text (fts only)

## 5. SQLite Schema (tight)

```sql
CREATE TABLE IF NOT EXISTS sessions (
  path TEXT PRIMARY KEY,
  mtime INTEGER NOT NULL,
  size INTEGER NOT NULL,
  hash TEXT,
  created_at TEXT,
  last_message_at TEXT,
  agent TEXT,
  workspace TEXT,
  title TEXT,
  message_count INTEGER,
  snippet TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
  content,
  path UNINDEXED
);

CREATE INDEX IF NOT EXISTS idx_sessions_last_message_at ON sessions(last_message_at);
CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent);
```

## 6. CLI Surface (proposed)

```txt
mmem index [--full] [--root <path>] [--json]
mmem find <query> [--days N] [--before <iso>] [--after <iso>] [--agent A] [--workspace W] [--limit N] [--json]
mmem stats [--json]
mmem doctor [--json]
```

Flags detail
- `--root`: default `~/.config/marvin/sessions`
- `--days`: lookback window (relative to now)
- `--before/--after`: absolute time bounds
- `--limit`: default 10

## 7. Directory Layout

```txt
mmem/
  Cargo.toml
  src/
    main.rs
    cli.rs
    scan.rs
    parse.rs
    index.rs
    query.rs
    model.rs
    time.rs
  tests/
    fixtures/
      session.jsonl
      session.json
      session.md
```

## 8. Parsing Logic (borrowed patterns)
From `cass.ts` in cass_memory_system
- `coerceContent` to normalize text blocks
- `formatSessionEntry` to handle `response_item` and skip `session_meta`
- `joinMessages` to build searchable content
- `handleSessionExportFailure` fallback for JSONL/JSON/MD

Short Rust-ish sketch

```rust
fn coerce_content(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() { return Some(s.to_string()); }
    if let Some(arr) = v.as_array() {
        let parts: Vec<String> = arr.iter().filter_map(coerce_content).collect();
        return if parts.is_empty() { None } else { Some(parts.join("\n")) };
    }
    if v.get("type").and_then(|t| t.as_str()) == Some("input_text") {
        return v.get("text").and_then(|t| t.as_str()).map(|s| s.to_string());
    }
    v.get("content").and_then(|c| c.as_str()).map(|s| s.to_string())
}
```

## 9. Incremental Indexing Strategy
- Maintain `sessions` rows keyed by `path`
- On scan:
  - If file missing: delete row + fts row
  - If `(mtime,size)` unchanged: skip
  - Else: parse + upsert metadata + replace fts content

## 10. Query Strategy
- Use `sessions_fts MATCH ?` for keywords
- Rank by `bm25(sessions_fts)`
- Join metadata to include `title`, `agent`, `last_message_at`
- Sort by rank, then `last_message_at` when tie

Example SQL

```sql
SELECT s.path, s.title, s.agent, s.last_message_at, s.snippet,
       bm25(sessions_fts) AS score
FROM sessions_fts
JOIN sessions s ON s.path = sessions_fts.path
WHERE sessions_fts MATCH ?
  AND (s.agent = ? OR ? IS NULL)
  AND (s.last_message_at >= ? OR ? IS NULL)
ORDER BY score ASC, s.last_message_at DESC
LIMIT ?;
```

## 11. Diagnostics
- `mmem stats` prints indexed count, oldest/newest dates, parse failures
- `mmem doctor` checks:
  - root directory exists
  - sqlite + fts5 availability
  - last index time

## 12. Milestones
1) Parser + fixtures
2) SQLite schema + indexer
3) CLI find + basic filters
4) Incremental indexing + delete handling
5) Stats + doctor
6) Perf tuning (batch inserts, WAL mode)
