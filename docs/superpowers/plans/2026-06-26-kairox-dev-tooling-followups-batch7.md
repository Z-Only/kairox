# Kairox Dev Tooling Followups Batch 7 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Continue executing the remaining Kairox optimization list with three independent tool/eval PRs: generated-file guardrails, model-health diagnostics, and broader executable evaluation coverage.

**Architecture:** Keep this batch in tracked developer tooling and eval fixtures/tests. Each lane owns disjoint files so it can be implemented, reviewed, validated, merged, and cleaned independently. Runtime no-op gating and stale project prune UX remain later functional lanes because they touch core runtime or GUI behavior.

**Tech Stack:** Node.js ESM CLI scripts, Node test runner, Rust `agent-eval` fixtures and CLI tests, Git diff/status inspection, Bun formatting/linting.

---

## Batch Boundaries

- Lane R owns a generated-file guard script and script tests only.
- Lane S owns a model/eval health summary script and script tests only.
- Lane T owns `agent-eval` deterministic fixtures/tests and may update `justfile` only if needed for an executable recipe.
- Lanes must not edit `.agents/skills/**`, GUI generated bindings, runtime crates, GUI components, package lockfiles, or each other's files.
- Dev App verification is not required because this batch changes local developer/evaluation tooling only, with no interactive GUI behavior.

## Task 1: Generated File Guardrails

**Files:**

- Create: `scripts/check-generated-guardrails.mjs`
- Create: `scripts/check-generated-guardrails.test.mjs`
- Modify: `package.json` only if adding a script alias is useful

- [ ] **Step 1: Add RED coverage for generated drift without source trigger**

Create a fixture-style Node test that builds a temporary git repo with:

```text
apps/agent-gui/src/generated/commands.ts
README.md
```

Make `git diff --name-only --cached` or a fake command runner report only `apps/agent-gui/src/generated/commands.ts`. Assert the guard fails with a message naming the generated file and the required source triggers.

Expected RED:

```bash
node --test scripts/check-generated-guardrails.test.mjs
```

fails because the script does not exist.

- [ ] **Step 2: Add RED coverage for allowed source-triggered generated updates**

In the same test file, assert the guard passes when changed paths include a generated file and at least one trigger such as:

```text
apps/agent-gui/src-tauri/src/commands/chat.rs
crates/agent-core/src/events.rs
```

Also assert it passes when no generated files changed.

Expected RED: the script still does not exist.

- [ ] **Step 3: Implement minimal guard**

Implement an exported function such as:

```js
export function evaluateGeneratedGuardrails(changedPaths) {
  // returns { ok, generatedPaths, triggerPaths, message }
}
```

Behavior requirements:

- Generated paths are `apps/agent-gui/src/generated/commands.ts` and `apps/agent-gui/src/generated/events.ts`.
- Trigger paths include `apps/agent-gui/src-tauri/**`, `crates/agent-core/src/events.rs`, and `crates/agent-core/src/**` files that define exported DTO/event types.
- Default CLI mode reads `git diff --name-only --cached` first; if empty, read `git diff --name-only`.
- `--base <ref>` mode reads `git diff --name-only <ref>...HEAD`.
- Failure text should say generated bindings changed without a Rust/Specta/event source trigger and suggest `just gen-types` only after generator/source changes.
- Keep the script side-effect free; it must not run `just gen-types` or edit files.

- [ ] **Step 4: Verify Lane R**

Run:

```bash
node --test scripts/check-generated-guardrails.test.mjs
bun run test:scripts
node scripts/check-generated-guardrails.mjs --help
bun run format:check
git diff --check
```

Expected: all commands exit 0 with non-zero test counts.

## Task 2: Model Health Diagnostics From Eval Output

**Files:**

- Create: `scripts/model-health-summary.mjs`
- Create: `scripts/model-health-summary.test.mjs`

- [ ] **Step 1: Add RED coverage for model-backend failure classification**

Create Node tests that pass synthetic eval result records containing failures/errors such as:

```json
{"scenario_id":"a","error":"model returned an empty response; check model availability, quota, or plan"}
{"scenario_id":"b","failures":["runtime error: HTTP 429 rate limit"]}
{"scenario_id":"c","failures":["runtime error: invalid API key"]}
```

Assert the summary groups them under stable categories:

- `empty_response`
- `rate_limited`
- `auth`

Expected RED:

```bash
node --test scripts/model-health-summary.test.mjs
```

fails because the script does not exist.

- [ ] **Step 2: Add RED coverage for mixed JSONL/report inputs**

Add tests proving the CLI can read either:

- a JSONL results file with one `EvalResult` per line; or
- a report JSON file shaped as `{ "summary": ..., "results": [...] }`.

Assert JSON output includes totals, failed scenario ids, category counts, and a concise recommendation for empty responses.

Expected RED: no parser exists yet.

- [ ] **Step 3: Implement read-only diagnostics CLI**

Implement exported helpers:

```js
export function classifyModelHealthIssue(text) {}
export function summarizeModelHealth(results) {}
export async function readEvalResults(path) {}
```

CLI behavior:

- Usage: `node scripts/model-health-summary.mjs <results.jsonl|report.json> [--json]`
- Human output lists total scenarios, failed scenarios, categories, and recommendations.
- `--json` prints only JSON.
- If all scenarios pass, print a healthy summary and exit 0.
- If failures exist, still exit 0; this script diagnoses completed eval output and should not replace `kairox-eval` failure semantics.

- [ ] **Step 4: Verify Lane S**

Run:

```bash
node --test scripts/model-health-summary.test.mjs
bun run test:scripts
node scripts/model-health-summary.mjs --help
bun run format:check
git diff --check
```

Expected: all commands exit 0 with non-zero test counts.

## Task 3: Broader Executable Eval Coverage

**Files:**

- Create or modify: `crates/agent-eval/fixtures/noop-guard.jsonl`
- Modify: `crates/agent-eval/tests/cli.rs`
- Modify: `justfile` only if adding a narrow recipe such as `eval-noop-guard`

- [ ] **Step 1: Add RED CLI coverage for a deterministic no-op guard fixture**

Add a CLI test that runs a new fixture and expects it to pass with the fake model/tool-call path. The fixture must encode a coding/evaluation requirement that would fail if an agent only produces an intent-only assistant message and never invokes tools or creates expected files.

Expected fixture shape:

```json
{
  "id": "noop-guard-requires-tool-and-file",
  "prompt": "Create target/noop-guard/output.txt containing exactly ok.",
  "profile": "fake",
  "expected": {
    "min_tool_invocations": 1,
    "max_tool_failures": 0,
    "workspace_files": [{ "path": "target/noop-guard/output.txt", "contains": ["ok"] }]
  }
}
```

Expected RED:

```bash
cargo test -p agent-eval --test cli noop_guard_fixture_runs_clean_through_cli
```

fails because the fixture/test does not exist or because the fake tool path does not create the file.

- [ ] **Step 2: Make the fixture executable without real model access**

Use existing deterministic harness capabilities instead of live model access. If the current fake tool-call mechanism can only emit one fixed tool, either:

- extend the fixture to assert existing deterministic tool behavior, or
- add the smallest test-only fake tool-call arguments needed to create an observable workspace artifact through an existing built-in tool.

Do not weaken the optimization intent: the fixture must prove `min_tool_invocations` and an external observable artifact, not just assistant text.

- [ ] **Step 3: Add optional recipe if it improves discoverability**

If no focused command exists, add a `just eval-noop-guard` recipe that builds `kairox-eval` and runs only the new fixture into `target/eval-noop-guard/`.

If `cargo test -p agent-eval --test cli ...` is enough, skip the recipe and state `written plan not needed for justfile: existing CLI test covers the executable path`.

- [ ] **Step 4: Verify Lane T**

Run:

```bash
cargo fmt --all
cargo test -p agent-eval --test cli noop_guard_fixture_runs_clean_through_cli
cargo test -p agent-eval --test cli
cargo fmt --all --check
git diff --check
```

If `justfile` changes, also run the new recipe in dry/non-live mode or document why the focused CLI test is the executable verification.

## Completion

- [ ] Each lane lands through its own PR.
- [ ] Each PR is observed until `MERGED`, and local `main` is fast-forwarded after merge.
- [ ] Each lane worktree, local branch, and remote branch is cleaned.
- [ ] Final audit confirms `main` is clean and no batch 7 PR remains open.
- [ ] Remaining optimization points are carried forward explicitly: runtime no-op success gating, stale project prune UX, executable evaluation harness hardening beyond fixture coverage, versioned SKILL synchronization, and functional model health UX if the script-only diagnostics are not sufficient.
