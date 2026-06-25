# Kairox Ops Followups Batch 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the next evaluation findings into three isolated, mergeable PRs that improve operator visibility without changing agent behavior.

**Architecture:** Keep read-only operational helpers in `scripts/`, keep GUI affordances in `apps/agent-gui/src/`, and avoid mixing diagnostic UI changes with repository maintenance tools. Each lane should be safe to merge independently.

**Tech Stack:** Node test runner, Git/GitHub CLI, Vue 3, Pinia, Vitest, Tauri command bindings, Dev App pilot.

---

## Batch Boundaries

- Lane D owns an eval worktree audit helper and focuses on stale local branch/worktree visibility.
- Lane E owns a PR status summary helper and focuses on structured watcher-friendly output.
- Lane F owns the GUI diagnostics export affordance and focuses on making the existing `export_session_diagnostics` command reachable from the trace panel.
- Lanes D and E must not edit GUI files. Lane F must not edit operational helper scripts.

## Task 1: Eval Worktree Audit Helper

**Files:**

- Create: `scripts/audit-eval-worktrees.mjs`
- Create: `scripts/audit-eval-worktrees.test.mjs`

- [ ] **Step 1: Write the failing CLI contract check**

Run before implementation:

```bash
node scripts/audit-eval-worktrees.mjs --help
```

Expected RED: script is missing.

- [ ] **Step 2: Implement read-only audit output**

Create a Node script that:

- parses `git worktree list --porcelain`;
- selects only worktrees whose branch starts with `eval/` or whose path basename starts with `eval-kairox-`;
- reports path, branch, HEAD, dirty status, and whether the worktree path still exists;
- supports `--json` for machine-readable output and a default concise table for humans;
- never deletes branches or worktrees.

- [ ] **Step 3: Verify helper behavior**

Run:

```bash
node --test scripts/audit-eval-worktrees.test.mjs
node scripts/audit-eval-worktrees.mjs --help
node scripts/audit-eval-worktrees.mjs --json
bun run format:check
git diff --check
```

Expected: all commands exit 0. The JSON output should list the existing eval worktrees when they are present.

## Task 2: PR Status Summary Helper

**Files:**

- Create: `scripts/pr-status-summary.mjs`
- Create: `scripts/pr-status-summary.test.mjs`

- [ ] **Step 1: Write the failing CLI contract check**

Run before implementation:

```bash
node scripts/pr-status-summary.mjs --help
```

Expected RED: script is missing.

- [ ] **Step 2: Implement structured summary output**

Create a Node script that:

- accepts PR numbers as positional arguments and optional `--json`;
- invokes `gh pr view <number> --json number,title,state,mergeStateStatus,headRefName,headRefOid,mergeCommit,statusCheckRollup`;
- normalizes status checks into counts for success, failure, pending, skipped, and unknown;
- prints compact JSON for automation and a concise table for humans;
- exits non-zero with a clear message when `gh` is missing, a PR cannot be read, or no PR number is provided.

- [ ] **Step 3: Verify helper behavior**

Run:

```bash
node --test scripts/pr-status-summary.test.mjs
node scripts/pr-status-summary.mjs --help
bun run format:check
git diff --check
```

Expected: all commands exit 0. Do not require live GitHub access in tests; use injected command fixtures.

## Task 3: GUI Trace Diagnostics Copy Action

**Files:**

- Modify: `apps/agent-gui/src/components/TraceTimeline.vue`
- Modify: `apps/agent-gui/src/components/TraceTimeline.test.ts`
- Modify only if needed: `apps/agent-gui/src/locales/en.json`
- Modify only if needed: `apps/agent-gui/src/locales/zh-CN.json`

- [ ] **Step 1: Write failing Vitest coverage**

Add a test that mounts `TraceTimeline`, clicks a diagnostics copy action, and asserts:

- `commands.exportSessionDiagnostics(activeSessionId)` was invoked;
- `navigator.clipboard.writeText` received stable compact JSON;
- a success toast is shown.

Expected RED: no copy action exists.

- [ ] **Step 2: Add the UI affordance**

Add a small icon/button to the trace header that:

- is disabled when no active session exists;
- calls the existing generated `commands.exportSessionDiagnostics`;
- copies `JSON.stringify(result)` or the unwrapped response data using existing command-result conventions;
- reports success/failure through the existing toast pattern;
- avoids adding explanatory in-app text beyond the button label/tooltip.

- [ ] **Step 3: Verify GUI behavior**

Run:

```bash
bun --filter agent-gui test -- TraceTimeline
bun run format:check
bun run lint
git diff --check
```

Then run Dev App verification for this lane only:

```bash
bash scripts/dev-pilot.sh
```

Expected: pilot connects, the trace panel renders, the diagnostics action can be exercised or inspected, `tauri-pilot logs --level error` returns no new app errors, and port `1420` is cleaned up.

## Final Batch Gates

- Each lane must run its focused tests and format checks.
- Only Lane F requires Dev App verification.
- Each PR must be watched until merged, then its worktree and local/remote branch must be cleaned from the main checkout.
- Final reporting must include merged PRs, merge commits, validation commands, CI result, cleanup status, and remaining followups.
