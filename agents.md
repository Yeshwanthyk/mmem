# Agents

This repo uses subagents for parallel searches and targeted reviews.

## Project: mmem

Marvin session memory search CLI. Indexes AI session transcripts (JSONL/JSON/MD) into SQLite+FTS5 for full-text search with metadata filtering.

**Stack:** Rust 2024 edition, SQLite (rusqlite), clap, serde, thiserror

## Module Map

| Module | Purpose |
|--------|---------|
| `cli` | Argument parsing (clap derive) |
| `scan` | Filesystem walk, incremental indexing, git info extraction |
| `parse` | JSONL/JSON/Markdown parsing, message extraction |
| `index` | SQLite schema, upsert/delete, FTS5 population |
| `query` | FTS5 search, filter application, context loading |
| `session` | Tool call extraction, turn/line lookup in session files |
| `model` | Core data structures (ParsedSession, SessionRecord, MessageHit, etc.) |
| `stats` | Index statistics queries |
| `doctor` | Health check diagnostics |

## Key Patterns

- Error types per module via `thiserror`
- `From` impls for error conversion
- Transaction-based batch operations (`_tx` suffix functions)
- Incremental indexing by mtime/size comparison
- Let-else and if-let chains (Rust 2024)
- Optional filters passed as `Option<T>` to SQL via params

## Subagents

| Agent | Use Case |
|-------|----------|
| `review-explain` | Fast triage, file ordering for reviews |
| `review-deep` | Deep file-by-file review with extended thinking |
| `review-verify` | Validate findings, reduce false positives |
| `librarian` | Documentation-quality explanations |
| `oracle` | Architecture/design feedback, debugging |

## Guidelines

- Keep tasks scoped and parallelize searches
- Use `rust-patterns.md` for Rust design/implementation patterns
- Prefer `ast-grep` over `rg` for structural code queries
- Run `cargo test` before committing changes
