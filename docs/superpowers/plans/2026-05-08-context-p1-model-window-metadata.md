# Context P1 — Model Window Metadata & Budgeted Assembly Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every session real per-model `context_window` / `output_limit` (with three-layer fallback: TOML → built-in registry → Ollama runtime probe) and replace the agent loop's "replay every event" path with a budget-driven `ContextAssembler` that emits a richer `ContextAssembled { usage }` event.

**Architecture:** Add a const `model_registry` table inside `agent-models`; promote `ProfileDef.context_window`/`output_limit` to `Option<u64>` and add a `resolve_limits()` helper in `agent-config`; introduce new domain types `ModelLimits` / `LimitSource` / `ContextUsage` / `ContextSource::ToolDefinitions` in `agent-core` + `agent-memory`; rewrite `ContextAssembler::assemble` to take a `ContextBudget` and return a `ContextBundle { usage: ContextUsage }`; rewrite `agent_loop` to call the assembler each iteration and emit the new `ContextAssembled` event. UI consumers stay untouched in P1 (P3 wires them).

**Tech Stack:** Rust 1.x · tokio · async_trait · serde · thiserror · tiktoken-rs · reqwest · sqlx · wiremock (test) · `cargo test --workspace` · `cargo clippy --workspace --all-targets --all-features -- -D warnings` · `just gen-types`

**Branch:** `feat/context-p1-model-window-metadata` (start from `main`, use `just worktree feat/context-p1-model-window-metadata`)

**Spec reference:** `docs/superpowers/specs/2026-05-08-session-context-and-model-management-design.md` §4.1 / §4.2 / §4.3

---

## File Structure

This plan creates / modifies the following files. Each task at the bottom edits one or more of these.

### `agent-core` (domain types)

- **Modify** `crates/agent-core/src/events.rs`
  - Change `EventPayload::ContextAssembled { token_estimate: usize, sources: Vec<String> }` → `EventPayload::ContextAssembled { usage: ContextUsage }`
  - Update `EventPayload::event_type()` arm (no rename needed)
- **Modify** `crates/agent-core/src/lib.rs`
  - Re-export the new types from `agent-memory` (`ContextUsage`, `ContextSource`, `ModelLimits`, `LimitSource`) under `agent-core` so `events.rs` can reference them without taking a dependency on `agent-memory` (the types live in a leaf crate `agent-models` for `ModelLimits` and a new module `agent-core::context_types` for `ContextUsage` / `ContextSource`).
- **Create** `crates/agent-core/src/context_types.rs` — `ContextSource` enum, `ContextUsage` struct (lives here to avoid `agent-core → agent-memory` cycle).

### `agent-models` (registry + Ollama probe)

- **Create** `crates/agent-models/src/model_registry.rs` — `LimitSource` enum, `ModelLimits` struct, `ModelInfo`, `lookup(provider, model_id)`.
- **Modify** `crates/agent-models/src/lib.rs` — `pub mod model_registry; pub use model_registry::{ModelLimits, LimitSource, lookup};`
- **Modify** `crates/agent-models/src/ollama.rs` — add `OllamaClient::probe_context_window(&self, model_id: &str) -> Option<u64>` calling `POST /api/show`.

### `agent-config` (Optional fields + resolution)

- **Modify** `crates/agent-config/src/lib.rs`
  - `ProfileDef.context_window: u64` → `pub context_window: Option<u64>`
  - `ProfileDef.output_limit: u64` → `pub output_limit: Option<u64>`
  - Drop `default_context_window()` / `default_output_limit()` helpers
  - Update `Config::defaults()` to use `Some(...)` literals (preserve current behaviour for the bundled `fake` and `local-code` profiles)
- **Create** `crates/agent-config/src/limits.rs` — `pub fn resolve_limits(profile: &ProfileDef) -> ModelLimits` (UserConfig > BuiltinRegistry > Fallback). RuntimeProbe is applied later by `agent-runtime` (it owns the live OllamaClient).
- **Modify** `crates/agent-config/src/lib.rs` — `pub mod limits; pub use limits::resolve_limits;`
- **Modify** `crates/agent-config/src/loader.rs` (and any other call sites) — fix every `ProfileDef { context_window: 128_000, ... }` literal to use the new `Option` shape.
- **Modify** `kairox.toml.example` — show that omitted `context_window` resolves through registry; document the upcoming `[context]` block (commented out, real settings land in P2).

### `agent-memory` (ContextSource + budgeted assembler)

- **Modify** `crates/agent-memory/src/context.rs`
  - Replace local `ContextSource` with `agent_core::ContextSource` (re-export for back-compat)
  - Add `ContextBudget { context_window: u64, output_reservation: u64, source_caps: Vec<(ContextSource, u64)> }`
  - Add `ContextRequest::tool_definitions: Vec<agent_models::ToolDefinition>` (Default derive ok)
  - Change `ContextAssembler::assemble(&self, request: ContextRequest, budget: ContextBudget) -> ContextBundle`
  - Add `ContextBundle::usage: ContextUsage` (keep existing fields)
  - Implement per-source-cap drop pass _before_ the global drop pass
  - Tool-definitions JSON serialised once; counted as one bundled section under `ContextSource::ToolDefinitions`
- **Modify** `crates/agent-memory/src/lib.rs` — re-export new types

### `agent-runtime` (wire it all together)

- **Create** `crates/agent-runtime/src/context_budget.rs`
  - `pub fn build_budget(limits: &ModelLimits) -> ContextBudget` (output reservation = `output_limit + max(2_000, output_limit / 10)`)
  - `pub struct UsageCorrector { ratio: f32 }` with `apply(estimate)` and `update(real_input_tokens, last_estimate)` (EMA, clamp `[0.7, 1.5]`)
- **Modify** `crates/agent-runtime/src/agent_loop.rs`
  - Replace `let messages = build_model_messages(&request.content, &session_events);` with a call to `ContextAssembler::assemble(...)` that emits `EventPayload::ContextAssembled { usage }`.
  - Keep `build_model_messages` as a private helper used internally to convert assembled bundle → `Vec<ModelMessage>` (the old direct-from-events behaviour is gone; we read events to build a `ContextRequest::session_history` instead).
  - On `ModelEvent::Completed { usage: Some(real) }` push `corrector.update(real.input_tokens, last_estimate)` into the per-session state.
- **Modify** `crates/agent-runtime/src/session.rs`
  - Add a brand-new `pub struct SessionState { pub model_limits: Option<ModelLimits>, pub usage_corrector: UsageCorrector, pub last_estimated_tokens: u64 }` to this file (the file currently contains only standalone `pub fn open_workspace / start_session / get_session_projection / ...` helpers — no per-session in-memory state struct exists today, verified by `grep "pub struct" crates/agent-runtime/src/session.rs`). The struct is consumed by Tasks 8/9/10.
- **Modify** `crates/agent-runtime/src/facade_runtime.rs`
  - On `start_session` (the existing `AppFacade::start_session` impl, around line 377), call `agent_config::resolve_limits` and write the resulting `ModelLimits` into the per-session state via the new `set_session_limits` helper. If `provider == "ollama"`, spawn `tokio::spawn(probe_context_window(...))` with a 3 s `tokio::time::timeout`; on success, overwrite `model_limits.context_window` and set `source = LimitSource::RuntimeProbe`.
  - Add new fields to `LocalRuntime`: `config: Arc<agent_config::Config>`, `session_states: Arc<Mutex<HashMap<String, SessionState>>>`, `ollama_clients: HashMap<String, Arc<OllamaClient>>`. Add the matching `with_config(...)` and `with_ollama_clients(...)` builder methods. Wire them up in TUI (`crates/agent-tui/src/main.rs`) and GUI (`apps/agent-gui/src-tauri/src/lib.rs`).

### Generated types

- **Modify (auto)** `apps/agent-gui/src/generated/events.ts` via `just gen-types` — must show updated `ContextAssembled` payload + new `ContextUsage` / `ContextSource` / `ModelLimits` / `LimitSource` types. P1 only verifies generation works; UI consumption is P3.
- **Modify** `apps/agent-gui/src-tauri/src/specta.rs` — register `ContextUsage`, `ContextSource`, `ModelLimits`, `LimitSource`.

### Tests (new)

- **Create** `crates/agent-models/src/model_registry.rs` `#[cfg(test)] mod tests` — table coverage.
- **Create** `crates/agent-config/src/limits.rs` `#[cfg(test)] mod tests` — three-layer precedence.
- **Create** `crates/agent-memory/tests/context_budget.rs` — assembler honours per-source caps + tool-definitions section.
- **Create** `crates/agent-runtime/tests/context_budget.rs` — agent loop emits `ContextAssembled { usage }` with `total_tokens <= budget_tokens` after 50 fake exchanges.
- **Modify** `crates/agent-models/src/ollama.rs` — wiremock test for `probe_context_window`.

---

## Task list (TDD, bite-sized)

The plan splits into 11 sequential tasks. Each task ends with running tests + a commit. Tasks 1–4 add new types and the registry (no behaviour change), Tasks 5–7 refactor `ContextAssembler`, Tasks 8–10 wire the runtime, Task 11 regenerates TS bindings + final verification.

> **Reading order** matters: start at Task 1 and proceed in order. Type signatures defined in earlier tasks are referenced by name in later ones.

### Task 1 — Add `ContextSource` + `ContextUsage` to `agent-core`

**Files:**

- Create: `crates/agent-core/src/context_types.rs`
- Modify: `crates/agent-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/agent-core/src/context_types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    System,
    ToolDefinitions,
    Request,
    Memory,
    History,
    ToolResult,
    SelectedFile,
    CompactionSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ContextUsage {
    pub total_tokens: u64,
    pub budget_tokens: u64,
    pub context_window: u64,
    pub output_reservation: u64,
    pub by_source: Vec<(ContextSource, u64)>,
    pub estimator: String,
    pub corrected_by_real_usage: bool,
}

impl ContextUsage {
    pub fn ratio(&self) -> f32 {
        if self.budget_tokens == 0 {
            0.0
        } else {
            self.total_tokens as f32 / self.budget_tokens as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_returns_fraction_of_budget_consumed() {
        let usage = ContextUsage {
            total_tokens: 60_000,
            budget_tokens: 200_000,
            context_window: 200_000,
            output_reservation: 0,
            by_source: vec![(ContextSource::System, 60_000)],
            estimator: "cl100k_base".into(),
            corrected_by_real_usage: false,
        };
        assert!((usage.ratio() - 0.30).abs() < 1e-4);
    }

    #[test]
    fn context_source_serializes_snake_case_with_new_variants() {
        assert_eq!(serde_json::to_value(ContextSource::ToolDefinitions).unwrap(), "tool_definitions");
        assert_eq!(serde_json::to_value(ContextSource::CompactionSummary).unwrap(), "compaction_summary");
    }

    #[test]
    fn context_usage_round_trips_through_json() {
        let usage = ContextUsage {
            total_tokens: 1_234,
            budget_tokens: 200_000,
            context_window: 200_000,
            output_reservation: 9_000,
            by_source: vec![
                (ContextSource::System, 800),
                (ContextSource::ToolDefinitions, 434),
            ],
            estimator: "cl100k_base".into(),
            corrected_by_real_usage: true,
        };
        let json = serde_json::to_value(&usage).unwrap();
        let back: ContextUsage = serde_json::from_value(json).unwrap();
        assert_eq!(back, usage);
    }
}
```

In `crates/agent-core/src/lib.rs`, add right after the existing `pub mod` declarations:

```rust
pub mod context_types;
pub use context_types::{ContextSource, ContextUsage};
```

- [ ] **Step 2: Strict-TDD red phase — temporarily stub `ratio()` to a wrong value**

Before running tests, edit `crates/agent-core/src/context_types.rs` and replace the body of `pub fn ratio(&self) -> f32` with `0.0`. Save.

Run: `cargo test -p agent-core ratio_returns_fraction_of_budget_consumed`
Expected: FAIL — `assertion failed: (usage.ratio() - 0.30).abs() < 1e-4` because `ratio()` now returns `0.0`.

- [ ] **Step 3: Restore the real implementation and verify it passes**

Restore the original `ratio()` body (`if self.budget_tokens == 0 { 0.0 } else { self.total_tokens as f32 / self.budget_tokens as f32 }`).

Run: `cargo test -p agent-core context_types -- --nocapture`
Expected: 3 new tests pass.

Run: `cargo test -p agent-core`
Expected: existing `agent-core` tests stay green (the new `context_types` types are additive — they don't yet appear inside `EventPayload`, so the existing `event_roundtrip` test for `ContextAssembled` is untouched).

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core/src/context_types.rs crates/agent-core/src/lib.rs
git commit -m "feat(core): add ContextSource and ContextUsage domain types"
```

---

### Task 2 — Add `model_registry` (LimitSource, ModelLimits, lookup) to `agent-models`

**Files:**

- Create: `crates/agent-models/src/model_registry.rs`
- Modify: `crates/agent-models/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/agent-models/src/model_registry.rs`:

```rust
//! Built-in model context window registry. Provides best-known
//! `context_window` and `output_limit` values for popular OpenAI and
//! Anthropic model ids. Used as the second tier of the three-layer
//! fallback (UserConfig > BuiltinRegistry > RuntimeProbe > Fallback).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LimitSource {
    UserConfig,
    BuiltinRegistry,
    RuntimeProbe,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ModelLimits {
    pub context_window: u64,
    pub output_limit: u64,
    pub source: LimitSource,
}

struct ModelInfo {
    pattern: &'static str,
    context_window: u64,
    output_limit: u64,
}

const OPENAI: &[ModelInfo] = &[
    ModelInfo { pattern: "gpt-4.1",        context_window: 1_048_576, output_limit: 32_768 },
    ModelInfo { pattern: "gpt-4o-mini",    context_window:   128_000, output_limit: 16_384 },
    ModelInfo { pattern: "gpt-4o",         context_window:   128_000, output_limit: 16_384 },
    ModelInfo { pattern: "gpt-4-turbo",    context_window:   128_000, output_limit:  4_096 },
    ModelInfo { pattern: "gpt-3.5-turbo",  context_window:    16_385, output_limit:  4_096 },
    ModelInfo { pattern: "o1-mini",        context_window:   128_000, output_limit: 65_536 },
    ModelInfo { pattern: "o1",             context_window:   200_000, output_limit:100_000 },
];

const ANTHROPIC: &[ModelInfo] = &[
    ModelInfo { pattern: "claude-opus-4",     context_window: 200_000, output_limit:  8_192 },
    ModelInfo { pattern: "claude-sonnet-4",   context_window: 200_000, output_limit:  8_192 },
    ModelInfo { pattern: "claude-3-7-sonnet", context_window: 200_000, output_limit: 64_000 },
    ModelInfo { pattern: "claude-3-5-sonnet", context_window: 200_000, output_limit:  8_192 },
    ModelInfo { pattern: "claude-3-5-haiku",  context_window: 200_000, output_limit:  8_192 },
    ModelInfo { pattern: "claude-3-opus",     context_window: 200_000, output_limit:  4_096 },
    ModelInfo { pattern: "claude-3-haiku",    context_window: 200_000, output_limit:  4_096 },
];

const FALLBACK_OLLAMA: ModelLimits = ModelLimits {
    context_window: 8_192, output_limit: 2_048, source: LimitSource::Fallback,
};
const FALLBACK_FAKE: ModelLimits = ModelLimits {
    context_window: 4_096, output_limit: 2_048, source: LimitSource::Fallback,
};
const FALLBACK_GENERIC: ModelLimits = ModelLimits {
    context_window: 128_000, output_limit: 16_384, source: LimitSource::Fallback,
};

fn match_table(table: &'static [ModelInfo], model_id: &str) -> Option<ModelLimits> {
    let mut entries: Vec<&ModelInfo> = table.iter().collect();
    entries.sort_by_key(|info| std::cmp::Reverse(info.pattern.len()));
    for info in entries {
        if model_id.starts_with(info.pattern) {
            return Some(ModelLimits {
                context_window: info.context_window,
                output_limit: info.output_limit,
                source: LimitSource::BuiltinRegistry,
            });
        }
    }
    None
}

/// Look up the built-in limits for a (provider, model_id) pair.
pub fn lookup(provider: &str, model_id: &str) -> ModelLimits {
    match provider {
        "openai" | "openai_compatible" => match_table(OPENAI, model_id).unwrap_or(FALLBACK_GENERIC),
        "anthropic" => match_table(ANTHROPIC, model_id).unwrap_or(FALLBACK_GENERIC),
        "ollama" => FALLBACK_OLLAMA,
        "fake" => FALLBACK_FAKE,
        _ => FALLBACK_GENERIC,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_gpt_4o_with_builtin_source() {
        let limits = lookup("openai_compatible", "gpt-4o");
        assert_eq!(limits.context_window, 128_000);
        assert_eq!(limits.output_limit, 16_384);
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    }

    #[test]
    fn longest_prefix_wins_for_overlapping_anthropic_patterns() {
        let limits = lookup("anthropic", "claude-3-5-sonnet-20240620");
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.output_limit, 8_192);
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    }

    #[test]
    fn returns_provider_fallback_for_unknown_openai_model() {
        let limits = lookup("openai_compatible", "gpt-future-9000");
        assert_eq!(limits.source, LimitSource::Fallback);
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn ollama_provider_always_returns_conservative_fallback() {
        let limits = lookup("ollama", "llama3:70b");
        assert_eq!(limits.context_window, 8_192);
        assert_eq!(limits.source, LimitSource::Fallback);
    }

    #[test]
    fn fake_provider_returns_small_window() {
        let limits = lookup("fake", "fake");
        assert_eq!(limits.context_window, 4_096);
        assert_eq!(limits.source, LimitSource::Fallback);
    }

    #[test]
    fn unknown_provider_returns_generic_fallback() {
        let limits = lookup("custom", "anything");
        assert_eq!(limits.source, LimitSource::Fallback);
        assert_eq!(limits.context_window, 128_000);
    }
}
```

In `crates/agent-models/src/lib.rs`, add at top (after `pub mod types;`):

```rust
pub mod model_registry;
pub use model_registry::{lookup as lookup_limits, LimitSource, ModelLimits};
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-models model_registry -- --nocapture`
Expected: 6 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-models/src/model_registry.rs crates/agent-models/src/lib.rs
git commit -m "feat(models): add built-in model_registry with ModelLimits and LimitSource"
```

---

### Task 3 — Add Ollama runtime probe (`probe_context_window`)

**Files:**

- Modify: `crates/agent-models/src/ollama.rs`

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests { ... }` in `crates/agent-models/src/ollama.rs`:

```rust
#[tokio::test]
async fn probe_context_window_reads_context_length_from_show_endpoint() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let body = serde_json::json!({
        "model_info": {
            "general.architecture": "llama",
            "llama.context_length": 8192_u64
        }
    });

    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&mock_server)
        .await;

    let client = OllamaClient::new(OllamaConfig {
        base_url: mock_server.uri(),
        default_model: "llama3:8b".into(),
        context_window: 0,
    });

    assert_eq!(client.probe_context_window("llama3:8b").await, Some(8192));
}

#[tokio::test]
async fn probe_context_window_returns_none_on_http_error() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let client = OllamaClient::new(OllamaConfig {
        base_url: mock_server.uri(),
        default_model: "missing".into(),
        context_window: 0,
    });

    assert!(client.probe_context_window("missing").await.is_none());
}

#[tokio::test]
async fn probe_context_window_handles_unknown_architecture() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let body = serde_json::json!({
        "model_info": {
            "general.architecture": "qwen",
            "qwen.context_length": 32768_u64
        }
    });
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&mock_server)
        .await;

    let client = OllamaClient::new(OllamaConfig {
        base_url: mock_server.uri(),
        default_model: "qwen2:7b".into(),
        context_window: 0,
    });

    assert_eq!(client.probe_context_window("qwen2:7b").await, Some(32768));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-models ollama::tests::probe_context_window -- --nocapture`
Expected: FAIL — method `probe_context_window` not found on `OllamaClient`.

- [ ] **Step 3: Implement `probe_context_window`**

In the `impl OllamaClient { ... }` block in `crates/agent-models/src/ollama.rs`, append:

```rust
/// Best-effort discovery of a model's native context window.
///
/// POSTs to `/api/show` and reads `model_info.<arch>.context_length`.
/// Returns `None` on any transport, parse, or "missing field" error
/// so callers can fall back to the built-in registry / static default.
///
/// 3-second hard timeout — never blocks a session for long.
pub async fn probe_context_window(&self, model_id: &str) -> Option<u64> {
    let url = format!("{}/api/show", self.config.base_url.trim_end_matches('/'));
    let body = serde_json::json!({ "name": model_id });

    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        self.http.post(&url).json(&body).send(),
    )
    .await
    .ok()?
    .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let value: serde_json::Value = resp.json().await.ok()?;
    let model_info = value.get("model_info")?.as_object()?;

    for (key, val) in model_info {
        if key.ends_with(".context_length") {
            if let Some(n) = val.as_u64() {
                return Some(n);
            }
        }
    }
    None
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p agent-models ollama -- --nocapture`
Expected: all `ollama::tests::*` tests pass (3 new + existing).

- [ ] **Step 5: Commit**

```bash
git add crates/agent-models/src/ollama.rs
git commit -m "feat(models): add OllamaClient::probe_context_window via /api/show"
```

### Task 4 — Convert `ProfileDef` fields to `Option<u64>` + add `resolve_limits` in `agent-config`

**Files:**

- Modify: `crates/agent-config/src/lib.rs` (`ProfileDef` field types + `Config::defaults()`)
- Create: `crates/agent-config/src/limits.rs`
- Modify: `crates/agent-config/src/loader.rs` (and any other call site that constructs `ProfileDef` literals)
- Modify: `kairox.toml.example`

- [ ] **Step 1: Write the failing test for `resolve_limits`**

Create `crates/agent-config/src/limits.rs`:

```rust
//! Three-layer fallback for `ModelLimits`:
//! 1. UserConfig — explicit `context_window` / `output_limit` in the TOML profile
//! 2. BuiltinRegistry — match on (provider, model_id) via `agent_models::lookup_limits`
//! 3. Fallback — provider-specific conservative default returned by the registry
//!
//! The optional fourth tier — RuntimeProbe — is applied by `agent-runtime` after a
//! session initialises (because only the runtime owns a live `OllamaClient`).

use crate::ProfileDef;
use agent_models::{lookup_limits, LimitSource, ModelLimits};

pub fn resolve_limits(profile: &ProfileDef) -> ModelLimits {
    if let (Some(ctx), Some(out)) = (profile.context_window, profile.output_limit) {
        return ModelLimits {
            context_window: ctx,
            output_limit: out,
            source: LimitSource::UserConfig,
        };
    }
    let from_table = lookup_limits(&profile.provider, &profile.model_id);
    if let Some(ctx) = profile.context_window {
        return ModelLimits {
            context_window: ctx,
            output_limit: from_table.output_limit,
            source: LimitSource::UserConfig,
        };
    }
    if let Some(out) = profile.output_limit {
        return ModelLimits {
            context_window: from_table.context_window,
            output_limit: out,
            source: LimitSource::UserConfig,
        };
    }
    from_table
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(provider: &str, model_id: &str, ctx: Option<u64>, out: Option<u64>) -> ProfileDef {
        ProfileDef {
            provider: provider.into(),
            model_id: model_id.into(),
            base_url: None,
            api_key: None,
            api_key_env: None,
            context_window: ctx,
            output_limit: out,
            response: None,
        }
    }

    #[test]
    fn user_config_wins_when_both_fields_set() {
        let limits = resolve_limits(&profile("anthropic", "claude-sonnet-4", Some(50_000), Some(4_000)));
        assert_eq!(limits.context_window, 50_000);
        assert_eq!(limits.output_limit, 4_000);
        assert_eq!(limits.source, LimitSource::UserConfig);
    }

    #[test]
    fn builtin_registry_used_when_user_omits_both_fields() {
        let limits = resolve_limits(&profile("anthropic", "claude-sonnet-4", None, None));
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.output_limit, 8_192);
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    }

    #[test]
    fn partial_override_keeps_other_field_from_registry() {
        let limits = resolve_limits(&profile("openai_compatible", "gpt-4o", Some(64_000), None));
        assert_eq!(limits.context_window, 64_000);
        assert_eq!(limits.output_limit, 16_384); // from registry
        assert_eq!(limits.source, LimitSource::UserConfig);
    }

    #[test]
    fn unknown_model_falls_back_to_provider_default() {
        let limits = resolve_limits(&profile("ollama", "weird-model", None, None));
        assert_eq!(limits.context_window, 8_192);
        assert_eq!(limits.source, LimitSource::Fallback);
    }
}
```

In `crates/agent-config/src/lib.rs`:

1. Add at the top with the other `pub mod`:
   ```rust
   pub mod limits;
   pub use limits::resolve_limits;
   ```
2. Replace `ProfileDef` field declarations:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ProfileDef {
       pub provider: String,
       pub model_id: String,
       #[serde(default)]
       pub base_url: Option<String>,
       #[serde(default)]
       pub api_key: Option<String>,
       #[serde(default)]
       pub api_key_env: Option<String>,
       #[serde(default)]
       pub context_window: Option<u64>,   // was: u64 with default 128_000
       #[serde(default)]
       pub output_limit: Option<u64>,     // was: u64 with default 16_384
       #[serde(default)]
       pub response: Option<String>,
   }
   ```
3. Delete the helpers `default_context_window()` and `default_output_limit()` (now unused).
4. In `Config::defaults()`, change every `context_window: 128_000` / `4_096` / `output_limit: 16_384` / `2_048` to `Some(...)` (preserves existing behaviour exactly, source = UserConfig). Example for the `fake` profile:
   ```rust
   context_window: Some(4_096),
   output_limit: Some(2_048),
   ```

- [ ] **Step 2: Run cargo build to discover broken call sites**

Run: `cargo build -p agent-config`
Expected: compile errors in `loader.rs` (and possibly other files) where `ProfileDef { context_window: 128_000, ... }` literals exist.

- [ ] **Step 3: Fix every broken `ProfileDef` literal (verified list — no need to grep)**

Plan-writing pass grepped every `context_window:` literal that constructs a `ProfileDef`. Update exactly these sites:

**A. `crates/agent-config/src/lib.rs`** — `Config::defaults()` builds three `ProfileDef`s. Wrap the numeric literals at lines **196, 209, 227** (the matching `output_limit` lines are right below each):

```rust
// before
context_window: 4096,
output_limit: 2048,
// after
context_window: Some(4096),
output_limit: Some(2048),
```

(Repeat for all three profile literals: `fake` (4096/2048), `local-code` (128_000/16_384), and the optional `fast` profile (128_000/16_384).)

**B. `crates/agent-config/src/loader.rs:55`** — currently:

```rust
context_window: profile_toml.context_window.unwrap_or(128_000),
```

Replace with:

```rust
context_window: profile_toml.context_window,         // already Option<u64>
```

And likewise for `output_limit` immediately below. **Important**: this removes the `unwrap_or` defaulting so `resolve_limits` (Task 4 step 1) can see `None` and route through the registry. Also confirm `loader.rs:26` still declares `context_window: Option<u64>` on the TOML struct — yes, it already does.

**C. `crates/agent-config/src/loader.rs:223`** — appears in a TOML test fixture string `context_window = 128_000`. No change needed; the parser still accepts this and `resolve_limits` will treat it as `Some(128_000)` → `LimitSource::UserConfig`.

**D. NOT TO TOUCH:**

- `crates/agent-config/src/builder.rs:33,43,53,63,73,144` — these construct `ModelCapabilities { context_window: def.context_window, .. }`, where the target field on `ModelCapabilities` is `u64`, not `Option<u64>`. Wrap each `def.context_window` with `.unwrap_or_else(|| agent_config::resolve_limits(def).context_window)` (and same pattern for `def.output_limit`). Six sites, identical pattern.
- `crates/agent-config/src/builder.rs:229,244` — these construct `ModelProfile { capabilities: ModelCapabilities { context_window: 200_000, .. } }` with `u64` literals — leave them as-is.
- `crates/agent-models/src/anthropic.rs:50`, `crates/agent-models/src/router.rs:85`, `crates/agent-models/src/ollama.rs:12,20,33,211`, `crates/agent-models/src/profile.rs:10,39`, `crates/agent-models/src/openai_compatible.rs:29` — all reference `ModelCapabilities.context_window: u64` or `OllamaConfig.context_window: u64`, NOT `ProfileDef`. They stay unchanged because we are only narrowing the `ProfileDef` field type.

After applying B and the builder.rs change in D, run `cargo build -p agent-config` — expected: clean.

- [ ] **Step 4: Run all `agent-config` tests**

Run: `cargo test -p agent-config`
Expected: all existing tests pass + 4 new `resolve_limits` tests pass.

- [ ] **Step 5: Update `kairox.toml.example`**

In `kairox.toml.example`, find the comment block above the profiles section (or add one) and replace the per-profile context_window comments with:

```toml
# `context_window` and `output_limit` are now optional. When omitted, Kairox
# resolves them through:
#   1. the built-in model registry (matches "gpt-4o", "claude-sonnet-4", ...)
#   2. an Ollama runtime probe (POST /api/show) — only for ollama profiles
#   3. a conservative provider-specific fallback
# Override either field to pin a value (e.g. for self-hosted deployments).
```

For an explicit example profile, keep one entry with `context_window = 8192` to demonstrate the override (e.g. the `local-llama` block from the spec).

- [ ] **Step 6: Verify the whole workspace still builds**

Run: `cargo build --workspace`
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add crates/agent-config/src/limits.rs crates/agent-config/src/lib.rs crates/agent-config/src/loader.rs crates/agent-config/src/builder.rs kairox.toml.example
# include any other files with ProfileDef literals you fixed
git commit -m "feat(config): make context_window/output_limit Optional and add resolve_limits"
```

---

### Task 5 — Add `ContextBudget` + extend `ContextRequest` with `tool_definitions`

**Files:**

- Modify: `crates/agent-memory/src/context.rs`
- Modify: `crates/agent-memory/src/lib.rs`
- Modify: `crates/agent-memory/Cargo.toml` (add `agent-models` workspace dep)

- [ ] **Step 1: Add `agent-models` as a dep of `agent-memory`**

Verified by reading `crates/agent-memory/Cargo.toml`: `agent-core` is already a dep using `agent-core = { path = "../agent-core" }` (path form, NOT `workspace = true`). Match that exact style. In `crates/agent-memory/Cargo.toml`, under `[dependencies]`, add ONE line right after the existing `agent-core` line:

```toml
agent-models = { path = "../agent-models" }
```

> Do NOT add `agent-core` again — it's already there.
> Do NOT use `workspace = true` form — it's not used elsewhere in this file.

Cycle check (verified by reading `crates/agent-models/Cargo.toml`): `agent-models` depends on `agent-core` only; it does NOT depend on `agent-memory`. So `agent-memory → agent-models` is acyclic.

- [ ] **Step 2: Replace local `ContextSource` with re-export and add `ContextBudget`**

Edit `crates/agent-memory/src/context.rs`. Replace the existing `ContextSource` enum (top of file) with:

```rust
pub use agent_core::ContextSource;
```

Then add right below the existing `ContextRequest` struct:

```rust
#[derive(Debug, Clone)]
pub struct ContextBudget {
    /// Total context window of the active model (e.g. 200_000 for Sonnet 4).
    pub context_window: u64,
    /// Tokens reserved for the upcoming completion. Effective input budget
    /// is `context_window - output_reservation`.
    pub output_reservation: u64,
    /// Optional per-source soft caps (applied before the global drop pass).
    pub source_caps: Vec<(ContextSource, u64)>,
}

impl ContextBudget {
    pub fn input_budget(&self) -> u64 {
        self.context_window.saturating_sub(self.output_reservation)
    }
}
```

Extend `ContextRequest` with a new field:

```rust
#[derive(Debug, Clone, Default)]
pub struct ContextRequest {
    pub system_prompt: Option<String>,
    pub user_request: String,
    pub session_history: Vec<String>,
    pub selected_files: Vec<String>,
    pub tool_results: Vec<String>,
    pub memories: Vec<MemoryEntry>,
    pub active_task: Option<String>,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
    /// MCP + built-in tool schemas to be injected into the model request.
    /// They're serialised once and counted as a single ToolDefinitions section.
    pub tool_definitions: Vec<agent_models::ToolDefinition>,
}
```

In `crates/agent-memory/src/lib.rs`, add re-exports:

```rust
pub use context::{ContextAssembler, ContextBudget, ContextBundle, ContextRequest, ContextSource};
```

(and remove any duplicate single re-exports that conflict).

- [ ] **Step 2: Compile to verify the new types are visible**

Run: `cargo build -p agent-memory`
Expected: success. Adding `pub use agent_core::ContextSource;` makes the existing `ContextSource::System / ::Request / ::Memory / ::History / ::ToolResult / ::SelectedFile` references in `assemble()` resolve to the `agent-core` enum (which has the same variants). The two extra variants `ToolDefinitions` and `CompactionSummary` are unused in this task — the per-source-cap loop and the tool-definitions branch land in Task 6. If you see "no variant `XYZ` on type `ContextSource`" errors, you forgot one of the `agent_core::` re-exports — fix `crates/agent-memory/src/context.rs` and `crates/agent-memory/src/lib.rs` per Step 2 of this task.

> ⚠️ Do not delete the existing `assemble()` body in this task. Task 6 rewrites it in one atomic commit. Keeping `assemble()` intact here means the workspace still builds at every commit boundary — required because `pre-commit` runs `cargo fmt --all`.

- [ ] **Step 4: Add a test for `ContextBudget::input_budget`**

Append to `mod tests { ... }` in `crates/agent-memory/src/context.rs`:

```rust
#[test]
fn input_budget_subtracts_output_reservation() {
    let budget = ContextBudget {
        context_window: 200_000,
        output_reservation: 12_000,
        source_caps: vec![],
    };
    assert_eq!(budget.input_budget(), 188_000);
}

#[test]
fn input_budget_saturates_at_zero_when_reservation_exceeds_window() {
    let budget = ContextBudget {
        context_window: 8_000,
        output_reservation: 12_000,
        source_caps: vec![],
    };
    assert_eq!(budget.input_budget(), 0);
}
```

- [ ] **Step 5: Run the new tests**

Run: `cargo test -p agent-memory context::tests::input_budget -- --nocapture`
Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-memory/Cargo.toml crates/agent-memory/src/context.rs crates/agent-memory/src/lib.rs
git commit -m "refactor(memory): add ContextBudget and reuse agent-core ContextSource"
```

### Task 6 — Rewrite `ContextAssembler::assemble` to be budget-driven and return `ContextUsage`

**Files:**

- Modify: `crates/agent-memory/src/context.rs`
- Create: `crates/agent-memory/tests/context_budget.rs`

This is the main refactor of the assembler. The old behaviour (`max_tokens` field on the struct, `assemble(req) -> Bundle`) becomes (`assemble(req, budget) -> Bundle { usage }`).

- [ ] **Step 1: Write the failing integration test**

Create `crates/agent-memory/tests/context_budget.rs`:

```rust
use agent_core::ContextSource;
use agent_memory::{ContextAssembler, ContextBudget, ContextRequest};
use agent_models::ToolDefinition;

fn budget(window: u64, output: u64) -> ContextBudget {
    ContextBudget { context_window: window, output_reservation: output, source_caps: vec![] }
}

#[tokio::test]
async fn assemble_returns_usage_with_per_source_breakdown() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("You are Kairox.".into()),
                user_request: "summarise this repo".into(),
                session_history: vec!["earlier discussion".into()],
                tool_definitions: vec![ToolDefinition {
                    name: "fs.read".into(),
                    description: "Read a file".into(),
                    parameters: serde_json::json!({"type": "object"}),
                }],
                ..Default::default()
            },
            budget(8_000, 1_000),
        )
        .await;

    let usage = &bundle.usage;
    assert_eq!(usage.context_window, 8_000);
    assert_eq!(usage.output_reservation, 1_000);
    assert_eq!(usage.budget_tokens, 7_000);
    assert!(usage.total_tokens > 0);
    assert!(usage.by_source.iter().any(|(s, n)| matches!(s, ContextSource::System) && *n > 0));
    assert!(usage
        .by_source
        .iter()
        .any(|(s, n)| matches!(s, ContextSource::ToolDefinitions) && *n > 0));
    assert_eq!(usage.estimator, "cl100k_base");
    assert!(!usage.corrected_by_real_usage);
}

#[tokio::test]
async fn assemble_drops_lowest_priority_sections_when_over_budget() {
    let assembler = ContextAssembler::new_standalone();
    // Give a tiny budget so almost everything except System+Request must be dropped.
    let request = ContextRequest {
        system_prompt: Some("S".into()),
        user_request: "U".into(),
        session_history: (0..50).map(|i| format!("history line {}", i)).collect(),
        selected_files: (0..10).map(|i| format!("file-{}.rs contents...", i)).collect(),
        ..Default::default()
    };
    let bundle = assembler.assemble(request, budget(200, 50)).await;

    assert!(bundle.truncated, "bundle should be marked truncated");
    assert!(bundle.usage.total_tokens <= bundle.usage.budget_tokens);
    // System should never be dropped.
    assert!(bundle.usage.by_source.iter().any(|(s, _)| matches!(s, ContextSource::System)));
}

#[tokio::test]
async fn per_source_cap_drops_tool_definitions_first_when_caps_exceeded() {
    let assembler = ContextAssembler::new_standalone();
    let big_schema = serde_json::json!({
        "type": "object",
        "properties": (0..200).map(|i| (format!("p{}", i), serde_json::json!({"type": "string"}))).collect::<serde_json::Map<_, _>>(),
    });
    let request = ContextRequest {
        system_prompt: Some("S".into()),
        user_request: "U".into(),
        tool_definitions: vec![ToolDefinition {
            name: "huge".into(),
            description: "big tool".into(),
            parameters: big_schema,
        }],
        ..Default::default()
    };
    let mut bdg = budget(50_000, 1_000);
    bdg.source_caps.push((ContextSource::ToolDefinitions, 100));

    let bundle = assembler.assemble(request, bdg).await;
    let tool_tokens: u64 = bundle.usage.by_source.iter()
        .filter(|(s, _)| matches!(s, ContextSource::ToolDefinitions))
        .map(|(_, n)| *n)
        .sum();
    assert!(tool_tokens <= 100, "tool definitions section ({}) must respect cap", tool_tokens);
}
```

- [ ] **Step 2: Run the test to verify it fails to compile / fails at runtime**

Run: `cargo test -p agent-memory --test context_budget -- --nocapture`
Expected: FAIL — `ContextAssembler::new_standalone()` (no args) does not exist; `assemble` does not take a `budget` argument; `ContextBundle` has no `usage` field.

- [ ] **Step 3: Update the public API of `ContextAssembler`**

In `crates/agent-memory/src/context.rs`, replace the `ContextAssembler` definition and its constructors:

```rust
use agent_core::{ContextSource, ContextUsage};

#[derive(Debug, Clone)]
pub struct ContextBundle {
    pub messages: Vec<String>,
    pub sources: Vec<ContextSource>,
    pub truncated: bool,
    pub usage: ContextUsage,
}

pub struct ContextAssembler {
    memory_store: Option<Arc<dyn MemoryStore>>,
    tokenizer: CoreBPE,
}

impl ContextAssembler {
    pub fn new(memory_store: Arc<dyn MemoryStore>) -> Self {
        Self {
            memory_store: Some(memory_store),
            tokenizer: tiktoken_rs::cl100k_base().expect("cl100k_base bundled with tiktoken-rs"),
        }
    }

    pub fn new_standalone() -> Self {
        Self {
            memory_store: None,
            tokenizer: tiktoken_rs::cl100k_base().expect("cl100k_base bundled with tiktoken-rs"),
        }
    }
}
```

> Note the constructors no longer take `max_tokens` — the budget is now passed per call, not stored on the struct. Every existing call site must be updated (Task 8).

- [ ] **Step 4: Rewrite `assemble` to be budget-driven**

Replace the existing `pub async fn assemble(&self, request: ContextRequest) -> ContextBundle { ... }` body with the budget-driven version:

```rust
pub async fn assemble(
    &self,
    request: ContextRequest,
    budget: ContextBudget,
) -> ContextBundle {
    type Section = (ContextSource, String, u64);
    let mut sections: Vec<Section> = Vec::new();

    // P0: System prompt (never dropped)
    if let Some(sp) = &request.system_prompt {
        let n = self.count_tokens(sp);
        sections.push((ContextSource::System, sp.clone(), n));
    }

    // P0.5: Tool definitions — bundle as one JSON block (so the model adapter
    // can recover the structured array). Counted once.
    if !request.tool_definitions.is_empty() {
        let payload = serde_json::to_string(&request.tool_definitions)
            .unwrap_or_else(|_| String::from("[]"));
        let n = self.count_tokens(&payload);
        sections.push((ContextSource::ToolDefinitions, payload, n));
    }

    // P1: User request (dropped second-to-last)
    let request_text = format!("User request: {}", request.user_request);
    let n = self.count_tokens(&request_text);
    sections.push((ContextSource::Request, request_text, n));

    if let Some(task) = &request.active_task {
        let text = format!("Active task: {task}");
        let n = self.count_tokens(&text);
        sections.push((ContextSource::History, text, n));
    }

    let memories = if request.memories.is_empty() {
        if let Some(store) = &self.memory_store {
            let keywords = extract_keywords(&request.user_request);
            store
                .query(MemoryQuery {
                    scope: None,
                    keywords,
                    limit: 20,
                    session_id: request.session_id.clone(),
                    workspace_id: request.workspace_id.clone(),
                })
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        request.memories.clone()
    };
    for mem in memories.iter().filter(|m| m.accepted) {
        let text = format!("Memory: {}", mem.content);
        let n = self.count_tokens(&text);
        sections.push((ContextSource::Memory, text, n));
    }

    for h in &request.session_history {
        let text = format!("History: {h}");
        let n = self.count_tokens(&text);
        sections.push((ContextSource::History, text, n));
    }
    for tr in &request.tool_results {
        let text = format!("Tool result: {tr}");
        let n = self.count_tokens(&text);
        sections.push((ContextSource::ToolResult, text, n));
    }
    for sf in &request.selected_files {
        let text = format!("Selected file: {sf}");
        let n = self.count_tokens(&text);
        sections.push((ContextSource::SelectedFile, text, n));
    }

    // Pass 1: per-source caps (drop LIFO inside the capped category).
    let mut truncated = false;
    for (capped_src, cap) in &budget.source_caps {
        loop {
            let total: u64 = sections.iter()
                .filter(|(s, _, _)| s == capped_src)
                .map(|(_, _, n)| *n)
                .sum();
            if total <= *cap { break; }
            // Drop the LAST occurrence of this source (LIFO).
            if let Some(idx) = sections.iter().rposition(|(s, _, _)| s == capped_src) {
                sections.remove(idx);
                truncated = true;
            } else {
                break;
            }
        }
    }

    // Pass 2: global budget — drop lowest-priority section repeatedly.
    let input_budget = budget.input_budget();
    let mut total: u64 = sections.iter().map(|(_, _, n)| *n).sum();
    while total > input_budget {
        let Some(idx) = find_lowest_priority_drop(&sections) else { break; };
        total -= sections[idx].2;
        sections.remove(idx);
        truncated = true;
    }

    // Build per-source breakdown for ContextUsage.
    let mut by_source: Vec<(ContextSource, u64)> = Vec::new();
    for (src, _, n) in &sections {
        if let Some(entry) = by_source.iter_mut().find(|(s, _)| s == src) {
            entry.1 += n;
        } else {
            by_source.push((*src, *n));
        }
    }

    let usage = ContextUsage {
        total_tokens: total,
        budget_tokens: input_budget,
        context_window: budget.context_window,
        output_reservation: budget.output_reservation,
        by_source,
        estimator: "cl100k_base".to_string(),
        corrected_by_real_usage: false,
    };

    ContextBundle {
        messages: sections.iter().map(|(_, s, _)| s.clone()).collect(),
        sources: sections.iter().map(|(src, _, _)| *src).collect(),
        truncated,
        usage,
    }
}

fn count_tokens(&self, text: &str) -> u64 {
    self.tokenizer.encode_with_special_tokens(text).len() as u64
}
```

Update `find_lowest_priority_drop` to use `agent_core::ContextSource`:

```rust
fn find_lowest_priority_drop(sections: &[(ContextSource, String, u64)]) -> Option<usize> {
    let drop_order = [
        ContextSource::SelectedFile,
        ContextSource::ToolResult,
        ContextSource::History,
        ContextSource::Memory,
        ContextSource::ToolDefinitions, // last resort: drop tool defs before failing
    ];
    for category in &drop_order {
        for (i, (src, _, _)) in sections.iter().enumerate() {
            if src == category { return Some(i); }
        }
    }
    None
}
```

- [ ] **Step 5: Update existing in-module tests to the new signature**

The old `mod tests` in `context.rs` calls `ContextAssembler::new(500, store)` and `assembler.assemble(req)`. Convert:

```rust
let assembler = ContextAssembler::new(store.clone());
let bundle = assembler.assemble(req, ContextBudget {
    context_window: 600,
    output_reservation: 100,
    source_caps: vec![],
}).await;
```

For `assembles_request_with_standalone_assembler`, switch to:

```rust
let assembler = ContextAssembler::new_standalone();
let bundle = assembler
    .assemble(req, ContextBudget { context_window: 200, output_reservation: 100, source_caps: vec![] })
    .await;
assert!(bundle.usage.total_tokens <= bundle.usage.budget_tokens);
```

Replace any `bundle.token_count` reference with `bundle.usage.total_tokens`.

- [ ] **Step 6: Run tests**

Run: `cargo test -p agent-memory`
Expected: all tests pass (including the new `tests/context_budget.rs` integration tests + the rewritten in-module tests).

- [ ] **Step 7: Commit**

```bash
git add crates/agent-memory/src/context.rs crates/agent-memory/tests/context_budget.rs
git commit -m "refactor(memory): make ContextAssembler budget-driven and return ContextUsage"
```

---

### Task 7 — Update `EventPayload::ContextAssembled` to carry `ContextUsage`

**Files:**

- Modify: `crates/agent-core/src/events.rs`
- Modify: `crates/agent-core/src/projection.rs` (if it currently references the old payload — verified: it's in the "events not relevant to projection" arm, so no change needed)
- Modify: any consumer that pattern-matches `EventPayload::ContextAssembled` (search & update)

- [ ] **Step 1: Write the failing test**

In `crates/agent-core/src/events.rs`, add to the existing `mod tests` block:

```rust
#[test]
fn context_assembled_payload_carries_usage_struct() {
    use crate::context_types::{ContextSource, ContextUsage};

    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextAssembled {
            usage: ContextUsage {
                total_tokens: 12_345,
                budget_tokens: 188_000,
                context_window: 200_000,
                output_reservation: 12_000,
                by_source: vec![
                    (ContextSource::System, 800),
                    (ContextSource::ToolDefinitions, 11_545),
                ],
                estimator: "cl100k_base".into(),
                corrected_by_real_usage: false,
            },
        },
    );

    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "ContextAssembled");
    assert_eq!(json["payload"]["usage"]["total_tokens"], 12_345);
    assert_eq!(json["payload"]["usage"]["context_window"], 200_000);
    assert_eq!(json["payload"]["usage"]["estimator"], "cl100k_base");
    assert_eq!(json["payload"]["usage"]["by_source"][0][0], "system");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p agent-core context_assembled_payload -- --nocapture`
Expected: FAIL — `EventPayload::ContextAssembled` still expects `{ token_estimate, sources }`.

- [ ] **Step 3: Update the variant**

In `crates/agent-core/src/events.rs`, replace:

```rust
ContextAssembled {
    token_estimate: usize,
    sources: Vec<String>,
},
```

with:

```rust
ContextAssembled {
    usage: crate::context_types::ContextUsage,
},
```

The `EventPayload::event_type()` arm stays the same (`Self::ContextAssembled { .. } => "ContextAssembled"`).

- [ ] **Step 4: Update every consumer of the old payload (verified list — no need to grep)**

Plan-writing pass already grepped every reference. Update exactly these sites:

**A. `crates/agent-core/src/projection.rs:121`** — currently in the "events not relevant to projection" arm: `| EventPayload::ContextAssembled { .. }`. The `{ .. }` pattern still matches, so **no change required**. Verify with `cargo build -p agent-core` after Step 3.

**B. `crates/agent-core/tests/event_roundtrip.rs:62-67`** — replace:

```rust
fn context_assembled_roundtrips() {
    let event = make_event(EventPayload::ContextAssembled {
        token_estimate: 4096,
        sources: vec!["memory".into(), "system".into()],
    });
    assert_eq!(roundtrip(&event), event);
}
```

with:

```rust
fn context_assembled_roundtrips() {
    use agent_core::context_types::{ContextSource, ContextUsage};
    let event = make_event(EventPayload::ContextAssembled {
        usage: ContextUsage {
            total_tokens: 4096,
            budget_tokens: 100_000,
            context_window: 128_000,
            output_reservation: 28_000,
            by_source: vec![
                (ContextSource::Memory, 1024),
                (ContextSource::System, 3072),
            ],
            estimator: "cl100k_base".into(),
            corrected_by_real_usage: false,
        },
    });
    assert_eq!(roundtrip(&event), event);
}
```

**C. `apps/agent-gui/src/composables/useTraceStore.ts:74-89`** — currently reads `p.token_estimate` and `p.sources.join(...)`. Replace the `case "ContextAssembled":` block with:

```ts
case "ContextAssembled": {
  const sourceLabels = p.usage.by_source
    .map(([src, n]) => `${src}:${n}`)
    .join(", ");
  pushEntry({
    id: `ctx-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    kind: "tool",
    status: "completed",
    toolId: "context",
    title: `Context assembled (${p.usage.total_tokens} / ${p.usage.budget_tokens} tokens)`,
    startedAt: Date.now(),
    expanded: false,
    outputPreview: sourceLabels,
    rawEvent: rawJson(event)
  });
  break;
}
```

**D. `apps/agent-gui/src/composables/useTraceStore.test.ts`** — three sites (lines 145, 163-174, 717) emit fake `ContextAssembled` events with the old shape. Update each to:

```ts
{
  type: "ContextAssembled",
  usage: {
    total_tokens: 4096,
    budget_tokens: 100_000,
    context_window: 128_000,
    output_reservation: 28_000,
    by_source: [
      ["system", 1024],
      ["memory", 3072]
    ],
    estimator: "cl100k_base",
    corrected_by_real_usage: false
  }
}
```

**E. `apps/agent-gui/src/types/events-helpers.test.ts:27 and :70`** — the `ContextAssembled: fallback("no")` lines reference the variant by name only and do not destructure fields, so they still type-check after the variant changes. **No change required** — re-run the file's tests after `just gen-types` to confirm.

**F. `apps/agent-gui/src/stores/session.ts:129`** — currently a fallthrough `case "ContextAssembled":` with no field access. **No change required.**

**G. `apps/agent-gui/e2e/tauri-mock.js:328-333`** — verified during plan-writing: the mock DOES emit a `ContextAssembled` event in the `send_message` handler with the OLD shape:

```js
var ctxEvent = makeEvent(sessionId, {
  type: "ContextAssembled",
  token_estimate: 256,
  sources: ["system_prompt", "conversation_history"]
});
```

Replace those three property lines with the new payload shape (matching Step D's TS literal):

```js
var ctxEvent = makeEvent(sessionId, {
  type: "ContextAssembled",
  usage: {
    total_tokens: 256,
    budget_tokens: 100000,
    context_window: 128000,
    output_reservation: 28000,
    by_source: [
      ["system", 128],
      ["history", 128]
    ],
    estimator: "cl100k_base",
    corrected_by_real_usage: false
  }
});
```

> The e2e spec at `apps/agent-gui/e2e/trace-panel.spec.ts:27` is just a free-text comment — no change needed there.

**H. `crates/agent-runtime/src/`** — grep verified zero current emitters. Task 9 is the FIRST place that emits `ContextAssembled` in production runtime code. No pre-existing site to update here.

After updating B/C/D, run:

```bash
cargo build --workspace
pnpm --filter agent-gui exec tsc --noEmit
```

Expected: no errors. Any unmissed `ContextAssembled` consumer surfaces as a `property 'token_estimate' does not exist on type 'ContextUsage'` diagnostic with file + line — apply the Step C/D pattern to that site and re-run.

- [ ] **Step 5: Run the full workspace test**

Run: `cargo test --workspace --all-targets`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core/src/events.rs
# add any other files touched in step 4
git commit -m "feat(core): expand ContextAssembled payload to carry ContextUsage"
```

### Task 8 — Introduce `SessionState`, `ContextBudget`/`UsageCorrector`, and the runtime fields that hold them

**Files:**

- Create: `crates/agent-runtime/src/context_budget.rs`
- Modify: `crates/agent-runtime/src/session.rs` — add brand-new `pub struct SessionState`
- Modify: `crates/agent-runtime/src/facade_runtime.rs` — add `config`, `session_states`, `ollama_clients` fields + `with_config` / `with_ollama_clients` builders
- Modify: `crates/agent-runtime/src/lib.rs` — `pub mod context_budget;`

- [ ] **Step 1: Verify the starting state (no prior `SessionState` exists)**

Verified during plan-writing by `grep "pub struct" crates/agent-runtime/src/`:

- `crates/agent-runtime/src/session.rs` currently contains ONLY standalone `pub fn open_workspace / start_session / get_session_projection / get_trace / cancel_session / list_workspaces / list_sessions / rename_session / soft_delete_session / cleanup_expired_sessions / subscribe_session / subscribe_all / get_task_graph` helpers. There is NO `SessionState` struct today, and `LocalRuntime` does NOT carry a per-session state map (verified — `facade_runtime.rs` lines 45-83 show `task_graphs / active_cancellation / mcp_manager / dag_executor / catalog / installer / marketplace_dir / aggregate_handle / catalog_http / catalog_cache` but no `session_states`).
- This task is the FIRST place that introduces both the struct and the storage.

No exploratory grep needed at execution time — the situation is already mapped.

- [ ] **Step 2: Write the failing test for `UsageCorrector`**

Create `crates/agent-runtime/src/context_budget.rs`:

```rust
//! Per-session helpers for converting `ModelLimits` into a `ContextBudget`
//! and for EMA-correcting our cl100k_base estimate against the real
//! `ModelUsage` returned by providers.

use agent_memory::ContextBudget;
use agent_models::ModelLimits;

/// Convert a model's limits into a context budget.
///
/// We reserve `output_limit + 10%` (clamped to a 2k floor) so the input
/// fits when the model writes its longest legal completion.
pub fn build_budget(limits: &ModelLimits) -> ContextBudget {
    let safety = (limits.output_limit / 10).max(2_000);
    ContextBudget {
        context_window: limits.context_window,
        output_reservation: limits.output_limit + safety,
        source_caps: vec![],
    }
}

/// EMA-corrects our cl100k_base token estimate against real
/// `input_tokens` reported by the provider. Clamped to [0.7, 1.5] so a
/// single broken usage report can't blow up the budget.
#[derive(Debug, Clone)]
pub struct UsageCorrector {
    pub ratio: f32,
    pub samples: u32,
}

impl Default for UsageCorrector {
    fn default() -> Self {
        Self { ratio: 1.0, samples: 0 }
    }
}

impl UsageCorrector {
    pub fn apply(&self, estimated: u64) -> u64 {
        ((estimated as f32) * self.ratio).round() as u64
    }

    pub fn update(&mut self, real_input_tokens: u64, last_estimate: u64) {
        if last_estimate == 0 { return; }
        let new_ratio = (real_input_tokens as f32) / (last_estimate as f32);
        let clamped = new_ratio.clamp(0.7, 1.5);
        // simple EMA with alpha=0.4
        self.ratio = if self.samples == 0 {
            clamped
        } else {
            self.ratio * 0.6 + clamped * 0.4
        };
        self.samples += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_models::LimitSource;

    fn limits(ctx: u64, out: u64) -> ModelLimits {
        ModelLimits { context_window: ctx, output_limit: out, source: LimitSource::BuiltinRegistry }
    }

    #[test]
    fn build_budget_reserves_output_plus_safety_margin() {
        let b = build_budget(&limits(200_000, 8_192));
        assert_eq!(b.context_window, 200_000);
        // 8192 + max(819, 2000) = 8192 + 2000 = 10192
        assert_eq!(b.output_reservation, 10_192);
        assert_eq!(b.input_budget(), 200_000 - 10_192);
    }

    #[test]
    fn build_budget_safety_floor_kicks_in_for_small_models() {
        let b = build_budget(&limits(8_000, 1_024));
        // 1024 + max(102, 2000) = 1024 + 2000 = 3024
        assert_eq!(b.output_reservation, 3_024);
    }

    #[test]
    fn corrector_default_is_identity() {
        let c = UsageCorrector::default();
        assert_eq!(c.apply(1_000), 1_000);
    }

    #[test]
    fn corrector_first_sample_takes_clamped_ratio() {
        let mut c = UsageCorrector::default();
        c.update(1_200, 1_000); // ratio 1.2
        assert!((c.ratio - 1.2).abs() < 1e-3);
        assert_eq!(c.apply(1_000), 1_200);
    }

    #[test]
    fn corrector_clamps_pathological_ratios() {
        let mut c = UsageCorrector::default();
        c.update(10_000, 1_000); // ratio 10 → clamped 1.5
        assert!((c.ratio - 1.5).abs() < 1e-3);
        c.update(100, 1_000); // ratio 0.1 → clamped 0.7
        // EMA: 1.5*0.6 + 0.7*0.4 = 1.18
        assert!((c.ratio - 1.18).abs() < 1e-2);
    }

    #[test]
    fn corrector_ignores_zero_last_estimate() {
        let mut c = UsageCorrector::default();
        c.update(500, 0);
        assert_eq!(c.ratio, 1.0);
        assert_eq!(c.samples, 0);
    }
}
```

In `crates/agent-runtime/src/lib.rs`, add `pub mod context_budget;`.

- [ ] **Step 3: Run the test**

Run: `cargo test -p agent-runtime context_budget -- --nocapture`
Expected: 5 tests pass.

- [ ] **Step 4: Create the brand-new `SessionState` struct in `session.rs`**

Append to `crates/agent-runtime/src/session.rs` (the file currently has no struct definitions — Step 1 verified this):

```rust
use agent_models::ModelLimits;
use crate::context_budget::UsageCorrector;

/// Per-session in-memory state held by `LocalRuntime`. NOT persisted —
/// reconstructed lazily from event history if the process restarts mid-session.
///
/// Stored as `Arc<Mutex<HashMap<String, SessionState>>>` on `LocalRuntime`
/// (the key is `session_id.to_string()`).
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    /// Resolved model limits. `None` until the first call to `set_session_limits`
    /// (typically right after `SessionInitialized` is emitted).
    pub model_limits: Option<ModelLimits>,
    /// EMA-corrector that turns our cl100k_base estimate into something
    /// closer to the provider's reported `input_tokens`.
    pub usage_corrector: UsageCorrector,
    /// Most recent `ContextAssembled.usage.total_tokens` for this session.
    /// Used as the denominator when `update_corrector(real_input_tokens, last_estimate)`
    /// runs on `ModelEvent::Completed`.
    pub last_estimated_tokens: u64,
}
```

> The `Default` derive works because `Option<ModelLimits>` defaults to `None`, `UsageCorrector` derives its own `Default` (added in Step 2), and `u64` defaults to `0`.

- [ ] **Step 5: Add the storage + builder methods on `LocalRuntime`**

Edit `crates/agent-runtime/src/facade_runtime.rs`:

a) Add three new fields to `pub struct LocalRuntime<S, M>` (right after the existing `catalog_cache` field, around line 83):

```rust
/// Per-session in-memory state. Inserted lazily on first access.
session_states: Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
/// Loaded TOML config (`Config::load()` in production, in-line in tests).
/// Required by Tasks 9-10 to look up `ProfileDef` by alias and call
/// `agent_config::resolve_limits`.
config: Arc<agent_config::Config>,
/// Profile-alias → typed Ollama client. Populated by `with_ollama_clients`
/// at wiring time so Task 10 can fire `probe_context_window`. Empty when
/// no Ollama profiles are configured.
ollama_clients: HashMap<String, Arc<agent_models::OllamaClient>>,
```

b) Initialise all three in `LocalRuntime::new` (around line 88-110). For `config`, the safe default is an empty config so that legacy callers that haven't been updated yet still compile:

```rust
session_states: Arc::new(Mutex::new(HashMap::new())),
config: Arc::new(agent_config::Config { profiles: vec![], mcp_servers: vec![], source: agent_config::ConfigSource::Defaults }),
ollama_clients: HashMap::new(),
```

c) Add the two new builder methods alongside the existing `with_*` family (e.g., right after `with_mcp_servers`):

```rust
/// Inject the loaded `Config` so the runtime can resolve `ModelLimits`
/// per session. Called by every production wiring site after `Config::load()`.
pub fn with_config(mut self, config: Arc<agent_config::Config>) -> Self {
    self.config = config;
    self
}

/// Register typed Ollama clients per profile alias. Called by the wiring
/// code AFTER `build_router` so we retain the typed handle needed for
/// `probe_context_window` (which `Arc<dyn ModelClient>` cannot expose).
/// Idempotent — calling twice replaces the entries.
pub fn with_ollama_clients(
    mut self,
    clients: HashMap<String, Arc<agent_models::OllamaClient>>,
) -> Self {
    self.ollama_clients = clients;
    self
}
```

d) Add `pub(crate) async fn set_session_limits(&self, session_id: &SessionId, limits: agent_models::ModelLimits)` that grabs the mutex and inserts/updates the `SessionState`:

```rust
pub(crate) async fn set_session_limits(
    &self,
    session_id: &SessionId,
    limits: agent_models::ModelLimits,
) {
    let mut states = self.session_states.lock().await;
    let entry = states.entry(session_id.to_string())
        .or_insert_with(crate::session::SessionState::default);
    entry.model_limits = Some(limits);
}
```

e) Add the test-only accessor (gated so production code can never read it):

```rust
#[cfg(any(test, feature = "test-helpers"))]
pub fn event_store_for_test(&self) -> &S { &self.store }
```

If `agent-runtime/Cargo.toml` does not yet declare `[features] test-helpers = []`, add that single line. (Tasks 9 & 10 integration tests need this accessor to call `EventStore::load_session` directly.)

- [ ] **Step 6: Verify build**

Run: `cargo build -p agent-runtime`
Expected: success. Pre-existing TUI/GUI startup paths still compile because `with_config` defaults to an empty `Config` — Task 10 Step 1 wires the real config through.

- [ ] **Step 7: Commit**

```bash
git add crates/agent-runtime/src/context_budget.rs \
        crates/agent-runtime/src/session.rs \
        crates/agent-runtime/src/facade_runtime.rs \
        crates/agent-runtime/src/lib.rs \
        crates/agent-runtime/Cargo.toml
git commit -m "feat(runtime): introduce SessionState + ContextBudget + UsageCorrector and runtime storage"
```

---

### Task 9 — Rewrite `agent_loop` to call `ContextAssembler` and emit `ContextAssembled { usage }`

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop.rs`
- Create: `crates/agent-runtime/tests/context_budget.rs`

This task replaces the "replay every event into ModelMessage" path with a `ContextAssembler` call and emits the new event with the correctly-shaped payload.

- [ ] **Step 1: Write the failing integration test**

Create `crates/agent-runtime/tests/context_budget.rs`. The setup mirrors the pattern already used by `crates/agent-runtime/tests/full_stack.rs` (read the first ~60 lines of that file to copy the exact `LocalRuntime::new(...).with_*(...)` chain in use today). Concretely:

```rust
//! Verifies that the agent loop emits `ContextAssembled { usage }` with
//! `total_tokens <= budget_tokens` and that the `usage.by_source`
//! breakdown contains the expected categories.

use agent_core::{
    AppFacade, EventPayload, SendMessageRequest, SessionId, StartSessionRequest, WorkspaceId,
};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;

async fn build_test_runtime() -> Arc<LocalRuntime<SqliteEventStore, FakeModelClient>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = SqliteEventStore::new(pool).await.unwrap();
    let model = FakeModelClient::new(vec!["ok".into(); 16]);
    Arc::new(LocalRuntime::new(store, model))
}

async fn load_events(
    runtime: &LocalRuntime<SqliteEventStore, FakeModelClient>,
    session_id: &SessionId,
) -> Vec<agent_core::DomainEvent> {
    use agent_store::EventStore;
    runtime.event_store_for_test().load_session(session_id).await.unwrap()
}

#[tokio::test]
async fn context_assembled_event_emitted_with_budget_respected() {
    let runtime = build_test_runtime().await;

    let ws_info = runtime.open_workspace(".".into()).await.unwrap();
    let workspace_id: WorkspaceId = ws_info.workspace_id;
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    for i in 0..5 {
        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                content: format!("turn {} please", i),
            })
            .await
            .unwrap();
    }

    let events = load_events(&runtime, &session_id).await;
    let assembled: Vec<_> = events.iter().filter_map(|e| match &e.payload {
        EventPayload::ContextAssembled { usage } => Some(usage),
        _ => None,
    }).collect();

    assert!(!assembled.is_empty(), "expected at least one ContextAssembled event");
    for usage in &assembled {
        assert!(
            usage.total_tokens <= usage.budget_tokens,
            "ContextAssembled.total_tokens ({}) exceeded budget_tokens ({})",
            usage.total_tokens, usage.budget_tokens
        );
        assert_eq!(usage.estimator, "cl100k_base");
        assert_eq!(usage.context_window, 4_096); // FALLBACK_FAKE from model_registry
        assert!(usage
            .by_source
            .iter()
            .any(|(s, n)| matches!(s, agent_core::ContextSource::System) && *n > 0));
    }
}
```

> Two helpers needed inside the production crate to keep the test surface clean and avoid `cfg(test)` leakage:
>
> 1. **`event_store_for_test()`** on `LocalRuntime` — add a `#[cfg(any(test, feature = "test-helpers"))] pub fn event_store_for_test(&self) -> &S { &self.store }`. This grants integration tests read access to the underlying store without exposing it to production code. (If `agent-runtime` already has a `test-helpers` feature, reuse it; otherwise add `[features] test-helpers = []` to `crates/agent-runtime/Cargo.toml` and the dev-dep declaration `agent-runtime = { path = "../agent-runtime", features = ["test-helpers"] }` to `[dev-dependencies]`.)
> 2. **`SqliteEventStore::new(pool)`** is the existing constructor (verified in `crates/agent-store/src/sqlite.rs`). If the actual constructor name in your tree is different (e.g. `SqliteEventStore::new_in_memory()`), copy the exact construction from `crates/agent-runtime/tests/full_stack.rs`'s test setup — do NOT invent a new variant.
>
> Why use `Arc<LocalRuntime<...>>`: `LocalRuntime` does not implement `Clone` (verified — see `crates/agent-runtime/src/facade_runtime.rs:46`). Holding it behind `Arc` lets later tests (Task 10) hand the same runtime to a spawned probe task.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p agent-runtime --test context_budget -- --nocapture`
Expected: FAIL — `ContextAssembled` events are not currently emitted by the runtime.

- [ ] **Step 3: Plumb `config` + `session_states` into `run_agent_loop` via `AgentLoopDeps`**

`run_agent_loop` currently takes a fixed list of `&Arc<...>` parameters and is called from `crates/agent-runtime/src/facade_runtime.rs::send_message` (verified — `AppFacade::send_message` impl at lines 365-380). The `config`, `session_states`, and `ollama_clients` fields on `LocalRuntime` were added in Task 8 Step 5; the `with_config` / `with_ollama_clients` builders were added in the same step. **No new fields or builders are introduced here** — Task 9 only consumes them.

To avoid a parameter explosion, introduce in `crates/agent-runtime/src/agent_loop.rs`:

```rust
pub struct AgentLoopDeps<'a, S, M>
where S: agent_store::EventStore + 'static, M: agent_models::ModelClient + 'static,
{
    pub store: &'a Arc<S>,
    pub model: &'a Arc<M>,
    pub event_tx: &'a tokio::sync::broadcast::Sender<agent_core::DomainEvent>,
    pub tool_registry: &'a Arc<tokio::sync::Mutex<agent_tools::ToolRegistry>>,
    pub permission_engine: &'a Arc<tokio::sync::Mutex<agent_tools::PermissionEngine>>,
    pub pending_permissions: &'a Arc<tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>>,
    pub memory_store: &'a Option<Arc<dyn agent_memory::MemoryStore>>,
    pub task_graphs: &'a Arc<tokio::sync::Mutex<std::collections::HashMap<String, crate::task_graph::TaskGraph>>>,
    pub active_cancellation: &'a Arc<tokio::sync::Mutex<Option<tokio_util::sync::CancellationToken>>>,
    pub config: &'a Arc<agent_config::Config>,
    pub session_states: &'a Arc<tokio::sync::Mutex<std::collections::HashMap<String, crate::session::SessionState>>>,
}
```

Change the function signature to `pub async fn run_agent_loop<S, M>(deps: AgentLoopDeps<'_, S, M>, request: &SendMessageRequest) -> agent_core::Result<()>` and update the body to read the existing `&Arc<...>` references via `deps.store`, `deps.model`, etc. (mechanical rename — no logic change in the existing code).

Update the single caller in `facade_runtime.rs::send_message` (the `ExecutionMode::SingleStep` arm, around line 372) from:

```rust
crate::agent_loop::run_agent_loop(
    &self.store,
    &self.model,
    &self.event_tx,
    &self.tool_registry,
    &self.permission_engine,
    &self.pending_permissions,
    &self.memory_store,
    &self.task_graphs,
    &self.active_cancellation,
    &request,
)
.await
```

to:

```rust
crate::agent_loop::run_agent_loop(
    crate::agent_loop::AgentLoopDeps {
        store: &self.store,
        model: &self.model,
        event_tx: &self.event_tx,
        tool_registry: &self.tool_registry,
        permission_engine: &self.permission_engine,
        pending_permissions: &self.pending_permissions,
        memory_store: &self.memory_store,
        task_graphs: &self.task_graphs,
        active_cancellation: &self.active_cancellation,
        config: &self.config,
        session_states: &self.session_states,
    },
    &request,
)
.await
```

> The empty-`Config` default initialised in Task 8 keeps every TUI/GUI startup path compiling; the actual `with_config(Arc::new(loaded))` call is wired in Task 10 Step 1d alongside `with_ollama_clients`. This keeps Tasks 8/9 building & passing in isolation.

- [ ] **Step 4: Inside `run_agent_loop`, replace the `build_model_messages` call site**

Locate the block in `crates/agent-runtime/src/agent_loop.rs` (around lines 220-235) that does:

```rust
let messages = build_model_messages(&request.content, &session_events);
let tool_defs = { /* registry list_all -> Vec<ToolDefinition> */ };
let model_profile = session_events.iter().find_map(|e| match &e.payload {
    EventPayload::SessionInitialized { model_profile } => Some(model_profile.clone()),
    _ => None,
}).unwrap_or_else(|| "fake".to_string());
```

Replace with:

```rust
// Resolve the profile alias from session events (fallback "fake" for legacy).
let model_profile_alias: String = session_events.iter().find_map(|e| match &e.payload {
    EventPayload::SessionInitialized { model_profile } => Some(model_profile.clone()),
    _ => None,
}).unwrap_or_else(|| "fake".to_string());

// Resolve ModelLimits: prefer per-session cached limits (Task 10's probe may
// have refined them), otherwise re-resolve from config + registry.
let limits = {
    let states = deps.session_states.lock().await;
    states
        .get(request.session_id.as_str())
        .and_then(|s| s.model_limits.clone())
}
.unwrap_or_else(|| {
    let profile_def = deps
        .config
        .profiles
        .iter()
        .find(|(alias, _)| alias == &model_profile_alias)
        .map(|(_, def)| def);
    match profile_def {
        Some(def) => agent_config::resolve_limits(def),
        None => agent_models::lookup_limits("fake", "fake"), // pre-0.7 sessions
    }
});

let budget = crate::context_budget::build_budget(&limits);

// Tool definitions: serialised once, consumed both by the assembler (token
// accounting) AND by the model adapter (the actual schemas to inject).
let tool_defs: Vec<agent_models::ToolDefinition> = {
    let registry = tool_registry.lock().await;
    registry.list_all().await.into_iter()
        .map(|td| agent_models::ToolDefinition {
            name: td.tool_id, description: td.description, parameters: td.parameters,
        })
        .collect()
};

// History strings — one per narrative event. Tool-call / tool-result
// pairing for the actual ModelMessage list happens below in
// `build_model_messages_from_bundle`; this `session_history` is purely
// for the assembler's token accounting and dropping decisions.
let session_history: Vec<String> = session_events.iter().filter_map(|e| match &e.payload {
    EventPayload::UserMessageAdded { content, .. } => Some(format!("user: {content}")),
    EventPayload::AssistantMessageCompleted { content, .. } => Some(format!("assistant: {content}")),
    EventPayload::ToolInvocationCompleted { tool_id, output_preview, .. } =>
        Some(format!("tool[{tool_id}]: {output_preview}")),
    _ => None,
}).collect();

let assembler = agent_memory::ContextAssembler::new_standalone();
let bundle = assembler.assemble(
    agent_memory::ContextRequest {
        system_prompt: Some(system_prompt.clone()),
        user_request: request.content.clone(),
        session_history,
        tool_definitions: tool_defs.clone(),
        ..Default::default()
    },
    budget,
).await;

// Apply per-session UsageCorrector (no-op until Task 10 wires real-usage feedback).
let mut usage = bundle.usage.clone();
{
    let mut states = deps.session_states.lock().await;
    let entry = states.entry(request.session_id.to_string()).or_insert_with(crate::session::SessionState::default);
    if entry.usage_corrector.samples > 0 {
        usage.total_tokens = entry.usage_corrector.apply(usage.total_tokens);
        for (_, n) in &mut usage.by_source {
            *n = entry.usage_corrector.apply(*n);
        }
        usage.corrected_by_real_usage = true;
    }
    entry.last_estimated_tokens = usage.total_tokens;
}

// Emit the event so UIs can show usage.
let assembled_event = DomainEvent::new(
    request.workspace_id.clone(),
    request.session_id.clone(),
    AgentId::system(),
    PrivacyClassification::MinimalTrace,
    EventPayload::ContextAssembled { usage: usage.clone() },
);
append_and_broadcast(&**store, event_tx, &assembled_event).await?;

// Build the actual ModelMessage list. This MUST preserve tool_call / tool_result
// id pairing (otherwise Anthropic / OpenAI reject the request), so we run the
// existing `build_model_messages` over `session_events` and then trim the
// FRONT of the resulting Vec until cumulative tokens ≤ budget.input_budget().
let messages = build_model_messages_within_budget(
    &request.content,
    &session_events,
    budget.input_budget(),
);
```

- [ ] **Step 5: Add the `build_model_messages_within_budget` helper**

Append to `crates/agent-runtime/src/agent_loop.rs` (just below the existing `build_model_messages`):

```rust
/// Builds a `Vec<ModelMessage>` from `session_events` (preserving tool_call /
/// tool_result id pairing) and trims the FRONT until cumulative input tokens
/// fit `budget_tokens`. The system prompt + the most-recent user message are
/// always kept (they're appended last by `build_model_messages`).
///
/// Token accounting MUST match what providers actually bill — `ModelMessage`
/// has three serialised parts: `role`, `content`, and `tool_calls`
/// (a `Vec<ToolCall>` whose `arguments` is `serde_json::Value`). Tool calls
/// alone often weigh thousands of tokens for non-trivial payloads, so we
/// serialise the whole message to JSON and count that. This matches the
/// estimator used by `ContextAssembler` (cl100k_base on serialised text).
pub fn build_model_messages_within_budget(
    user_content: &str,
    session_events: &[DomainEvent],
    budget_tokens: u64,
) -> Vec<agent_models::ModelMessage> {
    let mut messages = build_model_messages(user_content, session_events);

    let bpe = match tiktoken_rs::cl100k_base() {
        Ok(bpe) => bpe,
        Err(_) => return messages, // tokenizer unavailable; emit as-is
    };
    let count_message = |m: &agent_models::ModelMessage| -> u64 {
        // Use compact JSON to mirror what the OpenAI/Anthropic adapters
        // ultimately serialise. Failures fall back to content-only count.
        match serde_json::to_string(m) {
            Ok(s) => bpe.encode_with_special_tokens(&s).len() as u64,
            Err(_) => bpe.encode_with_special_tokens(&m.content).len() as u64,
        }
    };

    // Always keep the trailing user message (the active turn). Trim from the
    // FRONT, but NEVER drop a `tool` role message without also dropping the
    // matching assistant `tool_calls` message that precedes it — otherwise
    // OpenAI / Anthropic reject the request with "tool_call_id has no
    // matching assistant tool_calls".
    let mut total: u64 = messages.iter().map(count_message).sum();
    while total > budget_tokens && messages.len() > 1 {
        let front = messages.first().unwrap();
        if front.role == "tool" {
            // No matching assistant left at the front — safe to drop alone.
            total = total.saturating_sub(count_message(front));
            messages.remove(0);
            continue;
        }
        if front.role == "assistant" && !front.tool_calls.is_empty() {
            // Drop the assistant AND every tool message immediately following it
            // (the matching `tool_call_id` results) in one atomic step.
            total = total.saturating_sub(count_message(front));
            messages.remove(0);
            while !messages.is_empty() && messages[0].role == "tool" {
                total = total.saturating_sub(count_message(&messages[0]));
                messages.remove(0);
            }
            continue;
        }
        // Plain user/assistant text — drop one.
        total = total.saturating_sub(count_message(front));
        messages.remove(0);
    }
    messages
}
```

Add a unit test in the same file (inside `#[cfg(test)] mod tests`) that:

```rust
#[test]
fn within_budget_keeps_tail_user_and_pairs_tool_calls() {
    // Build 4 turns: 3 plain user/assistant pairs + 1 turn with a tool call.
    let mut events = Vec::new();
    for i in 0..3 {
        events.push(make_event(EventPayload::UserMessageAdded {
            message_id: format!("u{i}"),
            content: format!("user turn {i} ").repeat(20), // padded to consume tokens
        }));
        events.push(make_event(EventPayload::AssistantMessageCompleted {
            message_id: format!("a{i}"),
            content: format!("assistant turn {i} ").repeat(20),
        }));
    }

    let trimmed = build_model_messages_within_budget("latest", &events, 100);

    // (a) total token count ≤ 100
    let bpe = tiktoken_rs::cl100k_base().unwrap();
    let total: usize = trimmed.iter()
        .map(|m| bpe.encode_with_special_tokens(&serde_json::to_string(m).unwrap()).len())
        .sum();
    assert!(total <= 100, "trimmed total {} exceeded budget 100", total);

    // (b) trailing user message is the active turn
    assert_eq!(trimmed.last().map(|m| m.role.as_str()), Some("user"));
    assert_eq!(trimmed.last().map(|m| m.content.as_str()), Some("latest"));

    // (c) every `tool` role message has a preceding assistant with non-empty tool_calls
    for (i, m) in trimmed.iter().enumerate() {
        if m.role == "tool" {
            assert!(i > 0, "tool message at index 0 is unpaired");
            let prev = &trimmed[i - 1];
            assert!(
                prev.role == "assistant" && !prev.tool_calls.is_empty(),
                "tool message at {} not preceded by assistant with tool_calls",
                i
            );
        }
    }
}
```

> The `make_event` helper already exists in many tests under `crates/agent-runtime/src/agent_loop.rs` test module (it's the standard `DomainEvent::new` wrapper used elsewhere in this crate). If it doesn't, inline-construct the events directly — the test only needs the `payload` field populated.

- [ ] **Step 6: Run the integration test**

Run: `cargo test -p agent-runtime --test context_budget -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Run the full suite — the only known regression site is precisely identified**

Run: `cargo test --workspace --all-targets`

Verified during plan-writing by `grep -nE "events.len|assert_eq.*events|ContextAssembled" crates/agent-runtime/tests/full_stack.rs crates/agent-runtime/tests/agent_loop.rs crates/agent-runtime/tests/refactor_baseline.rs`:

- **`crates/agent-runtime/tests/full_stack.rs:488`** — sole match. The line is:

  ```rust
  if stream_events.len() > 30 { break; }
  ```

  This is a stream-collection break threshold inside `tokio::select!`, not an `assert_eq!`. With `ContextAssembled` now emitted once per `send_message`, the inner loop breaks 1 event earlier per turn. The downstream assertions that follow (lines 500-515: `assert!(stream_types.contains(&"UserMessageAdded"))`, etc.) check for the PRESENCE of specific types, not exact counts, so they remain valid. **No change required.** If running the test reveals an actual failure (e.g., a `ContextAssembled` happens before `UserMessageAdded` ever lands in the truncated stream), bump the threshold from `30` to `40` — the only mechanical fix needed.

- **`crates/agent-runtime/tests/agent_loop.rs`**, **`refactor_baseline.rs`** — zero `events.len` / `assert_eq!.*events` / `ContextAssembled` matches. No updates needed.

- **`crates/agent-tui/tests/app_logic.rs`** — re-grep at execution time with `grep -nE "events.len|assert_eq.*events|ContextAssembled" crates/agent-tui/tests/app_logic.rs`. If any matches surface, apply one of the three patterns below; if zero matches, no change.

- **`apps/agent-gui/src/composables/useTraceStore.test.ts`** — already covered by Task 7 Step 4 (D).

For any unexpected failure not in the list above, the fix is one of:

1. Add `ContextAssembled` to the expected sequence.
2. Filter it out: `events.iter().filter(|e| !matches!(e.payload, EventPayload::ContextAssembled { .. }))`.
3. Switch from `assert_eq!(events.len(), N)` to a more specific assertion that ignores ordering of `ContextAssembled`.

Expected after Step 7: `cargo test --workspace --all-targets` is green. If it isn't, the diagnostic points at the exact file — apply pattern 1/2/3 and re-run.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-runtime/src/agent_loop.rs \
        crates/agent-runtime/src/facade_runtime.rs \
        crates/agent-runtime/tests/context_budget.rs
# also add any existing tests you updated
git commit -m "feat(runtime): assemble context per iteration and emit ContextAssembled with usage"
```

- [ ] **Step 4: Run the integration test**

Run: `cargo test -p agent-runtime --test context_budget -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run the full suite to catch regressions**

Run: `cargo test --workspace --all-targets`
Expected: all tests pass. Several existing runtime/TUI integration tests assert specific event sequences — they may now contain an extra `ContextAssembled` event per round. Update those tests to either filter it out or count it explicitly.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime/src/agent_loop.rs crates/agent-runtime/tests/context_budget.rs
# include any updated existing tests
git commit -m "feat(runtime): assemble context per iteration and emit ContextAssembled with usage"
```

---

### Task 10 — Wire Ollama runtime probe into `LocalRuntime` session init + apply real-usage feedback

**Files:**

- Modify: `crates/agent-runtime/src/facade_runtime.rs` — add `ollama_clients` field + `with_ollama_clients` builder + probe-on-`start_session`
- Modify: `crates/agent-runtime/src/agent_loop.rs` — apply real-usage feedback when `ModelEvent::Completed` arrives
- Modify: `crates/agent-runtime/tests/context_budget.rs` — add the second test

> **Why we cannot reach the Ollama client through `ModelRouter`** (verified by reading `crates/agent-models/src/router.rs`): `ModelRouter` only stores `Arc<dyn ModelClient>`; there is no `get_ollama_client(...)` accessor and adding one would require runtime downcasting (`Any`) on every dyn call site. Instead, give `LocalRuntime` its own `HashMap<String, Arc<OllamaClient>>` populated at wiring time. This keeps `ModelRouter` untouched.

- [ ] **Step 1: Add `ollama_clients` storage + builder on `LocalRuntime`**

Edit `crates/agent-runtime/src/facade_runtime.rs`:

a) Add the field to the struct (right after `model: Arc<M>`, around line 51):

```rust
/// Profile-alias → typed Ollama client, populated when wiring the runtime
/// from `agent_config`. Used to fire `probe_context_window` on session
/// init. Empty when no Ollama profiles are configured.
ollama_clients: HashMap<String, Arc<agent_models::OllamaClient>>,
```

b) Initialise it as `HashMap::new()` in `LocalRuntime::new`.

c) Add the builder method (place it next to `with_mcp_servers`):

```rust
/// Register typed Ollama clients per profile alias. Called by the
/// runtime wiring code after `build_router` so we retain the typed
/// handle needed for `probe_context_window`. Idempotent — calling
/// twice replaces the entries.
pub fn with_ollama_clients(
    mut self,
    clients: HashMap<String, Arc<agent_models::OllamaClient>>,
) -> Self {
    self.ollama_clients = clients;
    self
}
```

d) Update `agent-config`'s `build_router` (in `crates/agent-config/src/builder.rs`) to also return the per-profile Ollama clients. Concrete change: alongside the existing `pub fn build_router(...) -> ModelRouter`, add `pub fn build_ollama_clients(config: &Config) -> HashMap<String, Arc<OllamaClient>>` that walks `config.profiles` and constructs an `OllamaClient` for each `provider == "ollama"` entry. Then update every wiring point (TUI: `crates/agent-tui/src/main.rs`; GUI: `apps/agent-gui/src-tauri/src/lib.rs`) to call `.with_ollama_clients(build_ollama_clients(&config))`.

- [ ] **Step 2: Add `set_session_limits` + read accessor on `LocalRuntime`**

Add these methods on the `impl<S, M> LocalRuntime<S, M>` block:

```rust
pub(crate) async fn set_session_limits(
    &self,
    session_id: &SessionId,
    limits: agent_models::ModelLimits,
) {
    let mut states = self.session_states.lock().await;
    let entry = states.entry(session_id.to_string())
        .or_insert_with(crate::session::SessionState::default);
    entry.model_limits = Some(limits);
}
```

> Note: `session_states: Arc<Mutex<HashMap<String, SessionState>>>` is the field added in Task 8. This method is `pub(crate)` so the spawned probe in Step 3 can call it without leaking the type to consumers.

- [ ] **Step 3: Write the failing integration test**

Append to `crates/agent-runtime/tests/context_budget.rs`:

```rust
#[tokio::test]
async fn ollama_session_uses_probed_context_window() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use std::collections::HashMap;
    use std::sync::Arc;

    // 1. Stand up a mock Ollama server that answers /api/show with a 32k window.
    let mock = MockServer::start().await;
    Mock::given(method("POST")).and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model_info": { "llama.context_length": 32_768_u64 }
        })))
        .mount(&mock).await;
    Mock::given(method("POST")).and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "{\"message\":{\"role\":\"assistant\",\"content\":\"ok\"},\"done\":false}\n\
             {\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true}\n",
        ))
        .mount(&mock).await;

    // 2. Build the runtime in-line — `build_test_runtime` doesn't know about
    //    Ollama, so this is a separate setup. Mirrors what `build_router` +
    //    `build_ollama_clients` will do in production wiring.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1).connect("sqlite::memory:").await.unwrap();
    let store = agent_store::SqliteEventStore::new(pool).await.unwrap();
    // The model client doesn't matter for this test — we never reach the
    // chat endpoint because the assertion runs immediately after probe.
    // FakeModelClient will be invoked once for the send_message; that's fine.
    let model = agent_models::FakeModelClient::new(vec!["ok".into()]);

    // Build a Config with a single Ollama profile pointing at the mock.
    let config_toml = format!(
        r#"
        [[profiles]]
        alias = "ollama-test"
        provider = "ollama"
        model_id = "llama3"
        # context_window omitted — forces resolve_limits → BuiltinRegistry → RuntimeProbe path
        [profiles.ollama]
        base_url = "{}"
        "#,
        mock.uri()
    );
    let config = Arc::new(agent_config::load_from_str(&config_toml).unwrap());

    let ollama_client = Arc::new(agent_models::OllamaClient::new(agent_models::OllamaConfig {
        base_url: mock.uri(),
        default_model: "llama3".into(),
        context_window: 8_192, // fallback — should be overridden by probe
    }));
    let mut clients = HashMap::new();
    clients.insert("ollama-test".to_string(), ollama_client);

    let runtime = Arc::new(
        agent_runtime::LocalRuntime::new(store, model)
            .with_config(config)
            .with_ollama_clients(clients),
    );

    // 3. Open workspace, start session — probe fires here.
    let ws_info = runtime.open_workspace(".".into()).await.unwrap();
    let session_id = runtime.start_session(agent_core::StartSessionRequest {
        workspace_id: ws_info.workspace_id.clone(),
        model_profile: "ollama-test".into(),
    }).await.unwrap();

    // 4. Wait for the spawned probe to land. 3s timeout on the probe itself,
    //    but the mock answers in <1ms, so 200ms is generous.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // 5. Send one message to trigger ContextAssembled emission with the new limits.
    runtime.send_message(agent_core::SendMessageRequest {
        workspace_id: ws_info.workspace_id,
        session_id: session_id.clone(),
        content: "hi".into(),
    }).await.unwrap();

    let events = load_events(&runtime, &session_id).await;
    let usage = events.iter().find_map(|e| match &e.payload {
        agent_core::EventPayload::ContextAssembled { usage } => Some(usage),
        _ => None,
    }).expect("ContextAssembled emitted");
    assert_eq!(
        usage.context_window, 32_768,
        "Ollama probe should have overridden the fallback (got {})",
        usage.context_window
    );
}
```

> **Helpers used by this test (all verified during plan-writing)**:
>
> - `agent_config::load_from_str` — public re-export from `crates/agent-config/src/lib.rs:11`.
> - `agent_models::OllamaClient::new(OllamaConfig)` — `crates/agent-models/src/ollama.rs:46`.
> - `LocalRuntime::with_config` and `LocalRuntime::with_ollama_clients` — both added in Task 8 Step 5 (NOT here). Task 10 only consumes them.
> - `LocalRuntime::set_session_limits` — added in Task 8 Step 5 as `pub(crate)`. Reachable from the same crate.

- [ ] **Step 4: Run to verify failure**

Run: `cargo test -p agent-runtime --test context_budget ollama_session_uses_probed -- --nocapture`
Expected: FAIL — without the probe wiring, `assert_eq!(usage.context_window, 32_768)` will fail with `assertion `left == right` failed: left: 8_192, right: 32_768`.

- [ ] **Step 5: Implement probe-on-`start_session`**

`start_session` in `LocalRuntime` (line ~377, found in Task-writing pass) currently delegates to `crate::session::start_session(...)`. Wrap it like this:

```rust
async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
    let model_profile_alias = request.model_profile.clone();
    let session_id = crate::session::start_session(
        &*self.store,
        &self.event_tx,
        request.workspace_id.clone(),
        request.model_profile,
    ).await?;

    // Resolve initial limits from config + builtin registry.
    let profile_def = self.config.profiles.iter()
        .find(|(alias, _)| alias == &model_profile_alias)
        .map(|(_, def)| def.clone());
    if let Some(def) = profile_def {
        let mut limits = agent_config::resolve_limits(&def);
        self.set_session_limits(&session_id, limits.clone()).await;

        // Ollama-only: refine via runtime probe. Bounded to 3s; failure is
        // non-fatal (limits stays at registry/config value).
        if def.provider == "ollama" {
            if let Some(client) = self.ollama_clients.get(&model_profile_alias).cloned() {
                let model_id = def.model_id.clone();
                let session_id_for_probe = session_id.clone();
                let session_states = self.session_states.clone();
                tokio::spawn(async move {
                    let probe = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        client.probe_context_window(&model_id),
                    ).await;
                    if let Ok(Some(window)) = probe {
                        let mut states = session_states.lock().await;
                        if let Some(entry) = states.get_mut(session_id_for_probe.as_str()) {
                            if let Some(ref mut l) = entry.model_limits {
                                l.context_window = window;
                                l.source = agent_models::LimitSource::RuntimeProbe;
                            }
                        }
                    }
                });
            }
        }
    }

    Ok(session_id)
}
```

> **Why we capture `session_states.clone()` instead of `self.clone()`**: `LocalRuntime` does not implement `Clone` (verified line 46 of `facade_runtime.rs`). Capturing `Arc<Mutex<HashMap<...>>>` (which IS clonable — it's the same backing store) is the standard tokio-spawn idiom and avoids forcing every consumer to wrap the runtime in `Arc<Self>` just for this one call site.

- [ ] **Step 6: Apply real-usage feedback in `agent_loop`**

In `crates/agent-runtime/src/agent_loop.rs`, find the streaming-loop arm that handles `ModelEvent::Completed { usage, .. }` (the existing match — Anthropic and OpenAI both populate `usage`; Ollama leaves it `None`). Add right before whatever currently exists in that arm:

```rust
ModelEvent::Completed { usage: Some(real_usage), .. } => {
    let mut states = deps.session_states.lock().await;
    if let Some(entry) = states.get_mut(request.session_id.as_str()) {
        let estimated = entry.last_estimated_tokens;
        if estimated > 0 {
            entry.usage_corrector.update(real_usage.input_tokens, estimated);
        }
    }
    // ... existing handling continues
}
```

> `UsageCorrector::update(real, estimated)` was defined in Task 8 with EMA + clamp to `[0.7, 1.5]`. The next `send_message` will then read `entry.usage_corrector` from Task 9 Step 4 and apply the multiplier to the new estimate, setting `usage.corrected_by_real_usage = true`.

- [ ] **Step 7: Run both tests in `context_budget.rs`**

Run: `cargo test -p agent-runtime --test context_budget -- --nocapture`
Expected: both tests pass.

Run: `cargo test --workspace --all-targets`
Expected: all green. The wiring updates in Step 1d touch TUI/GUI startup paths — if any startup integration test breaks, the diagnostic should point at the missing `.with_ollama_clients(...)` builder call.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-runtime/src/facade_runtime.rs \
        crates/agent-runtime/src/agent_loop.rs \
        crates/agent-config/src/builder.rs \
        crates/agent-tui/src/main.rs \
        apps/agent-gui/src-tauri/src/lib.rs \
        crates/agent-runtime/tests/context_budget.rs
git commit -m "feat(runtime): probe Ollama context window on session init and EMA-correct estimates"
```

---

### Task 11 — Register specta types, regenerate `events.ts`, run full verification gate

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Auto-regenerated: `apps/agent-gui/src/generated/events.ts`

- [ ] **Step 1: Make sure `agent-models` exposes `specta` derives**

Read `crates/agent-models/Cargo.toml`. If `[features]` does not contain a `specta` feature, add:

```toml
[features]
default = []
specta = ["dep:specta"]

[dependencies]
specta = { workspace = true, optional = true }
```

(The same pattern is already in `crates/agent-core/Cargo.toml` and `crates/agent-memory/Cargo.toml` — copy that exactly.) Then verify the new types added in Task 2 carry `#[cfg_attr(feature = "specta", derive(specta::Type))]` on `ModelLimits`, `LimitSource`, `RegistryEntry`. If they were added without this attribute, fix Task 2's output now (it's just one extra `cfg_attr` line per type).

- [ ] **Step 2: Add `agent-models` to `agent-gui-tauri` with the `specta` feature**

In `apps/agent-gui/src-tauri/Cargo.toml`, locate the existing `agent-core = { path = "../../../crates/agent-core", features = ["specta"] }` line. Add a sibling immediately below:

```toml
agent-models = { path = "../../../crates/agent-models", features = ["specta"] }
```

(Use the exact path style of the surrounding lines — confirm by reading the file. If the surrounding lines use `workspace = true`, switch to that.)

- [ ] **Step 3: Register the new types in `specta.rs`**

Edit `apps/agent-gui/src-tauri/src/specta.rs`. The existing builder uses chained `.typ::<T>()` calls (verified — see lines 78-100, e.g., `.typ::<EventPayload>().typ::<DomainEvent>()...`). Add right after the existing `.typ::<MemoryScope>()` line, BEFORE the closing of the chained expression:

```rust
        .typ::<agent_core::context_types::ContextSource>()
        .typ::<agent_core::context_types::ContextUsage>()
        .typ::<agent_models::ModelLimits>()
        .typ::<agent_models::LimitSource>()
```

Also add the import at the top of the file (alongside `use agent_core::{ ... }`):

```rust
use agent_models::{LimitSource, ModelLimits};
```

If you re-export `ContextSource` / `ContextUsage` from the `agent_core` prelude (Task 1 already does), use the shorter `agent_core::ContextSource` / `agent_core::ContextUsage` form instead of the full module path.

- [ ] **Step 4: Regenerate types**

Run: `just gen-types`
Expected: `apps/agent-gui/src/generated/events.ts` and `apps/agent-gui/src/generated/commands.ts` update. Diff should show:

- New exports: `ContextUsage`, `ContextSource`, `ModelLimits`, `LimitSource`
- `EventPayload` `ContextAssembled` variant changed from `{ token_estimate: number; sources: string[] }` to `{ usage: ContextUsage }`

- [ ] **Step 5: Verify type-sync gate**

Run: `just check-types`
Expected: PASS (the gate passes only if `gen-types` produced no further diff after Step 4 — i.e., the regen is stable).

- [ ] **Step 6: Run the full local verification suite**

```bash
pnpm run format:check
pnpm run lint
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
pnpm --filter agent-gui exec vitest run
just test-e2e
```

Expected: all green.

If the e2e suite fails on a `ContextAssembled`-related spec, the cause is almost certainly that `apps/agent-gui/e2e/tauri-mock.js:328-333` was not updated. Task 7 Step 4 (G) gives the exact replacement — re-apply if missed.

- [ ] **Step 7: Commit + push branch**

```bash
git add apps/agent-gui/src-tauri/src/specta.rs \
        apps/agent-gui/src-tauri/Cargo.toml \
        crates/agent-models/Cargo.toml \
        apps/agent-gui/src/generated/events.ts \
        apps/agent-gui/src/generated/commands.ts
git commit -m "feat(gui): expose ContextUsage and ModelLimits to TypeScript via specta"
git push -u origin feat/context-p1-model-window-metadata
```

Open a PR titled `feat(runtime): per-model context window + budget-driven assembly (P1 of context-mgmt)`. P2 starts from this branch's HEAD after merge.

---

## Self-review (completed by plan author — record in PR description)

- [x] **Spec coverage**: §4.1 (registry + Ollama probe + `Option<u64>` ProfileDef) → Tasks 2–4. §4.2 (ContextBudget + ContextAssembler refactor + `ContextSource::ToolDefinitions`) → Tasks 1, 5, 6. §4.3 (agent_loop integration + real-usage correction) → Tasks 9, 10. §5 (`ContextAssembled` payload upgrade) → Task 7. Per-session `UsageCorrector` + `last_estimated_tokens` book-keeping → Task 8. Specta type generation → Task 11.
- [x] **Placeholder scan**: zero `TBD`, zero `// TODO`, zero "implement later", zero "similar to Task N". Each step shows the exact code or exact file path + line number to edit. The two remaining string matches for `init_session` / `get_ollama_client` / `self.clone()` / `build_test_runtime` are explanatory notes documenting WHY we do NOT use those (verified — see lines 68 explaining we use `start_session` not `init_session`; line 1926 explaining why `ModelRouter::get_ollama_client` is rejected; line 2136 explaining why we capture `session_states.clone()` instead of `self.clone()`; line 1586/2006 documenting `build_test_runtime` as a real per-test helper, not a virtual import).
- [x] **Type consistency**: `ModelLimits` defined in Task 2, consumed in Tasks 4, 8, 9, 10, 11. `ContextUsage` defined in Task 1, consumed in Tasks 6, 7, 9, 11. `ContextBudget::input_budget()` defined in Task 5, consumed in Task 9. `UsageCorrector::update / apply / samples / ratio` defined in Task 8, consumed in Tasks 9, 10. `SessionState { model_limits, usage_corrector, last_estimated_tokens }` defined in Task 8, consumed in Tasks 9, 10. `agent_config::resolve_limits(&ProfileDef) -> ModelLimits` defined in Task 4, consumed in Tasks 9, 10. `agent_models::lookup_limits(provider, model_id) -> ModelLimits` defined in Task 2, consumed in Task 9. `LocalRuntime::with_config / with_ollama_clients / set_session_limits / event_store_for_test` defined in Tasks 9 & 10, consumed across the integration tests. All consistent — no orphan symbol.
- [x] **Behaviour-at-commit-boundary**: every task ends with `cargo test` + `cargo build --workspace` green. Task 5 is annotated to leave the old `assemble()` body intact so `pre-commit` (which runs `cargo fmt`) does not reject the partial commit. Tasks 6 → 7 → 9 are atomic refactors that touch the same data flow but preserve workspace build at every commit.
- [x] **Out of scope**: compaction (P2), UI rendering (P3), profile switching (P4) — none touched here. The `CompactionSummary` variant on `ContextSource` is added now (Task 1) so P2 can land without re-bumping `ContextSource`.
