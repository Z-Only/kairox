# GUI Eval Trace Clarity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Make GUI evaluation sessions distinguish successful final outcomes from intermediate command failures, while making failed tool rows and trajectory completion easier to inspect.

**Architecture:** Add a structured tool invocation input preview to trace events, then consume it in the GUI trace store. Keep task graph semantics unchanged: task failures remain task failures, while non-zero command exits are treated as failed trace rows for trace filtering and display.

**Tech Stack:** Rust event payloads, Specta-generated GUI bindings, Vue 3 Composition API, Pinia, Vitest, Tauri pilot.

---

## Scope

Adopt in this PR:

- Show tool invocation input previews in GUI trace/chat rows once a tool starts.
- Prefer trace entry titles over raw tool ids in the right-side trace list.
- Treat completed command-style trace entries with non-zero `exitCode` as failed for Trace row icon/classes and status filters.
- Surface trajectory completion as a trace entry so the final success/failure is visible near the end of the session timeline.
- Add a small trajectory summary to the Trajectory tab/list so final outcomes are easier to scan.

Do not adopt in this PR:

- Automatic replacement of `New conversation` titles. That needs separate product rules for user-renamed sessions, timing, and historical sessions.
- Large trace noise reclassification beyond trajectory completion and non-zero exit handling.

## Files

- Modify: `crates/agent-core/src/events.rs`
- Modify: `crates/agent-runtime/src/agent_loop/tool_loop.rs`
- Modify: Rust tests that construct `EventPayload::ToolInvocationStarted`
- Modify generated GUI bindings via `just gen-types`
- Modify: `apps/agent-gui/src/stores/trace.ts`
- Modify: `apps/agent-gui/src/components/TraceEntry.vue`
- Modify: `apps/agent-gui/src/components/TraceTimeline.vue`
- Modify: `apps/agent-gui/src/components/TrajectoryViewer.vue`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`
- Test: `apps/agent-gui/src/stores/trace.test.ts`
- Test: `apps/agent-gui/src/components/TraceEntry.test.ts`
- Test: `apps/agent-gui/src/components/TraceTimeline.test.ts`
- Test: `apps/agent-gui/src/components/TrajectoryViewer.test.ts`

### Task 1: Tool Invocation Input Preview

- [x] **Step 1: Write RED store tests**

Add tests in `apps/agent-gui/src/stores/trace.test.ts` for `ToolInvocationStarted.input_preview` populating `input` and a descriptive title, plus a negative case where an empty preview keeps the generic title.

- [x] **Step 2: Run RED**

Run:

```bash
bun --filter agent-gui test -- src/stores/trace.test.ts
```

Expected: FAIL because `ToolInvocationStarted` has no `input_preview` binding and the store does not populate `input` or title from it.

- [x] **Step 3: Implement event field**

Add `input_preview` to `EventPayload::ToolInvocationStarted` in `crates/agent-core/src/events.rs`:

```rust
ToolInvocationStarted {
    invocation_id: String,
    tool_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    input_preview: String,
},
```

In `crates/agent-runtime/src/agent_loop/tool_loop.rs`, pass `format!("{}({})", tc.name, tc.arguments)` for normal tools and task confirmation tools.

Update Rust tests that instantiate `ToolInvocationStarted` with `input_preview: String::new()` unless the test needs a meaningful preview.

- [x] **Step 4: Generate bindings**

Run:

```bash
just gen-types
```

Expected: generated command/event TypeScript reflects `input_preview: string`.

- [x] **Step 5: Implement GUI store consumption**

In `apps/agent-gui/src/stores/trace.ts`, add a helper that converts preview text into a compact title suffix. Prefer JSON command extraction for previews like `shell.exec({"command":"..."})`, falling back to the first non-empty line capped around 120 chars.

When `ToolInvocationStarted` arrives:

- if an entry already exists for the invocation, update `toolId`, `input`, `title`, `rawEvent`, and keep status running;
- otherwise create the running entry with the input preview and derived title.

- [x] **Step 6: Run GREEN**

Run:

```bash
bun --filter agent-gui test -- src/stores/trace.test.ts
```

Expected: PASS.

### Task 2: Effective Failed Trace Rows

- [x] **Step 1: Write RED component/filter tests**

Add tests:

- `TraceEntry.test.ts`: a completed entry with `exitCode: 1` renders failed icon/classes and still shows title.
- `TraceTimeline.test.ts`: status filter counts and failed filtering include completed entries with non-zero `exitCode`.
- negative assertion: a completed entry with `exitCode: 0` stays done and does not appear under Failed.

- [x] **Step 2: Run RED**

Run:

```bash
bun --filter agent-gui test -- src/components/TraceEntry.test.ts src/components/TraceTimeline.test.ts
```

Expected: FAIL because the right trace list currently uses raw `entry.status`.

- [x] **Step 3: Implement effective status helpers**

In `TraceEntry.vue`, compute:

```ts
const effectiveStatus = computed(() =>
  props.entry.status === "completed" && props.entry.exitCode != null && props.entry.exitCode !== 0
    ? "failed"
    : props.entry.status
);
const displayTitle = computed(() => props.entry.title || props.entry.toolId || "");
```

Use `effectiveStatus` for class/icon and `displayTitle` in `.entry-tool`.

In `TraceTimeline.vue`, use the same non-zero exit logic inside `traceMatchesFilter`.

- [x] **Step 4: Run GREEN**

Run:

```bash
bun --filter agent-gui test -- src/components/TraceEntry.test.ts src/components/TraceTimeline.test.ts
```

Expected: PASS.

### Task 3: Trajectory Completion Signal

- [x] **Step 1: Write RED store and viewer tests**

In `trace.test.ts`, assert `TrajectoryCompleted` creates a terminal trace entry. Add a failed-outcome assertion.

In `TrajectoryViewer.test.ts`, assert a summary row renders counts for success/failed/in-progress trajectories.

- [x] **Step 2: Run RED**

Run:

```bash
bun --filter agent-gui test -- src/stores/trace.test.ts src/components/TrajectoryViewer.test.ts
```

Expected: FAIL because trajectory completion is ignored by trace store and no summary is rendered.

- [x] **Step 3: Implement store and viewer changes**

In `trace.ts`, add a `TrajectoryCompleted` case that pushes a single final entry using the trajectory id for dedup.

In `TrajectoryViewer.vue`, compute outcome counts from fetched trajectories and render a compact summary above the list. Add locale keys:

- `trajectory.summary`
- `trajectory.summarySuccess`
- `trajectory.summaryFailed`
- `trajectory.summaryInProgress`
- `trajectory.summaryCancelled`

- [x] **Step 4: Run GREEN**

Run:

```bash
bun --filter agent-gui test -- src/stores/trace.test.ts src/components/TrajectoryViewer.test.ts
```

Expected: PASS.

### Task 4: Quality Gates and Dev App

- [x] **Step 1: Format**

Run:

```bash
cargo fmt --all
bun run format
```

If `bun run format` is not a project script, run the repository's formatter command discovered from `package.json`/`justfile`.

- [x] **Step 2: Focused checks**

Run:

```bash
bun --filter agent-gui test -- src/stores/trace.test.ts src/components/TraceEntry.test.ts src/components/TraceTimeline.test.ts src/components/TrajectoryViewer.test.ts
cargo test -p agent-core -p agent-runtime ToolInvocationStarted
```

- [x] **Step 3: Final local gates**

Run:

```bash
cargo fmt --all --check
bun run format:check
bun --filter agent-gui test -- src/stores/trace.test.ts src/components/TraceEntry.test.ts src/components/TraceTimeline.test.ts src/components/TrajectoryViewer.test.ts
```

- [x] **Step 4: Dev App / pilot verification**

Start from this worktree with pilot enabled:

```bash
KAIROX_HOME="$(mktemp -d)" scripts/dev-pilot.sh
```

Verify:

- app loads without frontend console errors;
- a session with a shell/tool call shows command input once invocation starts;
- non-zero exit rows show failed styling in Trace filters;
- trajectory completion appears in Trace/Trajectory panels.

### Task 5: PR and Cleanup

- [x] Commit with a conventional message.
- [x] Push branch and create one PR.
- [x] Enable auto-merge.
- [x] Run `pr-watcher.sh` until merged.
- [x] Clean only this feature worktree/branch after merge; leave preserved eval GUI and eval worktrees untouched.
