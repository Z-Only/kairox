# Kairox Ops Followups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the next evaluation followups into small, independently mergeable PRs that reduce Dev App startup friction and make session diagnostics easier to collect.

**Architecture:** Keep each lane isolated by responsibility. Shell/tooling changes stay in `scripts/`; Tauri diagnostics changes stay in `apps/agent-gui/src-tauri/`; workflow documentation captures the acceptance gates without changing runtime behavior.

**Tech Stack:** Bash, Node/Bun workspace scripts, Rust/Tauri commands, Specta-generated TypeScript bindings, GitHub PR workflow.

---

## Batch Boundaries

- Lane A owns `scripts/dev-pilot.sh` and focuses on worktree dependency bootstrap.
- Lane B owns `apps/agent-gui/src-tauri/src/commands/session.rs`, `apps/agent-gui/src-tauri/src/commands.rs`, generated command bindings, and focuses on richer session diagnostics.
- Lane C owns new or existing `scripts/*diagnostics*` helper files and focuses on a CLI-friendly diagnostics snapshot collector.
- The lanes must not edit each other's owned files. If a lane discovers it needs another lane's file, stop and report the dependency.

## Task 1: Dev Pilot Worktree Dependency Bootstrap

**Files:**

- Modify: `scripts/dev-pilot.sh`

- [ ] **Step 1: Write the failing dry-run expectation**

Add a shell-level verification target for the expected user-facing behavior before editing production logic:

```bash
KAIROX_DEV_PILOT_DRY_RUN=1 bash scripts/dev-pilot.sh
```

Expected before implementation: output does not mention how missing worktree `node_modules` would be detected or linked. Record this as the RED behavior in the PR body.

- [ ] **Step 2: Add bootstrap logic**

Implement helpers with these exact behaviors:

```bash
_dependency_ready() {
    [[ -d "$REPO_ROOT/node_modules/.bun" ]] &&
        [[ -e "$REPO_ROOT/apps/agent-gui/node_modules/.bin/tauri" ]]
}

_find_dependency_donor() {
    git worktree list --porcelain |
        awk '/^worktree /{print substr($0, 10)}' |
        while IFS= read -r candidate; do
            [[ "$candidate" != "$REPO_ROOT" ]] || continue
            [[ -d "$candidate/node_modules/.bun" ]] || continue
            [[ -e "$candidate/apps/agent-gui/node_modules/.bin/tauri" ]] || continue
            printf "%s\n" "$candidate"
            return 0
        done
}
```

Then add `_ensure_workspace_dependencies` that:

- returns immediately when `_dependency_ready` succeeds;
- prints a precise warning when no donor exists;
- links only missing ignored dependency paths from a donor worktree;
- never overwrites a real directory or non-symlink file;
- is skipped when `KAIROX_DEV_PILOT_SKIP_DEPS=1`.

- [ ] **Step 3: Verify dry run and syntax**

Run:

```bash
bash -n scripts/dev-pilot.sh
KAIROX_DEV_PILOT_DRY_RUN=1 bash scripts/dev-pilot.sh
```

Expected: both commands exit 0. Dry run prints the dependency status and the selected default/fallback commands.

## Task 2: Richer Session Diagnostics

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/session.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/bin/export_specta.rs`
- Generate: `apps/agent-gui/src/generated/commands.ts`

- [ ] **Step 1: Write the failing Rust test**

Extend `summarize_trace_export_counts_messages_and_tool_calls` or add a new test named:

```rust
summarize_trace_export_reports_terminal_and_stuck_signals
```

The test should build a `TraceExport` where a model request starts, a tool invocation starts, no assistant completion follows, and one trajectory fails. Assert the diagnostics response exposes:

```rust
assert_eq!(summary.running_model_requests, 1);
assert_eq!(summary.running_tool_invocations, 1);
assert_eq!(summary.trajectory_failed_count, 1);
assert_eq!(summary.has_terminal_assistant_message, false);
```

Expected RED: fields do not exist.

- [ ] **Step 2: Add minimal DTO fields and summary logic**

Add fields to `SessionDiagnosticsResponse`:

```rust
pub running_model_requests: u32,
pub running_tool_invocations: u32,
pub trajectory_failed_count: u32,
pub has_terminal_assistant_message: bool,
```

Count `ModelRequestStarted` minus `AssistantMessageCompleted` with saturating arithmetic. Count tool starts minus terminal tool completed/failed events. Count failed trajectory outcomes from `TrajectoryCompleted`. Set `has_terminal_assistant_message` when at least one `AssistantMessageCompleted` appears.

- [ ] **Step 3: Regenerate bindings and verify**

Run:

```bash
cargo fmt --all
cargo test -p agent-gui-tauri summarize_trace_export_reports_terminal_and_stuck_signals
just gen-types
cargo fmt --all --check
cargo clippy -p agent-gui-tauri --all-targets -- -D warnings
cargo test -p agent-gui-tauri
```

Expected: all commands exit 0 and generated `commands.ts` contains the new diagnostics fields.

## Task 3: Diagnostics Snapshot Collector

**Files:**

- Create: `scripts/session-diagnostics-snapshot.mjs`
- Optionally modify: `package.json`

- [ ] **Step 1: Write the failing script contract check**

Run before implementation:

```bash
node scripts/session-diagnostics-snapshot.mjs --help
```

Expected RED: script is missing.

- [ ] **Step 2: Implement the collector**

Create a Node script that:

- accepts `--session <id>` and optional `--out <path>`;
- invokes `tauri-pilot ipc export_session_diagnostics --args '{"sessionId":"<id>"}' --json`;
- writes a compact JSON file when `--out` is provided;
- prints the same compact JSON to stdout;
- exits non-zero with a clear message when `tauri-pilot` is missing or the IPC call fails.

The compact JSON must include:

```json
{
  "session_id": "ses_example",
  "event_count": 0,
  "event_type_counts": [],
  "user_message_count": 0,
  "assistant_message_count": 0,
  "running_model_requests": 0,
  "running_tool_invocations": 0,
  "trajectory_started_count": 0,
  "trajectory_completed_count": 0,
  "trajectory_failed_count": 0,
  "has_terminal_assistant_message": false
}
```

- [ ] **Step 3: Verify helper behavior**

Run:

```bash
node scripts/session-diagnostics-snapshot.mjs --help
node scripts/session-diagnostics-snapshot.mjs --session ses_missing
```

Expected: `--help` exits 0; missing app/IPC exits non-zero with an actionable error and does not create output.

## Final Batch Gates

- Every lane must run its focused tests and format checks.
- Functional Tauri/GUI changes must run Dev App verification with pilot and record command, socket/ping result, scenario, errors log result, and cleanup.
- Each PR must be watched until merged, then worktree and branches are cleaned from the main checkout.
- The final report must list merged PRs, merge commits, local verification, CI result, cleanup result, and any remaining followup lanes.
