# Release Version Doc Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep runtime version access and public docs release state in sync from the Cargo workspace version, with CI checks that catch stale README, ROADMAP, and site examples.

**Architecture:** `Cargo.toml` remains the single version source. A small Node script reads the workspace version, normalizes `docs/current-release.json`, rewrites known docs snippets, and checks that the committed docs match generated output. Existing `BuildInfo` remains the runtime path for Rust/Tauri/GUI version access.

**Tech Stack:** Node.js ESM scripts, `node:test`, existing Bun package scripts, existing GitHub Actions CI.

---

### Task 1: Add Release Doc Sync Script Tests

**Files:**

- Create: `scripts/release-version-docs.test.mjs`

- [x] **Step 1: Write failing tests**

Add tests for:

- parsing `[workspace.package].version` from `Cargo.toml`
- deriving `minorLine` and `compatRange`
- rewriting stale README, ROADMAP, site roadmap, and plugin compatibility snippets
- check mode failing when committed docs differ from generated content

- [x] **Step 2: Verify RED**

Run: `node --test scripts/release-version-docs.test.mjs`
Expected: FAIL because `scripts/release-version-docs.mjs` does not exist.

### Task 2: Add Release Doc Sync Implementation

**Files:**

- Create: `scripts/release-version-docs.mjs`
- Create: `docs/current-release.json`

- [x] **Step 1: Implement minimal exported helpers**

Implement `readWorkspaceVersion`, `deriveReleaseFields`, `syncReleaseDocs`, and `checkReleaseDocs`.

- [x] **Step 2: Verify GREEN**

Run: `node --test scripts/release-version-docs.test.mjs`
Expected: PASS.

### Task 3: Wire Scripts and CI

**Files:**

- Modify: `package.json`
- Modify: `.github/workflows/ci.yml`
- Modify: `justfile`
- Modify: `docs/releasing.md`
- Modify: `AGENTS.md`

- [x] **Step 1: Add commands**

Add `release-docs:sync`, `release-docs:check`, and root `test:scripts` package scripts.

- [x] **Step 2: Add CI checks**

Run release-docs check and root script tests in CI after the existing formatting checks.

- [x] **Step 3: Update release workflow docs**

Document that `just bump-version` also synchronizes current-release docs and that release bumps must pass `bun run release-docs:check`.

### Task 4: Sync Current Docs

**Files:**

- Modify: `README.md`
- Modify: `ROADMAP.md`
- Modify: `docs/ROADMAP.md`
- Modify: `site/community/roadmap.md`
- Modify: `site/zh/community/roadmap.md`
- Modify: `site/concepts/extensibility.md`
- Modify: `site/zh/concepts/extensibility.md`

- [x] **Step 1: Run writer**

Run: `node scripts/release-version-docs.mjs --write`
Expected: docs show `0.41.0`, `v0.41.x`, and `>=0.41.0 <0.42.0`.

- [x] **Step 2: Run check**

Run: `node scripts/release-version-docs.mjs --check`
Expected: PASS with no stale docs.

### Task 5: Verification

**Files:** all changed files

- [x] Run: `node --test scripts/release-version-docs.test.mjs`
- [x] Run: `bun run release-docs:check`
- [x] Run: `bunx oxfmt --check README.md ROADMAP.md docs/ROADMAP.md docs/current-release.json docs/superpowers/plans/2026-06-18-release-version-doc-sync.md site/community/roadmap.md site/zh/community/roadmap.md site/concepts/extensibility.md site/zh/concepts/extensibility.md package.json .github/workflows/ci.yml justfile docs/releasing.md AGENTS.md scripts/release-version-docs.mjs scripts/release-version-docs.test.mjs`
- [x] Run: `bun run format:check`
- [x] Run: `bun run test:scripts`
- [x] Run: `bun run site:build`
- [x] Dev App verification: skipped because this is docs/tooling only and does not change runtime or GUI behavior.
