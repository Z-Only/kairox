# Race-Free Auto-Compaction Design

> Date: 2026-05-27
> Status: Approved direction, single-PR slice
> Scope: `agent-runtime` execution actor integration for context auto-compaction; small `agent-core` event additions

## 1. Background

Kairox already has auto-compaction wiring. After PRs #517–#532 it sits in the wrong place relative to the new session execution runtime:

- `agent_loop::turn_context::prepare_turn_context` fires the compaction at the **start** of every turn. When `should_trigger_auto_compaction` returns true it does `tokio::spawn(compact_session(...))` and returns immediately so the turn keeps running.
- `compact_session` writes new domain events into the same session that the in-flight turn is also writing to.
- Compaction also goes through `SessionFacade::compact_session` for explicit user-triggered compaction; after PR #532 that path is queued behind active turns through `SessionExecutionRuntime::run_operation`.
- The `DagExecution` path inside `LocalRuntimeTurnExecutor` does not call the agent loop trigger at all, so DAG-only sessions never auto-compact.

This produces three concrete problems:

- A turn can be writing `AssistantMessageStreaming`/`ToolInvocationCompleted` events while a spawned compaction writes `ContextCompactionCompleted` and rewrites session state. Ordering inside the event log becomes non-deterministic.
- `should_trigger_auto_compaction` reads `session_states.compacting`, but that flag is only set on entry to the compactor body — between the check and the flag flip, a second turn can fire a duplicate compaction.
- The system silently skips auto-compaction in two situations (`AlreadyCompacting`, `ThresholdDisabled`) with no domain event, so the GUI/TUI cannot show why a session that crossed the threshold did not compact.

## 2. Goal

Make auto-compaction race-free and observable while keeping the existing trigger policy intact.

- Move the auto-compaction trigger from turn **start** to turn **end**.
- Route the trigger through the same `SessionExecutionRuntime::run_operation` path that explicit user-triggered compaction already uses, so the actor serializes it behind any active turn.
- Cover both `SingleStep` and `DagExecution` paths from one call site in `LocalRuntimeTurnExecutor::execute_turn`.
- Add a `ContextCompactionSkipped` domain event for the two skip reasons that matter, so UIs can explain inaction.
- Keep `should_trigger_auto_compaction` as the single decision function. Keep the existing `CompactionReason::Threshold { ratio }` payload.

This is intentionally a **one-PR slice**. It is not a redesign of the compactor.

## 3. Non-Goals

- Do not change `ContextCompactor` behavior, prompt, or chunking.
- Do not change the default `auto_compact_threshold` or its config surface.
- Do not introduce a generic `PostTurnHook` trait or a hook registry; YAGNI.
- Do not add user-facing copy for the new skipped event in this PR; emitting the event is enough for downstream consumers to render later.
- Do not change `SessionFacade::compact_session` semantics for explicit user requests.
- Do not change `SessionBusy` rejection rules for `send_message` while a turn or compaction is in flight; turn-end enqueue happens after `execute_turn` returns and uses a non-blocking schedule.

## 4. Architecture

### 4.1 Boundary Decision

The actor enqueues the compaction but does **not** block the next user input. Concretely, `LocalRuntimeTurnExecutor::execute_turn` returns to its caller as soon as the turn finishes; the auto-compaction is scheduled into the same `SessionExecutionRuntime` so that:

- If the user sends nothing else, the actor runs the compaction as its next operation.
- If the user sends another `RunTurn` first, that turn runs first; the queued compaction runs after it.
- A compaction that is already queued cannot be queued again for the same session in the same window — the `compacting` flag plus the decision function continue to guard duplicates.

This trades a small risk (a fast user follow-up can defer compaction by one turn) for the property that the user is never blocked at a prompt boundary by a compaction the system chose on its own.

### 4.2 Trigger Move

The current trigger block in `agent_loop/turn_context.rs` (the `tokio::spawn` near the end of `prepare_turn_context`) is **deleted**. The decision function `should_trigger_auto_compaction` and the `CompactionReason::Threshold { ratio }` variant stay where they are.

A new `maybe_schedule_auto_compaction` function lives next to `LocalRuntimeTurnExecutor::execute_turn`. After `execute_turn` finishes a `SingleStep` or `DagExecution` turn successfully (including the case where the agent loop ended with a normal `AssistantMessageCompleted`), the executor:

1. Reads `session_states[session_id]` once: pulls `last_estimated_tokens`, `compacting` flag, and `model_limits` snapshot.
2. Reconstructs the `ContextUsage` ratio using the same `context_budget::build_budget(&limits)` and the recorded estimate.
3. Calls `should_trigger_auto_compaction(&usage, threshold, already_compacting)`.
4. On `true`, calls `SessionExecutionRuntime::run_operation(session_id, Box::pin(compact_session(...)))`.
5. On `false`, if the reason is `already_compacting || threshold_disabled`, emits `ContextCompactionSkipped` and returns. Below-threshold returns silently.

`run_operation` is fire-and-forget from `execute_turn`'s perspective. The executor calls `tokio::spawn` around `rt.run_operation(session_id, Box::pin(compact_session(...)))` so awaiting the queued operation does not block return to the caller. Inside the actor mailbox the operation is still strictly serialized after any already-mailboxed `RunTurn`. The spawned future is detached; failures are handled per §4.5.

### 4.3 New Event

Add to `agent_core::EventPayload`:

```rust
ContextCompactionSkipped {
    reason: CompactionSkipReason,
    ratio: f32,
},
```

with

```rust
pub enum CompactionSkipReason {
    AlreadyCompacting,
    ThresholdDisabled,
}
```

`BelowThreshold` is **not** modeled. Below-threshold is the steady state and would flood the event log.

Privacy classification: `PrivacyClassification::MinimalTrace`, same as `ContextAssembled`.

### 4.4 Files Touched

- `crates/agent-core/src/events.rs` — add `CompactionSkipReason` enum + `ContextCompactionSkipped` variant + Specta derives.
- `crates/agent-runtime/src/agent_loop/turn_context.rs` — remove the `tokio::spawn(compact_session(...))` block (~38 lines).
- `crates/agent-runtime/src/facade_turn_executor.rs` — add `maybe_schedule_auto_compaction` and call it at the tail of `execute_turn` for both branches.
- Tests: extend `crates/agent-runtime/src/facade_runtime.rs` (or a new sibling test module) with the four integration tests below.

Estimated diff: ~3 production files + tests; comparable in scope to PR #532.

### 4.5 Failure Behavior

If the queued auto-compaction fails inside `compact_session`, the compactor already emits a `ContextCompactionFailed` event. The new turn-end scheduler does not retry. The next turn that crosses the threshold re-triggers it.

If `run_operation` itself returns an error (actor stopped, shutdown), the scheduler logs at `warn!` and drops the request. The next turn will retry.

## 5. Test Plan

All tests run under `cargo test -p agent-runtime --lib`.

1. `auto_compaction_queues_after_threshold_turn`
   Drive `LocalRuntimeTurnExecutor::execute_turn` with a `FakeModelClient` that returns enough usage to push the recorded estimate over threshold. After the executor returns, assert the actor's operation queue contains one compaction op, then drive the actor and assert a `ContextCompactionCompleted` event is appended after the turn's `AssistantMessageCompleted`.

2. `auto_compaction_emits_skipped_when_already_compacting`
   Pre-set `session_states[id].compacting = true`. Run a turn over threshold. Assert `ContextCompactionSkipped { reason: AlreadyCompacting, ratio }` event is appended after the turn and no second compaction runs.

3. `auto_compaction_skipped_when_threshold_disabled`
   Set `config.context.auto_compact_threshold = 1.0`. `should_trigger_auto_compaction` already returns `false` when `threshold >= 1.0`; the scheduler treats this as `ThresholdDisabled`. Run a turn whose usage ratio would otherwise trip. Assert `ContextCompactionSkipped { reason: ThresholdDisabled, ratio }` and no compaction.

4. `dag_turn_also_triggers_auto_compaction`
   Route a `DagExecution` request that ends with usage over threshold. Assert the scheduler still calls `run_operation`, proving the DAG path is covered.

Keep existing unit tests for `should_trigger_auto_compaction` and the existing `compact_session_queues_behind_active_actor_turn` test from PR #532 unchanged. They continue to verify the decision function and the explicit-trigger path.

## 6. Acceptance

The change is complete when:

- `prepare_turn_context` no longer spawns a compaction.
- Both `SingleStep` and `DagExecution` turns end by consulting `should_trigger_auto_compaction` and, on true, enqueuing through `SessionExecutionRuntime::run_operation`.
- The four new tests pass alongside the full `cargo test --workspace --all-targets`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `bun run format:check`, `bun run lint`.
- `just gen-types` regenerates `apps/agent-gui/src/generated/events.ts` to include the new event variants; no manual edits to the generated file.
- The Playwright mock at `apps/agent-gui/e2e/tauri-mock.js` is updated if it asserts the closed set of event types; otherwise unchanged.

## 7. Risks

- **Fast follow-up turn defers compaction.** If the user sends another turn within milliseconds, the queued compaction runs after that next turn. Accepted: same total throughput, no missed user input.
- **Estimated vs real token drift.** The trigger uses `last_estimated_tokens` corrected by the per-session `UsageCorrector`. This is the same data the old trigger used; no regression but no improvement either.
- **Specta surface change.** Adding an `EventPayload` variant is an additive change but still requires `just gen-types`. Downstream GUI rendering may want to ignore unknown variants; today's GUI already falls back on unknown payloads.
- **`run_operation` ordering vs `RunTurn`.** Both go through the same actor mailbox. The actor's FIFO guarantees turn-end compaction never overtakes an already-mailboxed `RunTurn` from the user.
