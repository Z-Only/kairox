# Kairox Ops Followups Batch 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve the new operational helpers so future Kairox batches can observe PRs and old eval worktrees with less manual interpretation, then update the local Kairox SKILL notes to point agents at those helpers.

**Architecture:** Keep automation improvements in versioned `scripts/` helpers with Node unit tests. Keep Kairox SKILL guidance in ignored `.agents/skills/**` files because that tree is intentionally local-only and must not be committed or pushed.

**Tech Stack:** Node test runner, GitHub CLI, Git worktree porcelain, Kairox ignored local skills, GitHub PR workflow.

---

## Batch Boundaries

- Lane G owns `scripts/pr-status-summary.mjs` and `scripts/pr-status-summary.test.mjs`.
- Lane H owns `scripts/audit-eval-worktrees.mjs` and `scripts/audit-eval-worktrees.test.mjs`.
- Lane I owns ignored local SKILL files under `.agents/skills/**` only.
- Lane G and Lane H must not edit GUI, Rust, generated bindings, or local skill files.
- Lane I must not edit versioned repository files, commit, push, or open a PR.

## Task 1: PR Status Summary Watch Mode

**Files:**

- Modify: `scripts/pr-status-summary.mjs`
- Modify: `scripts/pr-status-summary.test.mjs`

- [ ] **Step 1: Add RED tests for neutral classification and watch output**

Add tests that assert:

- `NEUTRAL` check conclusions are counted separately as `neutral`, not `unknown`;
- the JSON shape includes `checks.counts.neutral`;
- a new `--watch` mode polls injected PR data until no `pending` checks remain;
- `--watch` exits non-zero on timeout while printing the last observed summary.

Expected RED: `neutral` count and `--watch` mode do not exist.

- [ ] **Step 2: Implement minimal neutral and watch support**

Extend the helper with:

- `neutral` in `createCounts()`;
- `NEUTRAL` classification to `neutral`;
- CLI flags `--watch`, `--interval-ms <n>`, and `--timeout-ms <n>`;
- validation that interval and timeout are positive integers;
- watch loop that reuses the existing summary output after each poll;
- exit `0` when all PRs have zero pending checks and no failure count;
- exit `1` on timeout or any failure count.

- [ ] **Step 3: Verify Lane G**

Run:

```bash
node --test scripts/pr-status-summary.test.mjs
node scripts/pr-status-summary.mjs --help
node scripts/pr-status-summary.mjs --json 1085
bun run format:check
git diff --check
```

Expected: all commands exit 0. Live `1085` output may be skipped if the PR is no longer available, but tests must not depend on live GitHub.

## Task 2: Eval Worktree Audit Filtering And Summary

**Files:**

- Modify: `scripts/audit-eval-worktrees.mjs`
- Modify: `scripts/audit-eval-worktrees.test.mjs`

- [ ] **Step 1: Add RED tests for filtered and summarized audit output**

Add tests that assert:

- `--dirty-only` returns only `dirty`, `missing`, or `error` worktrees;
- `--clean-only` returns only clean worktrees;
- invalid filter combinations fail with usage text;
- JSON output includes a stable `summary` object with total, clean, dirty, missing, and error counts.

Expected RED: these filters and summary output do not exist.

- [ ] **Step 2: Implement filter and summary support**

Extend the helper with:

- `summarizeAudit(worktrees)`;
- `filterAuditResults(worktrees, filter)`;
- CLI flags `--dirty-only`, `--clean-only`, and `--summary`;
- stable JSON shape `{ "summary": ..., "worktrees": [...] }`;
- human output that prints a one-line summary before the existing table.

Do not add any destructive cleanup command.

- [ ] **Step 3: Verify Lane H**

Run:

```bash
node --test scripts/audit-eval-worktrees.test.mjs
node scripts/audit-eval-worktrees.mjs --json
node scripts/audit-eval-worktrees.mjs --dirty-only
bun run format:check
git diff --check
```

Expected: all commands exit 0. The live dirty-only output should list dirty existing eval worktrees when present.

## Task 3: Local Kairox SKILL Notes

**Files:**

- Modify: `.agents/skills/kairox-dev-workflow/SKILL.md`
- Modify if useful: `.agents/skills/kairox-github-pr-ops/SKILL.md`

- [ ] **Step 1: Confirm ignored local skill scope**

Run:

```bash
git check-ignore -v .agents/skills .agents/skills/kairox-dev-workflow/SKILL.md
```

Expected: `.agents/` ignore rule matches both paths. Because this tree is ignored, edit in the main checkout and do not commit or push these changes.

- [ ] **Step 2: Add concise helper references**

Update local skill text to mention:

- `node scripts/pr-status-summary.mjs --json <pr...>` for structured PR probes;
- `node scripts/pr-status-summary.mjs --watch <pr...>` when watcher scripts are too noisy or a compact probe is enough;
- `node scripts/audit-eval-worktrees.mjs --json` before deleting old eval worktrees;
- never auto-delete dirty eval worktrees without explicit confirmation.

Keep changes concise and avoid duplicating script help text.

- [ ] **Step 3: Verify local skill edits**

Run:

```bash
git diff -- .agents/skills/kairox-dev-workflow/SKILL.md .agents/skills/kairox-github-pr-ops/SKILL.md
git status --short --branch
```

Expected: git status does not show ignored skill files; the diff command can still show local ignored changes when explicit paths are passed.

## Final Batch Gates

- Lane G and H each need focused Node tests, `bun run format:check`, `git diff --check`, PR creation, CI observation until merge, and cleanup.
- Lane I has no PR because `.agents/skills/**` is ignored; final report must call out that it is a local-only SKILL update.
- No lane requires Dev App verification unless it changes GUI or runtime behavior.
- Final reporting must include merged PRs, merge commits, local validation commands, CI result, cleanup status, local SKILL edit status, and remaining followups.
