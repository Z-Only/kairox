# Test Coverage Expansion — Design Spec

**Date:** 2026-05-03
**Status:** Approved
**Scope:** Expand test coverage for core crates (agent-store, agent-tools, agent-runtime, agent-core) with a focus on integration tests and critical untested modules.

---

## Problem

Kairox has ~260 unit tests and ~8 integration tests across 10 crates, but coverage is uneven:

1. **`agent-store` metadata module has zero integration tests** — the recently merged session management feature added 8 new metadata methods (CRUD, soft delete, cleanup) but only 6 unit tests within `event_store.rs`, none covering cross-module behavior
2. **`agent-tools/filesystem.rs` has zero tests** — file I/O tool with workspace escape protection is completely untested
3. **`agent-runtime` has only 5 unit + 3 integration tests** — the most complex crate (agent loop, memory protocol, permission flow) is critically undertested
4. **`agent-core` event serialization has no roundtrip tests** — DomainEvent serde correctness is assumed but never verified across all EventPayload variants
5. **`agent-runtime/task_graph.rs` has only 1 test** — dependency resolution, state transitions, and error paths are untested

The recent session management merge (PR #35) arrived with 5+ bugfix commits, indicating insufficient pre-merge test coverage.

## Design Decisions

| Decision          | Choice                                                                                   | Rationale                                                                          |
| ----------------- | ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Primary test type | Integration tests                                                                        | Cross-module flows are where bugs cluster; unit tests already cover basic logic    |
| Scope             | Core crates only (store, tools, runtime, core)                                           | Highest ROI — these are upstream of all UIs                                        |
| Approach          | Add test modules to existing files + new integration test files                          | Follows project conventions; no infra changes needed                               |
| Test helpers      | Reuse FakeModelClient, SqliteEventStore::in_memory(), SqliteMemoryStore::new_in_memory() | No new test infrastructure                                                         |
| TDD               | Tests written first where adding new logic                                               | For existing code with gaps, tests are written against the existing implementation |

## Task Breakdown

### Task 1: agent-store — Metadata Edge Cases & Event Correlation

**New tests in `crates/agent-store/src/event_store.rs`**:

- `upsert_session_for_nonexistent_workspace_still_inserts` — SQLite doesn't enforce FK without PRAGMA, verify current behavior
- `list_active_sessions_returns_empty_for_unknown_workspace` — boundary: no sessions exist
- `cleanup_expired_also_deletes_associated_events` — verify events table rows are removed for expired sessions
- `cleanup_expired_skips_recently_deleted` — sessions deleted < threshold are retained
- `upsert_session_updates_existing_record` — second upsert changes title/model_id/provider
- `metadata_survives_across_reopen` — file-backed SQLite: insert metadata, reconnect, verify persistence

### Task 2: agent-tools — Filesystem Tool Tests

**New tests in `crates/agent-tools/src/filesystem.rs`** (add `#[cfg(test)] mod tests`):

- `read_file_within_workspace` — read a temp file, get expected content
- `read_file_truncates_at_output_limit` — large file is truncated, `truncated` flag is true
- `read_file_outside_workspace_returns_escape_error` — path traversal `../etc/passwd` is rejected
- `read_nonexistent_file_returns_error` — file doesn't exist
- `definition_has_correct_tool_id` — verify tool ID is `"fs.read"`

### Task 3: agent-runtime — TaskGraph Comprehensive Tests

**New tests in `crates/agent-runtime/src/task_graph.rs`** (expand existing `mod tests`):

- `empty_graph_has_no_ready_tasks` — default graph returns empty
- `independent_tasks_are_all_ready` — two tasks with no dependencies
- `diamond_dependency_unblocks_after_all_parents_complete` — A→B, A→C, B+C→D
- `mark_completed_unknown_task_returns_error` — invalid TaskId
- `partial_completion_only_unblocks_fully_resolved` — one dep done, one pending
- `multiple_tasks_share_dependency` — A→B, A→C; completing A unblocks both

### Task 4: agent-core — EventPayload Serde Roundtrip

**New integration test file `crates/agent-core/tests/event_roundtrip.rs`**:

- Test every `EventPayload` variant serializes to JSON and deserializes back to the same value
- Verify `DomainEvent` with each payload type roundtrips correctly
- Covers all 20+ variants with representative data

### Task 5: agent-runtime — Session Lifecycle Integration Test

**New tests in `crates/agent-runtime/tests/session_lifecycle.rs`**:

- `full_workspace_session_round_trip` — open_workspace → start_session → send_message → get_projection → cancel → get_trace
- `session_metadata_persists_across_reopen` — file-backed SQLite: create workspace+session, reconnect, list_workspaces + list_sessions recover data
- `rename_and_soft_delete_flow` — create → rename → verify title → soft_delete → verify hidden from list
- `multiple_sessions_in_same_workspace` — 3 sessions, list returns all, delete one, list returns 2
- `cleanup_expired_removes_old_sessions_and_events` — delete session, advance time past threshold, cleanup, verify events removed

### Task 6: agent-runtime — Memory Protocol Integration Test

**New tests in `crates/agent-runtime/tests/memory_protocol.rs`**:

- `session_scope_memory_auto_accepted` — model returns `<memory scope="session">note</memory>`, verify stored and MemoryAccepted event emitted
- `user_scope_memory_requires_approval_in_suggest_mode` — `<memory scope="user" key="lang">Rust</memory>`, verify MemoryProposed but NOT stored in Suggest mode
- `workspace_scope_memory_auto_accepted_in_autonomous_mode` — Autonomous mode auto-accepts workspace memories
- `memory_markers_stripped_from_display` — assistant content includes `<memory>` tags, but AssistantMessageCompleted content has them stripped
- `memories_injected_into_system_prompt` — stored memory is included in next message's system prompt context

## File Changes Summary

| Crate         | File                                              | Change                                                                   |
| ------------- | ------------------------------------------------- | ------------------------------------------------------------------------ |
| agent-store   | `crates/agent-store/src/event_store.rs`           | +6 metadata edge-case tests                                              |
| agent-tools   | `crates/agent-tools/src/filesystem.rs`            | +5 filesystem tool tests (new `mod tests`)                               |
| agent-runtime | `crates/agent-runtime/src/task_graph.rs`          | +5 task graph tests (expand existing `mod tests`)                        |
| agent-core    | `crates/agent-core/tests/event_roundtrip.rs`      | +1 integration test file (serde roundtrip for all EventPayload variants) |
| agent-runtime | `crates/agent-runtime/tests/session_lifecycle.rs` | +5 integration tests                                                     |
| agent-runtime | `crates/agent-runtime/tests/memory_protocol.rs`   | +5 integration tests                                                     |

**No production code changes.** This is purely additive testing.

## Testing Strategy

| Layer         | Test                | What it verifies                                                 |
| ------------- | ------------------- | ---------------------------------------------------------------- |
| agent-store   | Metadata edge cases | FK behavior, cleanup-event correlation, persistence              |
| agent-tools   | Filesystem tool     | Read, truncation, escape protection, errors                      |
| agent-runtime | TaskGraph           | Dependency resolution, state transitions, error paths            |
| agent-core    | EventPayload serde  | Every variant roundtrips through JSON                            |
| agent-runtime | Session lifecycle   | Full CRUD + persistence + multi-session                          |
| agent-runtime | Memory protocol     | Marker parsing, scope auth, content stripping, context injection |

## Non-Goals

- GUI/Vue tests (separate iteration)
- TUI tests (71 unit tests already reasonable)
- Performance/benchmark tests
- Agent-models adapter tests (require real HTTP)
- Adding new test infrastructure or fixtures beyond what exists
