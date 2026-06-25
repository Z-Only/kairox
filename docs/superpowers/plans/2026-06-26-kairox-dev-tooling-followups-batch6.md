# Kairox Dev Tooling Followups Batch 6 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Continue executing the remaining Kairox optimization list with small independent developer-tooling PRs that reduce manual PR interpretation and local eval cleanup friction.

**Architecture:** Keep this batch in versioned Node CLI helpers and tests only. Each lane owns a disjoint script/test pair so it can be implemented, validated, merged, and cleaned independently without touching Rust runtime, GUI, generated bindings, or local ignored skills.

**Tech Stack:** Node.js ESM CLI scripts, Node test runner, GitHub CLI JSON output, Git worktree/status commands, Bun formatting/linting.

---

## Batch Boundaries

- Lane O owns `scripts/pr-status-summary.mjs` and `scripts/pr-status-summary.test.mjs`.
- Lane P owns `scripts/cleanup-merged-worktree.mjs` and `scripts/cleanup-merged-worktree.test.mjs`.
- Lane Q owns `scripts/audit-eval-worktrees.mjs` and `scripts/audit-eval-worktrees.test.mjs`.
- Lanes must not edit Rust crates, GUI files, generated bindings, package metadata, or local `.agents/skills/**`.
- Dev App verification is not required for any lane because these are local developer CLI helpers with no runtime or GUI behavior changes.

## Task 1: Merged PR Status Display

**Files:**

- Modify: `scripts/pr-status-summary.mjs`
- Modify: `scripts/pr-status-summary.test.mjs`

- [ ] **Step 1: Add RED coverage for merged PR display**

Add a focused test that passes a summarized PR with `state: "MERGED"` and `merge_state_status: null` into `formatHumanSummary`. Assert the top table displays `MERGED` in the Merge column instead of `-` or `UNKNOWN`.

Expected test shape:

```js
test("formatHumanSummary displays merged PRs without misleading unknown merge state", () => {
  const output = formatHumanSummary([
    {
      number: 1094,
      title: "chore(dev): add merged worktree cleanup script",
      state: "MERGED",
      merge_state_status: null,
      head_ref_name: "chore/merged-worktree-cleanup",
      head_ref_oid: "e108270a28834b81193fd33da1d9347ba614f354",
      merge_commit_oid: "779ede82663edd461004f4ebcbda8d9b9751f2d8",
      checks: {
        counts: { success: 0, failure: 0, pending: 0, skipped: 0, neutral: 0, unknown: 0 },
        items: []
      }
    }
  ]);

  assert.match(output, /#1094\\s+MERGED\\s+MERGED\\s+chore\\/merged-worktree-cleanup@e108270a/);
});
```

Run:

```bash
node --test scripts/pr-status-summary.test.mjs
```

Expected RED: the test fails because the Merge column currently renders the raw missing merge state.

- [ ] **Step 2: Implement minimal display mapping**

Add a tiny formatter used by `formatHumanSummary`:

```js
function mergeStateCell(summary) {
  if (summary.state === "MERGED") {
    return "MERGED";
  }
  return summary.merge_state_status || "-";
}
```

Use `mergeStateCell(summary)` for the Merge column. Do not change JSON output fields; the machine-readable shape should keep raw `merge_state_status`.

- [ ] **Step 3: Verify Lane O**

Run:

```bash
node --test scripts/pr-status-summary.test.mjs
bun run test:scripts
bun run format:check
git diff --check
```

Expected: all commands exit 0 with non-zero test counts.

## Task 2: Cleanup Helper Dry Run, JSON, And Inference

**Files:**

- Modify: `scripts/cleanup-merged-worktree.mjs`
- Modify: `scripts/cleanup-merged-worktree.test.mjs`

- [ ] **Step 1: Add RED coverage for dry-run safety**

Add a test that runs:

```js
await runCli(["--branch", "feature/x", "--dry-run"], fakeIo);
```

Fake commands should include repo/worktree/PR/merge/dirty checks, but the test must assert no `git worktree remove`, `git worktree prune`, `git branch -D`, or `git push origin --delete` command is invoked. Assert stdout includes:

```text
dry-run: would remove worktree: /repo/.worktrees/feature-x
dry-run: would delete local branch: feature/x
dry-run: would delete remote branch: feature/x
```

Expected RED: `--dry-run` is currently an unknown argument.

- [ ] **Step 2: Add RED coverage for JSON output**

Add a test that runs:

```js
await runCli(["--branch", "feature/x", "--dry-run", "--json"], fakeIo);
```

Assert stdout parses as JSON with:

```js
{
  branch: "feature/x",
  worktree_path: "/repo/.worktrees/feature-x",
  pr_number: 1099,
  merge_commit_oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  dry_run: true,
  actions: [
    { action: "remove_worktree", status: "planned", target: "/repo/.worktrees/feature-x" },
    { action: "prune_worktrees", status: "planned" },
    { action: "delete_local_branch", status: "planned", target: "feature/x" },
    { action: "delete_remote_branch", status: "planned", target: "feature/x" }
  ]
}
```

Expected RED: `--json` is currently an unknown argument.

- [ ] **Step 3: Add RED coverage for branch inference**

Add a test that runs:

```js
await runCli(["--dry-run"], fakeIo);
```

Fake commands should include:

```text
git branch --show-current
```

returning `feature/x`. Assert the rest of the cleanup plan uses `feature/x`.

Expected RED: the CLI currently requires `--branch <branch>`.

- [ ] **Step 4: Implement plan-first cleanup result**

Refactor the helper so validation builds an in-memory cleanup result before destructive actions. The result should include branch, resolved worktree path, PR number, merge commit oid, `dry_run`, and ordered action records.

Behavior requirements:

- `--branch` remains supported.
- Without `--branch`, use `git branch --show-current` and fail if detached or empty.
- `--dry-run` performs all non-destructive validation and prints planned actions without deleting anything.
- `--json` prints only JSON to stdout.
- Human output keeps the existing concise action lines for real cleanup.
- Remote branch deletion remains best-effort and records `remote branch already absent` as a skipped action rather than failure.

- [ ] **Step 5: Verify Lane P**

Run:

```bash
node --test scripts/cleanup-merged-worktree.test.mjs
bun run test:scripts
node scripts/cleanup-merged-worktree.mjs --help
bun run format:check
git diff --check
```

Expected: all commands exit 0 with non-zero test counts.

## Task 3: Eval Worktree Dirty Status Summaries

**Files:**

- Modify: `scripts/audit-eval-worktrees.mjs`
- Modify: `scripts/audit-eval-worktrees.test.mjs`

- [ ] **Step 1: Add RED coverage for dirty detail collection**

Add a test where a selected eval worktree returns:

```text
 M crates/agent-runtime/src/lib.rs
?? scratch.txt
```

Assert the audit result includes:

```js
dirty_file_count: 2,
dirty_files: ["crates/agent-runtime/src/lib.rs", "scratch.txt"]
```

Keep the existing `dirty_status: "dirty"` behavior.

Expected RED: audit results currently only expose `dirty_status`.

- [ ] **Step 2: Add RED coverage for human table visibility**

Update or add a `formatHumanTable` test asserting the table includes a `DIRTY_FILES` column and renders compact values such as:

```text
2: crates/agent-runtime/src/lib.rs, scratch.txt
```

Expected RED: the human table currently lacks dirty-file details.

- [ ] **Step 3: Implement compact dirty summaries**

Update `dirtyStatus` to preserve bounded detail:

- clean: `dirty_file_count: 0`, `dirty_files: []`
- dirty: count status lines and parse file paths from porcelain-short output
- missing/error: `dirty_file_count: 0`, `dirty_files: []`
- cap `dirty_files` at 5 entries to keep JSON and table output compact

Add a helper that handles rename/status prefixes conservatively by trimming the two-character status prefix and leading whitespace.

- [ ] **Step 4: Verify Lane Q**

Run:

```bash
node --test scripts/audit-eval-worktrees.test.mjs
bun run test:scripts
node scripts/audit-eval-worktrees.mjs --dirty-only
bun run format:check
git diff --check
```

Expected: all commands exit 0 with non-zero test counts, and live dirty-only output includes file counts without deleting anything.

## Completion

- [ ] Each lane lands through its own PR.
- [ ] Each PR is observed until `MERGED`, and local `main` is fast-forwarded after merge.
- [ ] Each lane worktree, local branch, and remote branch is cleaned.
- [ ] Final audit confirms `main` is clean and no batch 6 PR remains open.
- [ ] Remaining optimization points are carried forward explicitly: runtime no-op success gating, stale project prune UX, model health diagnostics, executable evaluation harness, versioned SKILL synchronization, generated-file guardrails, and broader next evaluation coverage.
