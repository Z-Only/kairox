# Session Context & Model Management — Design Spec

> **Status:** Draft (awaiting user approval)
> **Date:** 2026-05-08
> **Author:** Aone Copilot (with user `蝉雨`)
> **Companion mockup:** `.superpowers/brainstorm/context-meter-mockup.html`
> **Implementation plans (to be authored next):**
> `docs/superpowers/plans/2026-05-08-context-p1-model-window-metadata.md`
> `docs/superpowers/plans/2026-05-08-context-p2-compaction.md`
> `docs/superpowers/plans/2026-05-08-context-p3-ui-context-meter.md`
> `docs/superpowers/plans/2026-05-08-context-p4-switch-model.md`

---

## 1. Goals & Non-Goals

### 1.1 Goals

1. **Per-model context windows.** Every session knows the real `context_window` and `output_limit` of its current model profile, derived from a layered fallback (TOML > built-in registry > runtime probe).
2. **Budget-driven assembly.** Each agent-loop iteration assembles a `ModelRequest` whose token estimate stays within the session's effective budget; nothing else in the pipeline silently truncates.
3. **Manual + automatic compaction.** Users can compact a session on demand. The runtime auto-compacts when usage crosses a configurable threshold (default 85%). Compaction is an LLM-driven summarisation with a sliding-window fallback. While compaction runs the session is busy and `send_message` is rejected.
4. **First-class observability.** Both GUI and TUI display live context usage with a per-source breakdown (system / MCP tools / memory / history / tool results / selected files / reserved-for-response).
5. **Mid-session model switching.** Users may switch the active model profile at any time; the switch takes effect on the next user turn (current streaming is not interrupted).

### 1.2 Non-Goals (this iteration)

- Hierarchical / multi-tier summarisation (one-shot summarisation only).
- Vector / embedding-based retrieval (`agent-memory` already does keyword recall; semantic memory is out of scope).
- Anthropic native `count_tokens` API integration (cl100k_base estimate everywhere; usage corrected from real `ModelUsage` after each call).
- Pause/resume of a running compaction.
- Per-tool token cost attribution finer than "all MCP definitions" (the bundle is one row).
- Compaction audit/edit UI ("show what was summarised away" beyond the raw event in the trace timeline).

### 1.3 Success Criteria

- A session with `claude-sonnet-4` shows `200k` budget; with `gpt-3.5-turbo` shows `16k`; with an Ollama model whose `/api/show` reports `8192` shows `8.2k`. Source label (`UserConfig` / `BuiltinRegistry` / `RuntimeProbe`) is visible in the popover.
- After ~150 turns on a 200k-window model, automatic compaction fires once usage crosses 85%; the next request fits under budget; the chat transcript still renders the original messages (with a visible `[Compacted N events]` divider).
- Pressing **Compact** in GUI / `:compact` in TUI works at any time the session is idle.
- Switching profile from `fast` to `local-code` mid-session emits one `ModelProfileSwitched` event; the _next_ user message uses the new profile while the in-flight stream finishes on the old one.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes; `pnpm run lint` passes; new event types appear in `apps/agent-gui/src/generated/events.ts`.

---

## 2. Background — current state (verified by code reading)

| Concern               | Where it lives today                                                                                                                                                                   | Gap                                                                                                               |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Model window metadata | `crates/agent-config/src/lib.rs` `ProfileDef.context_window` (default 128_000) and `output_limit` (default 16_384).                                                                    | Identical defaults for every provider; no source-of-truth per real model; no runtime probe.                       |
| Context assembly      | `crates/agent-memory/src/context.rs` `ContextAssembler::assemble`, drops lowest-priority sources when `total > max_tokens`.                                                            | Not invoked from `agent_loop`. Has no notion of "MCP tool definitions". Tokenizer is hard-coded to `cl100k_base`. |
| Agent loop messages   | `crates/agent-runtime/src/agent_loop.rs` `build_model_messages` replays the _entire_ session_events into `ModelMessage`s every iteration.                                              | No budget enforcement; history grows unbounded until provider rejects.                                            |
| Compaction            | None.                                                                                                                                                                                  | All four sub-features (manual, auto, busy gate, fallback) need to be added.                                       |
| Model selection       | `EventPayload::SessionInitialized { model_profile }`; `agent_loop` resolves profile from that event once.                                                                              | No way to change profile after session start.                                                                     |
| Token usage UI        | `EventPayload::ContextAssembled { token_estimate, sources }` is emitted but never assembled. The trace timeline shows it as a row; ChatPanel and TUI StatusBar don't show live budget. | Need projection field + dedicated UI component.                                                                   |
| Real usage feedback   | `ModelEvent::Completed { usage: Option<ModelUsage> }` is wired through OpenAI/Anthropic adapters.                                                                                      | Currently swallowed (the runtime ignores `usage`).                                                                |

---

## 3. Architecture overview

```
                                                 ┌──────────────────────────┐
   kairox.toml [profiles.*]                       │   model_registry         │
   [context] auto_compact_threshold=0.85          │ (built-in const table)   │
              compactor_profile="fast"            └─────────────┬────────────┘
                       │                                        │
                       ▼                                        ▼
              ┌──────────────────────┐  resolve()    ┌──────────────────────┐
              │  agent-config        │──────────────▶│  ModelLimits         │
              │   ProfileDef (+ctx,  │               │  { ctx, out, source }│
              │   compactor cfg)     │               └──────────┬───────────┘
              └──────────┬───────────┘                          │
                         │ (ContextPolicy)                      │
                         ▼                                      │
              ┌──────────────────────┐                          │
              │  agent-memory        │                          │
              │   ContextAssembler   │◀─────────────────────────┘
              │   ContextBudgeter    │   ┌──────────────────────┐
              │   Compactor          │──▶│  ContextUsage        │ (snapshot,
              │   (LLM + fallback)   │   │  by source           │  emitted via
              └──────────┬───────────┘   └──────────────────────┘  ContextAssembled)
                         │
                         ▼
              ┌─────────────────────────────────────────────────────────────┐
              │  agent-runtime                                              │
              │   agent_loop ─ uses ContextAssembler each iteration         │
              │   compaction_manager ─ triggers manual/auto compaction      │
              │   session_state.compacting / model_profile (latest wins)    │
              │   facade_runtime.compact_session() / switch_model()         │
              │   send_message() rejects when compacting                    │
              └──────────┬──────────────────────────────────────────────────┘
                         │ DomainEvents
       ┌─────────────────┴────────────────┐
       ▼                                  ▼
┌──────────────────┐               ┌──────────────────┐
│ agent-tui        │               │ apps/agent-gui   │
│  StatusBar       │               │  ContextMeter    │
│  shows ContextU. │               │  Popover         │
│  :compact cmd    │               │  ModelSwitcher   │
└──────────────────┘               └──────────────────┘
```

### 3.1 Crate-level responsibilities (deltas)

- **agent-core (P1, P2, P3, P4)** — new domain types `ModelLimits`, `ContextUsage`, `ContextSource` (enum), `CompactionReason`, `CompactionStatus`. New event variants (see §5). Projection gains `model_profile: String`, `model_limits: ModelLimits`, `last_context_usage: Option<ContextUsage>`, `compaction: CompactionStatus`.
- **agent-config (P1, P2, P4)** — `ProfileDef` keeps existing fields (back-compat). New `[context]` TOML section parsed into `ContextPolicy { auto_compact_threshold: f32, compactor_profile: Option<String>, max_tool_definition_tokens: Option<u64> }`.
- **agent-models (P1)** — new `model_registry.rs` with `ModelInfo` const table + `lookup(provider, model_id)`. `OllamaClient` gains `probe_context_window(model_id)` async method. Existing `ModelClient` trait unchanged.
- **agent-memory (P1, P2)** — `ContextAssembler` refactor: takes `ContextBudget` (struct with per-source quotas) instead of bare `max_tokens`; returns a richer `ContextBundle` carrying `ContextUsage`. New `ContextSource::ToolDefinitions`. New `compactor.rs` providing `Compactor::compact_with_llm` and `Compactor::sliding_window_fallback`.
- **agent-runtime (P1, P2, P4)** — `agent_loop` rewritten to use `ContextAssembler` each iteration and emit `ContextAssembled { usage }`. New module `compaction.rs` orchestrating triggers, busy state, and event emission. `LocalRuntime` gains `compact_session(SessionId, CompactionReason)` and `switch_model(SessionId, profile_alias)` facade methods. `send_message` checks `session_state.compacting` and returns `CoreError::SessionBusy`.
- **agent-tui (P3, P4)** — `StatusBar` rendering of `ContextUsage` & profile name. `:compact` and `:model <alias>` commands.
- **apps/agent-gui (P3, P4)** — new component `ContextMeter.vue` (with embedded popover) injected at the top of `ChatPanel.vue`. New Pinia store fields. New Tauri commands `compact_session`, `switch_model`, `list_profiles_with_limits`. Browser-side `tauri-mock.js` updated.

### 3.2 Dependency direction (unchanged, no reverse deps)

`agent-core` ← `agent-config` ← `agent-models` ← `agent-memory` ← `agent-runtime` ← `agent-tui` / `apps/agent-gui`. The new code sits inside existing crate boundaries — **no new crate is created**.

---

## 4. Detailed design — module by module

### 4.1 Model window metadata (P1)

**`crates/agent-models/src/model_registry.rs` (new):**

```rust
/// Source of a `ModelLimits` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LimitSource {
    UserConfig,      // explicit context_window in kairox.toml
    BuiltinRegistry, // matched the const table in this module
    RuntimeProbe,    // fetched from provider (currently Ollama /api/show)
    Fallback,        // none of the above; conservative default
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ModelLimits {
    pub context_window: u64,
    pub output_limit: u64,
    pub source: LimitSource,
}

/// Static descriptor used inside the const registry.
struct ModelInfo {
    pattern: &'static str, // "gpt-4o", "claude-sonnet-4", "claude-3-5*", ...
    context_window: u64,
    output_limit: u64,
}

/// Per-provider entries. Ordering matters: the first match wins.
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

const FALLBACK_OLLAMA: ModelLimits =
    ModelLimits { context_window: 8_192, output_limit: 2_048, source: LimitSource::Fallback };

const FALLBACK_FAKE: ModelLimits =
    ModelLimits { context_window: 4_096, output_limit: 2_048, source: LimitSource::Fallback };

const FALLBACK_GENERIC: ModelLimits =
    ModelLimits { context_window: 128_000, output_limit: 16_384, source: LimitSource::Fallback };

/// Look up the built-in limits for a (provider, model_id) pair.
/// Matching: longest pattern-prefix-of `model_id` wins. Returns
/// a provider-specific fallback when no entry matches.
pub fn lookup(provider: &str, model_id: &str) -> ModelLimits { /* ... */ }
```

**Resolution function** (lives in `agent-config::builder` so it can read `ProfileDef`):

```rust
pub fn resolve_limits(profile: &ProfileDef) -> ModelLimits {
    // 1. Honour explicit user config when it's not the legacy default.
    //    The legacy default for every profile is (128_000, 16_384). To distinguish
    //    "user wrote 128_000" from "user omitted the field" we change ProfileDef
    //    to use Option<u64> and only honour Some(...) here.
    if let (Some(ctx), Some(out)) = (profile.context_window, profile.output_limit) {
        return ModelLimits { context_window: ctx, output_limit: out, source: LimitSource::UserConfig };
    }
    // 2. Built-in registry.
    let from_table = agent_models::model_registry::lookup(&profile.provider, &profile.model_id);
    if from_table.source != LimitSource::Fallback {
        return from_table;
    }
    // 3. Caller may run a RuntimeProbe (only Ollama; see OllamaClient::probe_context_window)
    //    and overwrite this result before storing it on the session.
    from_table
}
```

> **Migration note.** `ProfileDef.context_window: u64` becomes `Option<u64>` (same for `output_limit`). The TOML `#[serde(default = "...")]` is dropped. Existing config files keep working — omitted fields now resolve through the registry instead of silently using `128_000`. We update `kairox.toml.example` and `Config::defaults()` accordingly.

**Ollama runtime probe** lives in `crates/agent-models/src/ollama.rs`:

```rust
impl OllamaClient {
    /// POST /api/show with {"name": model_id} — read model_info."<arch>.context_length".
    /// Returns None on transport / parse error so callers can fall back gracefully.
    pub async fn probe_context_window(&self, model_id: &str) -> Option<u64> { /* ... */ }
}
```

The probe is fired **once per session** by `LocalRuntime` after a session initialises with an Ollama profile, and the result is cached in `session_state.model_limits`. Probe failures simply leave the registry/fallback value in place; they never block the session.

### 4.2 Context budgeter & assembler refactor (P1)

**`crates/agent-memory/src/context.rs` rework:**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    System,
    ToolDefinitions, // NEW — MCP / built-in tool schemas
    Request,
    Memory,
    History,
    ToolResult,
    SelectedFile,
    CompactionSummary, // NEW — produced by Compactor
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ContextUsage {
    pub total_tokens: u64,
    pub budget_tokens: u64,        // = context_window - output_reservation
    pub context_window: u64,
    pub output_reservation: u64,
    pub by_source: Vec<(ContextSource, u64)>,
    pub estimator: &'static str,   // "cl100k_base" today
    pub corrected_by_real_usage: bool, // true after first ModelUsage feedback
}

#[derive(Debug, Clone)]
pub struct ContextBudget {
    pub context_window: u64,
    pub output_reservation: u64,
    /// Optional per-source soft caps (e.g. cap MCP tool defs at 25k).
    pub source_caps: Vec<(ContextSource, u64)>,
}
```

`ContextAssembler::assemble` keeps the same drop-lowest-priority semantics, but now (a) accepts `tool_definitions: Vec<ToolDefinition>` (already serialised JSON, counted as one bundle), (b) accepts `compaction_summary: Option<String>` to insert at history's head, and (c) returns a `ContextBundle` whose `usage: ContextUsage` is later emitted as `EventPayload::ContextAssembled { usage }`.

**Token estimator behaviour.** MVP: `cl100k_base` for _all_ providers. The runtime keeps a per-session `usage_correction_ratio: f32` (initially `1.0`); when `ModelEvent::Completed { usage: Some(real) }` arrives it computes `ratio = real.input_tokens as f32 / last_estimated as f32`, clamps to `[0.7, 1.5]`, and EMA-smooths it. The next assembly multiplies the per-section estimate by `ratio` before summing. `corrected_by_real_usage = ratio != 1.0`.

### 4.3 agent-loop integration (P1)

`crates/agent-runtime/src/agent_loop.rs` changes:

1. At iteration start, construct a `ContextRequest` (system prompt, MCP tool defs JSON, memory entries, session history reduced to messages, latest user_request, pending tool_results).
2. Call `assembler.assemble(request, budget)` and emit `EventPayload::ContextAssembled { usage }` (replacing the current empty stub).
3. Build the `ModelRequest` from the assembled bundle, **not** from raw `session_events.iter()`.
4. After `ModelEvent::Completed { usage }`, persist the corrected ratio and (in P2) trigger an auto-compaction check.

`build_model_messages` becomes a private helper that operates on the assembled bundle, not on the raw event log. The previous "replay every event" path is kept _only_ as a fallback used by `Compactor::sliding_window_fallback`.

### 4.4 Compaction (P2)

**Trigger paths:**

- **Manual:** `LocalRuntime::compact_session(session_id, CompactionReason::UserRequested)` from a Tauri command / TUI `:compact`.
- **Automatic:** after every `ContextAssembled` event with `usage.total_tokens >= threshold * usage.budget_tokens`, the runtime fires `compact_session(session_id, CompactionReason::Threshold { ratio })`.

**State machine** (per session, in-memory `Mutex<SessionState>`):

```
        ┌──────────┐  compact_session()    ┌────────────┐
        │  Idle    │──────────────────────▶│ Compacting │
        └────┬─────┘                       └──────┬─────┘
             │  send_message()                    │  CompactionCompleted
             │  ────────────▶ Idle                │  CompactionFailed
             │                                    ▼
             │                              ┌──────────┐
             │  send_message() while busy   │   Idle   │
             │  ──────▶ Err(SessionBusy) ◀──┴──────────┘
```

**Algorithm:**

1. Load `session_events`, locate the boundary: keep the last K=6 user/assistant messages (3 pairs) + every event after them. Everything strictly before that index is a _compaction candidate range_ `[first_id ..= last_id]`.
2. Render the candidate range into a transcript (markdown with role tags + tool-call summaries).
3. Build a `ModelRequest` against `ContextPolicy.compactor_profile` (or session's current profile if unset). Prompt template (ships in `agent-memory/src/compactor_prompt.txt`):

   ```
   You are summarising a developer-AI conversation so it fits a smaller context.
   Output FOUR markdown sections, no preamble:

   ## User goal
   ## Key decisions & constraints
   ## Tool calls executed and their outcomes
   ## Open questions / pending work
   ```

4. Append a `EventPayload::CompactionSummary { summary_id, content, replaces_event_range, reason, before_tokens, after_tokens, summarised_by_profile }` event. The event is `MinimalTrace` (no full message bodies).
5. The next `ContextAssembler::assemble` call automatically picks up the summary because `build_model_messages` now treats events as: "if a `CompactionSummary` exists, skip every event whose id is within `replaces_event_range` and inject the summary as a single system-tagged message at that position."
6. On LLM failure (3 retries with exponential backoff), invoke `Compactor::sliding_window_fallback` which produces a _synthetic_ `CompactionSummary` containing only `[Dropped N earlier turns by sliding window]`. Emit `EventPayload::CompactionFailed { error }` _and_ `CompactionCompleted { fallback_used: true }` so the UI can show a degraded state.

**Busy gate:** `LocalRuntime::send_message` checks `session_state.compacting`. If true it returns `Err(CoreError::SessionBusy)` with a clear message; the GUI surfaces this as a toast and disables the send button.

**New events (additions to `EventPayload`):**

```rust
ContextCompactionStarted {
    reason: CompactionReason,         // UserRequested | Threshold { ratio: f32 }
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
    fallback_used: bool,              // true if sliding-window kicked in
},
CompactionSummary {
    summary_id: String,
    content: String,
    replaces_event_range: (DateTime<Utc>, DateTime<Utc>), // first..=last timestamp inclusive
    reason: CompactionReason,
    before_tokens: u64,
    after_tokens: u64,
    summarised_by_profile: String,
},
```

`replaces_event_range` is timestamps (not event ids) because `DomainEvent` currently has no stable id — using timestamps avoids a schema change to the event envelope.

### 4.5 Mid-session model switch (P4)

```rust
// agent-core
ModelProfileSwitched {
    from_profile: String,
    to_profile: String,
    effective_at: DateTime<Utc>,
    new_limits: ModelLimits,
},
```

`LocalRuntime::switch_model(session_id, alias)`:

1. Validates `alias` exists in the current `Config`.
2. Refuses if `session_state.compacting` (returns `Err(SessionBusy)`).
3. Records the event. Does **not** cancel any in-flight stream — the existing iteration finishes with the old model.
4. The next `agent_loop` iteration reads `latest_model_profile_for(session)` which scans events from newest to oldest looking for the most recent `ModelProfileSwitched` (falling back to `SessionInitialized.model_profile`).
5. The session's cached `model_limits` is recomputed from the new profile (registry + Ollama probe if applicable).

**Streaming-vs-switch semantics:** because `agent_loop` resolves the model at the _top_ of each iteration but a single `send_message` call can run many iterations until tool calls drain, "next user turn" effectively means "next time `send_message` triggers a fresh loop". This matches the `effective_after_event_id` spirit of the user's choice without requiring iteration-level cutover (which would introduce mid-stream provider differences and is fragile).

### 4.6 UI layer (P3, P4)

#### GUI

- **New component** `apps/agent-gui/src/components/ContextMeter.vue`:
  - Compact bar at the top of `ChatPanel.vue` (replaces nothing; sits above the message list).
  - Reads `session.lastContextUsage` and `session.modelLimits` from the session store.
  - Displays segmented bar (System / ToolDefs / Memory / History / ToolResult / SelectedFile / CompactionSummary) using CSS custom properties from `theme.css` (we add `--src-system`, `--src-tools`, etc.).
  - Status badges: `>=70%` warn, `>=85%` err, `compacting` busy with pulsing dot, `compaction-failed` warn-with-icon.
  - Compact button → `invoke("compact_session")`. Disabled while busy.
  - Click body → toggles `<dialog>`-less popover (absolute-positioned div) with the per-source breakdown table + "Reserved for response" row + actions row containing **Switch model…** button (P4) and **Compact now**.
- **New Tauri commands** (`#[specta::specta]`, registered in both `generate_handler!` and `collect_commands!`):
  - `compact_session(session_id) -> Result<()>` (P2)
  - `switch_model(session_id, profile_alias) -> Result<()>` (P4)
  - `list_profiles_with_limits() -> Vec<ProfileWithLimits>` (P1; used by the model switcher dropdown)
- **Pinia store fields** added to `useSessionStore`:
  - `lastContextUsage: ContextUsage | null`
  - `modelLimits: ModelLimits | null`
  - `currentProfile: string`
  - `compacting: boolean` (toggled on `ContextCompactionStarted/Completed/Failed`)
  - `lastCompactionError: string | null`
- **i18n keys** added to BOTH `en.json` and `zh-CN.json`:
  ```
  context.title, context.estimated, context.compactNow, context.busy,
  context.switchModel, context.reservedForResponse, context.failedFallback,
  status.compacting, status.contextNearFull, errors.sessionBusy
  ```
- **E2E test mock** (`apps/agent-gui/e2e/tauri-mock.js`) gets handlers for `compact_session`, `switch_model`, `list_profiles_with_limits`, and emits the new event payloads.

#### TUI

- `crates/agent-tui/src/components/status_bar.rs` (new file; current StatusBar lives inline in `app.rs`) renders one line:
  ```
  profile: <name>  perm: <mode>  ctx: 12.3k/200k [▓▓▓░░░░] sys 2k tools 22k mem 9k hist 64k tres 13k ~est
  ```
  Falls back to short form `ctx: 152k/200k (76%) ⚠` when terminal width < 100 cols.
- New keybindings parsed by `chat.rs` input layer:
  - `:compact` → `Command::CompactSession`
  - `:model <alias>` → `Command::SwitchModel(alias)`
  - Both validated against current `Config::profile_names()`.
- `App::handle_command` dispatches to the new facade methods. Errors are surfaced in the existing notification area.

### 4.7 Configuration surface

`kairox.toml.example` adds:

```toml
[context]
# When the assembled context reaches this fraction of the budget, the runtime
# triggers automatic compaction. Set to 1.0 to disable auto-compaction.
auto_compact_threshold = 0.85

# Optional: profile alias to use for the summarisation LLM call. When unset,
# the session's currently active profile is used. Useful to point at a
# cheap fast model (e.g. "fast") even when the session runs on "claude-opus".
# compactor_profile = "fast"

# Optional: cap on MCP tool definitions tokens. When the serialised tool
# schemas exceed this, the assembler drops the *lowest priority* tools first
# (alphabetical by server_id, currently — future work to support explicit
# pinning).
# max_tool_definition_tokens = 25_000
```

`ProfileDef` keeps `context_window` / `output_limit` (now `Option<u64>`) for explicit overrides:

```toml
[profiles.local-llama]
provider = "ollama"
model_id = "llama3:8b"
base_url = "http://localhost:11434"
context_window = 8192   # explicit; skips registry & probe
output_limit   = 2048
```

---

## 5. Event additions (single source of truth)

| Variant                                                                                                                       | Privacy | Emitted from              | Consumed by                                                              |
| ----------------------------------------------------------------------------------------------------------------------------- | ------- | ------------------------- | ------------------------------------------------------------------------ |
| `ContextCompactionStarted { reason, before_tokens, candidate_event_count }`                                                   | Minimal | runtime/compaction.rs     | TUI StatusBar, GUI ContextMeter (sets `compacting=true`), trace timeline |
| `ContextCompactionCompleted { summary_id, after_tokens, fallback_used }`                                                      | Minimal | runtime/compaction.rs     | sets `compacting=false`, refreshes ContextUsage                          |
| `ContextCompactionFailed { error, fallback_used }`                                                                            | Minimal | runtime/compaction.rs     | toast / TUI notification                                                 |
| `CompactionSummary { summary_id, content, replaces_event_range, reason, before_tokens, after_tokens, summarised_by_profile }` | Minimal | runtime/compaction.rs     | `build_model_messages` (substitution), trace timeline shows fold         |
| `ModelProfileSwitched { from_profile, to_profile, effective_at, new_limits }`                                                 | Minimal | runtime/facade_runtime.rs | StatusBar profile label, ContextMeter limits                             |

`EventPayload::ContextAssembled` gains a richer payload (breaking the existing payload — but the variant is brand-new in production use):

```rust
ContextAssembled {
    usage: ContextUsage,           // replaces (token_estimate, sources)
}
```

The existing GUI/TUI consumers are updated in P3.

---

## 6. Testing strategy

Per the project's `AGENTS.md`:

- **Unit tests** in each crate — `model_registry::lookup` table coverage, `Compactor` boundary picking, `ContextBudget` truncation invariants, `LimitSource` precedence in `resolve_limits`.
- **Integration tests** in `crates/agent-runtime/tests/`:
  - `context_budget.rs` (P1) — feeds 100 fake messages, asserts each iteration's request stays within budget.
  - `compaction.rs` (P2) — uses `FakeModelClient` programmed to return a known summary; asserts `CompactionSummary` emitted, `build_model_messages` substitutes correctly, second `send_message` while compacting returns `SessionBusy`.
  - `model_switch.rs` (P4) — switches profile mid-stream; asserts no event reorder, next iteration uses new profile.
- **Vitest** in `apps/agent-gui` — `ContextMeter.test.ts` covers all three states (healthy/warn/compacting) and popover content; `useSessionStore` tests for the new event handlers.
- **E2E (Playwright)** — `apps/agent-gui/e2e/context-meter.spec.ts` (new) drives `tauri-mock` to push synthetic `ContextAssembled` and `ContextCompaction*` events and verifies the UI.
- **TUI integration** — `crates/agent-tui/tests/app_logic.rs` gets cases for `:compact` and `:model` commands.

CI gate before any plan can be marked green: `pnpm run format:check && pnpm run lint && cargo test --workspace --all-targets && just check-types`.

---

## 7. Migration & backward compatibility

- **Sessions persisted before this change** still work: missing `ContextAssembled.usage` payloads project to `last_context_usage = None` and the meter shows `?? / 200k (—)` until the next iteration emits a fresh event.
- **TOML files without `[context]`** behave as if `auto_compact_threshold = 0.85` and `compactor_profile = None`.
- **`ProfileDef` field type change** (`u64 → Option<u64>`) requires updating every constructor in the codebase; `serde(default)` is removed so omitted fields parse as `None` (which then resolves through the registry).
- **Event schema:** new variants are additive to a `#[serde(tag = "type")]` enum, which is non-breaking for serialised JSON. The `ContextAssembled` payload shape changes — but in production today no consumer relies on the old shape (only the trace timeline displays it as text).

---

## 8. Risks & mitigations

| Risk                                                                                                     | Mitigation                                                                                                                                                                                                     |
| -------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| LLM summary loses critical context (tool outputs, decisions)                                             | Prompt template explicitly enumerates four categories; integration test asserts a known input → known summary; user can always inspect `CompactionSummary` event content.                                      |
| `cl100k_base` underestimates Claude tokens by ~15-20%, leading to over-budget requests that get rejected | Real-usage feedback (`ratio` correction) kicks in after the first request; `output_reservation` defaults to `output_limit + 10% safety margin`; provider rejection is caught and surfaces a clear error toast. |
| Auto-compaction races with a long tool-call chain                                                        | The trigger evaluates _after_ a full agent-loop iteration ends; mid-iteration we never compact. If the user sends a new message _exactly_ when the trigger fires, busy-gate handles it.                        |
| Ollama `/api/show` blocks session start                                                                  | Probe is fire-and-forget after session init, with a 3 s timeout; fallback value is used immediately.                                                                                                           |
| GUI ContextMeter visual noise distracts from chat                                                        | Bar is 6px tall with grey segments at <50% — visually quiet. Popover only appears on click.                                                                                                                    |
| Switching model mid-task breaks tool-call format compatibility (e.g. Anthropic ↔ OpenAI tool_use blocks) | Switch only takes effect at next user turn (a fresh request boundary), so the in-flight assistant message keeps its provider's format end-to-end.                                                              |

---

## 9. Out-of-scope follow-ups (tracked, not built now)

1. Anthropic native `count_tokens` API for exact pre-flight estimates.
2. Hierarchical compaction (multi-tier summaries) for sessions > 1M cumulative tokens.
3. Per-MCP-server pinning ("never drop GitHub MCP tools").
4. Interactive "Edit summary" UI before accepting an LLM-produced compaction.
5. Cross-provider token estimators (anthropic-tokenizer, llama-tokenizer crates).
6. `Switch model` from a settings keyboard shortcut (not just the popover button).

---

## 10. Implementation plan map

| Plan                                               | Crates touched                                                                                                                     | Output is independently testable?                                    |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| **P1 — Model window metadata & budgeted assembly** | agent-config, agent-models, agent-memory, agent-runtime, agent-core (ContextUsage type)                                            | ✅ Sessions on real models show correct limits; no UI needed.        |
| **P2 — Compaction (manual + auto + busy gate)**    | agent-core (events), agent-memory (compactor.rs), agent-runtime (compaction.rs, facade)                                            | ✅ Verified via integration tests + TUI `:compact` (UI lands in P3). |
| **P3 — GUI/TUI context observability**             | agent-core (projection fields), apps/agent-gui (ContextMeter + commands + i18n + e2e), agent-tui (status_bar)                      | ✅ Visible meter on a fake session in dev mode.                      |
| **P4 — Mid-session model switch**                  | agent-core (event), agent-runtime (facade + agent_loop resolution), apps/agent-gui (switcher in popover), agent-tui (`:model` cmd) | ✅ End-to-end switch test with two profiles.                         |

Each plan starts from `main` on its own branch (`feat/context-p1-...`, etc.) and merges sequentially because P2 depends on P1's `ContextUsage`, P3 depends on P1+P2 events, P4 depends on P1's `ModelLimits`.

---

## 11. Open questions for user review

If anything in this doc still needs to change, surface it now — the next step is to write four detailed implementation plans with TDD-grained tasks, after which we go heads-down.

1. Are the **default thresholds** (auto 0.85, K=6, fallback after 3 LLM retries) acceptable? _(default answer per Q&A: yes)_
2. Is changing `ProfileDef.context_window` from `u64` to `Option<u64>` acceptable as a config-loading-internal change? _(it is API-internal — `kairox.toml` syntax doesn't change.)_
3. The `CompactionSummary` event uses **timestamps** (not event ids) for `replaces_event_range` because `DomainEvent` has no stable id. OK to live with this, or should we add an event-id field as part of P2? _(default: live with timestamps.)_
4. Should the **Switch model** dropdown live inside the ContextMeter popover (current plan) or also in the global settings page? _(current plan: only in the popover; settings can come later.)_
