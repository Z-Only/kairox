# GUI Dev App Verification Campaign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verify high-value Kairox GUI agent workflows with `kairox-live`, fix or optimize issues found during validation, and preserve valuable scenarios in the automated suite.

**Architecture:** The control lane runs live GUI validation in a dedicated worktree and records evidence. Any product bug or high-value test addition gets its own isolated fix lane following `kairox-dev-workflow`; the control lane continues validation unless a shared local resource requires serialization.

**Tech Stack:** Rust workspace, Tauri 2, Vue 3, Bun, Vitest, Playwright, tauri-pilot, `kairox-eval`, `kairox-live` model profile.

---

## File Structure

- Modify: `docs/superpowers/plans/2026-06-11-gui-dev-verification-campaign.md` records the campaign plan and progress.
- Potentially modify: `apps/agent-gui/e2e-pilot/*.toml` for GUI flows that are best captured through tauri-pilot.
- Potentially modify: `apps/agent-gui/e2e-pilot/fixtures/*` for small deterministic attachment or project fixtures.
- Potentially modify: `crates/agent-eval/fixtures/*.jsonl` for high-value model/runtime scenarios that do not require GUI interaction.
- Potentially modify: `Justfile` only if a new scenario group needs a stable recipe.
- Forbidden without a dedicated issue lane: broad runtime, model router, event schema, permission, store schema, generated TypeScript bindings, and unrelated UI restyling.

## Campaign Progress

### 2026-06-12 - Control lane setup and deterministic pilot pass

- Worktree: `test/gui-dev-verification-campaign` at `.worktrees/test-gui-dev-verification-campaign`.
- Baseline command passed after `bun install`: `bun --filter agent-gui test --run src/components/ChatComposer.test.ts src/components/AttachmentTray.test.ts src/components/ChatModelSelector.test.ts` (3 files, 55 tests).
- `tauri-pilot` is available at version `0.7.2`; port `1420` was clear before pilot startup.
- First pilot launch was blocked by StarPoint for `target/debug/agent-gui-tauri`; after confirming the prompt matched this worktree binary, `AXPress` on `允许运行` unblocked the run.
- Deterministic pilot command passed: `PILOT_SCENARIOS="app-bootstrap session-lifecycle chat-flow model-switch audit-mcp audit-skills" scripts/run-pilot-tests.sh`.
- Pilot results: `app-bootstrap` 3/3, `session-lifecycle` 2/2, `chat-flow` 18/18, `model-switch` 19/19, `audit-mcp` 20/20, `audit-skills` 22/22.

### 2026-06-12 - Real `kairox-live` GUI validation

- Manual Dev App command: `KAIROX_DEV_PORT=1420 KAIROX_DEV_STRICT_PORT=1 KAIROX_DEV_DYNAMIC_IDENTIFIER=1 bun --filter agent-gui tauri dev --features pilot`.
- Profile availability: GUI runtime listed `kairox-live`; credential values were not recorded.
- Ordinary live chat passed in session `ses_780c9e58ab7640d3a5f58f0f0a5f308f`: selected `kairox-live`, sent sentinel prompt, received `KAIROX-LIVE-GUI-0612`, task graph showed 1 completed task and 0 failed tasks.
- Project file edit passed in session `ses_1a229f27975f41b5961b3d9665e1187e`: temp project `/tmp/kairox-gui-live-project.xDcMl4`, model invoked `fs.write`, trace showed 1 `ToolInvocationCompleted`, 0 `ToolInvocationFailed`, and `PermissionGranted`; `agent-note.txt` contained `KAIROX_FILE_EDIT_0612`.
- Attachment path passed after a long live-model wait in session `ses_780c9e58ab7640d3a5f58f0f0a5f308f`: visible user message did not include `KAIROX_PILOT_ATTACHMENT_7F3C9A`; trace `UserMessageAdded.content` did include the attachment sentinel; assistant completed with `KAIROX_PILOT_ATTACHMENT_7F3C9A`; no failed trace events were observed.
- Harness-induced GUI errors were produced by an incorrect manual `window.location.hash` change while testing `start_session`; do not treat the two `WorkbenchView restoreProjectSession` errors from that timestamp as product findings without a clean reproduction.

### 2026-06-12 - Findings and blockers

- Finding candidate: manual `compact_session` on the live session returned without error but did not append `ContextCompactionStarted`, `ContextCompactionCompleted`, `ContextCompactionSkipped`, or `CompactionSummary` events. This needs a focused issue lane before any fix because the root cause may be command state, runtime gating, or projection/event behavior.
- Blocker: `just eval-live kairox-live` could not run because `target/debug/kairox-eval` was killed with exit code 137. A direct `target/debug/kairox-eval --help` also returned 137. No current StarPoint prompt was visible in screenshot or accessible UI, so no automatic allow action was taken for this binary.
- Durable coverage decision: no repo pilot/eval scenario was added in this control lane. Existing deterministic pilot coverage already passed; `kairox-live` automation still needs a separate `test/gui-live-profile-selector` lane if the live TOML should stop hardcoding `github-gpt4o-mini`.

### 2026-06-12 - Final gates and cleanup

- `bun run format:check` initially flagged this plan markdown; `bunx oxfmt --write docs/superpowers/plans/2026-06-11-gui-dev-verification-campaign.md` fixed it, and the rerun passed.
- `bun run lint` passed. `oxlint` reported an existing warning in `apps/agent-gui/src/stores/modelProfiles.test.ts`, but the command exited 0; workspace clippy, parity matrix, and no-inline-tests checks completed successfully.
- Removed the temporary GUI project records for `/tmp/kairox-gui-live-project.xDcMl4`, deleted the temp project directory, stopped the manual Dev App processes, and verified port `1420` had no listener.

## Task 1: Baseline And App Harness

**Files:**

- Modify: `docs/superpowers/plans/2026-06-11-gui-dev-verification-campaign.md`

- [ ] **Step 1: Confirm isolated worktree state**

Run:

```bash
git status --short --branch
```

Expected: branch `test/gui-dev-verification-campaign` with no unexpected changes except this plan after it is created.

- [ ] **Step 2: Confirm lightweight GUI baseline**

Run:

```bash
bun --filter agent-gui test --run src/components/ChatComposer.test.ts src/components/AttachmentTray.test.ts src/components/ChatModelSelector.test.ts
```

Expected: the selected component tests pass.

- [ ] **Step 3: Confirm pilot and port readiness**

Run:

```bash
command -v tauri-pilot
lsof -nP -iTCP:1420 -sTCP:LISTEN || true
```

Expected: `tauri-pilot` is on `PATH`; no stale listener is occupying the Tauri dev port.

## Task 2: Existing Pilot Scenario Pass

**Files:**

- Inspect: `apps/agent-gui/e2e-pilot/chat-flow.toml`
- Inspect: `apps/agent-gui/e2e-pilot/chat-live.toml`
- Inspect: `apps/agent-gui/e2e-pilot/model-switch.toml`
- Inspect: `apps/agent-gui/e2e-pilot/audit-mcp.toml`
- Inspect: `apps/agent-gui/e2e-pilot/audit-skills.toml`

- [ ] **Step 1: Start the Dev App in pilot mode**

Run:

```bash
bun --filter agent-gui tauri dev --features pilot
```

Expected: the Tauri app starts and exposes a tauri-pilot socket.

- [ ] **Step 2: Verify pilot connectivity**

Run:

```bash
tauri-pilot ping
tauri-pilot snapshot -i
```

Expected: ping succeeds and interactive elements include the app shell or sessions sidebar.

- [ ] **Step 3: Run the existing high-value deterministic scenarios**

Run each scenario separately so failures identify the affected surface:

```bash
tauri-pilot run apps/agent-gui/e2e-pilot/app-bootstrap.toml
tauri-pilot run apps/agent-gui/e2e-pilot/session-lifecycle.toml
tauri-pilot run apps/agent-gui/e2e-pilot/chat-flow.toml
tauri-pilot run apps/agent-gui/e2e-pilot/model-switch.toml
tauri-pilot run apps/agent-gui/e2e-pilot/audit-mcp.toml
tauri-pilot run apps/agent-gui/e2e-pilot/audit-skills.toml
```

Expected: each scenario exits 0; failures are recorded with the exact command, step name, screenshot path, and logs.

- [ ] **Step 4: Run the live model scenario with `kairox-live`**

Run:

```bash
tauri-pilot run apps/agent-gui/e2e-pilot/chat-live.toml
```

Expected: live assistant response includes the requested sentinel, attachment content is usable, task graph shows completion, and no failed task node appears.

## Task 3: Manual High-Value User Journeys

**Files:**

- Inspect: `apps/agent-gui/e2e-pilot/*.toml`
- Inspect: `crates/agent-eval/fixtures/*.jsonl`

- [ ] **Step 1: Ordinary live chat**

Use tauri-pilot interactions to start a new session, select `kairox-live` if not already active, send a deterministic prompt, wait for stream completion, assert the assistant text, and check `tauri-pilot logs --level error`.

- [ ] **Step 2: Project chat and file edit**

Register a disposable project or use the pilot project fixture, ask the agent to inspect and edit a disposable file, approve permissions according to the configured policy, assert the file content changed only in the disposable location, and confirm the GUI trace shows the tool call and result.

- [ ] **Step 3: Attachment upload**

Attach `apps/agent-gui/e2e-pilot/fixtures/live-model-attachment.txt`, ask the agent to answer from the attachment only, assert the visible chat message does not leak enriched attachment internals, and assert trace export contains the enriched content.

- [ ] **Step 4: MCP tool call**

Enable `pilot-mcp`, invoke the pilot tool from an agent prompt or settings connectivity flow, assert tool call visibility in chat/trace, and confirm no failed MCP task.

- [ ] **Step 5: Skill invocation**

Use a deterministic built-in skill prompt or settings flow to verify skill discovery, selection, and trace-visible instruction injection behavior. If only settings coverage is deterministic, preserve the interactive skill settings evidence and add runtime skill invocation to eval instead of GUI pilot.

- [ ] **Step 6: Model switch and context compaction**

Switch model/profile controls in the composer, verify the selected profile display, create enough conversation context or use an existing low-threshold eval path to trigger compaction, assert the GUI shows a compaction event and the next turn still responds.

- [ ] **Step 7: Browser tool and computer-use tool surfaces**

Exercise permission/tool-call rendering for browser and computer-use via the safest available deterministic path. Prefer existing pilot/mock coverage for UI surfaces; use live tool invocation only with disposable targets and no sensitive data.

## Task 4: Findings Triage And Fix Dispatch

**Files:**

- Inspect affected files discovered by CodeGraph or `rg`.
- Modify only files owned by the issue-specific lane.

- [ ] **Step 1: Record each finding with reproduction evidence**

For every failure or optimization candidate, record scenario, command, exact UI step, expected result, actual result, logs, screenshots, affected selectors, and whether the issue blocks further validation.

- [ ] **Step 2: Root-cause before fixing**

For each actionable bug, complete the systematic-debugging phases: reproduce consistently, inspect recent changes and similar working examples, state a concrete hypothesis, and identify the source file or contract causing the behavior.

- [ ] **Step 3: Create an issue-specific implementation lane**

Use a branch name with project prefix such as `fix/gui-<short-issue>` or `test/gui-<scenario-name>`. The lane handoff must include owned files, forbidden files, TDD requirement, Dev App verification command, quality gates, and PR title scope.

- [ ] **Step 4: Verify fixes before integration**

The fix lane must run the focused failing test or pilot scenario red-green where feasible, then run relevant format/lint/test gates and Dev App verification before commit or PR.

## Task 5: Eval And Pilot Suite Preservation

**Files:**

- Potentially modify: `apps/agent-gui/e2e-pilot/*.toml`
- Potentially modify: `apps/agent-gui/e2e-pilot/fixtures/*`
- Potentially modify: `crates/agent-eval/fixtures/*.jsonl`
- Potentially modify: `Justfile`

- [ ] **Step 1: Classify high-value scenarios**

Preserve GUI-specific workflows in `apps/agent-gui/e2e-pilot/*.toml`. Preserve model/runtime behavior that can run headlessly in `crates/agent-eval/fixtures/*.jsonl`.

- [ ] **Step 2: Add deterministic assertions**

Use sentinel prompts, disposable files, fixture attachments, explicit selectors, trace export checks, and no credential-bearing values.

- [ ] **Step 3: Run the new or changed scenario**

For pilot:

```bash
tauri-pilot run apps/agent-gui/e2e-pilot/<scenario>.toml
```

For eval:

```bash
cargo build --quiet -p agent-eval --bin kairox-eval
target/debug/kairox-eval run --scenarios crates/agent-eval/fixtures/<fixture>.jsonl --output target/<name>/results.jsonl --report target/<name>/report.json --workspace "$(mktemp -d)" --profile kairox-live --enable-mcp
```

Expected: the new scenario passes locally or reports a precise external-service blocker.

## Task 6: Final Gates And Cleanup

**Files:**

- Inspect: all changed files from `git diff --name-only`

- [ ] **Step 1: Run focused checks for changed areas**

Run the smallest complete checks covering changed files, such as:

```bash
bun --filter agent-gui test --run <changed-test-files>
cargo test -p agent-eval
```

Expected: all focused checks pass.

- [ ] **Step 2: Run required Kairox gates before PR**

Run:

```bash
bun run format:check
bun run lint
```

Expected: both commands exit 0, or any failure is investigated and fixed before PR.

- [ ] **Step 3: Clean local app processes**

Run:

```bash
lsof -nP -iTCP:1420 -sTCP:LISTEN | awk 'NR>1{print $2}' | xargs kill 2>/dev/null || true
tauri-pilot logs --level error || true
```

Expected: no stale Tauri dev listener remains. Error logs are either empty or tied to a recorded finding.

- [ ] **Step 4: Report evidence**

Include scenario commands, pass/fail outcomes, fixes or PRs created, eval/pilot additions, cleanup result, and any blocker with exact command output.
