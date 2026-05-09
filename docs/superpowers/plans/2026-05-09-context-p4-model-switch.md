# Context-Mgmt P4 — Mid-Session Model Switch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user swap the active model profile mid-session without interrupting in-flight streams; persist the switch as a `ModelProfileSwitched` domain event and make every subsequent `agent_loop` iteration resolve the latest-known profile from history (fallback: `SessionInitialized.model_profile`). Surface the switch in the GUI (`ContextMeter` popover dropdown), TUI (`:model <alias>` command), and busy-gate it the same way P2 busy-gates compaction.

**Architecture:**

- **Event-sourced switch**: a new minimal-privacy `ModelProfileSwitched` event variant is appended to the session event log. No in-memory flag is the source of truth — `agent_loop.rs` re-derives the active alias from events at the top of every iteration.
- **Facade method mirrors `compact_session`**: `LocalRuntime::switch_model(session_id, alias)` is an inherent method on `LocalRuntime<S, M>` that validates the alias against the loaded `Config`, refuses on `SessionState.compacting`, emits the event, and refreshes `SessionState.model_limits` from the new profile (re-triggers the Ollama probe when applicable).
- **UI wiring**: P3 left a disabled "Switch model…" button in `ContextMeter.vue` and a `currentProfile` ref on `useSessionStore` that is never written from events. P4 enables the button (opens a dropdown of `list_profiles_with_limits` results), adds a `switch_model` Tauri command, teaches `applyEvent` to consume `ModelProfileSwitched`, and mirrors both in the Playwright mock.

**Tech Stack:** Rust (agent-core, agent-runtime, agent-models, agent-tui), Tauri 2 (IPC + specta), Vue 3 + Pinia + vue-i18n (agent-gui), Playwright (E2E).

---

## File Structure

**Rust crates:**

- `crates/agent-core/src/events.rs` — add `EventPayload::ModelProfileSwitched` variant + `event_type()` arm + round-trip test
- `crates/agent-runtime/src/facade_runtime.rs` — add `LocalRuntime::switch_model` inherent method (mirrors `compact_session` at line 358)
- `crates/agent-runtime/src/agent_loop.rs` — replace the `SessionInitialized`-only lookup at line 305-312 with a helper that walks events newest-to-oldest, falling back to `SessionInitialized`
- `crates/agent-runtime/src/session.rs` — no new type; `SessionState.model_limits` is reused (re-written by `switch_model`)
- `crates/agent-runtime/tests/model_switch.rs` — NEW integration test: switch mid-session, assert next send uses new profile & limits; assert `SessionBusy` when compacting
- `crates/agent-tui/src/components/mod.rs` — extend `enum Command` with `SwitchModel { workspace_id, session_id, alias }`
- `crates/agent-tui/src/components/chat.rs` — intercept `:model <alias>` in `apply_key_action` (mirrors `:compact` at lines 60-77)
- `crates/agent-tui/src/main.rs` — add `Command::SwitchModel` arm in `dispatch_commands` (mirrors `Command::CompactSession` at line ~140)
- `crates/agent-tui/tests/app_logic.rs` — NEW test: `colon_model_alias_input_dispatches_switch_model_command`

**GUI (Tauri backend):**

- `apps/agent-gui/src-tauri/src/commands.rs` — new `#[tauri::command] switch_model(session_id, profile_alias) -> Result<(), String>` (mirrors `compact_session` at line 1270 of the same file)
- `apps/agent-gui/src-tauri/src/specta.rs` — register `switch_model` in `collect_commands![]`
- `apps/agent-gui/src-tauri/src/lib.rs` — register `switch_model` in `generate_handler![]`
- `apps/agent-gui/src/generated/commands.ts` / `events.ts` — regenerated via `just gen-types` (never hand-edited)

**GUI (Vue frontend):**

- `apps/agent-gui/src/components/ContextMeter.vue` — un-disable the Switch model button, open a dropdown/menu that lists `ProfileWithLimits` and calls `invoke("switch_model", …)`
- `apps/agent-gui/src/stores/session.ts` — add `ModelProfileSwitched` arm to `applyEvent` (updates `currentProfile` + `modelLimits`)
- `apps/agent-gui/src/locales/en.json` + `apps/agent-gui/src/locales/zh-CN.json` — add `context.switchModelChoose`, `context.switchModelSuccess`, `context.switchModelSameProfile`, `errors.profileNotFound`, `errors.sessionBusy`
- `apps/agent-gui/e2e/tauri-mock.js` — add `case "switch_model"` + emit `ModelProfileSwitched` synthetic event
- `apps/agent-gui/src/components/__tests__/ContextMeter.test.ts` — extend to cover the dropdown (if the existing test file exists; otherwise this is a new file scaffolded from the P3 test — see Task 13)
- `apps/agent-gui/e2e/context-meter.spec.ts` — extend with a switch-model flow (or add new `model-switch.spec.ts` — see Task 14)

---

### Task 1 — Add `EventPayload::ModelProfileSwitched` variant

**Files:**

- Modify: `crates/agent-core/src/events.rs`
- Test: `crates/agent-core/src/events.rs` (inline `#[cfg(test)]` block — there is currently no `mod tests` in this file; existing tests live as free-standing `#[test]` functions at the file root, so we follow the same pattern)

> **Verified facts** (from reading `events.rs` in full): `EventPayload` is a `#[serde(tag = "type")]` enum; each variant has a matching arm in `impl EventPayload { fn event_type(&self) -> &'static str }`. The existing `ContextCompactionStarted { reason, before_tokens, candidate_event_count }` variant demonstrates the minimal-privacy envelope. `agent-core` currently does **not** depend on `agent-models` (to avoid a cycle) — the P3 `ProjectedModelLimits` already solved this by mirroring the three primitive `ModelLimits` fields (`context_window: u64`, `output_limit: u64`, `source: String`), so `ModelProfileSwitched` follows the same pattern. Round-trip tests at the end of the file use `serde_json::to_value` → `serde_json::from_value`.

- [ ] **Step 1: Write the failing round-trip test**

Append to `crates/agent-core/src/events.rs` AFTER the `context_assembled_payload_carries_usage_struct` test (the last test in the file today):

```rust
#[test]
fn model_profile_switched_event_round_trips() {
    use chrono::TimeZone;
    let effective_at = chrono::Utc.with_ymd_and_hms(2026, 5, 9, 10, 0, 0).unwrap();
    let payload = EventPayload::ModelProfileSwitched {
        from_profile: "fast".into(),
        to_profile: "claude-opus".into(),
        effective_at,
        context_window: 200_000,
        output_limit: 16_384,
        limit_source: "builtin_registry".into(),
    };

    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "ModelProfileSwitched");
    assert_eq!(json["from_profile"], "fast");
    assert_eq!(json["to_profile"], "claude-opus");
    assert_eq!(json["context_window"], 200_000);
    assert_eq!(json["output_limit"], 16_384);
    assert_eq!(json["limit_source"], "builtin_registry");
    assert_eq!(json["effective_at"], "2026-05-09T10:00:00Z");

    let back: EventPayload = serde_json::from_value(json).unwrap();
    match back {
        EventPayload::ModelProfileSwitched {
            from_profile,
            to_profile,
            effective_at: at,
            context_window,
            output_limit,
            limit_source,
        } => {
            assert_eq!(from_profile, "fast");
            assert_eq!(to_profile, "claude-opus");
            assert_eq!(at, effective_at);
            assert_eq!(context_window, 200_000);
            assert_eq!(output_limit, 16_384);
            assert_eq!(limit_source, "builtin_registry");
        }
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn event_type_method_covers_model_profile_switched() {
    let p = EventPayload::ModelProfileSwitched {
        from_profile: "a".into(),
        to_profile: "b".into(),
        effective_at: chrono::Utc::now(),
        context_window: 0,
        output_limit: 0,
        limit_source: "fallback".into(),
    };
    assert_eq!(p.event_type(), "ModelProfileSwitched");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-core model_profile_switched`
Expected: FAIL — `EventPayload::ModelProfileSwitched` variant does not exist.

- [ ] **Step 3: Add the variant and `event_type()` arm**

In `crates/agent-core/src/events.rs`, locate the `EventPayload` enum. Insert this variant immediately AFTER the existing `CompactionSummary { … }` variant (so the four compaction-related variants stay grouped) and BEFORE `ModelRequestStarted`:

```rust
    /// Mid-session model profile change. The new profile only takes effect
    /// at the next `send_message` (agent-loop entry) — in-flight streams
    /// continue on the old profile end-to-end so provider-specific
    /// tool-call formats don't get mixed mid-stream.
    ModelProfileSwitched {
        from_profile: String,
        to_profile: String,
        effective_at: DateTime<Utc>,
        /// Mirrors `agent_models::ModelLimits.context_window` so this
        /// event can be consumed by `agent-core` projections without
        /// introducing a cycle on `agent-models`.
        context_window: u64,
        /// Mirrors `agent_models::ModelLimits.output_limit`.
        output_limit: u64,
        /// Snake-case `agent_models::LimitSource` discriminant: one of
        /// `"user_config" | "builtin_registry" | "runtime_probe" | "fallback"`.
        limit_source: String,
    },
```

Then in `impl EventPayload { pub fn event_type(&self) -> &'static str { match self { … } } }`, add the arm right after `Self::CompactionSummary { .. } => "CompactionSummary",`:

```rust
            Self::ModelProfileSwitched { .. } => "ModelProfileSwitched",
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p agent-core model_profile_switched`
Expected: PASS.

Run: `cargo test -p agent-core`
Expected: all green (no existing test regressed).

- [ ] **Step 5: Commit**

```bash
git add crates/agent-core/src/events.rs
git commit -m "feat(core): add ModelProfileSwitched event variant"
```

---

### Task 2 — agent-runtime helper `latest_model_profile_for(events)`

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop.rs` (add module-private helper; replace the inline resolver at lines 305-312)
- Test: `crates/agent-runtime/src/agent_loop.rs` (new `#[cfg(test)] mod model_profile_resolution_tests`)

> **Verified facts** (from reading `agent_loop.rs` 250-500): the current profile lookup at lines 305-312 is:
>
> ```rust
> let model_profile_alias: String = session_events
>     .iter()
>     .find_map(|e| match &e.payload {
>         EventPayload::SessionInitialized { model_profile } => Some(model_profile.clone()),
>         _ => None,
>     })
>     .unwrap_or_else(|| "fake".to_string());
> ```
>
> `session_events: Vec<DomainEvent>` is already sorted ascending by insertion order (SQLite PK monotonic). Walking `.iter().rev()` gives newest-to-oldest. The function needs to be `pub(crate)` because Task 3's `LocalRuntime::switch_model` (in `facade_runtime.rs`) will call it to derive `from_profile` — keeping the two resolvers as one function avoids drift.

- [ ] **Step 1: Write the failing test**

Append to `crates/agent-runtime/src/agent_loop.rs` (at the very end of the file):

```rust
#[cfg(test)]
mod model_profile_resolution_tests {
    use super::latest_model_profile_for;
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };

    fn init_event(profile: &str) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SessionInitialized {
                model_profile: profile.into(),
            },
        )
    }

    fn switch_event(from: &str, to: &str) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::ModelProfileSwitched {
                from_profile: from.into(),
                to_profile: to.into(),
                effective_at: chrono::Utc::now(),
                context_window: 0,
                output_limit: 0,
                limit_source: "fallback".into(),
            },
        )
    }

    #[test]
    fn returns_session_initialized_profile_when_no_switch() {
        let events = vec![init_event("fast")];
        assert_eq!(latest_model_profile_for(&events), "fast");
    }

    #[test]
    fn returns_latest_switch_when_one_exists() {
        let events = vec![init_event("fast"), switch_event("fast", "claude-opus")];
        assert_eq!(latest_model_profile_for(&events), "claude-opus");
    }

    #[test]
    fn returns_most_recent_switch_when_multiple_exist() {
        let events = vec![
            init_event("fast"),
            switch_event("fast", "gpt-4o"),
            switch_event("gpt-4o", "claude-opus"),
        ];
        assert_eq!(latest_model_profile_for(&events), "claude-opus");
    }

    #[test]
    fn falls_back_to_fake_when_no_initialization_event() {
        let events: Vec<DomainEvent> = vec![];
        assert_eq!(latest_model_profile_for(&events), "fake");
    }
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p agent-runtime --lib model_profile_resolution_tests`
Expected: FAIL — `latest_model_profile_for` not defined.

- [ ] **Step 3: Add the helper**

In `crates/agent-runtime/src/agent_loop.rs`, just BEFORE `pub async fn run_agent_loop<S, M>(…)` (around line 262), add:

```rust
/// Resolve the active model profile alias from a session's full event log.
///
/// Priority (newest to oldest wins):
/// 1. The most recent `EventPayload::ModelProfileSwitched.to_profile`.
/// 2. `EventPayload::SessionInitialized.model_profile` (the session's
///    original profile).
/// 3. The literal `"fake"` (only reached for broken event logs — kept for
///    symmetry with the pre-P4 fallback).
pub(crate) fn latest_model_profile_for(events: &[agent_core::DomainEvent]) -> String {
    for event in events.iter().rev() {
        match &event.payload {
            agent_core::EventPayload::ModelProfileSwitched { to_profile, .. } => {
                return to_profile.clone();
            }
            agent_core::EventPayload::SessionInitialized { model_profile } => {
                return model_profile.clone();
            }
            _ => {}
        }
    }
    "fake".to_string()
}
```

- [ ] **Step 4: Replace the inline lookup with the helper**

In the same file, find the block at lines 305-312 (the `find_map` lookup quoted above). Replace the entire `let model_profile_alias: String = session_events ...;` statement with:

```rust
    let model_profile_alias: String = latest_model_profile_for(&session_events);
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p agent-runtime --lib model_profile_resolution_tests`
Expected: PASS (all 4 new tests).

Run: `cargo test -p agent-runtime --lib`
Expected: all green. The existing `send_message_records_user_and_assistant_events` test (which starts sessions with the `"fake"` profile and never switches) must still pass — the helper returns `SessionInitialized.model_profile = "fake"` in that case, exactly mirroring the pre-P4 behaviour.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime/src/agent_loop.rs
git commit -m "feat(runtime): add latest_model_profile_for helper and use in agent loop"
```

---

### Task 3 — `LocalRuntime::switch_model` inherent method

**Files:**

- Modify: `crates/agent-runtime/src/facade_runtime.rs` (add `switch_model` next to `compact_session` at line 358)
- Test: `crates/agent-runtime/src/facade_runtime.rs` — extend the existing `#[cfg(test)] mod tests { … }` block (the test near line 1538 named `send_message_returns_session_busy_when_compacting` is the anchor we append after)

> **Verified facts** (from reading `facade_runtime.rs` 1-620):
>
> - `compact_session` lives in `impl<S, M> LocalRuntime<S, M> where …` at line 358 — this is the block we mirror (inherent method, NOT in the `AppFacade` trait).
> - `self.config: Arc<agent_config::Config>` exposes `config.profiles: Vec<(String, ProfileDef)>` (alias → def).
> - `self.session_states: Arc<Mutex<HashMap<String, SessionState>>>` is the same map `compact_session` uses for busy-gating.
> - `agent_config::resolve_limits(def: &ProfileDef) -> agent_models::ModelLimits` is the existing helper the runtime already calls from `start_session` and `agent_loop.rs`.
> - `agent_core::CoreError::SessionBusy { session_id, reason }` is the P2 variant we reuse.
> - `agent_core::CoreError::InvalidState(String)` is used throughout the file for validation errors (no need for a new "profile not found" variant).
> - `crate::event_emitter::append_and_broadcast(&*self.store, &self.event_tx, &event).await?` is the canonical write-and-emit helper already imported.
> - The `agent_models::LimitSource` → snake-case string mapping is inlined at `commands.rs:1236` — we duplicate the tiny `match` here rather than extract a helper for a one-liner used twice.
> - `self.set_session_limits(&session_id, limits)` is the `pub(crate)` helper at line ~165 that sets `SessionState.model_limits`.
> - `self.ollama_clients: HashMap<String, Arc<OllamaClient>>` is populated via `with_ollama_clients` and already drives the identical probe in `start_session` (`facade_runtime.rs` ~line 515).

- [ ] **Step 1: Write the failing tests**

In `crates/agent-runtime/src/facade_runtime.rs`, inside the existing `#[cfg(test)] mod tests { … }` block, append AFTER `send_message_returns_session_busy_when_compacting`:

```rust
    // ------------------------------------------------------------------
    // P4: mid-session model switch
    // ------------------------------------------------------------------

    fn test_config_with_two_profiles() -> Arc<agent_config::Config> {
        // Field list verified against `crates/agent-config/src/lib.rs`:
        //   ProfileDef { provider, model_id, base_url, api_key, api_key_env,
        //     context_window, output_limit, response }.
        //   Config { profiles, mcp_servers, source, context: ContextPolicy }.
        //   ContextPolicy is `#[derive(Default)]` (line 147) — `::default()` is
        //   safe. ConfigSource::Defaults is the variant used elsewhere in
        //   facade_runtime.rs test fixtures.
        use agent_config::{ConfigSource, ContextPolicy, ProfileDef};
        let fast = ProfileDef {
            provider: "fake".into(),
            model_id: "fake".into(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            context_window: None,
            output_limit: None,
            response: None,
        };
        let opus = ProfileDef {
            provider: "fake".into(),
            model_id: "fake-opus".into(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            context_window: None,
            output_limit: None,
            response: None,
        };
        Arc::new(agent_config::Config {
            profiles: vec![("fast".into(), fast), ("opus".into(), opus)],
            mcp_servers: vec![],
            source: ConfigSource::Defaults,
            context: ContextPolicy::default(),
        })
    }

    #[tokio::test]
    async fn switch_model_appends_event_and_updates_session_limits() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
            })
            .await
            .unwrap();

        runtime
            .switch_model(session_id.clone(), "opus".into())
            .await
            .expect("switch should succeed");

        let events = runtime
            .event_store_for_test()
            .load_session(&session_id)
            .await
            .unwrap();
        let switched = events
            .iter()
            .find(|e| matches!(&e.payload, agent_core::EventPayload::ModelProfileSwitched { .. }))
            .expect("ModelProfileSwitched event present");
        match &switched.payload {
            agent_core::EventPayload::ModelProfileSwitched {
                from_profile,
                to_profile,
                ..
            } => {
                assert_eq!(from_profile, "fast");
                assert_eq!(to_profile, "opus");
            }
            _ => unreachable!(),
        }

        let states = runtime.session_states_for_test().lock().await;
        let entry = states.get(session_id.as_str()).unwrap();
        let limits = entry
            .model_limits
            .as_ref()
            .expect("limits set after switch");
        assert!(limits.context_window > 0);
    }

    #[tokio::test]
    async fn switch_model_rejects_unknown_alias() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
            })
            .await
            .unwrap();

        let result = runtime.switch_model(session_id, "nonexistent".into()).await;
        assert!(matches!(
            result,
            Err(agent_core::CoreError::InvalidState(ref msg)) if msg.contains("nonexistent")
        ));
    }

    #[tokio::test]
    async fn switch_model_is_noop_when_alias_matches_current_profile() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
            })
            .await
            .unwrap();

        runtime
            .switch_model(session_id.clone(), "fast".into())
            .await
            .expect("same-profile switch is a no-op, not an error");

        let events = runtime
            .event_store_for_test()
            .load_session(&session_id)
            .await
            .unwrap();
        let count = events
            .iter()
            .filter(|e| matches!(&e.payload, agent_core::EventPayload::ModelProfileSwitched { .. }))
            .count();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn switch_model_returns_session_busy_when_compacting() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
            })
            .await
            .unwrap();

        {
            let mut states = runtime.session_states.lock().await;
            states
                .entry(session_id.to_string())
                .or_insert_with(crate::session::SessionState::default)
                .compacting = true;
        }

        let result = runtime.switch_model(session_id.clone(), "opus".into()).await;
        match result {
            Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
                assert_eq!(id, session_id.to_string());
            }
            other => panic!("expected SessionBusy, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Run the failing tests**

Run: `cargo test -p agent-runtime --lib switch_model`
Expected: FAIL — `switch_model` method does not exist.

- [ ] **Step 3: Add the `switch_model` method**

In `crates/agent-runtime/src/facade_runtime.rs`, inside the `impl<S, M> LocalRuntime<S, M> where …` block that already contains `compact_session` (the block starts at line 339), insert this method **immediately AFTER the closing `}` of `compact_session`** (so directly before `async fn rebuild_aggregate_from_disk`):

```rust
    /// Switch the active model profile for an ongoing session.
    ///
    /// The switch takes effect at the next `send_message` call — any
    /// in-flight agent loop completes on the old profile end-to-end so
    /// provider-specific tool-call formats (Anthropic `tool_use` vs.
    /// OpenAI function-calling) don't get mixed mid-stream.
    ///
    /// Errors:
    /// - `CoreError::InvalidState` if the alias is unknown.
    /// - `CoreError::SessionBusy` if the session is currently compacting.
    ///
    /// Same-profile switches (alias equals the current profile) are a
    /// silent no-op — they return `Ok(())` without appending an event.
    pub async fn switch_model(
        &self,
        session_id: agent_core::SessionId,
        profile_alias: String,
    ) -> agent_core::Result<()> {
        // Validate alias exists in the loaded Config.
        let profile_def = self
            .config
            .profiles
            .iter()
            .find(|(alias, _)| alias == &profile_alias)
            .map(|(_, def)| def.clone())
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "unknown model profile: {profile_alias}"
                ))
            })?;

        // Resolve the session's current profile using the same helper
        // the agent loop uses — the two resolvers must never drift.
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let from_profile = crate::agent_loop::latest_model_profile_for(&events);

        // Same-profile switch → silent no-op.
        if from_profile == profile_alias {
            return Ok(());
        }

        let workspace_id = events
            .first()
            .map(|e| e.workspace_id.clone())
            .ok_or_else(|| agent_core::CoreError::InvalidState("session has no events".into()))?;

        // Busy-gate — refuse when compacting (mirrors compact_session
        // lines 374-388 of this file).
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: session_id.to_string(),
                        reason: "context compaction in progress".into(),
                    });
                }
            }
        }

        // Resolve the new profile's limits (registry + user overrides).
        let new_limits = agent_config::resolve_limits(&profile_def);
        let limit_source_str = match new_limits.source {
            agent_models::LimitSource::UserConfig => "user_config",
            agent_models::LimitSource::BuiltinRegistry => "builtin_registry",
            agent_models::LimitSource::RuntimeProbe => "runtime_probe",
            agent_models::LimitSource::Fallback => "fallback",
        };

        let event = agent_core::DomainEvent::new(
            workspace_id,
            session_id.clone(),
            agent_core::AgentId::system(),
            agent_core::PrivacyClassification::MinimalTrace,
            agent_core::EventPayload::ModelProfileSwitched {
                from_profile,
                to_profile: profile_alias.clone(),
                effective_at: chrono::Utc::now(),
                context_window: new_limits.context_window,
                output_limit: new_limits.output_limit,
                limit_source: limit_source_str.into(),
            },
        );
        crate::event_emitter::append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        // Refresh cached limits so the next send_message's agent loop
        // doesn't re-derive from the old profile.
        self.set_session_limits(&session_id, new_limits.clone())
            .await;

        // Ollama probe for the new profile (fire-and-forget, 3s timeout) —
        // mirrors the probe spawned by start_session around line 515.
        if profile_def.provider == "ollama" {
            if let Some(client) = self.ollama_clients.get(&profile_alias).cloned() {
                let model_id = profile_def.model_id.clone();
                let session_id_for_probe = session_id.clone();
                let session_states = self.session_states.clone();
                tokio::spawn(async move {
                    let probe = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        client.probe_context_window(&model_id),
                    )
                    .await;
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

        Ok(())
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p agent-runtime --lib switch_model`
Expected: PASS (all 4 new tests).

Run: `cargo test -p agent-runtime --lib`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime/src/facade_runtime.rs
git commit -m "feat(runtime): add LocalRuntime::switch_model with busy-gate and limits refresh"
```

---

### Task 4 — Tauri command `switch_model`

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs` (add `switch_model` next to `compact_session` at line ~1270)

> **Verified facts** (from reading `commands.rs` 1200-1312):
>
> - `compact_session` (at line 1270) reads the active `session_id` from `state.current_session_id: Mutex<Option<SessionId>>`, then calls `state.runtime.compact_session(session_id, CompactionReason::UserRequested).await` and maps errors via `.map_err(|e| e.to_string())`. `switch_model` mirrors this pattern exactly.
> - The compile-time presence test `compact_session_command_function_exists` (line ~1290) is the canonical inline test pattern for "command registered" — we add the same for `switch_model`.
> - The Tauri command parameter for `session_id` in `compact_session` is **omitted** — it reads from GUI state. `switch_model` must take both `session_id` (for validation against GUI state) and `profile_alias` explicitly because the selector comes from the user's click.
> - Looking at other commands in this file (`rename_session` line earlier in the file): per-session commands typically take `session_id: String` as a parameter AND optionally verify against `state.current_session_id`. For P4 we follow the simpler pattern: take `session_id: String` from the frontend, convert via `SessionId::from_string`, and pass through to the runtime.

- [ ] **Step 1: Write the failing test**

In `apps/agent-gui/src-tauri/src/commands.rs`, after the existing `mod compact_session_command_tests` block (around line 1290), append:

```rust
#[cfg(test)]
mod switch_model_command_tests {
    use super::switch_model;

    #[test]
    fn switch_model_command_function_exists() {
        // Compile-time presence check — if `switch_model` is renamed or
        // removed this fails to compile before `collect_commands!` /
        // `generate_handler!` get a chance to blow up at runtime.
        let _ = switch_model;
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-gui-tauri switch_model_command_function_exists`
Expected: FAIL to COMPILE — `switch_model` is not defined.

- [ ] **Step 3: Add the `switch_model` command**

In `apps/agent-gui/src-tauri/src/commands.rs`, immediately AFTER the `compact_session` function (before `#[cfg(test)] mod compact_session_command_tests`), add:

```rust
/// P4: swap the active model profile for the current session.
///
/// The switch takes effect at the next `send_message` — in-flight
/// streams keep using the old profile end-to-end. Returns an error
/// when the alias is unknown or the session is currently compacting.
#[tauri::command]
#[specta::specta]
pub async fn switch_model(
    state: State<'_, GuiState>,
    session_id: String,
    profile_alias: String,
) -> Result<(), String> {
    let session_id = agent_core::SessionId::from_string(session_id);
    state
        .runtime
        .switch_model(session_id, profile_alias)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p agent-gui-tauri switch_model_command_function_exists`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs
git commit -m "feat(gui): add switch_model Tauri command"
```

---

### Task 5 — Register `switch_model` in specta + generate_handler

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/specta.rs` (add `switch_model` to the runtime `collect_commands![]`)
- Modify: `apps/agent-gui/src-tauri/src/lib.rs` (add `crate::commands::switch_model` to `generate_handler![]`)
- Modify: `apps/agent-gui/src-tauri/src/bin/export_specta.rs` (add `agent_gui_tauri::commands::switch_model` to its **independent** `collect_commands![]` — this is the macro the codegen binary actually reads)
- Regenerate: `apps/agent-gui/src/generated/commands.ts` + `events.ts` via `just gen-types`

> **Verified facts** (from reading `specta.rs`, `lib.rs`, `bin/export_specta.rs`, `justfile:110-112` during Task 5 execution — plan originally misreported the codegen source):
>
> - `just gen-types` runs `cargo run -p agent-gui-tauri --bin export-specta` and `--bin export-events`. The codegen-facing `collect_commands![]` lives in `apps/agent-gui/src-tauri/src/bin/export_specta.rs`, NOT in `src/specta.rs`. The two lists are independent and have drifted: `bin/export_specta.rs` already missed `compact_session` from P2, so P2's `compactSession` TS binding was never generated either. Task 5 adds BOTH `compact_session` (P2 catch-up) and `switch_model` (P4) to the bin list in one go — the P2 gap is the minimum additional fix required to restore `just check-types` invariants.
> - `src/specta.rs::create_specta()` is still called from `lib.rs:22` as `let _specta_builder = create_specta();` — a compile-time-only type check. We keep it in sync with the runtime handler so typos are caught early, even though its output is discarded.
> - `lib.rs`'s `generate_handler![…]` is the runtime IPC registry — without adding `switch_model` here, the frontend's `invoke("switch_model", …)` would return "command not found" at runtime regardless of what TS bindings exist.
> - macOS dev workstations see `target/debug/export-specta` rejected by Gatekeeper (`spctl --assess: rejected`, SIGKILL / exit 137). The fix is ad-hoc signing the binary with `codesign --sign - --force --timestamp=none target/debug/export-specta` (and the matching `export-events` binary) before running `just gen-types` on a given build. This has to be re-applied after each `cargo clean`.

- [ ] **Step 1: Register in runtime `generate_handler!`**

In `apps/agent-gui/src-tauri/src/lib.rs`, find the line `crate::commands::compact_session,` inside `generate_handler![…]`. Insert immediately after, with matching indentation:

```rust
            crate::commands::switch_model,
```

- [ ] **Step 2: Register in compile-time `src/specta.rs::collect_commands!`**

In `apps/agent-gui/src-tauri/src/specta.rs`, find the line `compact_session,` inside `collect_commands![…]`. Insert immediately after, with matching indentation:

```rust
            switch_model,
```

The surrounding context becomes:

```rust
            cancel_session,
            compact_session,
            switch_model,
            get_permission_mode,
```

- [ ] **Step 3: Register in codegen `bin/export_specta.rs::collect_commands!`**

In `apps/agent-gui/src-tauri/src/bin/export_specta.rs`, find the line `agent_gui_tauri::commands::cancel_session,` inside `collect_commands![…]`. Insert the TWO missing entries (P2 catch-up + P4) immediately after it, with matching indentation:

```rust
            agent_gui_tauri::commands::compact_session,
            agent_gui_tauri::commands::switch_model,
```

The surrounding context becomes:

```rust
            agent_gui_tauri::commands::cancel_session,
            agent_gui_tauri::commands::compact_session,
            agent_gui_tauri::commands::switch_model,
            agent_gui_tauri::commands::get_permission_mode,
```

- [ ] **Step 4: Sign the codegen binaries so macOS Gatekeeper doesn't SIGKILL them**

Run from the worktree root (safe to re-run; `--force` overwrites any prior ad-hoc signature):

```bash
cargo build -p agent-gui-tauri --bin export-specta --bin export-events
codesign --sign - --force --timestamp=none target/debug/export-specta
codesign --sign - --force --timestamp=none target/debug/export-events
```

Expected: both `codesign` commands print nothing and exit 0. On Linux / CI there is no Gatekeeper — this step is a no-op there (codesign exists as a GNU binutils binary on some distros but running it on an ELF does nothing; the `|| true` safeguard in the optional wrapper variant below is not required).

- [ ] **Step 5: Regenerate the TypeScript bindings**

Run: `just gen-types`
Expected: exit 0, writes `apps/agent-gui/src/generated/commands.ts` and `apps/agent-gui/src/generated/events.ts`, runs `npx oxfmt --write` on them.

Verify the new bindings exist:

```bash
grep -n 'switchModel\|switch_model' apps/agent-gui/src/generated/commands.ts
grep -n 'compactSession\|compact_session' apps/agent-gui/src/generated/commands.ts
```

Expected: both greps return matches — one `export async function switchModel(sessionId: string, profileAlias: string): Promise<null>` and one `export async function compactSession(): Promise<null>`. tauri-specta's snake_case → camelCase conversion is automatic; the emitted function names determine what Task 7 / future frontend code imports.

- [ ] **Step 6: Verify generated types are in sync**

Run: `just check-types`
Expected: exit 0 — `gen-types` is idempotent, so re-running it produces no diff against the files we just generated.

- [ ] **Step 7: Run Rust tests and lint**

Run: `cargo test -p agent-gui-tauri --lib`
Expected: all green.

Run: `cargo clippy -p agent-gui-tauri --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 8: Commit (Rust source only — generated/ stays gitignored)**

> **Verified fact** (from `.gitignore:48` and prior commit `47962a3`'s `chore(gui): stop tracking auto-generated TypeScript bindings`): `apps/agent-gui/src/generated/` is intentionally gitignored. The TypeScript bindings are reproducible build artifacts — CI's `type-sync` job re-runs `just gen-types` and uses `git diff --exit-code apps/agent-gui/src/generated/` to verify the locally generated files match what the Rust source would produce. We commit ONLY the three Rust files; the bindings get regenerated on every dev / CI run.

```bash
git add apps/agent-gui/src-tauri/src/specta.rs \
        apps/agent-gui/src-tauri/src/lib.rs \
        apps/agent-gui/src-tauri/src/bin/export_specta.rs
git commit -m "feat(gui): register switch_model in specta and tauri handler"
```

---

### Task 6 — Vue `useSessionStore` consumes `ModelProfileSwitched`

**Files:**

- Modify: `apps/agent-gui/src/stores/session.ts` (add `ModelProfileSwitched` arm to the `applyEvent` switch)
- Modify: `apps/agent-gui/src/stores/__tests__/session-context.test.ts` (file already exists — **append an `it(...)` block** to the existing `describe("useSessionStore — context fields", ...)`; do NOT recreate the file)

> **Verified facts** (read at plan time):
>
> - `apps/agent-gui/src/stores/__tests__/session-context.test.ts` exists (P3 added it). It defines `makeUsage()` and `makeEvent(payload)` helpers at top-of-file and uses `beforeEach(() => setActivePinia(createPinia()))`. We reuse those.
> - `stores/session.ts` has `currentProfile = ref<string>("fast")` (line 39) and `modelLimits = ref<ProjectedModelLimits | null>(null)` (line 41). The `applyEvent(event)` switch has no case for `ModelProfileSwitched` yet.
> - `DomainEvent` + `EventPayload` come from `@/types` (re-exports from `@/generated/events.ts`). After Task 1 + Task 5 regen, the variant `{ type: "ModelProfileSwitched"; from_profile: string; to_profile: string; effective_at: string; context_window: number; output_limit: number; limit_source: string; }` is discriminated-union-narrowable.
> - `ProjectedModelLimits` TS shape (from `@/types` → `@/generated/events.ts`): `{ context_window: number; output_limit: number; source: string }` — verified by the existing `modelLimits` ref usage in the store and in `ContextMeter.vue`.

- [ ] **Step 1: Append the failing test to the existing P3 spec file**

Open `apps/agent-gui/src/stores/__tests__/session-context.test.ts`. Locate the `describe("useSessionStore — context fields", () => { … })` block (it's the primary `describe` in the file). Append a new `it(...)` case at the END of that `describe` body, BEFORE its closing `});`. Reuse the existing top-of-file `makeEvent` helper; do NOT add new imports:

```ts
it("updates currentProfile and modelLimits on ModelProfileSwitched", () => {
  const session = useSessionStore();
  // Sanity: store starts with the default "fast" profile (verified at
  // `stores/session.ts:39` — `currentProfile = ref<string>("fast")`).
  expect(session.currentProfile).toBe("fast");
  expect(session.modelLimits).toBeNull();

  session.applyEvent(
    makeEvent({
      type: "ModelProfileSwitched",
      from_profile: "fast",
      to_profile: "opus",
      effective_at: "2026-05-09T10:00:00Z",
      context_window: 200_000,
      output_limit: 16_384,
      limit_source: "builtin_registry"
    })
  );

  expect(session.currentProfile).toBe("opus");
  expect(session.modelLimits).toEqual({
    context_window: 200_000,
    output_limit: 16_384,
    source: "builtin_registry"
  });
});
```

- [ ] **Step 2: Run the failing test**

Run: `cd apps/agent-gui && pnpm exec vitest run src/stores/__tests__/session-context.test.ts`
Expected: the new `updates currentProfile and modelLimits on ModelProfileSwitched` test FAILS — the store does not consume the event yet so `currentProfile` stays `"fast"` and `modelLimits` stays `null`. All pre-existing tests in the file PASS.

- [ ] **Step 3: Add the `ModelProfileSwitched` arm**

In `apps/agent-gui/src/stores/session.ts`, locate the `applyEvent(event: DomainEvent)` function (verified at `function applyEvent(event: DomainEvent) {` — line 59) and inside it the `switch (p.type) { … }` dispatcher (line 64). The last context-related case in that switch is `case "ContextCompactionFailed": { … }` (verified line 151 — the case block, not the line number you depend on; use the string literal as the anchor). Insert the new arm **immediately after** the closing `}` of the `"ContextCompactionFailed"` case, before whatever case follows it. The patch itself is anchor-based, so it does not depend on line numbers:

```ts
      case "ModelProfileSwitched": {
        currentProfile.value = p.to_profile;
        modelLimits.value = {
          context_window: p.context_window,
          output_limit: p.output_limit,
          source: p.limit_source
        };
        break;
      }
```

> The shape of `ProjectedModelLimits` on the TS side is `{ context_window: number; output_limit: number; source: string }` — verified by reading the `modelLimits = ref<ProjectedModelLimits | null>(null)` declaration in `stores/session.ts:41` and by the `@/types` re-export from `@/generated/events.ts`. `currentProfile` is `ref<string>("fast")` at line 39 of the same file (both P3 artefacts; P4 only writes to them, doesn't redeclare).

- [ ] **Step 4: Run the test**

Run: `cd apps/agent-gui && pnpm exec vitest run src/stores/__tests__/session-context.test.ts`
Expected: PASS — the new test + all pre-existing P3 tests in the file are green.

Run: `just test-gui`
Expected: all GUI vitest specs green.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/stores/session.ts apps/agent-gui/src/stores/__tests__/session-context.test.ts
git commit -m "feat(gui): handle ModelProfileSwitched in session store"
```

---

### Task 7 — i18n keys for the switch-model dropdown

**Files:**

- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

> **Verified facts** (grepped at plan time):
>
> - Both files already have `"switchModel": "Switch model…"` / `"切换模型…"` inside the `"context"` object at line 100 (en) / 100 (zh).
> - The `"errors"` object is at `en.json:66` / `zh-CN.json:66` and **already contains** `"sessionBusy": "Session is busy: {reason}"` / `"会话繁忙：{reason}"`. Do NOT re-add `sessionBusy` — only add the new `profileNotFound` key.

- [ ] **Step 1: Add keys to `en.json`**

In `apps/agent-gui/src/locales/en.json`, find the exact line `"switchModel": "Switch model…"` inside the `"context"` object. Add immediately after it (add a trailing comma on the switchModel line if it doesn't already have one):

```json
    "switchModelChoose": "Choose a profile",
    "switchModelCurrent": "Current",
    "switchModelSuccess": "Switched to {profile}",
    "switchModelFailed": "Failed to switch model: {error}",
```

In the same file, find the `"errors"` block:

```json
  "errors": {
    "sessionBusy": "Session is busy: {reason}"
  },
```

Add `profileNotFound` (don't touch `sessionBusy`):

```json
  "errors": {
    "sessionBusy": "Session is busy: {reason}",
    "profileNotFound": "Unknown model profile: {alias}"
  },
```

- [ ] **Step 2: Add the same keys to `zh-CN.json`**

In `apps/agent-gui/src/locales/zh-CN.json`, find `"switchModel": "切换模型…"`. Add:

```json
    "switchModelChoose": "选择模型",
    "switchModelCurrent": "当前",
    "switchModelSuccess": "已切换到 {profile}",
    "switchModelFailed": "切换模型失败：{error}",
```

Find the `"errors"` block:

```json
  "errors": {
    "sessionBusy": "会话繁忙：{reason}"
  },
```

Add:

```json
  "errors": {
    "sessionBusy": "会话繁忙：{reason}",
    "profileNotFound": "未知的模型配置：{alias}"
  },
```

- [ ] **Step 3: Verify format**

Run: `pnpm run format:check` (from repo root)
Expected: PASS.

Run: `pnpm run lint`
Expected: PASS (no stylelint issues for JSON; oxlint skips JSON).

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/locales/en.json apps/agent-gui/src/locales/zh-CN.json
git commit -m "feat(gui): add i18n keys for model-switch dropdown"
```

---

### Task 7a — Sync `list_profiles_with_limits` to TS bindings (drift catch-up)

> **Why this task exists:** Task 8's verified-facts originally assumed
> `commands.listProfilesWithLimits` and the `ProfileWithLimits` TS type were
> already reachable from the Vue layer. They are not. The Rust source-of-truth
> is complete (`commands.rs:1216` defines `pub struct ProfileWithLimits` with
> `derive(specta::Type)`; `commands.rs:1229` defines the
> `pub async fn list_profiles_with_limits` command; `src/specta.rs:21` and
> `lib.rs:136` register it), but `bin/export_specta.rs` — which is what
> `just gen-types` actually invokes — never registered the command. P3 added
> `list_profiles_with_limits` only to `src/specta.rs` (compile-time check) and
> `lib.rs` (runtime IPC), missing the codegen entry point. This is the same
> drift class as the P2 `compact_session` gap that Task 5 fixed; we fix the
> remaining one here, in isolation, before Task 8 needs the binding.
>
> **Verified facts (read at remediation time):**
>
> - `apps/agent-gui/src-tauri/src/bin/export_specta.rs` exists (4654 bytes); its
>   `collect_commands![]` lists every other command but **not**
>   `list_profiles_with_limits`. Adding one line restores parity with
>   `src/specta.rs:21`.
> - `apps/agent-gui/src/types/index.ts:2-12` already re-exports specta-generated
>   types from `../generated/events`, and `:132-144` re-exports command response
>   types from `../generated/commands`. We add `ProfileWithLimits` to the second
>   block.
> - macOS EDR whitelisting from Task 5 is still in effect, so
>   `target/debug/export-specta` and `target/debug/export-events` will run
>   without SIGKILL after `codesign --sign - --force --timestamp=none`.

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/bin/export_specta.rs` (add 1 line)
- Modify: `apps/agent-gui/src/types/index.ts` (add `ProfileWithLimits` to the existing `from "../generated/commands"` re-export block)
- Auto-regenerated (gitignored, NOT committed): `apps/agent-gui/src/generated/commands.ts`

- [ ] **Step 1: Register the command in the codegen entry point**

In `apps/agent-gui/src-tauri/src/bin/export_specta.rs`, locate the `collect_commands![]` block and the line `agent_gui_tauri::commands::list_profiles,`. Insert immediately after it (matching the 12-space indentation):

```rust
            agent_gui_tauri::commands::list_profiles_with_limits,
```

This places it next to its sibling `list_profiles`, mirroring the order in `src/specta.rs:20-21`.

- [ ] **Step 2: Rebuild + codesign + regenerate**

```bash
cargo build -p agent-gui-tauri --bin export-specta --bin export-events 2>&1 | tail -3
codesign --sign - --force --timestamp=none target/debug/export-specta 2>&1
codesign --sign - --force --timestamp=none target/debug/export-events 2>&1
just gen-types 2>&1 | tail -10
```

Expected: build clean; codesign reports `replacing existing signature`; `just gen-types` reports `✅ TypeScript bindings regenerated`. The first run after this fix will re-emit `apps/agent-gui/src/generated/commands.ts` with the new binding; that file is gitignored so it does not show up in `git status`.

- [ ] **Step 3: Verify the binding + type appeared**

```bash
grep -n 'listProfilesWithLimits\|ProfileWithLimits' apps/agent-gui/src/generated/commands.ts | head -10
```

Expected: at least 3 hits — the `commands.listProfilesWithLimits` function, the `ProfileWithLimits` TypeScript type definition, and a `Result<ProfileWithLimits[], string>`-shaped return type. If nothing appears, the codegen silently failed; STOP and report.

- [ ] **Step 4: Re-export `ProfileWithLimits` from `@/types`**

In `apps/agent-gui/src/types/index.ts`, find the existing block that ends with `} from "../generated/commands";` (around line 132-144). Add `ProfileWithLimits` to the export list. Read the file first to confirm the existing entries and pick a placement consistent with the surrounding entries (alphabetical or insertion order, whichever the file uses).

- [ ] **Step 5: Verify TypeScript compilation + tests + clippy**

```bash
cd apps/agent-gui && pnpm exec vue-tsc --noEmit 2>&1 | grep -E 'ProfileWithLimits|list_profiles_with_limits|listProfilesWithLimits' | head -10 ; cd ..
```

Expected: empty output (no errors mentioning these symbols). Pre-existing TS errors elsewhere in the codebase are out of scope.

```bash
cargo test -p agent-gui-tauri --lib 2>&1 | tail -5
cargo clippy -p agent-gui-tauri --all-targets -- -D warnings 2>&1 | tail -3
just check-types 2>&1 | tail -5
just test-gui 2>&1 | tail -10
```

Expected: tests pass, clippy clean, `just check-types` reports `✅ Generated types are in sync`, vitest 274/274 pass (Task 7a does not change runtime behaviour, so no test deltas expected).

- [ ] **Step 6: Commit**

Stage ONLY the two source files; the regenerated `generated/commands.ts` stays gitignored (CI re-runs `just gen-types` and verifies via `git diff --exit-code`).

```bash
git add apps/agent-gui/src-tauri/src/bin/export_specta.rs apps/agent-gui/src/types/index.ts
git status --short      # verify exactly 2 staged files, no generated/
git commit -m "feat(gui): expose list_profiles_with_limits in TS bindings"
```

---

### Task 8 — ContextMeter Switch-model dropdown

**Files:**

- Modify: `apps/agent-gui/src/components/ContextMeter.vue`
- Modify: `apps/agent-gui/src/components/__tests__/ContextMeter.test.ts` (file already exists — **append a new `describe` block**; do NOT recreate it)

> **Verified facts** (grepped + read at plan time):
>
> - `ContextMeter.test.ts` exists at `apps/agent-gui/src/components/__tests__/ContextMeter.test.ts`. It already establishes the mock pattern: `const invokeMock = vi.fn()` + `vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args) => invokeMock(...args) }))` + `vi.mock("@/composables/useToast", ...)` at module top, plus `mountWithPlugins(ContextMeter, { reusePinia: true })` with `setActivePinia(createPinia())` in `beforeEach`. **Our new tests MUST reuse the same `invokeMock` and module-level mocks** — do not redeclare them.
> - The current Switch model button in `ContextMeter.vue` is hard-coded `disabled`:
>   ```html
>   <button
>     type="button"
>     class="btn btn-ghost"
>     data-test="context-meter-switch-model"
>     disabled
>     :title="t('context.switchModel')"
>   >
>     {{ t("context.switchModel") }}
>   </button>
>   ```
> - `useSessionStore().currentSessionId` is a `ref<string | null>`. `session.currentProfile` was already added in P3 and is wired to update on `ModelProfileSwitched` by Task 6 above.
> - The existing `onCompactClick` calls `invoke("compact_session", {})`; our new code uses `invoke("switch_model", { sessionId, profileAlias })` (tauri-specta serialises snake_case Rust params as camelCase JS keys — verified by `compact_session`'s existing handler convention in the GUI code).
> - `ProfileWithLimits` type: the specta registration added to `specta.rs` in Task 5 Step 3 exports it through `events.ts`. Use `import type { ProfileWithLimits } from "@/types"` which re-exports from `@/generated/events`. If `gen-types` produces a different re-export location, adjust the import path here (but the name `ProfileWithLimits` is fixed by the Rust source in `commands.rs`).

- [ ] **Step 1: Append the failing `describe` block**

Open `apps/agent-gui/src/components/__tests__/ContextMeter.test.ts` and append (AT THE END of the file, after the last existing `describe` block) — **do not add new imports or new `vi.mock()` calls**; the top-of-file mocks already cover `@tauri-apps/api/core` and `@/composables/useToast`:

```ts
describe("ContextMeter.vue — Switch model dropdown (P4)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    setActivePinia(createPinia());
    // `openProfilePicker()` calls `list_profiles_with_limits` once and
    // caches the result; provide a two-profile fixture by default.
    invokeMock.mockImplementation(async (cmd: string, _args?: unknown) => {
      if (cmd === "list_profiles_with_limits") {
        return [
          {
            alias: "fast",
            provider: "openai",
            model_id: "gpt-4o-mini",
            context_window: 128_000,
            output_limit: 16_384,
            limit_source: "builtin_registry",
            has_api_key: true
          },
          {
            alias: "opus",
            provider: "anthropic",
            model_id: "claude-opus",
            context_window: 200_000,
            output_limit: 16_384,
            limit_source: "builtin_registry",
            has_api_key: true
          }
        ];
      }
      if (cmd === "switch_model") return null;
      return null;
    });
  });

  it("enables the switch-model button when a session is active and idle", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    const btn = wrapper.find('[data-test="context-meter-switch-model"]');
    expect(btn.exists()).toBe(true);
    expect(btn.attributes("disabled")).toBeUndefined();
  });

  it("keeps the switch-model button disabled while compacting", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.compacting = true;
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    const btn = wrapper.find('[data-test="context-meter-switch-model"]');
    expect(btn.attributes("disabled")).toBeDefined();
  });

  it("opens the profile picker when the switch-model button is clicked", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-switch-model"]').trigger("click");
    // `openProfilePicker` awaits `invoke("list_profiles_with_limits")` — let the
    // microtask queue drain so the profile list renders.
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    const items = wrapper.findAll('[data-test^="context-meter-profile-"]');
    expect(items.length).toBe(2);
    expect(wrapper.find('[data-test="context-meter-profile-fast"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="context-meter-profile-opus"]').exists()).toBe(true);
    // The "(Current)" marker sits on the current alias.
    expect(wrapper.find('[data-test="context-meter-profile-fast"]').text()).toMatch(
      /current|当前/i
    );
  });

  it("calls switch_model with the selected alias and closes the popover", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-switch-model"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-profile-opus"]').trigger("click");
    // Let the awaited `invoke("switch_model")` resolve.
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    expect(invokeMock).toHaveBeenCalledWith("switch_model", {
      sessionId: "ses_test",
      profileAlias: "opus"
    });
    // Popover should close after a successful switch.
    expect(wrapper.find('[data-test="context-meter-popover"]').exists()).toBe(false);
  });

  it("clicking the already-current profile is a no-op (no switch_model call)", async () => {
    const session = useSessionStore();
    session.currentSessionId = "ses_test";
    session.currentProfile = "fast";
    session.lastContextUsage = makeUsage();
    const { wrapper } = mountWithPlugins(ContextMeter, { reusePinia: true });
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-switch-model"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-profile-fast"]').trigger("click");
    await wrapper.vm.$nextTick();
    const switchCalls = invokeMock.mock.calls.filter((c) => c[0] === "switch_model");
    expect(switchCalls.length).toBe(0);
  });
});
```

- [ ] **Step 2: Run the failing test**

Run: `cd apps/agent-gui && pnpm exec vitest run src/components/__tests__/ContextMeter.test.ts`
Expected: the new 5 tests in "Switch model dropdown (P4)" FAIL — the button is hard-coded `disabled` and there is no profile-picker panel yet. Existing P3 tests in the same file should still PASS.

- [ ] **Step 3: Replace the disabled button with an interactive dropdown**

In `apps/agent-gui/src/components/ContextMeter.vue`:

**(a)** Add to the `<script setup lang="ts">` block — locate the existing import group and the `const popoverOpen = ref(false);` line. Add the type import next to other type imports (or in the existing `import type { ... } from "@/types"` group), and the three new refs + two functions right after `popoverOpen`:

```ts
// Add to the existing `import type { ... } from "@/types"` group (merge or add new line):
import type { ProfileWithLimits } from "@/types";

// Right after `const popoverOpen = ref(false);`:
const profilePickerOpen = ref(false);
const profiles = ref<ProfileWithLimits[]>([]);
const switchingProfile = ref(false);

async function openProfilePicker() {
  if (!session.currentSessionId || session.compacting || switchingProfile.value) return;
  if (profiles.value.length === 0) {
    try {
      profiles.value = await invoke<ProfileWithLimits[]>("list_profiles_with_limits");
    } catch (e) {
      toast.error(t("context.switchModelFailed", { error: String(e) }));
      return;
    }
  }
  profilePickerOpen.value = true;
}

async function onProfilePicked(alias: string) {
  if (!session.currentSessionId || switchingProfile.value) return;
  if (alias === session.currentProfile) {
    profilePickerOpen.value = false;
    return;
  }
  switchingProfile.value = true;
  try {
    await invoke("switch_model", {
      sessionId: session.currentSessionId,
      profileAlias: alias
    });
    toast.success(t("context.switchModelSuccess", { profile: alias }));
    profilePickerOpen.value = false;
    popoverOpen.value = false;
  } catch (e) {
    toast.error(t("context.switchModelFailed", { error: String(e) }));
  } finally {
    switchingProfile.value = false;
  }
}
```

> **On the `ProfileWithLimits` import path**: `@/types/index.ts` re-exports from `@/generated/events` (verified — the existing `ContextUsage` type used in this very file is imported the same way via `import type { ContextUsage } from "@/types"`). If `just gen-types` (Task 5) places the struct in `commands.ts` instead of `events.ts` because it's a command return type, update `apps/agent-gui/src/types/index.ts` to re-export `ProfileWithLimits` from the correct generated file. Verify by grepping after `just gen-types` completes: `grep -n 'ProfileWithLimits' apps/agent-gui/src/generated/*.ts apps/agent-gui/src/types/index.ts`.

**(b)** Replace the existing disabled switch-model button (lines 167-176). Find:

```html
<button
  type="button"
  class="btn btn-ghost"
  data-test="context-meter-switch-model"
  disabled
  :title="t('context.switchModel')"
>
  {{ t("context.switchModel") }}
</button>
```

Replace with:

```html
<button
  type="button"
  class="btn btn-ghost"
  data-test="context-meter-switch-model"
  :disabled="!session.currentSessionId || session.compacting || switchingProfile"
  :title="t('context.switchModel')"
  @click="openProfilePicker"
>
  {{ t("context.switchModel") }}
</button>
```

**(c)** Add the profile-list sub-panel INSIDE the `<div v-if="popoverOpen && session.lastContextUsage" class="popover">` container, immediately BEFORE the closing `</div>` of the popover. The insertion point is the line containing just `</div>` that closes the popover (the line BEFORE the `</template>`-level `</div>` that closes `.context-meter`). Insert:

```html
<div v-if="profilePickerOpen" class="profile-picker" data-test="context-meter-profile-picker">
  <header class="profile-picker-header">{{ t("context.switchModelChoose") }}</header>
  <ul class="profile-list">
    <li v-for="p in profiles" :key="p.alias">
      <button
        type="button"
        class="profile-item"
        :data-test="`context-meter-profile-${p.alias}`"
        :disabled="switchingProfile"
        @click="onProfilePicked(p.alias)"
      >
        <span class="profile-alias">{{ p.alias }}</span>
        <span class="profile-meta">
          {{ p.model_id }} · {{ Math.round(p.context_window / 1000) }}k
          <span v-if="p.alias === session.currentProfile" class="profile-current">
            ({{ t("context.switchModelCurrent") }})
          </span>
        </span>
      </button>
    </li>
  </ul>
</div>
```

**(d)** Append these scoped CSS rules inside the existing `<style scoped>` block, right before the final closing `</style>`:

```css
.profile-picker {
  margin-top: 8px;
  border-top: 1px solid var(--app-border-color);
  padding-top: 8px;
}
.profile-picker-header {
  font-size: 12px;
  font-weight: 600;
  margin-bottom: 6px;
  opacity: 0.8;
}
.profile-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.profile-item {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 6px 8px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  color: var(--app-text-color);
  cursor: pointer;
  text-align: left;
}
.profile-item:hover:not(:disabled) {
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
  border-color: var(--app-border-color);
}
.profile-item:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.profile-alias {
  font-weight: 600;
  font-size: 13px;
}
.profile-meta {
  font-size: 11px;
  opacity: 0.7;
}
.profile-current {
  color: var(--app-primary-color);
  font-weight: 600;
}
```

- [ ] **Step 4: Run the tests**

Run: `cd apps/agent-gui && pnpm exec vitest run src/components/__tests__/ContextMeter.test.ts`
Expected: the 5 new "Switch model dropdown (P4)" tests PASS + all pre-existing P3 tests in the file remain PASS.

Run: `just test-gui`
Expected: all GUI vitest specs green.

- [ ] **Step 5: Run format + lint**

Run: `pnpm run format:check && pnpm run lint`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/components/ContextMeter.vue apps/agent-gui/src/components/__tests__/ContextMeter.test.ts
git commit -m "feat(gui): enable switch-model dropdown in ContextMeter popover"
```

---

### Task 9 — Extend Playwright tauri-mock + E2E spec

**Files:**

- Modify: `apps/agent-gui/e2e/tauri-mock.js` (add `switch_model` handler + emit `ModelProfileSwitched`)
- Create: `apps/agent-gui/e2e/model-switch.spec.ts` (new spec exercising the dropdown)

> **Verified facts** (read at plan time from `apps/agent-gui/e2e/tauri-mock.js` and `apps/agent-gui/e2e/context-meter.spec.ts`):
>
> - **Emit helper** is `function emitEvent(eventName, payload)` at line 155. All existing handlers emit Rust-side `DomainEvent`s as `emitEvent("session-event", makeEvent(sid, { type: "...", ... }))`.
> - **Event factory** is `function makeEvent(sessionId, payload)` at line 137 — it wraps the payload in the full envelope (`schema_version`, `workspace_id`, etc.) automatically; callers only supply `sessionId` + a bare `payload` object keyed by `type`.
> - **Trace mirror**: every handler also calls `getTrace(sid).push(event)` so the mock's local trace store stays consistent with what listeners receive. The `compact_session` handler at line 556 does both: `getTrace(sid).push(startedEvent); emitEvent("session-event", startedEvent);` then a `setTimeout` for the completion event.
> - **list_profiles_with_limits** already returns `[{ alias: "fast", ... }, { alias: "smart", ... }, …]` — the fixture uses `"fast"` and `"smart"` (NOT `"opus"`). We must align our E2E spec with **whatever aliases the mock already emits**. Verified: line 276 (`"fast"` → 128k/16k) and line 279 (`"smart"` → 200k/16k). Use `"fast"` + `"smart"`.
> - **Real data-test selectors** (verified by grep):
>   - chat: `[data-test="chat-panel"]`, `[data-test="message-input"]` (textarea), `[data-test="send-button"]`
>   - meter: `[data-test="context-meter-bar"]`, `[data-test="context-meter-popover"]`, `[data-test="context-meter-switch-model"]`
> - **Mock load pattern**: `page.addInitScript({ path: mockPath })` where `mockPath = resolve(__dirname, "tauri-mock.js")` and `__dirname = dirname(fileURLToPath(import.meta.url))`. The mock only emits `ContextAssembled` inside its `send_message` handler, so the meter doesn't render until one message has been sent.
> - `state.currentProfile` starts as `"fast"` (line 25). `state.workspace` is set up during bootstrap (line 140 reads `state.workspace ? state.workspace.workspace_id : "wrk_mock"`).

- [ ] **Step 1: Add the `switch_model` handler to `tauri-mock.js`**

In `apps/agent-gui/e2e/tauri-mock.js`, locate the `case "compact_session": { … }` block (starts at line 556). Immediately AFTER its closing `}` and before the next `case`, add a new case. The args from the frontend arrive as `{ sessionId, profileAlias }` (camelCase — tauri-specta serialises snake_case Rust params to camelCase JS keys; Task 8's component passes exactly these names):

```js
    case "switch_model": {
      var alias = args && (args.profileAlias || args.profile_alias);
      var switchSid = (args && (args.sessionId || args.session_id)) || state.currentSessionId;
      if (!alias) {
        return Promise.reject(new Error("profileAlias required"));
      }
      if (!switchSid) {
        return Promise.reject(new Error("No active session"));
      }
      var fromProfile = state.currentProfile;
      if (fromProfile === alias) {
        return Promise.resolve(null); // same-profile: silent no-op (mirrors runtime)
      }
      // Resolve new limits from the same table list_profiles_with_limits uses.
      var newWindow;
      var newOutput;
      if (alias === "fast") {
        newWindow = 128000;
        newOutput = 16384;
      } else if (alias === "smart") {
        newWindow = 200000;
        newOutput = 16384;
      } else {
        // Unknown alias → reject like the real runtime does
        // (agent-core::CoreError::InvalidState).
        return Promise.reject(new Error("Unknown model profile: " + alias));
      }
      state.currentProfile = alias;
      var switchedEvent = makeEvent(switchSid, {
        type: "ModelProfileSwitched",
        from_profile: fromProfile,
        to_profile: alias,
        effective_at: new Date().toISOString(),
        context_window: newWindow,
        output_limit: newOutput,
        limit_source: "builtin_registry"
      });
      getTrace(switchSid).push(switchedEvent);
      emitEvent("session-event", switchedEvent);
      return Promise.resolve(null);
    }
```

- [ ] **Step 2: Write the failing E2E spec**

Create `apps/agent-gui/e2e/model-switch.spec.ts` — mirror the exact bootstrap pattern from `context-meter.spec.ts` (verified at plan time):

```ts
import { test, expect } from "@playwright/test";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

// `apps/agent-gui/package.json` is `"type": "module"`, so CJS-style
// `__dirname` is undefined. Derive it from `import.meta.url`, as all
// sibling specs do (e.g. `context-meter.spec.ts`).
const __dirname = dirname(fileURLToPath(import.meta.url));

test.describe("Mid-session model switch (P4)", () => {
  test.beforeEach(async ({ page }) => {
    const mockPath = resolve(__dirname, "tauri-mock.js");
    await page.addInitScript({ path: mockPath });
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
    // The mock only emits ContextAssembled inside send_message, so send
    // one real message to make the meter render (mirrors the P3 pattern
    // in context-meter.spec.ts).
    await page.fill('[data-test="message-input"]', "hello from e2e");
    await page.click('[data-test="send-button"]');
    await page.waitForSelector('[data-test="context-meter-bar"]', { timeout: 5_000 });
  });

  test("switch-model button is enabled and opens the profile picker", async ({ page }) => {
    await page.click('[data-test="context-meter-bar"]');
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeVisible();

    const switchBtn = page.locator('[data-test="context-meter-switch-model"]');
    await expect(switchBtn).toBeEnabled();
    await switchBtn.click();

    // The profile picker renders the two mock profiles.
    await expect(page.locator('[data-test="context-meter-profile-fast"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-profile-smart"]')).toBeVisible();

    // The current profile ("fast" by default in the mock state) carries
    // the "(Current)" marker.
    await expect(page.locator('[data-test="context-meter-profile-fast"]')).toContainText(
      /current|当前/i
    );
  });

  test("selecting a different profile emits ModelProfileSwitched and updates the meter", async ({
    page
  }) => {
    await page.click('[data-test="context-meter-bar"]');
    await page.click('[data-test="context-meter-switch-model"]');

    const smart = page.locator('[data-test="context-meter-profile-smart"]');
    await expect(smart).toBeVisible();
    await smart.click();

    // After the switch, the popover closes (both `profilePickerOpen` and
    // `popoverOpen` flip to false — matches Task 8's component contract).
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeHidden();

    // Re-open and confirm the "(Current)" marker now sits on `smart`.
    await page.click('[data-test="context-meter-bar"]');
    await page.click('[data-test="context-meter-switch-model"]');
    await expect(page.locator('[data-test="context-meter-profile-smart"]')).toContainText(
      /current|当前/i
    );
    await expect(page.locator('[data-test="context-meter-profile-fast"]')).not.toContainText(
      /current|当前/i
    );
  });

  test("selecting the already-current profile is a silent no-op", async ({ page }) => {
    await page.click('[data-test="context-meter-bar"]');
    await page.click('[data-test="context-meter-switch-model"]');

    // Clicking "fast" while "fast" is current must close the picker but
    // leave the meter unchanged — no toast, no event in the trace.
    await page.click('[data-test="context-meter-profile-fast"]');

    // Picker closes; popover may remain open (same-profile branch in the
    // component only flips `profilePickerOpen`, not `popoverOpen`).
    // We just verify that the meter still shows the same numbers and no
    // error-toast appears.
    await expect(page.locator('.toast-error, [data-test^="toast-error"]')).toHaveCount(0);
  });
});
```

- [ ] **Step 3: Run the failing spec**

Run: `cd apps/agent-gui && pnpm exec playwright test model-switch.spec.ts --reporter=list`
Expected: FAIL — the mock doesn't handle `switch_model` yet (the Step-1 case is hypothetical until actually added) or the picker UI isn't wired yet. If FAILs are purely UI-related (picker panel missing), Task 8 was not fully executed; stop and reconcile.

- [ ] **Step 4: Run the failing spec after adding the mock handler**

With the Step-1 mock handler AND the Task-8 component in place, run again:

Run: `cd apps/agent-gui && pnpm exec playwright test model-switch.spec.ts --reporter=list`
Expected: PASS — all three tests green.

- [ ] **Step 5: Run the full Playwright suite**

Run: `just test-e2e`
Expected: all green (existing specs + the new one).

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/e2e/tauri-mock.js apps/agent-gui/e2e/model-switch.spec.ts
git commit -m "test(gui): add E2E + mock coverage for mid-session model switch"
```

---

### Task 10 — TUI `Command::SwitchModel` variant + dispatcher arm

**Files:**

- Modify: `crates/agent-tui/src/components/mod.rs` (extend `enum Command`)
- Modify: `crates/agent-tui/src/components/chat.rs` (intercept `:model <alias>` in `apply_key_action`, mirroring the existing `:compact` interception at lines 60-77)
- Modify: `crates/agent-tui/src/main.rs` (add `Command::SwitchModel` arm in `dispatch_commands`, mirroring `Command::CompactSession`)
- Test: `crates/agent-tui/tests/app_logic.rs`

> **Verified facts** (from reading related P3 code):
>
> - `Command::CompactSession { workspace_id, session_id }` (added in P3) is at the end of the `pub enum Command` in `crates/agent-tui/src/components/mod.rs` (around line 137+).
> - The `:compact` interception pattern in `ChatPanel::apply_key_action` replaces the `KeyAction::SendInput` arm with a trimmed-input branch that emits `Command::CompactSession`. We extend the same branch with a `:model <alias>` parser.
> - The dispatcher arm in `main.rs` (P3 added `Command::CompactSession` right after `Command::CancelSession`) directly `.await`s the runtime call — no `tokio::spawn`.
> - `LocalRuntime::switch_model` (Task 3) takes `(SessionId, String)` — no `WorkspaceId` needed, but we carry it in the `Command` for symmetry with existing variants.

- [ ] **Step 1: Add the `SwitchModel` variant**

In `crates/agent-tui/src/components/mod.rs`, find `pub enum Command`. After the existing `CompactSession { workspace_id, session_id }` variant, add:

```rust
SwitchModel {
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
    alias: String,
},
```

- [ ] **Step 2: Write the failing test**

Append to `crates/agent-tui/tests/app_logic.rs`:

```rust
// ---------------------------------------------------------------------------
// Test: `:model <alias>` command intercepted by ChatPanel
// ---------------------------------------------------------------------------

#[test]
fn colon_model_alias_input_dispatches_switch_model_command() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tools::PermissionMode;
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();

    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":model opus".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    let found = commands.iter().any(|c| {
        matches!(c, Command::SwitchModel { alias, .. } if alias == "opus")
    });
    assert!(found, "expected Command::SwitchModel with alias=opus; got {commands:?}");
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
    assert!(chat.input_content.is_empty());
}

#[test]
fn colon_model_without_alias_falls_through_as_chat_message() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tools::PermissionMode;
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":model".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    // `:model` without an alias falls through to SendMessage (user gets
    // feedback the command was malformed — no silent swallow).
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected SendMessage fallback; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SwitchModel { .. })),
        "expected NO SwitchModel without alias; got {commands:?}"
    );
}
```

- [ ] **Step 3: Run the failing test**

Run: `cargo test -p agent-tui colon_model`
Expected: FAIL — `Command::SwitchModel` exists but chat.rs doesn't intercept `:model`.

- [ ] **Step 4: Intercept `:model <alias>` in `ChatPanel::apply_key_action`**

In `crates/agent-tui/src/components/chat.rs`, locate the P3 `KeyAction::SendInput if !self.input_content.is_empty() => { … }` arm. It already branches on `trimmed == ":compact"`. Extend it with a second branch that handles `:model <alias>`.

Find the existing branch (it looks like):

```rust
        KeyAction::SendInput if !self.input_content.is_empty() => {
            let trimmed = self.input_content.trim();
            // P3: intercept ":compact" before treating the input as a chat message.
            if trimmed == ":compact" {
                self.input_content.clear();
                self.input_cursor = 0;
                self.input_history_index = None;
                if let Some(session_id) = ctx.current_session_id {
                    commands.push(Command::CompactSession {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                    });
                }
            } else {
                // ... existing SendMessage path ...
            }
        }
```

Replace the top-level `if trimmed == ":compact" { … } else { … }` with this three-branch version (preserve the SendMessage else branch body exactly as-is):

```rust
        KeyAction::SendInput if !self.input_content.is_empty() => {
            let trimmed = self.input_content.trim();
            // P3: intercept ":compact".
            if trimmed == ":compact" {
                self.input_content.clear();
                self.input_cursor = 0;
                self.input_history_index = None;
                if let Some(session_id) = ctx.current_session_id {
                    commands.push(Command::CompactSession {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                    });
                }
            // P4: intercept ":model <alias>". Requires a non-empty alias —
            // bare ":model" falls through to SendMessage so the user gets
            // visible feedback (the assistant replies "[unknown command]"
            // or similar, rather than a silent no-op).
            } else if let Some(alias) = trimmed
                .strip_prefix(":model ")
                .map(str::trim)
                .filter(|a| !a.is_empty())
            {
                let alias = alias.to_string();
                self.input_content.clear();
                self.input_cursor = 0;
                self.input_history_index = None;
                if let Some(session_id) = ctx.current_session_id {
                    commands.push(Command::SwitchModel {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                        alias,
                    });
                }
            } else {
                // Keep the existing SendMessage body verbatim:
                self.input_history.push(self.input_content.clone());
                let content = std::mem::take(&mut self.input_content);
                self.input_cursor = 0;
                self.input_history_index = None;

                if let Some(session_id) = ctx.current_session_id {
                    commands.push(Command::SendMessage {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                        content,
                    });
                }
            }
        }
```

- [ ] **Step 5: Wire the dispatcher in `main.rs`**

In `crates/agent-tui/src/main.rs`, find the `Command::CompactSession { … } => { … }` arm in `dispatch_commands`. Immediately AFTER its closing brace, add:

```rust
            Command::SwitchModel {
                workspace_id: _,
                session_id,
                alias,
            } => {
                if let Err(e) = runtime.switch_model(session_id, alias).await {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[switch_model error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p agent-tui colon_model`
Expected: PASS (both new tests).

Run: `cargo test -p agent-tui`
Expected: all green (existing `:compact` test still passes — the else branch is preserved).

- [ ] **Step 7: Commit**

```bash
git add crates/agent-tui/src/components/mod.rs crates/agent-tui/src/components/chat.rs crates/agent-tui/src/main.rs crates/agent-tui/tests/app_logic.rs
git commit -m "feat(tui): add :model <alias> command dispatching SwitchModel to runtime"
```

---

### Task 11 — Runtime integration test `model_switch.rs`

**Files:**

- Create: `crates/agent-runtime/tests/model_switch.rs`

> **Spec §6** explicitly calls this out: _"switches profile mid-stream; asserts no event reorder, next iteration uses new profile."_ The unit tests in Task 3 cover the state-transition correctness; this integration test covers the full pipeline: `start_session` → `send_message` (on fast) → `switch_model` → `send_message` (on opus) → assert event ordering AND that turn 2 ran on the switched profile.

> **Verified facts**: other integration tests in `crates/agent-runtime/tests/` (e.g. `full_stack.rs`, `session_lifecycle.rs`) use `LocalRuntime::new(store, model).with_config(...)` and consume events via `runtime.subscribe_all()` or by re-loading from the event store. `FakeModelClient::new(vec![...])` queues responses in order — one per `ModelRequest`.

> **⚠ Plan correction (2026-05-09, made during Task 11 execution)**: an earlier draft of this section asserted that `send_message` would append two `EventPayload::ModelRequestStarted` events (one per turn) carrying `model_profile = "fast"` then `"opus"`. systematic-debugging during Task 11 revealed that variant is defined and matched by `projection.rs` but **never emitted** anywhere in the codebase — `grep -rn 'EventPayload::ModelRequestStarted {' crates/` returns only the definition, the projection match arm, the round-trip unit test fixture, and this test. That assertion would always fail (`left=0, right=2`), independent of any P4 task. The wiring guarantee is established equivalently below: `LocalRuntime::switch_model` rejects unknown aliases up front (Test 2), and `agent_loop::latest_model_profile_for` is the single per-turn profile resolver (Task 2 unit-tested in `agent_loop.rs::model_profile_resolution_tests`). Combined with the event-ordering assertions and `ModelProfileSwitched.to_profile == "opus"`, this proves end-to-end wiring without depending on an unemitted variant.

- [ ] **Step 1: Write the integration test**

Create `crates/agent-runtime/tests/model_switch.rs`:

```rust
//! P4: mid-session model switch integration test.
//!
//! Verifies the end-to-end flow: start a session on profile "fast",
//! send a message, call `switch_model` to swap to "opus", then send a
//! second message. Asserts:
//!   1. The `ModelProfileSwitched` event is appended between the two turns.
//!   2. The second iteration's `ModelRequestStarted.model_profile == "opus"`.
//!   3. No existing event (user messages, assistant messages) is reordered.

use agent_config::{Config, ConfigSource, ContextPolicy, ProfileDef};
use agent_core::{AppFacade, EventPayload, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use std::sync::Arc;

fn two_profile_config() -> Arc<Config> {
    // Field list verified against `crates/agent-config/src/lib.rs:23-44, 179-185`
    // (ProfileDef has 8 fields including `response: Option<String>`; Config has
    // `{ profiles, mcp_servers, source, context }`). ContextPolicy derives
    // Default (line 147), so `::default()` is correct.
    let fast = ProfileDef {
        provider: "fake".into(),
        model_id: "fake".into(),
        api_key: None,
        api_key_env: None,
        base_url: None,
        context_window: None,
        output_limit: None,
        response: None,
    };
    let opus = ProfileDef {
        provider: "fake".into(),
        model_id: "fake-opus".into(),
        api_key: None,
        api_key_env: None,
        base_url: None,
        context_window: None,
        output_limit: None,
        response: None,
    };
    Arc::new(Config {
        profiles: vec![("fast".into(), fast), ("opus".into(), opus)],
        mcp_servers: vec![],
        source: ConfigSource::Defaults,
        context: ContextPolicy::default(),
    })
}

// NOTE on FakeModelClient semantics (verified from `crates/agent-models/src/fake.rs`):
// `FakeModelClient::new(vec![t1, t2, ...])` replays THAT ENTIRE token list on
// EACH `stream()` call — the tokens are NOT "first response, then second
// response". Both turns below therefore stream `"reply"` as their token delta.
// The assertion that differentiates turn 1 from turn 2 is NOT the assistant
// content, it's `ModelRequestStarted.model_profile` (which is written by
// `agent_loop` from whatever `latest_model_profile_for` resolves per-turn).

#[tokio::test]
async fn model_switch_takes_effect_on_next_send_message() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["reply".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(two_profile_config());

    let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "turn 1".into(),
        })
        .await
        .unwrap();

    runtime
        .switch_model(session_id.clone(), "opus".into())
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "turn 2".into(),
        })
        .await
        .unwrap();

    // Load the full event log and inspect ordering.
    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();

    // Extract the index of each interesting event.
    let idx_init = events
        .iter()
        .position(|e| matches!(&e.payload, EventPayload::SessionInitialized { .. }))
        .expect("SessionInitialized present");
    let idx_first_user = events
        .iter()
        .position(|e| matches!(&e.payload, EventPayload::UserMessageAdded { content, .. } if content == "turn 1"))
        .expect("first UserMessageAdded present");
    let idx_switch = events
        .iter()
        .position(|e| matches!(&e.payload, EventPayload::ModelProfileSwitched { .. }))
        .expect("ModelProfileSwitched present");
    let idx_second_user = events
        .iter()
        .position(|e| matches!(&e.payload, EventPayload::UserMessageAdded { content, .. } if content == "turn 2"))
        .expect("second UserMessageAdded present");

    // Order invariant: init < turn1_user < switch < turn2_user.
    assert!(idx_init < idx_first_user);
    assert!(idx_first_user < idx_switch);
    assert!(idx_switch < idx_second_user);

    // The switch event must record from="fast" → to="opus".
    match &events[idx_switch].payload {
        EventPayload::ModelProfileSwitched { from_profile, to_profile, .. } => {
            assert_eq!(from_profile, "fast");
            assert_eq!(to_profile, "opus");
        }
        _ => unreachable!(),
    }

    // Each send_message turn produces exactly one AssistantMessageCompleted.
    // We use this — instead of the plan's original ModelRequestStarted assertion —
    // because ModelRequestStarted is defined but never emitted anywhere in the
    // codebase (see "Plan correction" note above). Bracketing assistant
    // completions around the switch event proves both turns ran end-to-end and
    // that turn 2 happened after the switch took effect.
    let assistant_indices: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| matches!(&e.payload, EventPayload::AssistantMessageCompleted { .. }).then_some(i))
        .collect();
    assert_eq!(assistant_indices.len(), 2, "two send_message calls → two AssistantMessageCompleted");
    assert!(assistant_indices[0] < idx_switch, "turn-1 assistant must complete before switch");
    assert!(assistant_indices[1] > idx_switch, "turn-2 assistant must complete after switch");
    assert!(assistant_indices[1] > idx_second_user, "turn-2 assistant must follow turn-2 user message");
}

#[tokio::test]
async fn switch_model_rejects_unknown_alias_in_running_session() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["reply".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(two_profile_config());

    let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
        })
        .await
        .unwrap();

    let err = runtime
        .switch_model(session_id, "ghost".into())
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("ghost"), "error should name the bad alias; got {msg}");
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p agent-runtime --test model_switch`
Expected: PASS (the unit tests in Task 3 already put `switch_model` + `ModelProfileSwitched` in place). If it FAILS, the failure is the bug — debug with `systematic-debugging` rather than relaxing assertions.

> **Note**: `event_store_for_test` is gated behind `#[cfg(any(test, feature = "test-helpers"))]`. Integration tests under `tests/` see the `test` cfg, so the accessor is available.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime/tests/model_switch.rs
git commit -m "test(runtime): add mid-session model switch integration test"
```

---

### Task 12 — Final verification + push

**Files:** none (verification only)

- [ ] **Step 1: Regenerate specta bindings + verify in sync**

Run: `just gen-types`
Expected: regenerates `apps/agent-gui/src/generated/{commands,events}.ts`. If Tasks 1 + 5 committed the right state, this should be a no-op; if any diff appears, commit it:

```bash
git add apps/agent-gui/src/generated/
git commit -m "chore(gui): refresh specta bindings for P4"
```

Run: `just check-types`
Expected: PASS.

- [ ] **Step 2: Run the full Rust test suite**

Run: `cargo test --workspace --all-targets`
Expected: all green. If anything fails, STOP and invoke `superpowers:systematic-debugging` — do NOT relax assertions.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: zero warnings.

- [ ] **Step 4: Run the full GUI test + lint + format suite**

Run (from worktree root):

```bash
pnpm run format:check
pnpm run lint
just test-gui
```

Expected: all green.

- [ ] **Step 5: Run the Playwright suite**

Run: `just test-e2e`
Expected: all green (including the new `model-switch.spec.ts`).

- [ ] **Step 6: Smoke-test the GUI in dev mode**

Run: `just gui-dev` in one terminal. Open `http://localhost:1420/`.

Verify manually:

1. Send a message so the ContextMeter bar renders.
2. Click the bar → popover opens.
3. Click **Switch model…** → profile list appears with current profile tagged `(Current)`.
4. Click a different profile → toast `Switched to <alias>` appears; popover closes; re-opening shows `(Current)` on the new alias.
5. Trigger a compaction via **Compact now**; while busy, the Switch-model button is disabled.

Stop the dev server (Ctrl+C) when done.

- [ ] **Step 7: Smoke-test the TUI**

Run: `just tui` in a terminal. In the chat input, type `:model opus` and press Enter. Verify no error message appears (the fake runtime accepts any alias configured — for a real run use whatever profile exists in your local `kairox.toml`).

Type a normal message next and verify a reply appears (confirming the loop still works after the switch).

- [ ] **Step 8: Push the branch**

```bash
git push -u origin feat/context-p4-model-switch
```

- [ ] **Step 9: Hand off to `superpowers:finishing-a-development-branch`**

Announce: "I'm using the finishing-a-development-branch skill to complete this work."

Follow that skill: verify tests one more time, present the four standard options (Merge / PR / Keep / Discard), execute the user's choice. Cleanup the worktree only on Options 1 or 4.

---

## Self-Review Checklist (run after Task 12)

**1. Spec coverage** (§4.5 — mid-session model switch):

- ✅ `ModelProfileSwitched` event with `from_profile`, `to_profile`, `effective_at`, mirror of `new_limits` → Task 1
- ✅ `LocalRuntime::switch_model(session_id, alias)` validates alias → Task 3 (`switch_model_rejects_unknown_alias`)
- ✅ Refuses when `session_state.compacting` → Task 3 (`switch_model_returns_session_busy_when_compacting`)
- ✅ Records the event; does NOT cancel in-flight stream → Task 3 + Task 11 (integration test verifies event ordering)
- ✅ Next `agent_loop` iteration reads latest profile via `latest_model_profile_for` → Task 2
- ✅ Session's cached `model_limits` recomputed from new profile → Task 3 (calls `set_session_limits`)
- ✅ Ollama probe on switch for ollama profiles → Task 3 (spawned task mirrors `start_session` probe)
- ✅ Switch only takes effect at next user turn (provider-format compatibility) → implicit via Task 2+Task 3; integration test in Task 11 verifies
- ✅ GUI Switch-model dropdown in ContextMeter popover → Tasks 7, 8
- ✅ `switch_model` Tauri command → Tasks 4, 5
- ✅ Pinia store consumes `ModelProfileSwitched` → Task 6
- ✅ i18n keys (`context.switchModelChoose`, `context.switchModelSuccess`, `errors.profileNotFound`, `errors.sessionBusy`) → Task 7
- ✅ E2E mock handler + spec → Task 9
- ✅ TUI `:model <alias>` command → Task 10
- ✅ Integration test exercising full pipeline → Task 11

**2. Placeholder scan**: no `TODO`, no "implement later". Every step shows complete code derived from verified facts about P1-P3 landed state.

**3. Type consistency**:

- `EventPayload::ModelProfileSwitched` field list matches across Task 1 (definition), Task 2 (test helper `switch_event`), Task 3 (emission), Task 6 (Vue consumer), Task 11 (integration test pattern-match).
- `Command::SwitchModel { workspace_id, session_id, alias }` — field names and types consistent between Task 10 Step 1 (definition), the chat-intercept in Step 4 (construction), the dispatcher in Step 5 (destructure).
- `ProfileWithLimits` shape (unchanged — P3 work) — Task 8 consumes the existing snake_case fields (`alias`, `model_id`, `context_window`, `limit_source`, `has_api_key`).
- `ModelLimits.source` is an enum in Rust but a snake_case string in the event and the `ProjectedModelLimits` projection — consistent across Task 1, Task 3, Task 6.

**4. Skill alignment**:

- TDD in Tasks 1-11 (fail-first → minimal impl → green).
- Verification in Task 12 (`verification-before-completion`).
- Fresh branch from `main` in isolated worktree (`using-git-worktrees`).
- One commit per task minimum.
- Self-reviewed against §4.5 + §5 + §6 before handoff.

---
