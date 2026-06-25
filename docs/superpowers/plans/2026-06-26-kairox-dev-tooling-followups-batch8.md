# Kairox Dev Tooling Followups Batch 8 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Continue executing the remaining Kairox optimization list by adding executable eval guardrails, versioned local-skill drift checks, stale project cleanup UX, and model-health UI diagnostics.

**Architecture:** Keep independent lanes in disjoint write scopes. Tooling lanes stay in tracked scripts/tests and do not affect runtime behavior. GUI lanes update store/component behavior with focused Vitest and Dev App verification. Runtime no-progress task completion remains a separate high-risk lane after the executable guardrails and UI diagnostics provide stronger evidence.

**Tech Stack:** Node.js ESM CLI scripts, Node test runner, Rust `agent-eval`, Just recipes, Vue 3/Pinia/Vitest, Tauri command DTOs, tauri-pilot Dev App verification.

---

## Batch Boundaries

- Lane U owns `justfile`, `crates/agent-eval/fixtures/noop-guard.jsonl`, and focused eval CLI/recipe coverage.
- Lane V owns tracked local-skill sync manifests/scripts/tests only; it must not edit `.agents/skills/**`.
- Lane W owns project store/sidebar missing-project cleanup UX and related tests.
- Lane X owns model-profile health result classification/display and related tests.
- Lane Y is a follow-up runtime behavior lane, not mixed into U-X, because it touches task/trajectory completion semantics.
- GUI lanes W and X require Dev App verification before PR. Tooling lanes U and V can skip Dev App with `no interactive behavior change`.

## Task 1: Executable Eval Guard Recipe

**Files:**

- Modify: `justfile`
- Modify or verify: `crates/agent-eval/fixtures/noop-guard.jsonl`
- Modify or verify: `crates/agent-eval/tests/cli.rs`

- [ ] **Step 1: Write RED coverage for the recipe surface**

Add a CLI or script test proving the repo has a focused command that runs only the no-op guard fixture into `target/eval-noop-guard/`.

Expected command:

```bash
cargo test -p agent-eval --test cli noop_guard_fixture_runs_clean_through_cli
```

Expected RED if missing: test fails because the fixture/recipe is absent or not discoverable.

- [ ] **Step 2: Add the narrow `just eval-noop-guard` recipe**

Add a recipe that:

```bash
cargo build --quiet -p agent-eval --bin kairox-eval
KAIROX_EVAL_HOME="$(mktemp -d)"
KAIROX_EVAL_WS="$(mktemp -d)"
trap 'rm -rf "$KAIROX_EVAL_HOME" "$KAIROX_EVAL_WS"' EXIT
mkdir -p target/eval-noop-guard
HOME="$KAIROX_EVAL_HOME" target/debug/kairox-eval run \
  --scenarios crates/agent-eval/fixtures/noop-guard.jsonl \
  --output target/eval-noop-guard/results.jsonl \
  --summary target/eval-noop-guard/summary.json \
  --workspace "$KAIROX_EVAL_WS" \
  --fake-emit-tool-call \
  --wait-timeout-ms 5000
```

The recipe must fail if the fixture does not observe a tool invocation and workspace artifact.

- [ ] **Step 3: Verify Lane U**

Run:

```bash
cargo fmt --all
cargo test -p agent-eval --test cli noop_guard_fixture_runs_clean_through_cli
just eval-noop-guard
cargo fmt --all --check
git diff --check
```

Expected: all commands exit 0, with the cargo test running a non-zero number of tests.

## Task 2: Versioned Kairox Skill Sync Guard

**Files:**

- Create: `scripts/check-kairox-skill-sync.mjs`
- Create: `scripts/check-kairox-skill-sync.test.mjs`
- Create: `docs/ai/kairox-skills/manifest.json`
- Create tracked mirrors only if needed: `docs/ai/kairox-skills/*.md`
- Modify: `package.json` only if adding the script to `test:scripts` is needed.

- [ ] **Step 1: Write RED coverage for ignored skill drift**

Create Node tests with temporary fixture directories:

```text
.agents/skills/kairox-dev-workflow/SKILL.md
docs/ai/kairox-skills/manifest.json
```

Assert the checker fails when the ignored local `SKILL.md` content hash does not match the tracked manifest hash.

Expected RED:

```bash
node --test scripts/check-kairox-skill-sync.test.mjs
```

fails because the checker does not exist.

- [ ] **Step 2: Add passing coverage for synchronized skills**

In the same test file, assert the checker passes when:

- every manifest entry points to an existing local `.agents/skills/<name>/SKILL.md`;
- the local file hash equals the manifest hash;
- tracked metadata includes `name`, `path`, `sha256`, and `updated_at`.

- [ ] **Step 3: Implement read-only sync checker**

Implement:

```js
export async function readSkillManifest(path) {}
export async function evaluateSkillSync({ repoRoot, manifestPath }) {}
```

CLI behavior:

```bash
node scripts/check-kairox-skill-sync.mjs
node scripts/check-kairox-skill-sync.mjs --json
```

The checker must not edit ignored skill files. It only reports missing files, unknown manifest entries, hash mismatches, and suggested refresh command text.

- [ ] **Step 4: Seed the tracked manifest**

Create `docs/ai/kairox-skills/manifest.json` for the Kairox-owned skills currently used by this workflow:

```json
{
  "version": 1,
  "skills": [
    {
      "name": "kairox-dev-workflow",
      "path": ".agents/skills/kairox-dev-workflow/SKILL.md",
      "sha256": "<computed>",
      "updated_at": "2026-06-26"
    },
    {
      "name": "kairox-evaluate-kairox",
      "path": ".agents/skills/kairox-evaluate-kairox/SKILL.md",
      "sha256": "<computed>",
      "updated_at": "2026-06-26"
    },
    {
      "name": "kairox-skill-proxy",
      "path": ".agents/skills/kairox-skill-proxy/SKILL.md",
      "sha256": "<computed>",
      "updated_at": "2026-06-26"
    }
  ]
}
```

Use a small Node one-liner or the checker helper to compute hashes; do not hand-type hashes.

- [ ] **Step 5: Verify Lane V**

Run:

```bash
node --test scripts/check-kairox-skill-sync.test.mjs
node scripts/check-kairox-skill-sync.mjs --json
bun run test:scripts
bun run format:check
git diff --check
```

Expected: all commands exit 0. Dev App verification is skipped because this is read-only developer tooling.

## Task 3: Stale Project Cleanup UX

**Files:**

- Modify: `apps/agent-gui/src/stores/project.ts`
- Modify: `apps/agent-gui/src/stores/project.test.ts`
- Modify: `apps/agent-gui/src/components/sidebar/ProjectSection.vue`
- Modify: `apps/agent-gui/src/components/sidebar/ProjectSection.test.ts`
- Modify locale files if the component uses localized visible text.

- [ ] **Step 1: Write RED store coverage for missing project cleanup**

Add a test named:

```ts
it("removeMissingProjects removes only active projects whose root path is missing", async () => {});
```

Setup:

- one active existing project;
- one active missing project;
- one removed missing project.

Assert only the active missing project triggers `remove_project`, and `loadProjects` refreshes state after removals.

- [ ] **Step 2: Implement minimal store action**

Add:

```ts
const missingProjects = computed(() =>
  activeProjects.value.filter((project) => !project.pathExists)
);

async function removeMissingProjects(): Promise<void> {
  const missingIds = missingProjects.value.map((project) => project.projectId);
  for (const projectId of missingIds) {
    await invoke("remove_project", { projectId });
  }
  if (missingIds.length > 0) {
    await loadProjects();
  }
}
```

Export `missingProjects` and `removeMissingProjects`.

- [ ] **Step 3: Write RED component coverage**

Add a component test asserting that when `activeProjects` includes missing projects:

- a compact missing-project notice is visible;
- the cleanup button invokes `removeMissingProjects`;
- missing projects remain hidden from the normal sidebar project list.

- [ ] **Step 4: Implement component UX**

Add a small notice in `ProjectSection.vue` near the projects header:

- shows count of missing projects;
- uses a destructive/secondary text button to clear them;
- does not render when count is zero;
- keeps existing project session rendering unchanged.

- [ ] **Step 5: Verify Lane W**

Run:

```bash
bun --filter agent-gui test -- project.test.ts ProjectSection.test.ts
bun run format:check
bun run lint
git diff --check
```

Then run Dev App with pilot and verify:

```bash
KAIROX_HOME="$(mktemp -d /tmp/kairox-dev-home.XXXXXX)" bun --filter agent-gui tauri dev --features pilot
tauri-pilot ping
tauri-pilot snapshot -i
tauri-pilot logs --level error
```

Expected: app starts, sidebar renders, no JS errors. If creating a real missing-project fixture through UI is impractical, use the strongest fixture/mock path and state that limitation in PR body.

## Task 4: Functional Model Health UX

**Files:**

- Modify: `apps/agent-gui/src/stores/modelProfiles.ts`
- Modify: `apps/agent-gui/src/stores/modelProfiles.test.ts`
- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue`
- Modify: `apps/agent-gui/src/components/ModelSettingsPane.test.ts`
- Modify locale files if visible strings are localized.

- [ ] **Step 1: Write RED store coverage for actionable health categories**

Add tests proving `testModelConnectivity` results with statuses:

- `empty_response`
- `auth_failed`
- `quota_or_plan_blocked`
- `rate_limited`
- `network_error`

are normalized into stable UI severity/category/recommendation fields.

- [ ] **Step 2: Implement model health presentation helper**

Add a small exported helper:

```ts
export function modelHealthAdvice(result: ConnectivityTestResult): {
  tone: "success" | "warning" | "danger";
  label: string;
  recommendation: string;
};
```

Keep it pure and unit-testable.

- [ ] **Step 3: Write RED component coverage**

Add a component test asserting an `empty_response` probe result displays a clear action such as checking model availability, quota, or plan, instead of only raw error text.

- [ ] **Step 4: Implement visible diagnostics**

Update `ModelSettingsPane.vue` so failed health checks show:

- status/category label;
- concise recommendation;
- raw error detail only as secondary text;
- response preview only for successful chat-ready probes.

- [ ] **Step 5: Verify Lane X**

Run:

```bash
bun --filter agent-gui test -- modelProfiles.test.ts ModelSettingsPane.test.ts
bun run format:check
bun run lint
git diff --check
```

Then run Dev App/pilot through the model settings page and verify a fake or mocked failed health result renders without JS errors.

## Task 5: Runtime No-Progress Completion Semantics Follow-Up

**Files:**

- Likely modify: `crates/agent-runtime/src/agent_loop/runner.rs`
- Likely modify: `crates/agent-runtime/tests/agent_loop/text_turns.rs`
- Likely modify: `crates/agent-runtime/tests/task_graph_integration.rs`

- [ ] **Step 1: Do not mix into GUI/tooling PRs**

Keep this lane separate because current tests intentionally assert that ordinary plain text assistant messages complete the root task.

- [ ] **Step 2: Define acceptance before implementation**

Before writing code, define the exact boundary between:

- valid no-tool assistant answer for ordinary chat;
- invalid no-progress completion for code/eval skill turns.

The first safe target is a diagnostic-only event or eval-only classification, not broad task failure for all no-tool messages.

- [ ] **Step 3: Add RED runtime coverage**

Add a focused runtime test that constructs a code-task or skill-task request and proves an intent-only assistant message with no tools and no workspace artifact is not reported as successful completion.

- [ ] **Step 4: Implement minimal runtime diagnostic/failure path**

Only after RED coverage, update task/trajectory semantics so ordinary chat remains successful but code/eval no-progress turns are observable as incomplete or failed.

- [ ] **Step 5: Verify Lane Y**

Run:

```bash
cargo fmt --all
cargo test -p agent-runtime --test task_graph_integration
cargo test -p agent-runtime --test agent_loop
cargo fmt --all --check
git diff --check
```

If user-visible task state changes in GUI, add GUI store/component coverage and Dev App verification.

## Completion

- [ ] Each implementation lane lands through its own PR unless two tooling lanes are explicitly combined because their files and validation remain disjoint.
- [ ] Functional PRs include Dev App/pilot evidence in the PR body.
- [ ] Each PR is observed until `MERGED`, and local `main` is fast-forwarded after merge.
- [ ] Each lane worktree, local branch, and remote branch is cleaned.
- [ ] Final audit confirms `main` is clean and no batch 8 PR remains open.
- [ ] Remaining optimization points from the user-visible list are either implemented and verified or explicitly carried forward with a blocker-free next lane.
