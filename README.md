# mmem

Full-text search and indexing for [Marvin](https://github.com/anthropics/claude-code) AI session transcripts.

## Overview

`mmem` indexes Marvin session files (JSONL, JSON, Markdown) into a SQLite database with FTS5 full-text search. Query across all your AI coding sessions by text content, filter by metadata (agent, repo, workspace, branch, time range), and inspect tool calls.

## Installation

```bash
cargo install --path .
```

Requires Rust 1.85+ (edition 2024).

## Quick Start

```bash
# Index all sessions (incremental by default)
mmem index

# Search for literal text (punctuation safe)
mmem find "quickdiff 2025-12-27"

# Search within last 7 days
mmem find "rust async" --days 7

# Filter by repository
mmem find "error handling" --repo my-project

# Use raw FTS5 query syntax
mmem find "title:rust AND async" --fts

# Show tool calls from a specific session
mmem show ~/.config/marvin/sessions/path/session.jsonl
```

## Commands

### `index`

Scan and index session files.

```bash
mmem index              # Incremental (skip unchanged files)
mmem index --full       # Re-index everything
mmem index --root /path # Custom sessions directory
mmem index --json       # JSON output
```

**Default paths:**
- Sessions: `~/.config/marvin/sessions/`
- Database: `~/.config/marvin/mmem.sqlite`

### `find`

Search across sessions or messages (literal by default).

```bash
mmem find <query> [options]
```

**Scope:**
- `--scope message` (default) - Search individual messages
- `--scope session` - Search entire sessions

**Filters:**
| Flag | Description |
|------|-------------|
| `--days N` | Last N days only |
| `--after DATE` | Messages after date (RFC3339) |
| `--before DATE` | Messages before date (RFC3339) |
| `--agent NAME` | Filter by agent name |
| `--workspace PATH` | Filter by workspace path |
| `--repo NAME` | Filter by repository name or path |
| `--branch NAME` | Filter by git branch |
| `--role ROLE` | Filter by message role (default: user) |
| `--include-assistant` | Include assistant messages |
| `--limit N` | Max results (default: 5) |
| `--fts` | Use raw FTS5 query syntax (advanced) |

**Output:**
| Flag | Description |
|------|-------------|
| `--json` | JSON array output |
| `--jsonl` | JSON Lines output |
| `--snippet` | Show text snippet in output |
| `--around N` | Include N messages of context |
| `--fields f1,f2` | Select output fields |

**Available fields:**
- Session: `path`, `title`, `agent`, `workspace`, `repo_root`, `repo_name`, `branch`, `last_message_at`, `snippet`, `score`
- Message: all session fields plus `turn_index`, `role`, `timestamp`, `text`, `context`

**Examples:**
```bash
# Find error discussions in last week
mmem find "error" --days 7 --snippet

# Search specific repo, JSON output
mmem find "refactor" --repo myapp --json

# Get context around matches
mmem find "bug fix" --around 2 --include-assistant

# Use raw FTS5 query syntax
mmem find "title:rust AND async" --fts

# Session-level search
mmem find "migration" --scope session --limit 10
```

### `show`

Inspect tool calls in a session JSONL (path or session id prefix).

```bash
mmem show <path|session_id> [options]
```

**Options:**
| Flag | Description |
|------|-------------|
| `--turn N` | Show specific turn (message index) |
| `--line N` | Show specific line number |
| `--tool NAME` | Filter by tool name |
| `--limit N` | Max tool calls to show |
| `--extract` | Extract and display file contents from read calls |
| `--json` | JSON output |

**Examples:**
```bash
# List all read tool calls (default)
mmem show session.jsonl

# Show by session id prefix
mmem show 1766632198584

# Show all tool calls from turn 5
mmem show session.jsonl --turn 5

# Extract file contents from read calls
mmem show session.jsonl --extract

# Filter specific tool
mmem show session.jsonl --tool write
```

### `stats`

Display index statistics.

```bash
mmem stats          # Human-readable
mmem stats --json   # JSON output
```

Output includes session count, oldest/newest message timestamps, and parse failures.

### `doctor`

Health check for mmem setup.

```bash
mmem doctor         # Human-readable
mmem doctor --json  # JSON output
```

Checks:
- Sessions root directory exists
- Database exists and is valid
- Schema integrity
- FTS5 availability
- Indexed session count

## Session Formats

### JSONL (Primary)

Marvin's native format. Each line is a JSON object representing a conversation event:

```jsonl
{"type":"summary","session":{"id":"...","created_at":"...","title":"..."}}
{"type":"response_item","payload":{"role":"user","content":[{"type":"text","text":"..."}]}}
{"type":"response_item","payload":{"role":"assistant","content":[{"type":"text","text":"..."}]}}
```

### JSON

Single JSON object with messages array:

```json
{
  "messages": [
    {"role": "user", "content": "..."},
    {"role": "assistant", "content": "..."}
  ]
}
```

### Markdown

Conversation in markdown format with role headers:

```markdown
# User
What is Rust's ownership model?

# Assistant
Rust's ownership model...
```

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Session   │────▶│    Index    │────▶│   SQLite    │
│    Files    │     │   (scan)    │     │   + FTS5    │
└─────────────┘     └─────────────┘     └─────────────┘
                                               │
                    ┌─────────────┐             │
                    │    Query    │◀────────────┘
                    │   (find)    │
                    └─────────────┘
```

**Modules:**
| Module | Purpose |
|--------|---------|
| `scan` | Filesystem traversal, incremental indexing |
| `parse` | JSONL/JSON/Markdown parsing |
| `index` | SQLite schema, upsert/delete operations |
| `query` | FTS5 search, filtering, context loading |
| `session` | Tool call extraction, entry lookup |
| `model` | Data structures |
| `stats` | Index statistics |
| `doctor` | Health diagnostics |
| `cli` | Argument parsing |

**Database Schema:**
- `sessions` - Session metadata (path, timestamps, agent, workspace, repo info)
- `sessions_fts` - FTS5 index on session content
- `messages` - Individual messages with turn index
- `messages_fts` - FTS5 index on message text

## Git Integration

During indexing, mmem attempts to extract repository information from the session's workspace:

- `repo_root` - Absolute path to git root
- `repo_name` - Repository directory name
- `branch` - Current branch at index time

This enables filtering searches by repository context.

## Performance

- Incremental indexing by mtime/size comparison
- WAL journal mode for concurrent reads
- BM25 ranking for search relevance
- Indexed columns for common filter predicates

## Development

```bash
# Run tests
cargo test

# Check types
cargo check

# Format
cargo fmt

# Lint
cargo clippy
```

## License

MIT
