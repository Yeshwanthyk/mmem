//! mmem - Marvin session memory search
//!
//! Indexes AI session transcripts (JSONL/JSON/MD) into SQLite+FTS5
//! for full-text search with metadata filtering.

// Safety lints - prevent common AI-generated mistakes
#![deny(unsafe_code)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]

// Panic prevention - warn in library code (allow in tests via #[cfg_attr])
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

// Code quality for AI readability
#![warn(clippy::cognitive_complexity)]
#![warn(clippy::too_many_arguments)]
#![warn(clippy::too_many_lines)]

pub mod doctor;
pub mod index;
pub mod model;
pub mod parse;
pub mod query;
pub mod scan;
pub mod session;
pub mod stats;
pub mod util;
