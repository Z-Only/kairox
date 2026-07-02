# Model Usage Recording Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist actual model token usage from provider `ModelEvent::Completed` events into the session event stream and expose compact totals through session diagnostics.

**Architecture:** Reuse the existing event-sourced path instead of adding a new store table. Add a small `ModelUsageRecorded` core event that mirrors `agent_models::ModelUsage` without introducing an `agent-core -> agent-models` dependency; emit it from the stream handler when a terminal completion carries usage; aggregate it in Tauri session diagnostics.

**Tech Stack:** Rust (`agent-core`, `agent-runtime`, `agent-gui-tauri`), Specta-generated TypeScript bindings, existing SQLite event store and Tauri diagnostics command.

---

## File Map

- Modify `crates/agent-core/src/events.rs`: add `ModelUsageRecorded` payload fields and `event_type()` arm.
- Modify `crates/agent-core/tests/event_roundtrip.rs`: prove JSON roundtrip for the new event.
- Modify `crates/agent-core/tests/event_coverage.rs`: add the new payload to coverage fixtures.
- Modify `crates/agent-runtime/src/agent_loop/stream_handler.rs`: emit `ModelUsageRecorded` when a non-progress `Completed` event carries usage.
- Modify `crates/agent-runtime/tests/agent_loop/text_turns.rs`: add a focused runtime test that a text turn records usage.
- Modify `apps/agent-gui/src-tauri/src/commands.rs`: add diagnostics DTOs for per-session totals.
- Modify `apps/agent-gui/src-tauri/src/commands/session.rs`: aggregate usage events in `summarize_trace_export`.
- Run type generation so `apps/agent-gui/src/generated/{commands.ts,events.ts}` reflect the new event and diagnostics fields.

---

### Task 1: Add Core Event Shape

**Files:**

- Modify: `crates/agent-core/src/events.rs`
- Test: `crates/agent-core/tests/event_roundtrip.rs`
- Test: `crates/agent-core/tests/event_coverage.rs`

- [x] **Step 1: Write the failing roundtrip test**

Add this test next to the existing model event roundtrip tests in `crates/agent-core/tests/event_roundtrip.rs`:

```rust
#[test]
fn model_usage_recorded_roundtrips() {
    let event = make_event(EventPayload::ModelUsageRecorded {
        model_profile: "fast".into(),
        input_tokens: 123,
        output_tokens: 45,
        cache_creation_input_tokens: Some(10),
        cache_read_input_tokens: Some(20),
    });
    assert_eq!(roundtrip(&event), event);
}
```

- [x] **Step 2: Run the focused failing test**

Run:

```bash
cargo test -p agent-core --test event_roundtrip model_usage_recorded_roundtrips
```

Expected: compile failure because `EventPayload::ModelUsageRecorded` does not exist.

- [x] **Step 3: Add the minimal event variant**

In `crates/agent-core/src/events.rs`, add this variant after `ModelRequestStarted`:

```rust
ModelUsageRecorded {
    model_profile: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    input_tokens: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    output_tokens: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cache_creation_input_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cache_read_input_tokens: Option<u64>,
},
```

Add the `event_type()` arm:

```rust
Self::ModelUsageRecorded { .. } => "ModelUsageRecorded",
```

- [x] **Step 4: Update coverage fixtures**

Add this payload next to `ModelRequestStarted` in `all_payloads()` and the serialization coverage fixture in `crates/agent-core/tests/event_coverage.rs`:

```rust
EventPayload::ModelUsageRecorded {
    model_profile: "fast".into(),
    input_tokens: 123,
    output_tokens: 45,
    cache_creation_input_tokens: Some(10),
    cache_read_input_tokens: Some(20),
},
```

- [x] **Step 5: Verify core tests**

Run:

```bash
cargo test -p agent-core --test event_roundtrip model_usage_recorded_roundtrips
cargo test -p agent-core --test event_coverage
```

Expected: both pass.

---

### Task 2: Emit Usage From Runtime

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop/stream_handler.rs`
- Test: `crates/agent-runtime/tests/agent_loop/text_turns.rs`

- [x] **Step 1: Write the failing runtime test**

Add this test in `crates/agent-runtime/tests/agent_loop/text_turns.rs` near `agent_loop_ignores_usage_only_completion_before_text`:

```rust
#[tokio::test]
async fn agent_loop_records_terminal_model_usage() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = EarlyUsageThenTextModel;
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-terminal-model-usage".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let usages = trace
        .iter()
        .filter_map(|event| match &event.payload {
            agent_core::EventPayload::ModelUsageRecorded {
                input_tokens,
                output_tokens,
                ..
            } => Some((*input_tokens, *output_tokens)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(usages, vec![(5, 1)]);
}
```

- [x] **Step 2: Run the focused failing test**

Run:

```bash
cargo test -p agent-runtime agent_loop_records_terminal_model_usage
```

Expected: fail because no `ModelUsageRecorded` event is emitted.

- [x] **Step 3: Emit the event in the completed branch**

In `crates/agent-runtime/src/agent_loop/stream_handler.rs`, inside the `Ok(agent_models::ModelEvent::Completed { usage: real_usage })` branch, keep the existing usage-corrector behavior and emit after the `usage_only_progress` check:

```rust
if let Some(u) = real_usage {
    let usage_only_progress =
        assistant_text.is_empty() && tool_calls.is_empty() && u.output_tokens == 0;
    let mut states = deps.session_states.lock().await;
    if let Some(entry) = states.get_mut(request.session_id.as_str()) {
        let estimated = entry.last_estimated_tokens;
        if estimated > 0 {
            entry.usage_corrector.update(u.input_tokens, estimated);
        }
    }
    if usage_only_progress {
        continue;
    }
    let event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::ModelUsageRecorded {
            model_profile: current_request.model_profile.clone(),
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            cache_creation_input_tokens: u.cache_creation_input_tokens,
            cache_read_input_tokens: u.cache_read_input_tokens,
        },
    );
    append_and_broadcast(&**deps.store, deps.event_tx, &event).await?;
}
```

- [x] **Step 4: Verify runtime tests**

Run:

```bash
cargo test -p agent-runtime agent_loop_records_terminal_model_usage
cargo test -p agent-runtime agent_loop_ignores_usage_only_completion_before_text
```

Expected: both pass. The second test protects against recording usage-only progress chunks as terminal usage.

---

### Task 3: Surface Usage In Diagnostics And Types

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/session.rs`
- Generated: `apps/agent-gui/src/generated/commands.ts`
- Generated: `apps/agent-gui/src/generated/events.ts`

- [x] **Step 1: Write the failing diagnostics test**

In `apps/agent-gui/src-tauri/src/commands/session.rs`, add a test beside existing `summarize_trace_export_*` tests:

```rust
#[test]
fn summarize_trace_export_totals_model_usage() {
    let trace = TraceExport::new(
        SessionId::from_string("session-usage"),
        vec![
            make_event(EventPayload::ModelUsageRecorded {
                model_profile: "fast".into(),
                input_tokens: 100,
                output_tokens: 40,
                cache_creation_input_tokens: Some(7),
                cache_read_input_tokens: Some(11),
            }),
            make_event(EventPayload::ModelUsageRecorded {
                model_profile: "fast".into(),
                input_tokens: 20,
                output_tokens: 5,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: Some(3),
            }),
        ],
    );

    let summary = summarize_trace_export(&trace);
    assert_eq!(summary.model_usage.total_input_tokens, 120);
    assert_eq!(summary.model_usage.total_output_tokens, 45);
    assert_eq!(summary.model_usage.total_cache_creation_input_tokens, 7);
    assert_eq!(summary.model_usage.total_cache_read_input_tokens, 14);
    assert_eq!(summary.model_usage.request_count, 2);
    assert_eq!(summary.model_usage.by_profile.len(), 1);
    assert_eq!(summary.model_usage.by_profile[0].model_profile, "fast");
    assert_eq!(summary.model_usage.by_profile[0].input_tokens, 120);
    assert_eq!(summary.model_usage.by_profile[0].output_tokens, 45);
}
```

- [x] **Step 2: Add diagnostics DTOs**

In `apps/agent-gui/src-tauri/src/commands.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type, Default)]
pub struct ModelUsageDiagnosticsResponse {
    pub request_count: u32,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_cache_creation_input_tokens: u32,
    pub total_cache_read_input_tokens: u32,
    pub by_profile: Vec<ModelUsageByProfileDiagnosticsResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
pub struct ModelUsageByProfileDiagnosticsResponse {
    pub model_profile: String,
    pub request_count: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
}
```

Add this field to `SessionDiagnosticsResponse`:

```rust
pub model_usage: ModelUsageDiagnosticsResponse,
```

- [x] **Step 3: Aggregate usage events**

In `summarize_trace_export`, create totals and a keyed `BTreeMap<String, ModelUsageByProfileDiagnosticsResponse>`. In the match arm for `EventPayload::ModelUsageRecorded`, increment request counts and saturating token totals. Convert `u64` to `u32` with `u32::try_from(value).unwrap_or(u32::MAX)` to match existing diagnostics DTO style.

- [x] **Step 4: Run focused diagnostics test**

Run:

```bash
cargo test -p agent-gui-tauri summarize_trace_export_totals_model_usage
```

Expected: pass.

- [x] **Step 5: Regenerate frontend bindings**

Run:

```bash
bun run gen:types
```

Expected: `apps/agent-gui/src/generated/commands.ts` includes `ModelUsageDiagnosticsResponse`, `ModelUsageByProfileDiagnosticsResponse`, and `SessionDiagnosticsResponse.model_usage`; generated event payloads include `ModelUsageRecorded`.

Note: local `just gen-types` was interrupted by StarPoint on `export-specta`. The bindings were regenerated with the same underlying export binaries after allowing/working around the prompt:

```bash
target/debug/export-specta apps/agent-gui/src/generated/commands.ts
cargo run -p agent-gui-tauri --features typegen --bin export-events -- apps/agent-gui/src/generated/events.ts
bun run format:web
```

- [x] **Step 6: Verify scoped checks**

Run:

```bash
cargo test -p agent-core --test event_roundtrip model_usage_recorded_roundtrips
cargo test -p agent-runtime agent_loop_records_terminal_model_usage
cargo test -p agent-gui-tauri summarize_trace_export_totals_model_usage
cargo fmt --all --check
bun run typecheck
```

Expected: all pass.

Actual: focused Rust tests, `cargo fmt --all --check`, `bun run format:check`, and `bun run lint:web` passed. `bunx vue-tsc --noEmit` was checked separately and still fails on existing/global project type issues outside this change; there is no `typecheck` npm script in this workspace.

---

## Final Verification

- [x] Run `cargo test -p agent-core --test event_coverage`.
- [x] Run `cargo test -p agent-runtime agent_loop_ignores_usage_only_completion_before_text`.
- [x] Run `cargo clippy -p agent-core --all-targets -- -D warnings`.
- [x] Run `cargo clippy -p agent-runtime --all-targets -- -D warnings`.
- [x] Run `cargo clippy -p agent-gui-tauri --all-targets -- -D warnings`.
- [x] Run `bun run format:check`.
- [ ] Create commit `feat(runtime): record model usage`.
- [ ] Push `feat/model-usage-recording` and open a ready PR.

## Self-Review

- Spec coverage: covers actual provider usage recording, persistence through event store, trace export inclusion through event stream, and compact diagnostics totals. It intentionally does not implement prices, budget stops, or dashboard UI.
- Placeholder scan: no TBD/TODO/fill-in-later steps.
- Type consistency: event fields use `model_profile`, `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, and `cache_read_input_tokens` consistently across core, runtime, diagnostics, and generated types.
