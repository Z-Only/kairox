# Test Coverage Improvement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raise test coverage for P0 crates (agent-config, agent-models, agent-tools, agent-memory) from ~3-7% to >=20%, P1 crates (agent-core, agent-mcp) from ~8-13% to >=15-25%, and P2 (agent-runtime) from ~27% to >=35%.

**Architecture:** Three phases. Phase 1 crates are independent — dispatch all 4 subagents in parallel. Phase 2 after Phase 1 passes. Phase 3 after Phase 2 passes. Each task adds unit tests to existing modules only. No new production code, no new dependencies.

**Tech Stack:** Rust, tokio, sqlx (in-memory SQLite), FakeModelClient, trait-based fakes

---

## Phase 1 — P0 Crates (parallel subagents)

### Task 1: agent-config — validation & env resolution tests

**Files:** Modify `crates/agent-config/src/loader.rs`

Add tests to the `#[cfg(test)] mod tests` block at the end of the file.

**Test cases to add:**

1. `validate_rejects_openai_compatible_without_base_url` — `load_from_str` a profile with `provider = "openai_compatible"` and no `base_url`, then call `validate()`. Assert `Err`.
2. `validate_allows_ollama_without_base_url` — Ollama with no `base_url` passes `validate()`. Assert `Ok`.
3. `resolve_api_keys_reads_from_env` — Set env var `TEST_KEY_123=sk-abc`, profile with `api_key_env = "TEST_KEY_123"`, call `resolve_api_keys`. Assert `profile.api_key = Some("sk-abc")`.
4. `resolve_api_keys_does_not_overwrite_existing_key` — Profile with `api_key = Some("hardcoded")` and `api_key_env = "SOME_VAR"`. After `resolve_api_keys`, assert `api_key` still `"hardcoded"`.
5. `resolve_api_keys_fallback_empty_if_no_env_and_not_anthropic` — Profile with `api_key_env = "NONEXISTENT_VAR"`, provider not anthropic. After resolve, assert `api_key = None`.
6. `resolve_api_keys_noop_when_no_env_var` — Profile with `api_key_env` set but env var missing, not anthropic. Assert `api_key` stays `None`.
7. `config_parse_includes_context_policy` — Parse TOML with `[context]` section. Assert `auto_compact_threshold = 0.85` (default) for empty context section; override works.
8. `config_parse_merges_multiple_mcp_servers` — Parse 2 MCP servers. Assert both present, order preserved alphabetically.
9. `config_parse_stdio_mcp_with_all_fields` — Verify all fields: `command`, `args`, `env`, `cwd`, `keep_alive`, `idle_timeout_secs`, `auto_restart`, `max_restart_attempts`.
10. `config_parse_disabled_profile_excluded_from_profile_names` — Profile with `enabled = false`. Assert `profile_names()` excludes it, `profile_info()` excludes it.
11. `config_parse_profile_with_all_optional_fields` — Parse a profile with `temperature`, `top_p`, `top_k`, `headers`, `supports_tools`, `supports_vision`, `supports_reasoning`, `extra_params`. Assert all values round-trip correctly.

---

### Task 2: agent-models — router & context window tests

**Files:** Modify `crates/agent-models/src/router.rs`, `crates/agent-models/src/model_registry.rs`

**For router.rs** — add to existing `#[cfg(test)]` block:

1. `new_router_is_empty` — `ModelRouter::new()` then `list_profiles()` yields empty vec.
2. `register_and_list_single_profile` — Register 1 profile. List returns 1. `get_profile` returns `Some`.
3. `register_and_list_multiple_sorted` — Register 3 profiles ("c", "a", "b"). List returns sorted by alias: "a", "b", "c".
4. `get_profile_unknown_returns_none` — `get_profile("nonexistent")` returns `None`.
5. `route_unknown_profile_returns_error` — Tokio test: `route(ModelRequest::user_text("unknown", "test"))` returns `Err`.
6. `route_twice_uses_same_client` — Register a FakeModelClient, route twice. Both produce stream events on the same client.
7. `router_implements_model_client_trait` — Call `.stream()` on router (as `dyn ModelClient`). Verify stream yields events.

**For model_registry.rs** — add to existing `#[cfg(test)]` block:

8. `lookup_openai_gpt41_with_fallback` — `lookup("openai_compatible", "gpt-4.1")` returns `LimitSource::BuiltinRegistry` with context_window > 0.
9. `lookup_openai_unknown_model_returns_fallback` — `lookup("openai", "unknown-model-xyz")` returns `LimitSource::Fallback` with context_window=128000.
10. `lookup_anthropic_claude_sonnet_4` — `lookup("anthropic", "claude-sonnet-4-20250514")` returns BuiltinRegistry with context_window > 0.
11. `lookup_anthropic_unknown_returns_fallback` — `lookup("anthropic", "unknown-model")` returns Fallback.
12. `lookup_ollama_always_fallback` — Any ollama model returns `LimitSource::Fallback` with context_window=8192, output_limit=2048.
13. `lookup_fake_always_fallback` — Fake models return Fallback with context_window=4096, output_limit=256.
14. `lookup_unknown_provider_returns_generic_fallback` — `lookup("deepseek", "deepseek-v3")` returns Fallback with 128k context.
15. `lookup_prefix_matching_chooses_longest_match` — `lookup("anthropic", "claude-3-5-sonnet-20241022")` matches the 3-5-sonnet entry, not the claude-3 generic.
16. `limits_source_variants_display` — `LimitSource::UserConfig` != `LimitSource::Fallback`. Verify discriminants are correct.

---

### Task 3: agent-tools — builtin tool & provider tests

**Files:** Modify `crates/agent-tools/src/filesystem.rs`, `crates/agent-tools/src/shell.rs`, `crates/agent-tools/src/patch/parse.rs`

**For filesystem.rs** — add to `#[cfg(test)]` block:

1. `resolve_read_path_rejects_parent_traversal` — `resolve_workspace_read_path("/tmp/ws", "../etc/passwd")` returns `Err(WorkspaceEscape)`.
2. `resolve_read_path_rejects_absolute_in_relative` — `resolve_workspace_read_path("/tmp/ws", "/etc/passwd")` returns `Err(WorkspaceEscape)`.
3. `resolve_read_path_allows_normal_file` — `resolve_workspace_read_path("/tmp/ws", "src/main.rs")` returns `Ok(PathBuf)` ending in `src/main.rs`.
4. `resolve_write_path_same_rules` — Same traversal tests for `resolve_workspace_write_path`.
5. `fs_list_returns_entries_for_real_dir` — `FsListTool::new(tmpdir)`, invoke with `path: "."`. Returns `ToolOutput` with JSON array.

**For shell.rs** — add to `#[cfg(test)]` block:

6. `classify_read_only_command` — `classify_command("ls", &["-la"])` returns `CommandRisk::ReadOnly`.
7. `classify_write_command` — `classify_command("cp", &["a", "b"])` returns `CommandRisk::Write`.
8. `classify_destructive_command` — `classify_command("rm", &["-rf", "/"])` returns `CommandRisk::Destructive`.
9. `classify_unknown_command_returns_unknown` — `classify_command("foobarbaz", &[])` returns `CommandRisk::Unknown`.
10. `parse_command_simple` — `parse_command("ls -la")` returns `("ls", vec!["-la"])`.
11. `parse_command_quoted_args` — `parse_command(r#"echo "hello world""#)` preserves quoted string.

**For patch/parse.rs** — add to `#[cfg(test)]` block:

12. `parse_single_file_single_hunk` — Parse a simple unified diff. Returns 1 FilePatch with 1 Hunk.
13. `parse_multi_file_diff` — Parse a diff with 2 file headers. Returns 2 FilePatches.
14. `parse_rejects_invalid_header` — Parse non-diff text. Returns `Err(PatchParseError::InvalidHeader)`.
15. `parse_line_types_correct` — Parse a hunk. Verify `Context`/`Remove`/`Add` lines are classified correctly.
16. `roundtrip_parse_and_inspect` — Parse diff, check file paths and line counts match expected values.

---

### Task 4: agent-memory — marker extraction & context assembly tests

**Files:** Modify `crates/agent-memory/src/marker.rs`, `crates/agent-memory/src/context.rs`

**For marker.rs** — add to `#[cfg(test)]` block:

1. `extract_single_session_marker` — Parse `<memory scope="session">hello</memory>`. Returns 1 marker, scope=Session, content="hello", key=None.
2. `extract_marker_with_key` — Parse `<memory scope="user" key="pref">value</memory>`. key=Some("pref").
3. `extract_multiple_markers` — 3 markers in one text. Returns 3, all extracted.
4. `extract_empty_text_returns_empty` — `""` returns `[]`.
5. `extract_no_markers_returns_empty` — Plain text without tags returns `[]`.
6. `strip_memory_markers_removes_tags` — Input with markers; output has tags removed, surrounding text preserved.
7. `strip_no_markers_returns_original` — Plain text unchanged by `strip_memory_markers`.
8. `extract_default_scope_is_session` — `<memory>content</memory>` (no scope attr) → scope=Session.
9. `extract_ignores_empty_content` — `<memory scope="session"></memory>` → filtered out (empty content).
10. `extract_malformed_tags_ignored` — `<memory broken>` not extracted as marker.

**For context.rs** (add to the existing test module):

11. `budget_input_equals_window_minus_output` — `ContextBudget { context_window: 10000, output_reservation: 2000 }` → `input_budget() = 8000`.
12. `assemble_with_no_memory_store` — `ContextAssembler::new_standalone()`. Call `assemble` with simple request. Returns non-empty bundle.
13. `assemble_respects_budget` — Request with many large session_history entries + small budget. Verify `truncated = true`.
14. `assemble_never_drops_system_or_request` — System prompt + request always present in output even with tiny budget.
15. `new_standalone_has_no_memory_store` — `new_standalone()` creates assembler, assemble works without memory store.

---

## Phase 2 — P1 Crates (sequential, after Phase 1 passes)

### Task 5: agent-core — facade & event roundtrip tests

**Files:** Modify `crates/agent-core/src/`, create `crates/agent-core/tests/event_coverage.rs`

**Test cases to add:**

1. `every_event_payload_variant_has_unique_event_type` — Iterate all `EventPayload` variants; each `event_type()` string is unique.
2. `session_id_creation_and_display` — `SessionId::new()`, verify `Display` format, verify `FromStr` roundtrip.
3. `workspace_id_creation_and_display` — Same for `WorkspaceId`.
4. `task_snapshot_state_transitions` — Create `TaskSnapshot`, verify `Pending` → `Running` → `Completed` / `Failed` states.
5. `task_graph_snapshot_contains_tasks` — Create `TaskGraphSnapshot` with multiple tasks, verify iteration.
6. `payload_serde_roundtrip_all_variants` — For every `EventPayload` variant: serialize to JSON, deserialize back. Assert equality.

---

### Task 6: agent-mcp — catalog, skills, installer tests

**Files:** Modify `crates/agent-mcp/src/catalog/builtin.rs`, `crates/agent-mcp/src/catalog/mod.rs`, `crates/agent-mcp/src/installer.rs`

**Test cases to add:**

**For catalog/builtin.rs:**

1. `builtin_catalog_has_entries` — `builtin_catalog()` returns non-empty vec.
2. `each_entry_has_all_required_fields` — Every entry has non-empty `id`, `display_name`, `description`, has at least 1 transport.
3. `entry_ids_are_unique` — No duplicate ids in builtin catalog.
4. `find_by_id_works` — Find a known entry by id.
5. `find_by_id_unknown_returns_none` — Unknown id returns None.

**For catalog/mod.rs:** 6. `catalog_merge_deduplicates_by_id` — Merge two catalogs with overlapping id; first source wins. 7. `catalog_sort_respects_priority` — Higher priority entries appear first after merge.

**For installer.rs:** 8. `installer_detects_already_installed` — Mock already-installed server. `is_installed` returns true. 9. `installer_reports_install_in_progress` — After starting install, `status()` returns Installing. 10. `installer_error_on_missing_transport` — Installing a server with missing required transport yields error.

---

## Phase 3 — P2 Crate (sequential, after Phase 2 passes)

### Task 7: agent-runtime — agent loop & lifecycle tests

**Files:** Modify `crates/agent-runtime/src/agent_loop/mod.rs`, `crates/agent-runtime/src/agent_loop/tool_loop.rs`, `crates/agent-runtime/tests/`

**Test cases to add:**

**For agent_loop/mod.rs:**

1. `agent_loop_stops_on_max_iterations` — Configure max_iterations=3 with FakeModelClient that always returns text. Loop stops after 3 iterations.
2. `agent_loop_exits_on_completion_event` — FakeModelClient returns `Completed` immediately. Loop exits cleanly.
3. `agent_loop_handles_model_error_gracefully` — FakeModelClient returns `Failed`. Loop exits without panic.

**For agent_loop/tool_loop.rs:** 4. `tool_loop_executes_single_tool_and_returns` — Mock tool + mock model. Tool call → execute → return result. 5. `tool_loop_stops_on_max_turns` — Configure max_turns=2. Tool loop exits after 2 iterations.

**New integration test file** `crates/agent-runtime/tests/permission_integration.rs`: 6. `permission_mode_transitions_are_respected` — Create runtime with ReadOnly. Verify write tool is denied. 7. `session_restore_preserves_context` — Create session, add messages, restore. Messages persist.

**New integration test file** `crates/agent-runtime/tests/event_emitter_integration.rs`: 8. `event_emitter_forwards_all_payload_types` — Verify event emitter forwards key payload variants to subscribers.

---

## Verification

After ALL phases complete, run:

```bash
cargo test --workspace --all-targets
```

All tests must pass. No existing tests broken. No new dependencies added.

## Task Summary

| Phase | Task | Crate         | Dispatch   | Est. new tests |
| ----- | ---- | ------------- | ---------- | -------------- |
| 1     | T1   | agent-config  | Parallel   | ~11            |
| 1     | T2   | agent-models  | Parallel   | ~16            |
| 1     | T3   | agent-tools   | Parallel   | ~16            |
| 1     | T4   | agent-memory  | Parallel   | ~15            |
| 2     | T5   | agent-core    | Sequential | ~6             |
| 2     | T6   | agent-mcp     | Sequential | ~10            |
| 3     | T7   | agent-runtime | Sequential | ~8             |

**Total: ~82 new tests**
