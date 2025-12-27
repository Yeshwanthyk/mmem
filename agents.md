# Agents

This repo uses subagents for parallel searches and targeted reviews.

Preferred agents
- review-explain: fast triage and file ordering
- review-deep: deep, file-by-file review
- review-verify: validate findings, reduce false positives
- librarian: documentation-quality explanations
- oracle: architecture/design feedback

Guidelines
- Keep tasks scoped and parallelize searches.
- Use rust-patterns.md as a baseline for Rust design/implementation patterns.
