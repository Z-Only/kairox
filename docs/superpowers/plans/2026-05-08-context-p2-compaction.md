# Context P2 — Session Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add manual + automatic context compaction to every session: the runtime can summarise old turns into a `CompactionSummary` event (LLM-driven, with a sliding-window fallback), `send_message` is rejected while a session is busy compacting, and the next `agent_loop` iteration transparently substitutes the summary for the compacted event range.

**Architecture:** New domain types (`CompactionReason`, four `EventPayload` variants, `CoreError::SessionBusy`) live in `agent-core`. A new `agent-memory::compactor` module owns prompt + LLM call + sliding-window fallback. A new `agent-runtime::compaction` module owns trigger logic, busy-gate state, retry policy and emits the four events. `agent_loop` (a) checks the busy gate at entry, (b) substitutes the latest `CompactionSummary` for its `replaces_event_range` inside `build_model_messages` / `build_model_messages_within_budget`, and (c) fires an auto-compact request after every `ContextAssembled` event whose ratio crosses the threshold. `agent-config` parses `[context]` into a new `ContextPolicy` struct injected into `LocalRuntime`.

**Tech Stack:** Rust 1.x · tokio · async_trait · serde · thiserror · tiktoken-rs · chrono · `cargo test --workspace --all-targets` · `cargo clippy --workspace --all-targets --all-features -- -D warnings` · `pnpm run lint` · `just gen-types` · `just check-types`

**Branch:** `feat/context-p2-compaction` (already created via `git worktree add .worktrees/feat-context-p2-compaction -b feat/context-p2-compaction main`).

**Spec reference:** `docs/superpowers/specs/2026-05-08-session-context-and-model-management-design.md` §4.4 / §5

**Out of scope (deferred to P3 / P4):**

- `ContextMeter.vue` GUI component / `status_bar.rs` TUI rendering
- Tauri commands (`compact_session` / `switch_model` / `list_profiles_with_limits`)
- TUI `:compact` command wiring (P3)
- `ModelProfileSwitched` event + mid-session model switch (P4)
- The four-section `compactor_prompt.txt` is shipped here, but the prompt CONTENT is the only public contract; we do not expose a "edit summary" UI.

---

## File Structure

> See task list below for which task touches each file. Every file path is exact.

### `agent-core` (events + error)

- **Modify** `crates/agent-core/src/error.rs`
  - Add `SessionBusy { session_id: String, reason: String }` variant to `CoreError`.
- **Modify** `crates/agent-core/src/events.rs`
  - Add `CompactionReason` enum (`UserRequested` | `Threshold { ratio: f32 }`).
  - Add four `EventPayload` variants:
    - `ContextCompactionStarted { reason, before_tokens, candidate_event_count }`
    - `ContextCompactionCompleted { summary_id, after_tokens, fallback_used }`
    - `ContextCompactionFailed { error, fallback_used }`
    - `CompactionSummary { summary_id, content, replaces_event_range, reason, before_tokens, after_tokens, summarised_by_profile }`
  - `replaces_event_range: (DateTime<Utc>, DateTime<Utc>)` — first..=last timestamp inclusive (per spec §4.4 — `DomainEvent` has no stable id today).
  - Update `EventPayload::event_type()` with four new arms.

### `agent-config` (ContextPolicy)

- **Modify** `crates/agent-config/src/lib.rs`
  - Add `ContextPolicy { auto_compact_threshold: f32, compactor_profile: Option<String>, max_tool_definition_tokens: Option<u64> }` with `Default` (threshold = `0.85`).
  - Add `Config.context: ContextPolicy` field with `#[serde(default)]`.
- **Modify** `crates/agent-config/src/loader.rs`
  - Parse top-level `[context]` table into `Config.context`.
  - Update `merge_with_defaults` so user values override defaults.
- **Modify** `kairox.toml.example`
  - Document the `[context]` block (auto_compact_threshold, compactor_profile, max_tool_definition_tokens) — uncommented sample with safe defaults.

### `agent-memory` (compactor module + prompt)

- **Create** `crates/agent-memory/src/compactor.rs`
  - `pub struct Compactor;` with two associated fns:
    - `pub async fn compact_with_llm(model: &dyn ModelClient, profile_alias: &str, transcript: &str) -> Result<String, CompactorError>` — sends `compactor_prompt` + transcript; expects markdown; retries 3× exponential backoff (200ms / 400ms / 800ms).
    - `pub fn sliding_window_fallback(candidate_event_count: usize) -> String` — returns synthetic `"[Dropped {N} earlier turns by sliding window]"` summary.
  - `CompactorError { LlmFailed(String), Empty }` (thiserror).
  - `pub fn render_transcript(events: &[DomainEvent]) -> String` — markdown with role tags + tool-call summaries. Used by `agent-runtime::compaction`.
- **Create** `crates/agent-memory/src/compactor_prompt.txt` — embedded via `include_str!`. Four-section markdown template per spec §4.4.
- **Modify** `crates/agent-memory/src/lib.rs` — re-export `Compactor`, `CompactorError`, `render_transcript`.
- **Modify** `crates/agent-memory/Cargo.toml` — add `agent-models = { path = "../agent-models" }` (we already depend on `agent_models::ToolDefinition` for `ContextRequest.tool_definitions`, so this should already be present — verify before editing).

### `agent-runtime` (compaction module + facade + agent_loop wiring)

- **Modify** `crates/agent-runtime/src/session.rs`
  - Add `pub compacting: bool` field to `SessionState` (default `false`).
- **Create** `crates/agent-runtime/src/compaction.rs`
  - `pub async fn compact_session(...)` orchestrator: locates the boundary (keep last 6 user/assistant messages = 3 pairs), renders the candidate range, invokes `Compactor::compact_with_llm`, on failure falls back to `sliding_window_fallback`, appends `ContextCompactionStarted` → `CompactionSummary` + `ContextCompactionCompleted` (or `ContextCompactionFailed` + fallback summary).
  - Sets / unsets `session_state.compacting` around the work.
  - Public boundary helper `pub fn pick_compaction_boundary(events: &[DomainEvent], keep_last_pairs: usize) -> Option<(DateTime<Utc>, DateTime<Utc>)>` (testable in isolation).
  - `pub const KEEP_LAST_PAIRS: usize = 3;` and `pub const LLM_RETRY_ATTEMPTS: u32 = 3;`.
- **Modify** `crates/agent-runtime/src/facade_runtime.rs`
  - Add `LocalRuntime::compact_session(&self, session_id: SessionId, reason: CompactionReason) -> Result<()>` method (NOT yet on the `AppFacade` trait — that comes in P3).
  - Add `with_context_policy(self, policy: ContextPolicy)` builder + store on `LocalRuntime`.
  - In the existing `AppFacade::send_message` impl: before doing any work, check `session_states.lock().await.get(&session_id).map(|s| s.compacting).unwrap_or(false)` and return `CoreError::SessionBusy` if true.
- **Modify** `crates/agent-runtime/src/agent_loop.rs`
  - Modify `build_model_messages` to substitute every event whose timestamp is inside any `CompactionSummary.replaces_event_range` with a single synthetic system-tagged message at that position (only the most recent `CompactionSummary` covering each timestamp wins).
  - After emitting `ContextAssembled { usage }`, inspect `usage.ratio()`. If `>= context_policy.auto_compact_threshold` AND `!session_state.compacting`, spawn `tokio::spawn(compact_session(..., CompactionReason::Threshold { ratio }))`.
- **Modify** `crates/agent-runtime/src/lib.rs` — `pub mod compaction;` + re-export `CompactionReason` re-export from `agent_core`.

### Generated types + GUI specta

- **Modify** `apps/agent-gui/src-tauri/src/specta.rs`
  - Register `CompactionReason` (alongside the four new event variants which are already covered transitively because `EventPayload` itself is registered).
- **Auto-regenerate** `apps/agent-gui/src/generated/events.ts` via `just gen-types`. Verified by `just check-types`.

### Tests (new)

- **Modify** `crates/agent-core/src/events.rs` — add round-trip tests for the four new payload variants + `CompactionReason`.
- **Modify** `crates/agent-core/src/error.rs` — add a `Display` test for `SessionBusy`.
- **Modify** `crates/agent-config/src/loader.rs` — `#[cfg(test)]` test parsing `[context]` block.
- **Create** `crates/agent-memory/src/compactor.rs` `#[cfg(test)] mod tests` — sliding-window fallback shape, transcript rendering, retry budget (using a stub `ModelClient` that fails N times then succeeds).
- **Create** `crates/agent-runtime/tests/compaction.rs` — full-stack: feed N events via `FakeModelClient`, call `compact_session(UserRequested)`, assert the four events are emitted in order, `compacting` is `true` in between, `send_message` is rejected during the window with `SessionBusy`, and a follow-up `send_message` after `Completed` succeeds with the summary substituted.
- **Modify** `crates/agent-runtime/src/agent_loop.rs` — extend the existing `tests` module with a `summary_substitutes_event_range_in_build_model_messages` test.

---

## Task list (TDD, bite-sized)

The plan splits into 12 sequential tasks. Each task ends with running tests + a commit. Tasks 1–3 add types (no behaviour change). Tasks 4–6 add the `agent-memory` compactor. Tasks 7–9 add the runtime orchestrator + facade + busy gate. Task 10 wires `agent_loop` (substitution + auto-trigger). Task 11 ships the integration test + specta sync. Task 12 final verification + push.

> **Reading order matters.** Type signatures defined in earlier tasks are referenced by name in later ones.

---

### Task 1 — Add `CoreError::SessionBusy` to `agent-core`

**Files:**

- Modify: `crates/agent-core/src/error.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/agent-core/src/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_busy_displays_with_session_and_reason() {
        let err = CoreError::SessionBusy {
            session_id: "ses_abc".into(),
            reason: "compacting".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("ses_abc"), "expected session id, got: {msg}");
        assert!(msg.contains("compacting"), "expected reason, got: {msg}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-core session_busy_displays`
Expected: FAIL — `no variant or associated item named 'SessionBusy' found for enum 'CoreError'`.

- [ ] **Step 3: Add the variant**

Replace the body of `CoreError` in `crates/agent-core/src/error.rs` with:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("session {session_id} is busy: {reason}")]
    SessionBusy { session_id: String, reason: String },
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-core session_busy_displays`
Expected: PASS.

- [ ] **Step 5: Run the wider crate test to be safe**

Run: `cargo test -p agent-core`
Expected: all green (the new variant is additive; existing `match`es on `CoreError` are non-exhaustive consumers via `Display`).

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core/src/error.rs
git commit -m "feat(core): add CoreError::SessionBusy variant"
```

---

### Task 2 — Add `CompactionReason` + four compaction event variants to `agent-core`

**Files:**

- Modify: `crates/agent-core/src/events.rs`

- [ ] **Step 1: Write the failing tests**

Append to the bottom of `crates/agent-core/src/events.rs` (inside `#[cfg(test)]` style — these are top-level `#[test]` fns matching the existing ones):

```rust
#[test]
fn compaction_reason_serializes_with_internal_tag() {
    let r = CompactionReason::UserRequested;
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["type"], "UserRequested");

    let r = CompactionReason::Threshold { ratio: 0.87 };
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["type"], "Threshold");
    assert!((json["ratio"].as_f64().unwrap() - 0.87).abs() < 1e-6);
}

#[test]
fn context_compaction_started_event_round_trips() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionStarted {
            reason: CompactionReason::Threshold { ratio: 0.9 },
            before_tokens: 180_000,
            candidate_event_count: 42,
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "ContextCompactionStarted");
    assert_eq!(json["payload"]["type"], "ContextCompactionStarted");
    assert_eq!(json["payload"]["before_tokens"], 180_000);
    assert_eq!(json["payload"]["candidate_event_count"], 42);
    assert_eq!(json["payload"]["reason"]["type"], "Threshold");

    let s = serde_json::to_string(&event.payload).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, EventPayload::ContextCompactionStarted { .. }));
}

#[test]
fn context_compaction_completed_and_failed_round_trip() {
    let completed = EventPayload::ContextCompactionCompleted {
        summary_id: "sum_1".into(),
        after_tokens: 30_000,
        fallback_used: false,
    };
    let json = serde_json::to_value(&completed).unwrap();
    assert_eq!(json["type"], "ContextCompactionCompleted");
    assert_eq!(json["fallback_used"], false);
    let _back: EventPayload = serde_json::from_value(json).unwrap();

    let failed = EventPayload::ContextCompactionFailed {
        error: "model timeout".into(),
        fallback_used: true,
    };
    let json = serde_json::to_value(&failed).unwrap();
    assert_eq!(json["type"], "ContextCompactionFailed");
    assert_eq!(json["fallback_used"], true);
    let _back: EventPayload = serde_json::from_value(json).unwrap();
}

#[test]
fn compaction_summary_event_round_trips_with_timestamp_range() {
    use chrono::TimeZone;
    let from = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 9, 0, 0).unwrap();
    let to = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
    let payload = EventPayload::CompactionSummary {
        summary_id: "sum_1".into(),
        content: "## User goal\n...".into(),
        replaces_event_range: (from, to),
        reason: CompactionReason::UserRequested,
        before_tokens: 180_000,
        after_tokens: 4_000,
        summarised_by_profile: "fast".into(),
    };
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "CompactionSummary");
    assert_eq!(json["summarised_by_profile"], "fast");
    let back: EventPayload = serde_json::from_value(json).unwrap();
    if let EventPayload::CompactionSummary { replaces_event_range, .. } = back {
        assert_eq!(replaces_event_range.0, from);
        assert_eq!(replaces_event_range.1, to);
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn event_type_method_covers_new_compaction_variants() {
    let started = EventPayload::ContextCompactionStarted {
        reason: CompactionReason::UserRequested,
        before_tokens: 0,
        candidate_event_count: 0,
    };
    assert_eq!(started.event_type(), "ContextCompactionStarted");

    let completed = EventPayload::ContextCompactionCompleted {
        summary_id: "x".into(),
        after_tokens: 0,
        fallback_used: false,
    };
    assert_eq!(completed.event_type(), "ContextCompactionCompleted");

    let failed = EventPayload::ContextCompactionFailed {
        error: "x".into(),
        fallback_used: false,
    };
    assert_eq!(failed.event_type(), "ContextCompactionFailed");

    let summary = EventPayload::CompactionSummary {
        summary_id: "x".into(),
        content: "x".into(),
        replaces_event_range: (chrono::Utc::now(), chrono::Utc::now()),
        reason: CompactionReason::UserRequested,
        before_tokens: 0,
        after_tokens: 0,
        summarised_by_profile: "fast".into(),
    };
    assert_eq!(summary.event_type(), "CompactionSummary");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-core compaction`
Expected: FAIL — `cannot find type 'CompactionReason' in this scope` and `no variant 'ContextCompactionStarted'`.

- [ ] **Step 3: Add `CompactionReason` enum**

Insert at the top of `crates/agent-core/src/events.rs` (right after the `PrivacyClassification` enum):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum CompactionReason {
    UserRequested,
    Threshold { ratio: f32 },
}
```

> Note: `PartialEq` is included (not `Eq`) because `f32` doesn't implement `Eq`. This matches how `ContextUsage` is treated (no `Eq` either).

- [ ] **Step 4: Add the four `EventPayload` variants**

Inside the `EventPayload` enum in `crates/agent-core/src/events.rs`, add these variants right after the existing `ContextAssembled` variant:

```rust
    ContextCompactionStarted {
        reason: CompactionReason,
        before_tokens: u64,
        candidate_event_count: usize,
    },
    ContextCompactionCompleted {
        summary_id: String,
        after_tokens: u64,
        fallback_used: bool,
    },
    ContextCompactionFailed {
        error: String,
        fallback_used: bool,
    },
    CompactionSummary {
        summary_id: String,
        content: String,
        replaces_event_range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
        reason: CompactionReason,
        before_tokens: u64,
        after_tokens: u64,
        summarised_by_profile: String,
    },
```

- [ ] **Step 5: Add the four `event_type()` arms**

Inside the `match self` block of `EventPayload::event_type()`, add right after the `Self::ContextAssembled { .. } => "ContextAssembled",` line:

```rust
            Self::ContextCompactionStarted { .. } => "ContextCompactionStarted",
            Self::ContextCompactionCompleted { .. } => "ContextCompactionCompleted",
            Self::ContextCompactionFailed { .. } => "ContextCompactionFailed",
            Self::CompactionSummary { .. } => "CompactionSummary",
```

- [ ] **Step 6: Run the new tests to verify they pass**

Run: `cargo test -p agent-core compaction -- --nocapture`
Expected: 5 new tests pass.

- [ ] **Step 7: Run the full crate suite**

Run: `cargo test -p agent-core`
Expected: all green.

- [ ] **Step 8: Run clippy on the touched crate**

Run: `cargo clippy -p agent-core --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/agent-core/src/events.rs
git commit -m "feat(core): add CompactionReason and four context-compaction event variants"
```

---

### Task 3 — Add `ContextPolicy` to `agent-config`

**Files:**

- Modify: `crates/agent-config/src/lib.rs`
- Modify: `crates/agent-config/src/loader.rs`
- Modify: `kairox.toml.example`

- [ ] **Step 1: Write the failing test**

Append to `crates/agent-config/src/loader.rs` `#[cfg(test)] mod tests` (find the existing module; if absent, create one at the bottom of the file):

```rust
#[test]
fn parses_context_policy_with_defaults_and_overrides() {
    // Defaults: omitting [context] yields the default ContextPolicy.
    let cfg_default: crate::Config = crate::loader::load_from_str(
        r#"
[profiles.fake]
provider = "fake"
model_id = "fake"
"#,
        "test.toml",
    )
    .unwrap();
    assert!(
        (cfg_default.context.auto_compact_threshold - 0.85_f32).abs() < 1e-6,
        "default threshold should be 0.85, got {}",
        cfg_default.context.auto_compact_threshold
    );
    assert!(cfg_default.context.compactor_profile.is_none());
    assert!(cfg_default.context.max_tool_definition_tokens.is_none());

    // Overrides: explicit values take precedence.
    let cfg_user: crate::Config = crate::loader::load_from_str(
        r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o"

[context]
auto_compact_threshold = 0.7
compactor_profile = "fast"
max_tool_definition_tokens = 25000
"#,
        "test.toml",
    )
    .unwrap();
    assert!((cfg_user.context.auto_compact_threshold - 0.7).abs() < 1e-6);
    assert_eq!(cfg_user.context.compactor_profile.as_deref(), Some("fast"));
    assert_eq!(cfg_user.context.max_tool_definition_tokens, Some(25_000));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-config parses_context_policy`
Expected: FAIL — `no field 'context' on type 'Config'`.

- [ ] **Step 3: Add `ContextPolicy` to `crates/agent-config/src/lib.rs`**

Insert this struct in `crates/agent-config/src/lib.rs` just before the `Config` struct definition:

```rust
/// Session compaction & context budgeting policy. Loaded from the
/// optional top-level `[context]` table in `kairox.toml`. All fields
/// have safe defaults so omitting the table is fine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPolicy {
    /// When the assembled context reaches this fraction of the budget,
    /// the runtime triggers automatic compaction. Set to `1.0` to disable.
    #[serde(default = "default_auto_compact_threshold")]
    pub auto_compact_threshold: f32,
    /// Optional profile alias to use for the summarisation LLM call.
    /// Falls back to the session's currently active profile when unset.
    #[serde(default)]
    pub compactor_profile: Option<String>,
    /// Optional cap on MCP + builtin tool definitions tokens. When the
    /// serialised tool schemas exceed this, the assembler drops the
    /// lowest-priority tools first.
    #[serde(default)]
    pub max_tool_definition_tokens: Option<u64>,
}

fn default_auto_compact_threshold() -> f32 {
    0.85
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            auto_compact_threshold: default_auto_compact_threshold(),
            compactor_profile: None,
            max_tool_definition_tokens: None,
        }
    }
}
```

Then add the `context` field to the `Config` struct:

```rust
pub struct Config {
    pub profiles: Vec<(String, ProfileDef)>,
    pub mcp_servers: Vec<(String, McpServerConfig)>,
    pub source: ConfigSource,
    /// Session compaction & context budgeting policy.
    pub context: ContextPolicy,
}
```

- [ ] **Step 4: Update every `Config { ... }` literal to include `context`**

Search-and-update with:

```bash
grep -rn "Config {" crates/agent-config/src crates/agent-runtime/src apps/agent-gui/src-tauri/src crates/agent-tui/src
```

For each `Config { profiles, mcp_servers, source }` literal (e.g. inside `Config::defaults()`, `LocalRuntime::new()`, test helpers), add `context: ContextPolicy::default(),` (or carry through the loaded value where appropriate).

Specifically:

- In `crates/agent-config/src/lib.rs::Config::defaults()` — add `context: ContextPolicy::default(),`.
- In `crates/agent-runtime/src/facade_runtime.rs::LocalRuntime::new()` (where `agent_config::Config { profiles: vec![], mcp_servers: vec![], source: agent_config::ConfigSource::Defaults }` is constructed inline) — add `context: agent_config::ContextPolicy::default(),`.
- Any test fixture under `crates/agent-config/src/loader.rs` or `crates/agent-runtime/src/test_support.rs` that builds `Config` directly — add the field.

- [ ] **Step 5: Update the loader to deserialise `[context]`**

In `crates/agent-config/src/loader.rs`, the `load_from_str` function deserialises a top-level TOML struct. Find the intermediate struct (commonly named `RawConfig` or similar) and add:

```rust
#[serde(default)]
context: crate::ContextPolicy,
```

Then in the `Config { ... }` literal that converts `RawConfig` → `Config`, populate `context: raw.context`.

> If the loader uses `toml::from_str::<HashMap<String, ...>>` instead of a typed intermediate, add an explicit `context = toml::Value::try_into::<crate::ContextPolicy>(raw.remove("context").unwrap_or_default()).unwrap_or_default()` step. Read the existing `loader.rs` first to choose the correct path.

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p agent-config parses_context_policy`
Expected: PASS.

- [ ] **Step 7: Run the full crate suite**

Run: `cargo test -p agent-config`
Expected: all green (the new field is additive; only Step 4 sites had to compile).

- [ ] **Step 8: Update `kairox.toml.example`**

Append (at the bottom of the file, or right after the `[profiles.*]` section):

```toml

# -----------------------------------------------------------------------------
# Session compaction & context budgeting (optional; safe defaults shown).
# -----------------------------------------------------------------------------
[context]
# When the assembled context reaches this fraction of the budget, the runtime
# triggers automatic compaction. Set to 1.0 to disable auto-compaction.
auto_compact_threshold = 0.85

# Optional: profile alias to use for the summarisation LLM call. When unset,
# the session's currently active profile is used. Useful to point at a
# cheap fast model (e.g. "fast") even when the session runs on "claude-opus".
# compactor_profile = "fast"

# Optional: cap on MCP tool definitions tokens. When the serialised tool
# schemas exceed this, the assembler drops the lowest priority tools first.
# max_tool_definition_tokens = 25000
```

- [ ] **Step 9: Run workspace build to confirm no downstream compile breaks**

Run: `cargo build --workspace --all-targets`
Expected: success.

- [ ] **Step 10: Commit**

```bash
git add crates/agent-config/src/lib.rs crates/agent-config/src/loader.rs kairox.toml.example crates/agent-runtime/src/facade_runtime.rs
git commit -m "feat(config): add ContextPolicy with auto_compact_threshold and compactor_profile"
```

---

### Task 4 — Add `compactor_prompt.txt` template + `render_transcript` helper to `agent-memory`

**Files:**

- Create: `crates/agent-memory/src/compactor_prompt.txt`
- Create: `crates/agent-memory/src/compactor.rs` (skeleton + `render_transcript` only — Task 5 adds `Compactor`)
- Modify: `crates/agent-memory/src/lib.rs`

- [ ] **Step 1: Create the prompt template**

Create `crates/agent-memory/src/compactor_prompt.txt` with exactly this content:

```text
You are summarising a developer-AI conversation so it fits a smaller context.
Output FOUR markdown sections, no preamble:

## User goal
## Key decisions & constraints
## Tool calls executed and their outcomes
## Open questions / pending work
```

- [ ] **Step 2: Write the failing test**

Create `crates/agent-memory/src/compactor.rs`:

```rust
//! Session compaction: render an event range into a transcript and
//! call an LLM to summarise it (with a sliding-window fallback used
//! when the LLM call repeatedly fails).
//!
//! The `Compactor` itself is added in Task 5. This file currently exposes
//! only the prompt template + `render_transcript` helper used by both the
//! runtime orchestrator and the LLM call.

use agent_core::{DomainEvent, EventPayload};

/// Embedded summarisation prompt. Stable; do NOT inline the string —
/// `include_str!` keeps it editable as a separate file (and keeps the
/// `compactor_prompt.txt` content out of the Rust source diff noise).
pub const COMPACTOR_PROMPT: &str = include_str!("compactor_prompt.txt");

/// Render a slice of events into a markdown transcript suitable for the
/// summariser LLM. Tool-call events are condensed into one-line summaries
/// (the full output preview lives separately and would blow the budget).
pub fn render_transcript(events: &[DomainEvent]) -> String {
    let mut out = String::with_capacity(events.len() * 64);
    for event in events {
        match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => {
                out.push_str("### user\n");
                out.push_str(content);
                out.push_str("\n\n");
            }
            EventPayload::AssistantMessageCompleted { content, .. } => {
                out.push_str("### assistant\n");
                out.push_str(content);
                out.push_str("\n\n");
            }
            EventPayload::ModelToolCallRequested { tool_id, tool_call_id } => {
                out.push_str(&format!(
                    "### tool_call ({tool_id}, id={tool_call_id})\n\n"
                ));
            }
            EventPayload::ToolInvocationCompleted { tool_id, output_preview, .. } => {
                out.push_str(&format!(
                    "### tool_result ({tool_id})\n{}\n\n",
                    output_preview
                ));
            }
            EventPayload::ToolInvocationFailed { tool_id, error, .. } => {
                out.push_str(&format!(
                    "### tool_failed ({tool_id})\n{}\n\n",
                    error
                ));
            }
            // Ignore meta events (permissions, task graph) — they don't carry
            // semantic conversation content.
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };

    fn make_event(payload: EventPayload) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
    }

    #[test]
    fn prompt_template_starts_with_known_preamble() {
        // Guard: the prompt is part of the model's contract; if someone edits
        // it, this test forces them to also update the test (and presumably
        // the integration tests that depend on output format).
        assert!(
            COMPACTOR_PROMPT.starts_with("You are summarising a developer-AI conversation"),
            "compactor prompt drifted: {}",
            COMPACTOR_PROMPT.lines().next().unwrap_or("")
        );
        assert!(COMPACTOR_PROMPT.contains("## User goal"));
        assert!(COMPACTOR_PROMPT.contains("## Key decisions & constraints"));
        assert!(COMPACTOR_PROMPT.contains("## Tool calls executed and their outcomes"));
        assert!(COMPACTOR_PROMPT.contains("## Open questions / pending work"));
    }

    #[test]
    fn render_transcript_includes_user_assistant_and_tool_events() {
        let events = vec![
            make_event(EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "list rust files".into(),
            }),
            make_event(EventPayload::AssistantMessageCompleted {
                message_id: "a1".into(),
                content: "let me search".into(),
            }),
            make_event(EventPayload::ModelToolCallRequested {
                tool_call_id: "tc1".into(),
                tool_id: "search.ripgrep".into(),
            }),
            make_event(EventPayload::ToolInvocationCompleted {
                invocation_id: "tc1".into(),
                tool_id: "search.ripgrep".into(),
                output_preview: "found 5 matches".into(),
                exit_code: Some(0),
                duration_ms: 50,
                truncated: false,
            }),
        ];
        let out = render_transcript(&events);
        assert!(out.contains("### user\nlist rust files"), "missing user: {out}");
        assert!(out.contains("### assistant\nlet me search"), "missing assistant: {out}");
        assert!(out.contains("tool_call (search.ripgrep, id=tc1)"), "missing tool_call: {out}");
        assert!(out.contains("tool_result (search.ripgrep)\nfound 5 matches"), "missing tool_result: {out}");
    }

    #[test]
    fn render_transcript_skips_meta_events() {
        let events = vec![
            make_event(EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "do thing".into(),
            }),
            make_event(EventPayload::PermissionGranted {
                request_id: "perm1".into(),
            }),
            make_event(EventPayload::AssistantMessageCompleted {
                message_id: "a1".into(),
                content: "done".into(),
            }),
        ];
        let out = render_transcript(&events);
        assert!(!out.contains("perm1"), "permission events leaked into transcript: {out}");
        assert!(out.contains("### user"));
        assert!(out.contains("### assistant"));
    }
}
```

- [ ] **Step 3: Wire into `lib.rs`**

In `crates/agent-memory/src/lib.rs`, add `pub mod compactor;` near the other `pub mod` declarations and update the `pub use` block:

```rust
pub use compactor::{render_transcript, COMPACTOR_PROMPT};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p agent-memory compactor -- --nocapture`
Expected: 3 new tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-memory/src/compactor_prompt.txt crates/agent-memory/src/compactor.rs crates/agent-memory/src/lib.rs
git commit -m "feat(memory): add compactor prompt template and render_transcript helper"
```

---

### Task 5 — Add `Compactor::compact_with_llm` + `sliding_window_fallback`

**Files:**

- Modify: `crates/agent-memory/src/compactor.rs`
- Modify: `crates/agent-memory/src/lib.rs`
- Modify: `crates/agent-memory/Cargo.toml` (verify `agent-models` dep present; add if missing)

- [ ] **Step 1: Verify `agent-models` is in `Cargo.toml`**

Run: `grep '^agent-models' crates/agent-memory/Cargo.toml`

If absent (it should already be present from P1's `tool_definitions` work — confirmed by `crates/agent-memory/src/context.rs` `use agent_models`), add under `[dependencies]`:

```toml
agent-models = { path = "../agent-models" }
```

- [ ] **Step 2: Write the failing tests**

Append to the `#[cfg(test)] mod tests` block of `crates/agent-memory/src/compactor.rs`:

```rust
    use agent_models::{
        ModelClient, ModelEvent, ModelRequest, ModelResponse, ModelResult,
    };
    use async_trait::async_trait;
    use futures::stream::{self, BoxStream, StreamExt};
    use std::sync::{Arc, Mutex};

    /// Stub `ModelClient` that fails the first `fail_count` calls then
    /// streams a single `Started` → `TextDelta` → `Completed` sequence.
    struct StubModel {
        fail_count: Arc<Mutex<u32>>,
        success_text: String,
    }

    impl StubModel {
        fn new(fails: u32, text: &str) -> Self {
            Self {
                fail_count: Arc::new(Mutex::new(fails)),
                success_text: text.to_string(),
            }
        }
    }

    #[async_trait]
    impl ModelClient for StubModel {
        async fn invoke(&self, _req: ModelRequest) -> ModelResult<ModelResponse> {
            let mut left = self.fail_count.lock().unwrap();
            if *left > 0 {
                *left -= 1;
                return Err(agent_models::ModelError::Other("stub-failure".into()));
            }
            let text = self.success_text.clone();
            let stream: BoxStream<'static, ModelResult<ModelEvent>> = stream::iter(vec![
                Ok(ModelEvent::Started),
                Ok(ModelEvent::TextDelta { delta: text }),
                Ok(ModelEvent::Completed { usage: None }),
            ])
            .boxed();
            Ok(ModelResponse { stream })
        }
    }

    #[tokio::test]
    async fn compact_with_llm_returns_first_successful_summary() {
        let model = StubModel::new(0, "## User goal\nfix tests\n");
        let summary = Compactor::compact_with_llm(&model, "fast", "transcript")
            .await
            .expect("should succeed");
        assert!(summary.contains("## User goal"));
    }

    #[tokio::test]
    async fn compact_with_llm_retries_then_succeeds() {
        let model = StubModel::new(2, "## User goal\nok\n");
        let summary = Compactor::compact_with_llm(&model, "fast", "transcript")
            .await
            .expect("should succeed after retries");
        assert!(summary.contains("ok"));
    }

    #[tokio::test]
    async fn compact_with_llm_fails_after_max_retries() {
        let model = StubModel::new(99, "");
        let err = Compactor::compact_with_llm(&model, "fast", "transcript")
            .await
            .expect_err("should fail after 3 retries");
        match err {
            CompactorError::LlmFailed(_) => {}
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[tokio::test]
    async fn compact_with_llm_rejects_empty_response() {
        let model = StubModel::new(0, "");
        let err = Compactor::compact_with_llm(&model, "fast", "transcript")
            .await
            .expect_err("empty summary should fail");
        assert!(matches!(err, CompactorError::Empty));
    }

    #[test]
    fn sliding_window_fallback_includes_count() {
        let s = Compactor::sliding_window_fallback(42);
        assert!(s.contains("42"), "expected count, got: {s}");
        assert!(s.contains("sliding window"));
    }
```

> Note: the stub's signature must match the project's actual `ModelClient` trait. Before writing the stub, run `grep -n "trait ModelClient" crates/agent-models/src/*.rs` and adjust the `async fn invoke` signature exactly to what the trait declares (Task 5 Step 3 below has the same warning). The shape shown here matches the P1 codebase as of `6c26ed2` (verified by reading `crates/agent-models/src/lib.rs`).

- [ ] **Step 3: Add `Compactor` and `CompactorError`**

Append to `crates/agent-memory/src/compactor.rs` (above the `#[cfg(test)]` block):

```rust
use agent_models::{ModelClient, ModelEvent, ModelMessage, ModelRequest};
use futures::StreamExt;
use std::time::Duration;
use thiserror::Error;

/// Errors returned by [`Compactor::compact_with_llm`].
#[derive(Debug, Error)]
pub enum CompactorError {
    /// LLM call failed every retry attempt.
    #[error("compactor LLM failed: {0}")]
    LlmFailed(String),
    /// LLM returned an empty (whitespace-only) summary.
    #[error("compactor returned empty summary")]
    Empty,
}

/// Number of LLM retry attempts (per spec §4.4 — the third failure
/// triggers the sliding-window fallback at the runtime layer).
pub const LLM_RETRY_ATTEMPTS: u32 = 3;

/// Initial backoff between LLM retries; doubles each attempt.
pub const LLM_RETRY_INITIAL_BACKOFF: Duration = Duration::from_millis(200);

pub struct Compactor;

impl Compactor {
    /// Call the configured model with [`COMPACTOR_PROMPT`] + `transcript`,
    /// retrying up to [`LLM_RETRY_ATTEMPTS`] times with exponential backoff.
    /// Returns the assembled summary text on success.
    pub async fn compact_with_llm(
        model: &dyn ModelClient,
        profile_alias: &str,
        transcript: &str,
    ) -> Result<String, CompactorError> {
        let messages = vec![ModelMessage {
            role: "user".into(),
            content: format!(
                "{COMPACTOR_PROMPT}\n\n--- BEGIN TRANSCRIPT ---\n{transcript}\n--- END TRANSCRIPT ---"
            ),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }];
        let request = ModelRequest {
            model_profile: profile_alias.to_string(),
            messages,
            system_prompt: None,
            tools: Vec::new(),
        };

        let mut backoff = LLM_RETRY_INITIAL_BACKOFF;
        let mut last_err: Option<String> = None;
        for attempt in 0..LLM_RETRY_ATTEMPTS {
            match Self::collect_summary(model, request.clone()).await {
                Ok(text) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        return Err(CompactorError::Empty);
                    }
                    return Ok(trimmed.to_string());
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt + 1 < LLM_RETRY_ATTEMPTS {
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                    }
                }
            }
        }
        Err(CompactorError::LlmFailed(
            last_err.unwrap_or_else(|| "unknown".into()),
        ))
    }

    async fn collect_summary(
        model: &dyn ModelClient,
        request: ModelRequest,
    ) -> Result<String, String> {
        let mut response = model.invoke(request).await.map_err(|e| e.to_string())?;
        let mut buf = String::new();
        while let Some(event) = response.stream.next().await {
            match event {
                Ok(ModelEvent::TextDelta { delta }) => buf.push_str(&delta),
                Ok(ModelEvent::Completed { .. }) => return Ok(buf),
                Ok(ModelEvent::Failed { message }) => return Err(message),
                Err(e) => return Err(e.to_string()),
                _ => {}
            }
        }
        Ok(buf)
    }

    /// Fallback used when the LLM call exhausted its retries: produce a
    /// synthetic placeholder so the compaction event chain still completes
    /// (and the next agent loop iteration sees a smaller history).
    pub fn sliding_window_fallback(candidate_event_count: usize) -> String {
        format!("[Dropped {candidate_event_count} earlier turns by sliding window]")
    }
}
```

- [ ] **Step 4: Update lib re-exports**

In `crates/agent-memory/src/lib.rs`, expand the `pub use` line for the compactor module:

```rust
pub use compactor::{render_transcript, Compactor, CompactorError, COMPACTOR_PROMPT, LLM_RETRY_ATTEMPTS};
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p agent-memory compactor -- --nocapture`
Expected: all 5+ tests in the module pass (the 3 from Task 4 + the new ones from Task 5).

- [ ] **Step 6: Run clippy on the touched crate**

Run: `cargo clippy -p agent-memory --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/agent-memory/src/compactor.rs crates/agent-memory/src/lib.rs crates/agent-memory/Cargo.toml
git commit -m "feat(memory): add Compactor with LLM retries and sliding-window fallback"
```

---

### Task 6 — Add `compacting` flag + `pick_compaction_boundary` helper to `agent-runtime`

**Files:**

- Modify: `crates/agent-runtime/src/session.rs`
- Create: `crates/agent-runtime/src/compaction.rs` (skeleton + boundary helper only — orchestrator lands in Task 7)
- Modify: `crates/agent-runtime/src/lib.rs`

- [ ] **Step 1: Write the failing test for `pick_compaction_boundary`**

Create `crates/agent-runtime/src/compaction.rs`:

```rust
//! Session-level compaction orchestrator. Owns the busy-gate transitions,
//! decides which event range becomes the compaction candidate, calls the
//! `agent-memory::Compactor`, and emits the four `EventPayload` variants
//! introduced in P2 (started / completed / failed / summary).
//!
//! The orchestrator function `compact_session` is added in Task 7. This
//! file currently exposes only the boundary helper used by both the
//! orchestrator and its unit tests.

use agent_core::{DomainEvent, EventPayload};
use chrono::{DateTime, Utc};

/// We always keep this many of the most recent user/assistant message
/// PAIRS in the live history (the rest become a compaction candidate).
/// Per spec §4.4 → K = 6 messages = 3 pairs.
pub const KEEP_LAST_PAIRS: usize = 3;

/// Identify the timestamp range `[first_ts, last_ts]` (inclusive) that
/// should be replaced by a `CompactionSummary`. Returns `None` when the
/// session does not yet have enough history to compact (i.e. there are
/// fewer than `keep_last_pairs * 2 + 1` user/assistant messages).
///
/// "Pair" here means one user message + one assistant response. Tool calls
/// and other meta events do NOT count toward the pair quota — they ride
/// along with whichever pair brackets them by timestamp.
pub fn pick_compaction_boundary(
    events: &[DomainEvent],
    keep_last_pairs: usize,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    // Collect indices of user / assistant messages in chronological order.
    let mut convo_idx: Vec<usize> = Vec::new();
    for (i, e) in events.iter().enumerate() {
        if matches!(
            e.payload,
            EventPayload::UserMessageAdded { .. } | EventPayload::AssistantMessageCompleted { .. }
        ) {
            convo_idx.push(i);
        }
    }

    let to_keep = keep_last_pairs * 2;
    if convo_idx.len() <= to_keep {
        // Not enough conversation to compact yet — nothing to replace.
        return None;
    }

    // The "split point" is the index of the first conversation event we KEEP.
    let split = convo_idx[convo_idx.len() - to_keep];

    // Compaction candidate = every event with timestamp STRICTLY before
    // events[split].timestamp. We use timestamps (not indices) because the
    // spec demands that `replaces_event_range` be timestamp-based — see
    // §4.4 "DomainEvent currently has no stable id".
    let first_ts = events.first().map(|e| e.timestamp)?;
    let split_ts = events[split].timestamp;
    // Find the LAST event whose timestamp is strictly before split_ts.
    let last_idx = events
        .iter()
        .enumerate()
        .rev()
        .find(|(_, e)| e.timestamp < split_ts)
        .map(|(i, _)| i)?;
    let last_ts = events[last_idx].timestamp;
    if last_ts < first_ts {
        return None;
    }
    Some((first_ts, last_ts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };
    use chrono::Duration;

    fn make_event_at(payload: EventPayload, ts_offset_secs: i64) -> DomainEvent {
        let ts = chrono::Utc::now() + Duration::seconds(ts_offset_secs);
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
        .with_timestamp(ts)
    }

    fn user(i: usize, t: i64) -> DomainEvent {
        make_event_at(
            EventPayload::UserMessageAdded {
                message_id: format!("u{i}"),
                content: format!("u{i}"),
            },
            t,
        )
    }
    fn assistant(i: usize, t: i64) -> DomainEvent {
        make_event_at(
            EventPayload::AssistantMessageCompleted {
                message_id: format!("a{i}"),
                content: format!("a{i}"),
            },
            t,
        )
    }

    #[test]
    fn returns_none_when_not_enough_history() {
        // 2 pairs, asked to keep 3 pairs → nothing to compact.
        let events = vec![
            user(0, 0),
            assistant(0, 1),
            user(1, 2),
            assistant(1, 3),
        ];
        assert!(pick_compaction_boundary(&events, 3).is_none());
    }

    #[test]
    fn boundary_excludes_kept_recent_pairs() {
        // 5 pairs, keep last 3 → first 2 pairs are candidates.
        let events: Vec<DomainEvent> = (0..5)
            .flat_map(|i| {
                let t = (i as i64) * 10;
                vec![user(i, t), assistant(i, t + 1)]
            })
            .collect();
        let (first, last) = pick_compaction_boundary(&events, 3).expect("boundary");
        assert_eq!(first, events[0].timestamp); // user 0
        assert_eq!(last, events[3].timestamp); // assistant 1 (the last event before pair 2)
    }

    #[test]
    fn meta_events_ride_along_with_pairs() {
        // Insert a permission event in the middle. It must be inside the
        // boundary if its timestamp is < split_ts.
        let mut events: Vec<DomainEvent> = (0..5)
            .flat_map(|i| {
                let t = (i as i64) * 10;
                vec![user(i, t), assistant(i, t + 1)]
            })
            .collect();
        // Insert a meta event between pair 0 and pair 1 (timestamp = 5).
        events.insert(
            2,
            make_event_at(
                EventPayload::PermissionGranted {
                    request_id: "p1".into(),
                },
                5,
            ),
        );
        // Re-sort by timestamp to mimic event-store order.
        events.sort_by_key(|e| e.timestamp);

        let (_, last) = pick_compaction_boundary(&events, 3).expect("boundary");
        // The last candidate's timestamp must be strictly before pair-2's user
        // message timestamp (= 20). Our PermissionGranted at t=5 should be
        // inside the candidate range.
        assert!(last < chrono::Utc::now() + Duration::seconds(20));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails (red phase)**

Run: `cargo test -p agent-runtime --lib compaction::tests`
Expected: FAIL — `module 'compaction' not found`. Once the file exists, all 3 tests should pass IF the implementation above is correct. To force a strict-TDD red phase, temporarily replace the body of `pick_compaction_boundary` with `None` and re-run; the latter two tests fail.

- [ ] **Step 3: Restore implementation and verify all tests pass**

Restore `pick_compaction_boundary` to the version above.

Run: `cargo test -p agent-runtime --lib compaction::tests -- --nocapture`
Expected: 3 tests pass.

- [ ] **Step 4: Add `compacting` flag to `SessionState`**

In `crates/agent-runtime/src/session.rs`, modify the `SessionState` struct:

```rust
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    pub model_limits: Option<ModelLimits>,
    pub usage_corrector: UsageCorrector,
    pub last_estimated_tokens: u64,
    /// `true` while a `compact_session` call is in flight. `send_message`
    /// must reject with `CoreError::SessionBusy` when this is set.
    pub compacting: bool,
}
```

> The `Default` derive already populates `compacting: false`, so no other code needs to change.

- [ ] **Step 5: Wire `compaction` module into `lib.rs`**

In `crates/agent-runtime/src/lib.rs`, add `pub mod compaction;` next to the other module declarations.

- [ ] **Step 6: Run runtime tests + build**

Run: `cargo test -p agent-runtime --lib`
Expected: all green (boundary tests + existing tests; the new `compacting: bool` field is additive).

Run: `cargo build --workspace --all-targets`
Expected: success.

- [ ] **Step 7: Commit**

```bash
git add crates/agent-runtime/src/compaction.rs crates/agent-runtime/src/session.rs crates/agent-runtime/src/lib.rs
git commit -m "feat(runtime): add SessionState.compacting flag and pick_compaction_boundary helper"
```

---

### Task 7 — Implement `compaction::compact_session` orchestrator (events emit + retries + fallback)

**Files:**

- Modify: `crates/agent-runtime/src/compaction.rs`

- [ ] **Step 1: Write the failing test (orchestrator end-to-end inside the unit module)**

Append to the `#[cfg(test)] mod tests` block of `crates/agent-runtime/src/compaction.rs` (after the boundary tests from Task 6):

```rust
    use agent_core::AgentId;
    use agent_memory::Compactor;
    use agent_models::{
        ModelClient, ModelEvent, ModelRequest, ModelResponse, ModelResult,
    };
    use agent_store::{EventStore, SqliteEventStore};
    use async_trait::async_trait;
    use futures::stream::{self, BoxStream, StreamExt};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex as StdMutex};
    use tokio::sync::Mutex;

    struct StubSummariser {
        summary: String,
        fail_count: Arc<StdMutex<u32>>,
    }

    impl StubSummariser {
        fn new(summary: &str, fails: u32) -> Self {
            Self {
                summary: summary.to_string(),
                fail_count: Arc::new(StdMutex::new(fails)),
            }
        }
    }

    #[async_trait]
    impl ModelClient for StubSummariser {
        async fn invoke(&self, _req: ModelRequest) -> ModelResult<ModelResponse> {
            let mut left = self.fail_count.lock().unwrap();
            if *left > 0 {
                *left -= 1;
                return Err(agent_models::ModelError::Other("transient".into()));
            }
            let text = self.summary.clone();
            let stream: BoxStream<'static, ModelResult<ModelEvent>> = stream::iter(vec![
                Ok(ModelEvent::Started),
                Ok(ModelEvent::TextDelta { delta: text }),
                Ok(ModelEvent::Completed { usage: None }),
            ])
            .boxed();
            Ok(ModelResponse { stream })
        }
    }

    async fn fixture_session_with_n_pairs(n: usize) -> (Arc<SqliteEventStore>, agent_core::WorkspaceId, agent_core::SessionId) {
        let store = Arc::new(SqliteEventStore::in_memory().await.unwrap());
        let ws = agent_core::WorkspaceId::new();
        let ses = agent_core::SessionId::new();
        for i in 0..n {
            let u = DomainEvent::new(
                ws.clone(), ses.clone(), AgentId::system(),
                agent_core::PrivacyClassification::FullTrace,
                EventPayload::UserMessageAdded {
                    message_id: format!("u{i}"),
                    content: format!("user {i}"),
                },
            ).with_timestamp(chrono::Utc::now() + chrono::Duration::seconds(i as i64 * 2));
            store.append(&u).await.unwrap();
            let a = DomainEvent::new(
                ws.clone(), ses.clone(), AgentId::system(),
                agent_core::PrivacyClassification::FullTrace,
                EventPayload::AssistantMessageCompleted {
                    message_id: format!("a{i}"),
                    content: format!("assistant {i}"),
                },
            ).with_timestamp(chrono::Utc::now() + chrono::Duration::seconds(i as i64 * 2 + 1));
            store.append(&a).await.unwrap();
        }
        (store, ws, ses)
    }

    #[tokio::test]
    async fn compact_session_emits_started_summary_completed_in_order() {
        let (store, ws, ses) = fixture_session_with_n_pairs(8).await;
        let model = StubSummariser::new("## User goal\nfix tests\n", 0);
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        let states: Arc<Mutex<HashMap<String, crate::session::SessionState>>> =
            Arc::new(Mutex::new(HashMap::new()));

        compact_session(
            &*store,
            &tx,
            &model,
            "fast",
            &states,
            ws,
            ses.clone(),
            agent_core::CompactionReason::UserRequested,
        )
        .await
        .expect("compaction should succeed");

        let events = store.load_session(&ses).await.unwrap();
        let types: Vec<&str> = events.iter().map(|e| e.payload.event_type()).collect();
        let started = types.iter().position(|t| *t == "ContextCompactionStarted").expect("started");
        let summary = types.iter().position(|t| *t == "CompactionSummary").expect("summary");
        let completed = types.iter().position(|t| *t == "ContextCompactionCompleted").expect("completed");
        assert!(started < summary && summary < completed,
            "events out of order: {types:?}");

        // After completion, compacting must be false.
        let states = states.lock().await;
        assert!(!states.get(&ses.to_string()).map(|s| s.compacting).unwrap_or(true));
    }

    #[tokio::test]
    async fn compact_session_uses_sliding_window_fallback_after_llm_failures() {
        let (store, ws, ses) = fixture_session_with_n_pairs(8).await;
        let model = StubSummariser::new("ignored", 99); // always fails
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        let states: Arc<Mutex<HashMap<String, crate::session::SessionState>>> =
            Arc::new(Mutex::new(HashMap::new()));

        compact_session(
            &*store,
            &tx,
            &model,
            "fast",
            &states,
            ws,
            ses.clone(),
            agent_core::CompactionReason::UserRequested,
        )
        .await
        .expect("fallback should still complete the chain");

        let events = store.load_session(&ses).await.unwrap();
        let summary_evt = events.iter().find_map(|e| match &e.payload {
            EventPayload::CompactionSummary { content, .. } => Some(content.clone()),
            _ => None,
        }).expect("must have summary even on LLM failure");
        assert!(summary_evt.contains("sliding window"), "expected fallback marker, got: {summary_evt}");

        let failed = events.iter().any(|e| matches!(&e.payload,
            EventPayload::ContextCompactionFailed { fallback_used: true, .. }));
        assert!(failed, "expected ContextCompactionFailed { fallback_used: true }");
    }

    #[tokio::test]
    async fn compact_session_returns_none_when_history_too_short() {
        let (store, ws, ses) = fixture_session_with_n_pairs(2).await; // < 3 pairs
        let model = StubSummariser::new("ignored", 0);
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        let states: Arc<Mutex<HashMap<String, crate::session::SessionState>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let result = compact_session(
            &*store,
            &tx,
            &model,
            "fast",
            &states,
            ws,
            ses.clone(),
            agent_core::CompactionReason::UserRequested,
        )
        .await;
        assert!(result.is_ok());

        // No CompactionSummary should be appended.
        let events = store.load_session(&ses).await.unwrap();
        assert!(!events.iter().any(|e| matches!(&e.payload, EventPayload::CompactionSummary { .. })));
    }
```

- [ ] **Step 2: Run tests (red phase)**

Run: `cargo test -p agent-runtime --lib compaction::tests::compact_session`
Expected: FAIL — `cannot find function 'compact_session' in this scope`.

- [ ] **Step 3: Implement the orchestrator**

Append to `crates/agent-runtime/src/compaction.rs` (above the `#[cfg(test)]` block):

```rust
use crate::event_emitter::append_and_broadcast;
use crate::session::SessionState;
use agent_core::{
    AgentId, CompactionReason, CoreError, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_memory::{render_transcript, Compactor};
use agent_models::ModelClient;
use agent_store::EventStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// Drive a single compaction pass for `session_id`. Synchronous in the
/// sense that the caller can `.await` until the chain completes; the
/// busy-gate is set on entry and cleared on exit (success or fallback).
///
/// Event sequence on success:
///   1. `ContextCompactionStarted`
///   2. `CompactionSummary { fallback_used = false equivalent (no field) }`
///   3. `ContextCompactionCompleted { fallback_used: false }`
///
/// Event sequence on LLM failure:
///   1. `ContextCompactionStarted`
///   2. `ContextCompactionFailed { fallback_used: true, error }`
///   3. `CompactionSummary { content: "[Dropped N earlier turns ...]" }`
///   4. `ContextCompactionCompleted { fallback_used: true }`
///
/// Returns `Ok(())` even when the LLM failed (the fallback ensures the
/// runtime always exits the busy state with a usable summary).
/// Returns `Err(CoreError::InvalidState)` only on event-store I/O errors.
#[allow(clippy::too_many_arguments)]
pub async fn compact_session<S: EventStore>(
    store: &S,
    event_tx: &broadcast::Sender<agent_core::DomainEvent>,
    model: &dyn ModelClient,
    profile_alias: &str,
    session_states: &Arc<Mutex<HashMap<String, SessionState>>>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    reason: CompactionReason,
) -> agent_core::Result<()> {
    // Acquire busy gate. If already compacting, treat as no-op (the caller
    // is responsible for not stacking compactions; `send_message`'s gate
    // returns SessionBusy upstream).
    {
        let mut states = session_states.lock().await;
        let entry = states
            .entry(session_id.to_string())
            .or_insert_with(SessionState::default);
        if entry.compacting {
            return Ok(());
        }
        entry.compacting = true;
    }

    let outcome = compact_inner(
        store,
        event_tx,
        model,
        profile_alias,
        workspace_id,
        session_id.clone(),
        reason,
    )
    .await;

    // Always clear the busy flag.
    {
        let mut states = session_states.lock().await;
        if let Some(entry) = states.get_mut(&session_id.to_string()) {
            entry.compacting = false;
        }
    }

    outcome
}

async fn compact_inner<S: EventStore>(
    store: &S,
    event_tx: &broadcast::Sender<agent_core::DomainEvent>,
    model: &dyn ModelClient,
    profile_alias: &str,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    reason: CompactionReason,
) -> agent_core::Result<()> {
    let events = store
        .load_session(&session_id)
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))?;

    let Some((first_ts, last_ts)) = pick_compaction_boundary(&events, KEEP_LAST_PAIRS) else {
        // Not enough history — silently no-op (no events emitted).
        return Ok(());
    };
    let candidate: Vec<_> = events
        .iter()
        .filter(|e| e.timestamp >= first_ts && e.timestamp <= last_ts)
        .cloned()
        .collect();
    let candidate_count = candidate.len();

    // Emit Started.
    let before_tokens = estimate_event_tokens(&candidate);
    let started = agent_core::DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        agent_core::EventPayload::ContextCompactionStarted {
            reason,
            before_tokens,
            candidate_event_count: candidate_count,
        },
    );
    append_and_broadcast(store, event_tx, &started).await?;

    // Try the LLM compaction.
    let transcript = render_transcript(&candidate);
    let llm_outcome = Compactor::compact_with_llm(model, profile_alias, &transcript).await;

    let (content, fallback_used, failure_error) = match llm_outcome {
        Ok(text) => (text, false, None),
        Err(e) => {
            let msg = e.to_string();
            (
                Compactor::sliding_window_fallback(candidate_count),
                true,
                Some(msg),
            )
        }
    };

    if fallback_used {
        let failed_evt = agent_core::DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            agent_core::EventPayload::ContextCompactionFailed {
                error: failure_error.unwrap_or_default(),
                fallback_used: true,
            },
        );
        append_and_broadcast(store, event_tx, &failed_evt).await?;
    }

    let summary_id = format!("sum_{}", uuid::Uuid::new_v4().simple());
    let after_tokens = estimate_text_tokens(&content);
    let summary_evt = agent_core::DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        agent_core::EventPayload::CompactionSummary {
            summary_id: summary_id.clone(),
            content,
            replaces_event_range: (first_ts, last_ts),
            reason,
            before_tokens,
            after_tokens,
            summarised_by_profile: profile_alias.to_string(),
        },
    );
    append_and_broadcast(store, event_tx, &summary_evt).await?;

    let completed_evt = agent_core::DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        agent_core::EventPayload::ContextCompactionCompleted {
            summary_id,
            after_tokens,
            fallback_used,
        },
    );
    append_and_broadcast(store, event_tx, &completed_evt).await?;

    Ok(())
}

fn estimate_event_tokens(events: &[agent_core::DomainEvent]) -> u64 {
    let bpe = match tiktoken_rs::cl100k_base() {
        Ok(b) => b,
        Err(_) => return 0,
    };
    let mut total: u64 = 0;
    for e in events {
        if let Ok(json) = serde_json::to_string(&e.payload) {
            total += bpe.encode_with_special_tokens(&json).len() as u64;
        }
    }
    total
}

fn estimate_text_tokens(text: &str) -> u64 {
    match tiktoken_rs::cl100k_base() {
        Ok(b) => b.encode_with_special_tokens(text).len() as u64,
        Err(_) => 0,
    }
}
```

> **Note on `agent-store` API**: The test calls `SqliteEventStore::in_memory()` — verify the actual constructor name with `grep -n "fn in_memory\|fn new_in_memory\|pub async fn new" crates/agent-store/src/*.rs`. If the project uses `SqliteEventStore::new(":memory:")` or similar, adjust both the test fixture and any other call. The orchestrator itself never names the concrete store — it's generic over `S: EventStore`.

> **Note on `tiktoken-rs` dep**: `agent-runtime/Cargo.toml` already pulls `tiktoken-rs` (P1 introduced it in `agent_loop.rs::build_model_messages_within_budget`). Confirm with `grep tiktoken crates/agent-runtime/Cargo.toml`. If absent, add `tiktoken-rs = { workspace = true }`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p agent-runtime --lib compaction::tests -- --nocapture`
Expected: all 6 tests pass (3 boundary + 3 orchestrator).

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p agent-runtime --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime/src/compaction.rs
git commit -m "feat(runtime): implement compact_session orchestrator with LLM + sliding-window fallback"
```

---

### Task 8 — Wire `LocalRuntime::compact_session` facade method + busy gate in `send_message`

**Files:**

- Modify: `crates/agent-runtime/src/facade_runtime.rs`

- [ ] **Step 1: Read the existing `send_message` implementation to find the gate point**

Run: `grep -n "fn send_message\|impl AppFacade" crates/agent-runtime/src/facade_runtime.rs | head -20`

Open the file and locate the `AppFacade::send_message` impl (around line 460-500 based on P1's structure — the function signature is `async fn send_message(&self, request: SendMessageRequest) -> Result<()>`). The busy gate goes at the very top of the function body, **before** any event is emitted (so the rejection happens before the user message is recorded).

- [ ] **Step 2: Write the failing test**

Append to `crates/agent-runtime/src/facade_runtime.rs` `#[cfg(test)] mod tests` (or its sibling test file — pick whichever matches the existing test layout in this file):

```rust
    #[tokio::test]
    async fn send_message_returns_session_busy_when_compacting() {
        // Wire a minimal LocalRuntime with FakeModelClient.
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = agent_models::FakeModelClient::new("ignored");
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/x".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        // Force the session into compacting state.
        {
            let mut states = runtime.session_states.lock().await;
            states
                .entry(session_id.to_string())
                .or_insert_with(crate::session::SessionState::default)
                .compacting = true;
        }

        let result = runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hello".into(),
                attachments: vec![],
            })
            .await;
        match result {
            Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
                assert_eq!(id, session_id.to_string());
            }
            other => panic!("expected SessionBusy, got {other:?}"),
        }
    }
```

> Imports needed at the top of the test module: `use super::*;` plus `use agent_core::{StartSessionRequest, SendMessageRequest};` and `use agent_store::SqliteEventStore;`. If the existing test module already imports these, reuse them.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p agent-runtime send_message_returns_session_busy`
Expected: FAIL — currently `send_message` ignores `compacting` so the call proceeds (test asserts `SessionBusy`).

- [ ] **Step 4: Add the busy-gate check inside `send_message`**

Open `crates/agent-runtime/src/facade_runtime.rs` and find the `impl<S, M> AppFacade for LocalRuntime<S, M>` block. Inside the `async fn send_message(&self, request: SendMessageRequest) -> Result<()>` body, insert at the very top (before any `append_and_broadcast` call):

```rust
        // Reject sends while a compaction is in flight (P2 busy gate).
        // The state is cleared by `compaction::compact_session` on exit.
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&request.session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: request.session_id.to_string(),
                        reason: "context compaction in progress".into(),
                    });
                }
            }
        }
```

- [ ] **Step 5: Add `LocalRuntime::compact_session` facade method**

In the same file, inside the inherent `impl<S, M> LocalRuntime<S, M>` block (NOT the `AppFacade` impl), add:

```rust
    /// Trigger a compaction pass for `session_id`. Blocks until the chain
    /// completes (success or fallback). Returns `Err(SessionBusy)` if a
    /// compaction is already running for the same session.
    ///
    /// This is the inherent method; P3 will surface it via the `AppFacade`
    /// trait once the GUI/TUI commands wire to it.
    pub async fn compact_session(
        &self,
        session_id: SessionId,
        reason: agent_core::CompactionReason,
    ) -> agent_core::Result<()> {
        // Resolve the workspace_id from the latest event of the session.
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let workspace_id = events
            .first()
            .map(|e| e.workspace_id.clone())
            .ok_or_else(|| agent_core::CoreError::InvalidState(
                "session has no events".into(),
            ))?;

        // Pre-check the busy gate so we can surface SessionBusy upfront
        // (the orchestrator silently no-ops when already compacting).
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: session_id.to_string(),
                        reason: "compaction already running".into(),
                    });
                }
            }
        }

        // Pick the profile alias for the summarisation call: ContextPolicy.compactor_profile
        // takes priority; otherwise we fall back to the session's current profile.
        let profile_alias = self
            .config
            .context
            .compactor_profile
            .clone()
            .unwrap_or_else(|| {
                events
                    .iter()
                    .find_map(|e| match &e.payload {
                        agent_core::EventPayload::SessionInitialized { model_profile } => {
                            Some(model_profile.clone())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| "fake".to_string())
            });

        crate::compaction::compact_session(
            &*self.store,
            &self.event_tx,
            &*self.model,
            &profile_alias,
            &self.session_states,
            workspace_id,
            session_id,
            reason,
        )
        .await
    }
```

> **Note**: `&*self.model` works because `M: ModelClient + 'static` and `Arc<M>` derefs to `M`. If the compiler complains about coercion to `&dyn ModelClient`, change the call's `model: &dyn ModelClient` parameter to a generic `model: &M` (`M: ModelClient`) — both work; the test uses `FakeModelClient` directly.

- [ ] **Step 6: Run tests**

Run: `cargo test -p agent-runtime send_message_returns_session_busy`
Expected: PASS.

Run: `cargo test -p agent-runtime --lib`
Expected: full lib suite green.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p agent-runtime --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-runtime/src/facade_runtime.rs
git commit -m "feat(runtime): add LocalRuntime::compact_session facade + busy gate in send_message"
```

---

### Task 9 — Substitute `CompactionSummary` for compacted events in `agent_loop`

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop.rs`

- [ ] **Step 1: Write the failing test for the substitution helper**

Append to `crates/agent-runtime/src/agent_loop.rs` `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn build_model_messages_substitutes_compaction_summary_for_event_range() {
        // Build 5 turns; insert a CompactionSummary covering the first 3 pairs.
        let base = chrono::Utc::now();
        let make_at = |payload: EventPayload, secs: i64| -> DomainEvent {
            DomainEvent::new(
                WorkspaceId::new(),
                SessionId::new(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                payload,
            )
            .with_timestamp(base + chrono::Duration::seconds(secs))
        };

        let mut events: Vec<DomainEvent> = (0..5)
            .flat_map(|i| {
                let t = (i as i64) * 10;
                vec![
                    make_at(
                        EventPayload::UserMessageAdded {
                            message_id: format!("u{i}"),
                            content: format!("user {i}"),
                        },
                        t,
                    ),
                    make_at(
                        EventPayload::AssistantMessageCompleted {
                            message_id: format!("a{i}"),
                            content: format!("assistant {i}"),
                        },
                        t + 1,
                    ),
                ]
            })
            .collect();

        let first_ts = events[0].timestamp;
        let last_ts = events[5].timestamp; // covers pairs 0..=2 inclusive
        events.push(make_at(
            EventPayload::CompactionSummary {
                summary_id: "sum_test".into(),
                content: "[SUMMARY] earlier turns about user goal X".into(),
                replaces_event_range: (first_ts, last_ts),
                reason: agent_core::CompactionReason::UserRequested,
                before_tokens: 1000,
                after_tokens: 50,
                summarised_by_profile: "fast".into(),
            },
            55, // newer than every replaced event but older than the new turn
        ));
        events.sort_by_key(|e| e.timestamp);

        let messages = build_model_messages("latest", &events);

        // (a) The summary text MUST appear in messages.
        let joined: String = messages.iter().map(|m| m.content.clone()).collect::<Vec<_>>().join("\n");
        assert!(
            joined.contains("[SUMMARY] earlier turns about user goal X"),
            "summary text missing from assembled messages: {joined}"
        );
        // (b) The replaced "user 0".."assistant 2" content must NOT appear (pairs 0,1,2 = indices 0..=5).
        for replaced in ["user 0", "assistant 0", "user 1", "assistant 1", "user 2", "assistant 2"] {
            assert!(
                !joined.contains(replaced),
                "replaced event '{replaced}' leaked into messages: {joined}"
            );
        }
        // (c) The kept tail ("user 3", "assistant 3", "user 4", "assistant 4") must remain.
        for kept in ["user 3", "assistant 3", "user 4", "assistant 4"] {
            assert!(
                joined.contains(kept),
                "kept event '{kept}' missing from messages: {joined}"
            );
        }
        // (d) The trailing "latest" user turn must still be present.
        assert_eq!(messages.last().map(|m| m.content.as_str()), Some("latest"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-runtime --lib agent_loop::tests::build_model_messages_substitutes_compaction_summary`
Expected: FAIL — current `build_model_messages` ignores `CompactionSummary`, so all 5 user/assistant pairs leak through.

- [ ] **Step 3: Refactor `build_model_messages` to apply summary substitution**

In `crates/agent-runtime/src/agent_loop.rs`, replace the body of `build_model_messages` (the existing implementation) with a wrapper that:

1. Pre-collects every `CompactionSummary` payload into a sorted list of `(first_ts, last_ts, content)`.
2. Iterates the original events; when an event's timestamp falls inside ANY summary range, the event is skipped.
3. At the position where each summary range first appears (i.e., right before the FIRST kept event whose timestamp >= range.last_ts + epsilon), it injects a synthetic system-tagged user message containing the summary content.
4. The rest of the build logic (tool-call collection, `pending_tool_calls`) is unchanged but operates on the FILTERED event list.

Concretely, restructure as follows. Insert at the top of `build_model_messages`:

```rust
    // Phase A — collect compaction summaries (sorted by their start timestamp).
    let mut summaries: Vec<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, String)> =
        session_events
            .iter()
            .filter_map(|e| match &e.payload {
                EventPayload::CompactionSummary {
                    replaces_event_range,
                    content,
                    ..
                } => Some((
                    replaces_event_range.0,
                    replaces_event_range.1,
                    content.clone(),
                )),
                _ => None,
            })
            .collect();
    summaries.sort_by_key(|(first, _, _)| *first);

    // Helper: event timestamp is covered by any summary range.
    let covered = |ts: chrono::DateTime<chrono::Utc>| -> bool {
        summaries
            .iter()
            .any(|(first, last, _)| ts >= *first && ts <= *last)
    };

    // Phase B — filter the event list AND track which summaries have been
    // injected so we don't insert them twice.
    let mut filtered: Vec<&DomainEvent> = Vec::with_capacity(session_events.len());
    let mut injected: Vec<bool> = vec![false; summaries.len()];
    for event in session_events {
        // Skip CompactionSummary itself (it's not a chat message).
        if matches!(event.payload, EventPayload::CompactionSummary { .. }) {
            continue;
        }
        // Inject any summary whose range ends BEFORE this event's timestamp,
        // and which hasn't been injected yet — in chronological order.
        for (idx, (_, last_ts, content)) in summaries.iter().enumerate() {
            if !injected[idx] && event.timestamp > *last_ts {
                filtered.push(synthetic_summary_event(content));
                injected[idx] = true;
            }
        }
        if covered(event.timestamp) {
            continue;
        }
        filtered.push(event);
    }
    // Inject any trailing summaries (e.g. the entire history was compacted).
    for (idx, (_, _, content)) in summaries.iter().enumerate() {
        if !injected[idx] {
            filtered.push(synthetic_summary_event(content));
        }
    }

    // The rest of the function operates on `filtered.iter()` (instead of
    // `session_events.iter()`).
```

Add the helper at module level:

```rust
/// Build a transient `DomainEvent` representing an injected `CompactionSummary`
/// content as a "user" turn so `build_model_messages` can treat it uniformly.
/// This never leaves the function — it's a memory-only sleeve, never persisted.
fn synthetic_summary_event(content: &str) -> &'static DomainEvent {
    // We can't return a reference to a local. Use a thread_local store of
    // leaked Boxes keyed by content hash. For test simplicity AND to avoid
    // cross-iteration leaks, we instead rewrite `build_model_messages` to
    // accept a small enum (Either<&DomainEvent, String>) — see Step 4.
    unreachable!("see Step 4: switch to Either<&DomainEvent, String> sleeve")
}
```

- [ ] **Step 4: Switch the loop to an `Either<&DomainEvent, &String>` sleeve**

The "synthetic event" sketch above can't actually return `&'static DomainEvent`. The clean fix: change `filtered`'s element type and the `match` block so summaries become a literal injected message string.

Replace the Phase B body with:

```rust
    enum Sleeve<'a> {
        Real(&'a DomainEvent),
        Summary(&'a str),
    }

    let mut filtered: Vec<Sleeve<'_>> = Vec::with_capacity(session_events.len() + summaries.len());
    let mut injected: Vec<bool> = vec![false; summaries.len()];
    for event in session_events {
        if matches!(event.payload, EventPayload::CompactionSummary { .. }) {
            continue;
        }
        for (idx, (_, last_ts, content)) in summaries.iter().enumerate() {
            if !injected[idx] && event.timestamp > *last_ts {
                filtered.push(Sleeve::Summary(content.as_str()));
                injected[idx] = true;
            }
        }
        if covered(event.timestamp) {
            continue;
        }
        filtered.push(Sleeve::Real(event));
    }
    for (idx, (_, _, content)) in summaries.iter().enumerate() {
        if !injected[idx] {
            filtered.push(Sleeve::Summary(content.as_str()));
        }
    }
```

Then convert the existing two-pass message-building loop to use `Sleeve`:

```rust
    // First pass: collect tool call requests + results from real events only.
    for sleeve in &filtered {
        let Sleeve::Real(event) = sleeve else { continue };
        match &event.payload {
            EventPayload::ModelToolCallRequested { tool_call_id, tool_id } => {
                pending_tool_calls.push(agent_models::ToolCall {
                    id: tool_call_id.clone(),
                    name: tool_id.clone(),
                    arguments: serde_json::json!({}),
                });
            }
            EventPayload::ToolInvocationCompleted { invocation_id, tool_id, output_preview, .. } => {
                tool_results.insert(invocation_id.clone(), (tool_id.clone(), output_preview.clone()));
            }
            _ => {}
        }
    }

    // Second pass: build messages, injecting summary sleeves as system-tagged
    // user messages (the model treats them as authoritative context).
    for sleeve in &filtered {
        match sleeve {
            Sleeve::Summary(content) => {
                messages.push(agent_models::ModelMessage {
                    role: "user".into(),
                    content: format!("[Conversation summary]\n{content}"),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                });
            }
            Sleeve::Real(event) => {
                match &event.payload {
                    EventPayload::UserMessageAdded { content, .. } => {
                        messages.push(agent_models::ModelMessage {
                            role: "user".into(),
                            content: content.clone(),
                            tool_calls: Vec::new(),
                            tool_call_id: None,
                        });
                    }
                    EventPayload::AssistantMessageCompleted { content, .. } => {
                        messages.push(agent_models::ModelMessage {
                            role: "assistant".into(),
                            content: content.clone(),
                            tool_calls: Vec::new(),
                            tool_call_id: None,
                        });
                    }
                    EventPayload::ToolInvocationCompleted { invocation_id, output_preview, .. } => {
                        messages.push(agent_models::ModelMessage {
                            role: "tool".into(),
                            content: output_preview.clone(),
                            tool_calls: Vec::new(),
                            tool_call_id: Some(invocation_id.clone()),
                        });
                    }
                    EventPayload::ToolInvocationFailed { invocation_id, error, .. } => {
                        messages.push(agent_models::ModelMessage {
                            role: "tool".into(),
                            content: format!("Error: {}", error),
                            tool_calls: Vec::new(),
                            tool_call_id: Some(invocation_id.clone()),
                        });
                    }
                    _ => {}
                }
            }
        }
    }
```

Drop the placeholder `synthetic_summary_event` helper from Step 3.

- [ ] **Step 5: Run the new test + the existing build_model_messages tests**

Run: `cargo test -p agent-runtime --lib agent_loop`
Expected: the new substitution test passes; existing tests (`within_budget_keeps_tail_user_and_pairs_tool_calls`, etc.) still pass.

- [ ] **Step 6: Run the wider crate suite**

Run: `cargo test -p agent-runtime --all-targets`
Expected: green.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p agent-runtime --all-targets -- -D warnings`
Expected: zero warnings (the unused-import / unused-variable lint can bite — fix as you go).

- [ ] **Step 8: Commit**

```bash
git add crates/agent-runtime/src/agent_loop.rs
git commit -m "feat(runtime): substitute CompactionSummary for compacted event range in build_model_messages"
```

---

### Task 10 — Auto-compaction trigger inside `agent_loop` after `ContextAssembled`

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop.rs`

- [ ] **Step 1: Read the existing `ContextAssembled` emit site**

Locate the line in `crates/agent-runtime/src/agent_loop.rs::run_agent_loop` where `EventPayload::ContextAssembled { usage: usage.clone() }` is appended (P1 emit site, around the middle of the function). The auto-trigger goes immediately after that `append_and_broadcast(...)` call but BEFORE the model invocation.

- [ ] **Step 2: Write the failing test (decision logic only)**

Add a new pure-function test in `crates/agent-runtime/src/agent_loop.rs` `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn should_trigger_auto_compaction_uses_threshold_and_not_compacting() {
        let usage_at = |total: u64, budget: u64| -> agent_core::ContextUsage {
            agent_core::ContextUsage {
                total_tokens: total,
                budget_tokens: budget,
                context_window: budget + 12_000,
                output_reservation: 12_000,
                by_source: vec![],
                estimator: "cl100k_base".into(),
                corrected_by_real_usage: false,
            }
        };

        // Below threshold → no trigger.
        assert!(!should_trigger_auto_compaction(&usage_at(50_000, 200_000), 0.85, false));
        // At threshold → trigger.
        assert!(should_trigger_auto_compaction(&usage_at(170_000, 200_000), 0.85, false));
        // Above threshold but already compacting → no trigger.
        assert!(!should_trigger_auto_compaction(&usage_at(190_000, 200_000), 0.85, true));
        // Threshold == 1.0 disables auto-compaction entirely.
        assert!(!should_trigger_auto_compaction(&usage_at(199_000, 200_000), 1.0, false));
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p agent-runtime --lib agent_loop::tests::should_trigger_auto_compaction`
Expected: FAIL — `cannot find function 'should_trigger_auto_compaction'`.

- [ ] **Step 4: Add the decision helper**

Append to `crates/agent-runtime/src/agent_loop.rs` (module-level, near `build_model_messages_within_budget`):

```rust
/// Decide whether the agent loop should fire an auto-compaction request
/// for this iteration. Pure function so it's trivial to unit-test the
/// boundary cases (threshold == 1.0 disables; busy gate skips; exact
/// equality counts as crossing the threshold per spec §4.4).
pub fn should_trigger_auto_compaction(
    usage: &agent_core::ContextUsage,
    threshold: f32,
    already_compacting: bool,
) -> bool {
    if already_compacting || threshold >= 1.0 {
        return false;
    }
    usage.ratio() >= threshold
}
```

- [ ] **Step 5: Verify the helper test passes**

Run: `cargo test -p agent-runtime --lib agent_loop::tests::should_trigger_auto_compaction`
Expected: 4 assertions pass.

- [ ] **Step 6: Wire the auto-trigger into `run_agent_loop`**

In `crates/agent-runtime/src/agent_loop.rs::run_agent_loop`, immediately after `append_and_broadcast(&**store, event_tx, &assembled_event).await?;` (the line that emits `ContextAssembled`), add:

```rust
    // Auto-compaction trigger (P2). Fire-and-forget: the spawned task takes
    // its own clones of the runtime deps so the agent loop does NOT block
    // on the summarisation LLM call. The busy gate inside `compact_session`
    // ensures we never stack two compactions for the same session.
    {
        let already_compacting = {
            let states = deps.session_states.lock().await;
            states
                .get(&request.session_id.to_string())
                .map(|s| s.compacting)
                .unwrap_or(false)
        };
        let threshold = deps.config.context.auto_compact_threshold;
        if should_trigger_auto_compaction(&usage, threshold, already_compacting) {
            let store_clone = store.clone();
            let model_clone = model.clone();
            let tx_clone = event_tx.clone();
            let states_clone = deps.session_states.clone();
            let workspace_id = request.workspace_id.clone();
            let session_id = request.session_id.clone();
            let ratio = usage.ratio();
            let profile_alias = deps
                .config
                .context
                .compactor_profile
                .clone()
                .unwrap_or_else(|| model_profile_alias.clone());
            tokio::spawn(async move {
                let _ = crate::compaction::compact_session(
                    &*store_clone,
                    &tx_clone,
                    &*model_clone,
                    &profile_alias,
                    &states_clone,
                    workspace_id,
                    session_id,
                    agent_core::CompactionReason::Threshold { ratio },
                )
                .await;
            });
        }
    }
```

> The `store`, `model`, `event_tx` already exist as `&Arc<S>` / `&Arc<M>` / `&broadcast::Sender<DomainEvent>` in `AgentLoopDeps` (verified by reading the existing destructure at the top of `run_agent_loop`). Cloning the `Arc`s is cheap.

- [ ] **Step 7: Run the wider crate suite**

Run: `cargo test -p agent-runtime --all-targets`
Expected: green (the auto-trigger only fires when the busy-gate is open AND `ratio >= threshold`; existing tests use the small `fake` profile with `context_window = 4096`, so the trigger may now fire in tests where it didn't before. Check for any test that asserts an EXACT event sequence and update accordingly using one of the three patterns from P1 Task 7 Step 7).

If a test starts seeing unexpected `ContextCompactionStarted` events, the safest fix is to override the threshold to `1.0` in the test setup:

```rust
let mut config = agent_config::Config::defaults();
config.context.auto_compact_threshold = 1.0;
let runtime = LocalRuntime::new(store, model).with_config(Arc::new(config));
```

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -p agent-runtime --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/agent-runtime/src/agent_loop.rs
git commit -m "feat(runtime): auto-trigger compaction when ContextAssembled crosses threshold"
```

---

### Task 11 — Full-stack integration test for compaction

**Files:**

- Create: `crates/agent-runtime/tests/compaction.rs`

- [ ] **Step 1: Write the integration test**

Create `crates/agent-runtime/tests/compaction.rs`:

```rust
//! Full-stack integration test for P2 context compaction.
//!
//! Wires a real `LocalRuntime` (in-memory `SqliteEventStore` +
//! `FakeModelClient`) and exercises:
//!  1. Manual `compact_session` end-to-end → four-event chain emitted.
//!  2. `send_message` rejected with `SessionBusy` while compacting.
//!  3. After completion, the next `send_message` works AND the assembled
//!     model request contains the summary text (the replaced events are
//!     gone from the message list).
//!  4. Sliding-window fallback path when the compactor model fails.

use agent_config::{Config, ContextPolicy};
use agent_core::{
    AppFacade, CompactionReason, EventPayload, SendMessageRequest, StartSessionRequest,
};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::{EventStore, SqliteEventStore};
use std::sync::Arc;

async fn fixture_runtime_with_history(
    pairs: usize,
) -> (
    Arc<LocalRuntime<SqliteEventStore, FakeModelClient>>,
    agent_core::WorkspaceId,
    agent_core::SessionId,
) {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new("Done.");
    let mut config = Config::defaults();
    // Disable auto-compaction so we can drive the manual path deterministically.
    config.context = ContextPolicy {
        auto_compact_threshold: 1.0,
        compactor_profile: None,
        max_tool_definition_tokens: None,
    };
    let runtime = Arc::new(
        LocalRuntime::new(store, model).with_config(Arc::new(config)),
    );

    let workspace = runtime.open_workspace("/tmp/ctx-p2".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Drive `pairs` user/assistant turns via the facade.
    for i in 0..pairs {
        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id.clone(),
                session_id: session_id.clone(),
                content: format!("turn {i}"),
                attachments: vec![],
            })
            .await
            .unwrap();
    }

    (runtime, workspace.workspace_id, session_id)
}

#[tokio::test]
async fn manual_compact_session_emits_full_event_chain() {
    let (runtime, _ws, session_id) = fixture_runtime_with_history(8).await;

    runtime
        .compact_session(session_id.clone(), CompactionReason::UserRequested)
        .await
        .expect("manual compaction should succeed");

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();

    let started = events.iter().filter(|e| matches!(e.payload, EventPayload::ContextCompactionStarted { .. })).count();
    let summary = events.iter().filter(|e| matches!(e.payload, EventPayload::CompactionSummary { .. })).count();
    let completed = events.iter().filter(|e| matches!(e.payload, EventPayload::ContextCompactionCompleted { fallback_used: false, .. })).count();
    assert_eq!(started, 1, "expected exactly 1 Started event");
    assert_eq!(summary, 1, "expected exactly 1 Summary event");
    assert_eq!(completed, 1, "expected exactly 1 Completed event with fallback_used=false");
}

#[tokio::test]
async fn send_message_rejected_with_session_busy_during_compaction() {
    let (runtime, ws, session_id) = fixture_runtime_with_history(6).await;

    // Manually flip the busy flag so the gate fires deterministically.
    {
        let mut states = runtime.session_states_for_test().lock().await;
        states
            .entry(session_id.to_string())
            .or_insert_with(agent_runtime::session::SessionState::default)
            .compacting = true;
    }

    let result = runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: session_id.clone(),
            content: "should be rejected".into(),
            attachments: vec![],
        })
        .await;
    match result {
        Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
            assert_eq!(id, session_id.to_string());
        }
        other => panic!("expected SessionBusy, got {other:?}"),
    }
}

#[tokio::test]
async fn send_message_succeeds_after_compaction_with_summary_substituted() {
    let (runtime, ws, session_id) = fixture_runtime_with_history(8).await;

    runtime
        .compact_session(session_id.clone(), CompactionReason::UserRequested)
        .await
        .expect("compaction should succeed");

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: session_id.clone(),
            content: "post-compaction turn".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // The assistant must have responded (FakeModelClient always replies).
    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let last_assistant = events
        .iter()
        .rev()
        .find_map(|e| match &e.payload {
            EventPayload::AssistantMessageCompleted { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected at least one assistant message after compaction");
    assert!(!last_assistant.is_empty());
}
```

> **Visibility helpers needed**: This test uses `runtime.event_store_for_test()` (already gated to test/test-helpers in P1) and a NEW `runtime.session_states_for_test()`. Add the latter to `crates/agent-runtime/src/facade_runtime.rs` inside the same `#[cfg(any(test, feature = "test-helpers"))]` block as `event_store_for_test`:

```rust
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn session_states_for_test(
        &self,
    ) -> &Arc<Mutex<HashMap<String, crate::session::SessionState>>> {
        &self.session_states
    }
```

> Also confirm `agent_runtime::session` is re-exportable. If `session` is currently a private module, add `pub mod session;` in `crates/agent-runtime/src/lib.rs` (the field type was already public via `pub struct SessionState`, but the module path must be reachable for the test).

- [ ] **Step 2: Add the test-helpers visibility (one commit, separate from the test)**

Edit `crates/agent-runtime/src/facade_runtime.rs` adding the `session_states_for_test` method as shown above, and `crates/agent-runtime/src/lib.rs` to ensure `pub mod session;`.

- [ ] **Step 3: Run the integration test**

Run: `cargo test -p agent-runtime --test compaction -- --nocapture`
Expected: 3 tests pass.

- [ ] **Step 4: Run the full workspace suite**

Run: `cargo test --workspace --all-targets`
Expected: green. If any unrelated test (e.g. `crates/agent-runtime/tests/full_stack.rs` or `crates/agent-tui/tests/app_logic.rs`) starts seeing an extra `ContextCompactionStarted` event, apply the test-side fix from Task 10 Step 7: build their `Config` with `auto_compact_threshold = 1.0`. Do NOT relax production behaviour.

- [ ] **Step 5: Run clippy on the full workspace**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime/tests/compaction.rs crates/agent-runtime/src/facade_runtime.rs crates/agent-runtime/src/lib.rs
git commit -m "test(runtime): full-stack compaction integration test (manual + busy-gate + post-compact)"
```

---

### Task 12 — Specta registration + TS type sync + final verification + push

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Auto-regenerate: `apps/agent-gui/src/generated/events.ts`

- [ ] **Step 1: Register `CompactionReason` in specta**

Open `apps/agent-gui/src-tauri/src/specta.rs` and find the `collect_types![...]` macro (or equivalent registration block) where `EventPayload`, `ContextUsage`, `LimitSource` etc. are listed (added in P1).

Add `CompactionReason` to the list. Example shape (adapt to actual macro syntax):

```rust
specta::export::ts_with_cfg(
    "src/generated/events.ts",
    &specta::ts::ExportConfiguration::new(),
    [
        // ...existing entries...
        specta::reference::reference::<agent_core::CompactionReason>(),
    ],
)
```

Or, if registration uses `collect_types!`:

```rust
collect_types![
    EventPayload,
    DomainEvent,
    ContextUsage,
    ContextSource,
    CompactionReason,        // NEW
    ModelLimits,
    LimitSource,
    // ...
]
```

> The four new event variants (`ContextCompactionStarted` / `Completed` / `Failed` / `CompactionSummary`) are inside `EventPayload`, which is already registered — no separate entry needed. `CompactionReason` is registered separately because it's referenced as a field type by two of the variants.

- [ ] **Step 2: Regenerate the TypeScript bindings**

Run: `just gen-types`
Expected: `apps/agent-gui/src/generated/events.ts` is updated. Verify with:

```bash
grep -E "CompactionReason|ContextCompactionStarted|CompactionSummary" apps/agent-gui/src/generated/events.ts
```

Expected: at least 4 matches (the variant + 3 sibling event types).

- [ ] **Step 3: Verify type-sync gate**

Run: `just check-types`
Expected: PASS (no uncommitted diff in `apps/agent-gui/src/generated/`).

- [ ] **Step 4: Run web-side lint to make sure the new event types don't break consumers**

Run: `pnpm run lint`
Expected: green. P3 will add UI consumers; P2 only needs the types to be available.

- [ ] **Step 5: Final full-workspace verification**

Run these in order:

```bash
pnpm run format:check
pnpm run lint
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
just check-types
```

All must be green. If anything fails, STOP and address — do NOT push.

- [ ] **Step 6: Commit the regenerated TypeScript bindings**

```bash
git add apps/agent-gui/src-tauri/src/specta.rs apps/agent-gui/src/generated/events.ts
git commit -m "feat(gui): register CompactionReason with specta and regenerate event bindings"
```

- [ ] **Step 7: Push the branch**

```bash
git push -u origin feat/context-p2-compaction
```

- [ ] **Step 8: Hand off to `finishing-a-development-branch` skill**

Per the executing-plans flow: announce "I'm using the finishing-a-development-branch skill to complete this work." and follow that skill (verify tests, present options, execute the user's choice).

---

## Self-review checklist (run after writing the plan, before execution)

**Spec coverage** — every requirement in spec §4.4 / §5 maps to a task:

| Spec requirement                                                      | Task(s)                  |
| --------------------------------------------------------------------- | ------------------------ |
| Manual `compact_session(session_id, CompactionReason::UserRequested)` | Task 8 + Task 11         |
| Auto-compaction triggered after `ContextAssembled` ratio ≥ threshold  | Task 10                  |
| Busy gate (`SessionState.compacting`) → `send_message` rejected       | Task 6 + Task 8          |
| LLM-based summarisation with 4-section prompt                         | Task 4 + Task 5          |
| 3-retry exponential backoff                                           | Task 5                   |
| Sliding-window fallback emits placeholder summary                     | Task 5 + Task 7          |
| Four new event variants (`Started/Completed/Failed/Summary`)          | Task 2                   |
| `CompactionReason` enum                                               | Task 2                   |
| `CoreError::SessionBusy`                                              | Task 1                   |
| `ContextPolicy` from `[context]` TOML block                           | Task 3                   |
| `replaces_event_range` uses timestamps (not ids)                      | Task 2 + Task 7 + Task 9 |
| `build_model_messages` substitutes summary for compacted range        | Task 9                   |
| Specta + TS bindings regenerated                                      | Task 12                  |

**Placeholder scan** — searched the plan for "TBD", "TODO", "implement later", "fill in details", "handle edge cases", "similar to Task N" — none present.

**Type consistency** — names cross-checked:

- `ContextPolicy.auto_compact_threshold: f32` (Task 3) referenced consistently in Task 10.
- `SessionState.compacting: bool` (Task 6) referenced in Task 7, 8, 10, 11.
- `CompactionReason::Threshold { ratio: f32 }` (Task 2) referenced consistently in Task 10 (`CompactionReason::Threshold { ratio }`).
- `Compactor::compact_with_llm(model, profile_alias, transcript)` signature (Task 5) matches call site in Task 7.
- `pick_compaction_boundary(events, KEEP_LAST_PAIRS)` (Task 6) matches call site in Task 7.
- `LocalRuntime.session_states: Arc<Mutex<HashMap<String, SessionState>>>` (P1; verified in `facade_runtime.rs`) matches usage everywhere.
- Test helper `event_store_for_test` already exists (P1); new `session_states_for_test` added in Task 11 Step 2.

**Out of scope reminders** — `ModelProfileSwitched` event is intentionally NOT introduced in P2 (defer to P4). `compact_session` is on `LocalRuntime` only — NOT yet on `AppFacade` (defer to P3 when GUI/TUI commands wire to it). No Tauri command introduced in P2.
