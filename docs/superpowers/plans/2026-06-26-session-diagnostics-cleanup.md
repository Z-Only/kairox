# Session Diagnostics Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish code optimizations for Kairox session diagnostics: direct DB metadata in Tauri diagnostics, stale runtime registry filtering, controlled diagnostics-only eval cleanup, and event DB source tracking.

**Architecture:** Keep the existing diagnostics pipeline. Add two optional fields to `SessionDiagnosticsResponse`, let the Tauri command fill them from `GuiState.home_dir`, keep Node registry inference as fallback, and make eval cleanup an explicit script flag that only cleans `.kairox-eval/` when the worktree is diagnostics-only dirty.

**Tech Stack:** Rust/Tauri/Specta, Node stdlib scripts, `node:test`, Cargo tests, Bun project gates.

---

## Scope

**Owned files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/session.rs`
- Modify: `apps/agent-gui/src/generated/commands.ts` via `just gen-types`
- Modify: `scripts/session-diagnostics-snapshot.mjs`
- Modify: `scripts/session-diagnostics-snapshot.test.mjs`
- Modify: `scripts/audit-eval-worktrees.mjs`
- Modify: `scripts/audit-eval-worktrees.test.mjs`

**Forbidden files:**

- Existing `eval/*` worktrees and their dirty contents.
- Unrelated GUI components, locale files, stores, or Rust crates.
- Manual edits to generated bindings; use `just gen-types`.

**Acceptance signals:**

- `export_session_diagnostics` responses include `event_db_path` and `event_db_path_source`.
- Snapshot JSON preserves `event_db_path_source`, reports `explicit_meta`, `runtime_registry`, or `default_kairox_home` when script inference supplies the path.
- Runtime registry fallback skips records with stale PIDs.
- `audit-eval-worktrees --clean-diagnostics-only` executes `git clean -fd -- .kairox-eval/` only for diagnostics-only dirty worktrees.
- Type generation reflects the new optional diagnostics fields.

### Task 1: Tauri Diagnostics DB Metadata

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/session.rs`
- Generated later: `apps/agent-gui/src/generated/commands.ts`

- [ ] **Step 1: Write the failing Rust test**

Add a focused assertion under `session_diagnostics_tests` that creates a summary, attaches metadata from a temp data dir, and expects:

```rust
assert_eq!(
    summary.event_db_path.as_deref(),
    Some(data_dir.join("kairox-gui.sqlite").to_string_lossy().as_ref())
);
assert_eq!(summary.event_db_path_source.as_deref(), Some("tauri_state"));
```

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test -p agent-gui-tauri session_diagnostics_event_db_metadata
```

Expected: fail because the fields/helper do not exist yet.

- [ ] **Step 3: Implement minimal Rust code**

Add optional fields:

```rust
pub event_db_path: Option<String>,
pub event_db_path_source: Option<String>,
```

Add a small helper in `commands/session.rs`:

```rust
fn attach_event_db_metadata(summary: &mut SessionDiagnosticsResponse, data_dir: &std::path::Path) {
    summary.event_db_path = Some(data_dir.join("kairox-gui.sqlite").to_string_lossy().into_owned());
    summary.event_db_path_source = Some("tauri_state".to_string());
}
```

Call it from `export_session_diagnostics` after `summarize_trace_export`.

- [ ] **Step 4: Run GREEN**

Run:

```bash
cargo test -p agent-gui-tauri session_diagnostics
```

Expected: non-zero tests run and pass.

### Task 2: Snapshot Source Tracking And Stale Registry Filtering

**Files:**

- Modify: `scripts/session-diagnostics-snapshot.mjs`
- Modify: `scripts/session-diagnostics-snapshot.test.mjs`

- [ ] **Step 1: Write failing JS tests**

Add tests for:

```js
assert.equal(JSON.parse(stdout.content).event_db_path_source, "explicit_meta");
assert.equal(JSON.parse(stdout.content).event_db_path_source, "default_kairox_home");
assert.equal(JSON.parse(stdout.content).event_db_path_source, "runtime_registry");
```

Add a stale registry case with a newer dead PID and an older live PID:

```js
processIsRunning: (pid) => pid === 222;
```

Expected path is the live record DB path.

- [ ] **Step 2: Run RED**

Run:

```bash
node --test scripts/session-diagnostics-snapshot.test.mjs
```

Expected: new tests fail because source and stale PID filtering are missing.

- [ ] **Step 3: Implement minimal JS code**

Make `inferEventDbPath` return `{ path, source }`, skip registry records whose numeric `pid` is not running, add `event_db_path_source` to compact JSON, and mark explicit `--meta event_db_path=...` as `explicit_meta` when no source is supplied.

- [ ] **Step 4: Run GREEN**

Run:

```bash
node --test scripts/session-diagnostics-snapshot.test.mjs
```

Expected: all snapshot tests pass.

### Task 3: Controlled Diagnostics-Only Cleanup

**Files:**

- Modify: `scripts/audit-eval-worktrees.mjs`
- Modify: `scripts/audit-eval-worktrees.test.mjs`

- [ ] **Step 1: Write the failing cleanup test**

Add a `runCli` test using the existing porcelain fixture. Make `eval-a` dirty only with `.kairox-eval/`, make `eval-kairox-b` code dirty, run:

```js
await runCli(["--json", "--clean-diagnostics-only"], ...)
```

Assert the recorded commands include:

```text
git -C /repo/.worktrees/eval-a clean -fd -- .kairox-eval/
```

and do not include a clean command for the code-dirty worktree.

- [ ] **Step 2: Run RED**

Run:

```bash
node --test scripts/audit-eval-worktrees.test.mjs
```

Expected: fail because `--clean-diagnostics-only` is unknown.

- [ ] **Step 3: Implement minimal cleanup flag**

Add `--clean-diagnostics-only` to usage and parser. After auditing/filtering, run:

```js
git -C <worktree.path> clean -fd -- .kairox-eval/
```

only for `dirty_status === "dirty" && dirty_scope === "diagnostics_only"`. Add `cleanup_diagnostics_only_cleaned` to summary when the flag is used.

- [ ] **Step 4: Run GREEN**

Run:

```bash
node --test scripts/audit-eval-worktrees.test.mjs
```

Expected: all audit tests pass.

### Task 4: Type Generation And Project Gates

**Files:**

- Modify via generator: `apps/agent-gui/src/generated/commands.ts`

- [ ] **Step 1: Check generated status before generation**

Run:

```bash
git status --short apps/agent-gui/src/generated
```

Expected: no unrelated generated drift.

- [ ] **Step 2: Generate types**

Run:

```bash
just gen-types
```

Expected: generated TypeScript includes `event_db_path` and `event_db_path_source`.

- [ ] **Step 3: Run final local gates**

Run:

```bash
cargo fmt --all
cargo fmt --all --check
node --test scripts/session-diagnostics-snapshot.test.mjs scripts/audit-eval-worktrees.test.mjs
bun run test:scripts
cargo test -p agent-gui-tauri session_diagnostics
cargo test -p agent-gui-tauri
bun run format:check
bun run lint
```

Expected: all commands exit 0 with non-zero relevant test counts.

### Task 5: Dev App Verification, PR, And Cleanup

**Files:** none unless verification finds a bug.

- [ ] **Step 1: Run Dev App verification**

Because this changes Tauri IPC, start the app with pilot and export diagnostics through the live command:

```bash
KAIROX_HOME="$(mktemp -d /tmp/kairox-dev-home.XXXXXX)" bun --filter agent-gui tauri dev --features pilot
tauri-pilot ping
tauri-pilot ipc export_session_diagnostics --args '{"sessionId":"<real-session-id>"}' --json
tauri-pilot logs --level error
```

If no session exists, create one through the GUI/pilot flow first, then export its diagnostics. Expected JSON contains `event_db_path` and `event_db_path_source`.

- [ ] **Step 2: Commit, push, PR, watch**

Run:

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/commands/session.rs apps/agent-gui/src/generated/commands.ts scripts/session-diagnostics-snapshot.mjs scripts/session-diagnostics-snapshot.test.mjs scripts/audit-eval-worktrees.mjs scripts/audit-eval-worktrees.test.mjs docs/superpowers/plans/2026-06-26-session-diagnostics-cleanup.md
git commit -m "chore(dev): finish session diagnostics cleanup"
git fetch origin main
git rebase origin/main
git push -u origin chore/session-diagnostics-cleanup
gh pr create --base main --head chore/session-diagnostics-cleanup --title "chore(dev): finish session diagnostics cleanup" --body-file <body-file>
gh pr merge <pr> --auto --squash --delete-branch
PR=<pr> WORKTREE=/Users/chanyu/AIProjects/kairox/.worktrees/chore-session-diagnostics-cleanup AUTO_REBASE=1 bash /Users/chanyu/AIProjects/kairox/.agents/skills/kairox-github-pr-ops/scripts/pr-watcher.sh
```

- [ ] **Step 3: Merge cleanup**

After PR is merged:

```bash
git -C /Users/chanyu/AIProjects/kairox checkout main
git -C /Users/chanyu/AIProjects/kairox fetch origin main --prune
git -C /Users/chanyu/AIProjects/kairox merge --ff-only origin/main
git -C /Users/chanyu/AIProjects/kairox worktree remove /Users/chanyu/AIProjects/kairox/.worktrees/chore-session-diagnostics-cleanup
git -C /Users/chanyu/AIProjects/kairox worktree prune
git -C /Users/chanyu/AIProjects/kairox branch -D chore/session-diagnostics-cleanup
```
