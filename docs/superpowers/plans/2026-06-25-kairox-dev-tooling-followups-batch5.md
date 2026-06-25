# Kairox Dev Tooling Followups Batch 5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce PR-operation friction from silent status watching, transient GitHub API failures, and squash-merge worktree cleanup errors.

**Architecture:** Keep the work in developer scripts, with focused unit-style script tests and no runtime/GUI behavior changes. Split PR status watching and merged-worktree cleanup into separate lanes so either improvement can merge independently.

**Tech Stack:** Node.js CLI scripts, GitHub CLI JSON output, Git worktree/branch commands, Bun formatting/linting.

---

## Batch Boundaries

- Lane M owns `scripts/pr-status-summary.mjs` and its focused script tests.
- Lane N owns a new merged-worktree cleanup helper under `scripts/` and its focused script tests.
- Neither lane should modify runtime, GUI, Cargo, generated bindings, or evaluation history.
- Dev App verification is not required for either lane because these are local developer CLI script changes with no app behavior.

## Task 1: PR Status Watch Visibility And Retry

**Files:**

- Modify: `scripts/pr-status-summary.mjs`
- Modify or create focused script tests in the existing script test location discovered from `package.json` / current tests.

- [ ] **Step 1: Discover existing script test pattern**

Run:

```bash
rg -n "pr-status-summary|audit-eval-worktrees|scripts/" package.json scripts tests .github
```

Expected: identify how repository script CLIs are tested and formatted. Use the existing pattern instead of inventing a new harness.

- [ ] **Step 2: Add RED coverage for watch heartbeat**

Add a focused test or fixture-driven assertion that exercises watch-mode output for a pending PR summary. The expected behavior is that each polling iteration prints a concise status line containing the PR number, head SHA, merge state, and pending/failing counts.

Expected RED:

```bash
bun run test:scripts
```

or the repository's existing equivalent should fail because watch mode currently stays silent during ordinary pending cycles.

- [ ] **Step 3: Add RED coverage for transient query retry**

Add a focused test for a transient `gh pr view` failure such as `Service Unavailable`. Expected behavior: the failed PR query is retried with bounded backoff and the watcher continues instead of exiting on the first transient GraphQL error.

Expected RED: the focused script test fails because the current implementation propagates the first transient error.

- [ ] **Step 4: Implement heartbeat and bounded retry**

Update `scripts/pr-status-summary.mjs` so watch mode:

- prints one compact heartbeat per poll when PRs are still pending;
- includes PR number, short head SHA, merge state, success/failure/pending counts, and pending check names when useful;
- retries transient GitHub/API failures with a small bounded delay before failing the whole watch;
- keeps non-transient failures visible with the original command and error text.

- [ ] **Step 5: Verify Lane M**

Run:

```bash
bun run format:check
bun run lint
node scripts/pr-status-summary.mjs --json 1091 || true
git diff --check
```

If the focused script test command is different, run that exact command and record the real non-zero test count. Do not treat `0 tests` as passing.

## Task 2: Safe Merged Worktree Cleanup Helper

**Files:**

- Create: `scripts/cleanup-merged-worktree.mjs`
- Modify or create focused script tests in the existing script test location discovered from `package.json` / current tests.

- [ ] **Step 1: Discover Git helper conventions**

Run:

```bash
rg -n "worktree remove|branch -D|ls-remote|mergeCommit|gh pr view|cleanup" scripts .agents docs package.json
```

Expected: identify the repository's current cleanup conventions and reuse output style from existing helper scripts.

- [ ] **Step 2: Add RED coverage for squash-merged branch cleanup**

Add a focused test that models a branch whose commit is not an ancestor of `main` because it was squash-merged, but whose PR state is `MERGED` and whose merge commit is present on `main`. Expected behavior: the helper removes the associated worktree first, then deletes the local branch with `git branch -D`.

Expected RED: the test fails because no helper exists.

- [ ] **Step 3: Add RED coverage for safety refusal**

Add coverage proving the helper refuses to delete when:

- the PR is not merged;
- the worktree path is outside the repository's `.worktrees/`;
- the worktree is dirty unless an explicit `--force-dirty` flag is supplied.

Expected RED: the test fails because no helper exists.

- [ ] **Step 4: Implement the cleanup helper**

Implement a Node CLI with this shape:

```bash
node scripts/cleanup-merged-worktree.mjs --branch <branch> [--worktree <path>] [--pr <number>] [--force-dirty]
```

Minimum behavior:

- resolve repo root with Git;
- resolve branch, worktree path, and PR number when omitted where practical;
- confirm GitHub reports the PR as merged before destructive cleanup;
- confirm the main branch contains the merge commit after `git fetch origin main`;
- refuse paths outside `<repo>/.worktrees/`;
- refuse dirty worktrees unless `--force-dirty`;
- run cleanup from the repo root, not inside the removed worktree;
- remove the worktree, prune, delete the local branch with `-D`, and try remote branch deletion best-effort;
- print a concise summary of each performed action.

- [ ] **Step 5: Verify Lane N**

Run:

```bash
bun run format:check
bun run lint
node scripts/cleanup-merged-worktree.mjs --help
git diff --check
```

If focused script tests exist, run them and confirm they execute non-zero tests.

## Completion

- [ ] Lane M and Lane N each land through their own PR.
- [ ] For each PR, CI is observed to merge, then main is fast-forwarded locally.
- [ ] Each implementation worktree and local/remote branch is cleaned.
- [ ] Final audit confirms main is clean, no lane PR remains open, and port 1420 has no listener.
