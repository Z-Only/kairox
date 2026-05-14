# Test Coverage Improvement — Design

Date: 2026-05-14. 987 tests pass. No coverage regression baseline yet.

## Goal

Raise test coverage for core modules, focusing on untested code paths that affect correctness, security, and reliability.

## Current state (test lines / total lines)

| Crate         | Ratio | Source lines | Test lines | Priority |
| ------------- | ----- | ------------ | ---------- | -------- |
| agent-config  | 3.4%  | 2,311        | 82         | P0       |
| agent-models  | 3.8%  | 3,233        | 130        | P0       |
| agent-tools   | 6.7%  | 4,313        | 312        | P0       |
| agent-memory  | 6.8%  | 1,504        | 110        | P0       |
| agent-core    | 8.1%  | 3,600        | 319        | P1       |
| agent-mcp     | 13.1% | 6,758        | 1,027      | P1       |
| agent-store   | 13.2% | 1,577        | 241        | —        |
| agent-runtime | 26.7% | 13,677       | 4,997      | P2       |
| agent-tui     | 15.7% | 5,259        | 982        | —        |

## Target: raise core ratios to >=20%, agent-runtime to >=35%

## Phase design

### Phase 1 — Critical-path unit tests (P0 crates)

**agent-config** (target: ~20%)

- TOML parsing: invalid profiles, missing fields, malformed API key envs
- Profile validation: duplicate names, missing required fields
- API key resolution: env var present/absent/malformed
- Router construction: valid/invalid provider strings
- `.kairox/` discovery: walking up directory tree, merging project config
- Skills config parsing: valid/invalid skill definitions

**agent-models** (target: ~20%)

- ModelRouter: select by name (exact/fuzzy/fallback), unknown model
- Context window: token counting, budget calculation, truncation boundaries
- Provider adapters: request serialization, response deserialization, error mapping
- FakeModelClient: deterministic streaming, configurable content, error injection
- Model registry: list, filter by provider, metadata lookup

**agent-tools** (target: ~20%)

- ToolRegistry: register, unregister, lookup, list-all, duplicate detection
- PermissionEngine: deny path, allow path, suggest path, risk-level thresholds
- Builtin tools: fs.read path validation (no traversal), fs.write sandbox, shell risk classification
- Patch tool: apply valid diff, reject malformed diff, dry-run mode

**agent-memory** (target: ~20%)

- Memory marker extraction: valid/invalid tags, scope parsing, key extraction
- Context assembly: token budget enforcement, message ordering, system prompt injection
- Compaction: summarization boundary detection, truncation of old messages

### Phase 2 — Facade + integration tests (P1 crates)

**agent-core** (target: ~15%)

- EventPayload roundtrip: every variant serde + event_type discriminator
- SessionId/WorkspaceId: creation, parsing, display
- TaskSnapshot/TaskGraphSnapshot: state transitions, projections
- AppFacade trait: method signatures verified against runtime impl

**agent-mcp** (target: ~25%)

- Catalog builtin entries: verify all built-in servers have name, description, transport
- Skills loader: parse, validate, resolve dependencies
- Installer: idempotent install, re-install, error on missing server
- Transport: stdio command construction, SSE URL validation, error handling
- Lifecycle: start, stop, restart, health check timeout

### Phase 3 — Runtime gap fill (P2 crates)

**agent-runtime** (target: ~35%)

- agent_loop/tool_loop: execute tools via FakeModelClient, handle tool errors, max iterations
- permission: mode transitions, interactive approval timeout, deny escalation
- session: create/restore lifecycle, context budget enforcement on session open
- event_emitter: verify all EventPayload variants emitted correctly
- memory_handler: process memory markers from model output, dedup by key
- facade_projects/facade_mcp/facade_skills: thin wrappers, verify delegation

## Conventions

- Use trait-based fakes/boundaries already present (FakeModelClient, SqliteEventStore in-memory)
- New tests follow existing `#[cfg(test)] mod tests` pattern or `tests/` layout
- No new test dependencies unless needed
- All tests must pass with `cargo test --workspace --all-targets`

## Risk & boundaries

- Do not change production code behavior. Tests must pass existing code.
- If test reveals a bug, flag it separately — do not fix in this pass.
- Do not add new dependencies to Cargo.toml unless required for testing (e.g., tempfile for temp dirs).
- Skip: agent-tui snapshot tests, E2E Playwright, tauri-pilot scenarios. Those are integration-layer concerns.
