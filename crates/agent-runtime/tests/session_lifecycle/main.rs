//! Session lifecycle integration tests for CRUD, persistence, and cleanup.
//!
//! Test bodies are split by behavior area into themed submodules. Shared
//! helpers live in `support`. Run with:
//!
//! ```text
//! cargo test -p agent-runtime --test session_lifecycle
//! ```

mod support;

mod blank_project;
mod cleanup;
mod mark_visible;
mod multi_session;
mod persistence;
mod project_archive;
mod project_metadata;
mod rename_delete;
mod round_trip;
mod worktree_session;
