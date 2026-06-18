---
title: Contributing
description: How to propose, build, test, and submit changes to Kairox — the full PR loop end to end.
outline: [2, 3]
---

# Contributing

Kairox is built almost entirely from community-authored PRs. This page is the end-to-end loop: how to propose a change, build and verify locally, write a PR that reviewers can merge, and follow it through to release.

::: tip Source of truth
The canonical contribution rules are in [`CONTRIBUTING.md`](https://github.com/Z-Only/kairox/blob/main/CONTRIBUTING.md) and [`AGENTS.md`](https://github.com/Z-Only/kairox/blob/main/AGENTS.md) at the repository root. If this page disagrees with either, the repository files win. This page expands on workflow, intent, and the why.
:::

## Before you start

Three things make a contribution land smoothly:

1. **Have the toolchain installed**. See [Installation](../guide/installation). Without Bun, Rust stable, Node 22+, and `just`, you cannot run the gates that CI runs.
2. **Have an opinion about scope**. Small, focused PRs merge in days. Sprawling PRs that touch six unrelated crates merge in weeks or never. If a change feels large, split it.
3. **Know whether you need a spec**. Bug fixes and small features go straight to a PR. Anything that changes the runtime contract, the event vocabulary, the permission model, or how UIs talk to the runtime — write a spec first under `docs/superpowers/specs/`. The repository's `superpowers` skill set explains the workflow.

If you are unsure whether to write a spec, open a [discussion](https://github.com/Z-Only/kairox/discussions) and ask. It is cheaper to align early than to redo a PR.

## Find something to work on

Several places to look:

- **[Good first issues](https://github.com/Z-Only/kairox/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)** — small, scoped, well-described tasks.
- **[ROADMAP.md](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md)** — the mid-term and long-term picture; pick a horizon item and propose how you would tackle it.
- **[Discussions](https://github.com/Z-Only/kairox/discussions)** — feature requests and integration questions that may not yet be issues.
- **Your own use case** — open a discussion describing the gap; the maintainers will help shape it into a PR.

## Set up a worktree

Kairox uses git worktrees for isolated development. The base checkout stays clean for rebases and emergency fixes:

```bash
just worktree feat/my-feature
cd .worktrees/feat-my-feature
```

`just worktree` creates `.worktrees/feat-my-feature/` branched from local `main` and runs `bun install` so Husky pre-commit hooks register. Sync local `main` first when you want a fresh upstream base.

Branch prefix conventions: `feat/`, `fix/`, `refactor/`, `test/`, `docs/`, `chore/`, `ci/`. Use one that matches the change.

## Implement the change

Follow [AGENTS.md](https://github.com/Z-Only/kairox/blob/main/AGENTS.md) and respect the dependency direction in the crate map. Three rules of thumb:

- **Start from `agent-core`** if you need a new domain type or event variant. The rest of the workspace depends on `agent-core`; adding the type there first lets every downstream crate use it.
- **Wire to UIs last**. TUI and GUI should consume what already works end to end in the runtime. If a feature needs a new IPC surface, add the Tauri command and `EventPayload` first, then the UI consumer.
- **Tests near the code**. Use `FakeModelClient` for runtime tests; use the in-memory SQLite event store for storage tests; use Playwright with the IPC mock for GUI behavior. Live GitHub Models smoke tests are gated by `GITHUB_TOKEN` and self-skip without it.

## Quality gates

The local equivalent of CI is one command:

```bash
just check
```

That is the union of three sub-gates:

| Gate            | What it runs                                                       | Why it fails                                 |
| --------------- | ------------------------------------------------------------------ | -------------------------------------------- |
| Format check    | `oxfmt` (TS/Vue/Markdown) + `cargo fmt --check`                    | Unformatted code. Fix with `bun run format`. |
| Lint            | `oxlint`, `cargo clippy --all-targets -- -D warnings`, `stylelint` | Warnings. Treat warnings as errors locally.  |
| Rust test suite | `cargo test --workspace --all-targets`                             | Failing or panicking tests.                  |

Individual recipes for focused work:

| Task                 | Command                                                       |
| -------------------- | ------------------------------------------------------------- |
| Format check         | `just fmt-check` / `bun run format:check`                     |
| Auto-format          | `bun run format`                                              |
| Lint                 | `just lint` / `bun run lint`                                  |
| Rust tests           | `just test`                                                   |
| TUI integration      | `just test-tui`                                               |
| Full-stack runtime   | `just test-fullstack`                                         |
| MCP focused tests    | `just test-mcp`                                               |
| GUI unit (Vitest)    | `just test-gui`                                               |
| GUI E2E (Playwright) | `just test-e2e` / `just test-e2e-headed` / `just test-e2e-ui` |
| Desktop E2E (pilot)  | `just test-pilot`                                             |
| Live model smoke     | `just test-live` (self-skips without `GITHUB_TOKEN`)          |
| All test layers      | `just test-all`                                               |
| Type sync check      | `just check-types`                                            |
| Regenerate types     | `just gen-types`                                              |

The CI job `ci-success` aggregates the parallel jobs and is the required check for merge. If `just check` is green and your changes do not touch IPC contracts, your PR will almost certainly pass CI.

## The type-sync workflow

TypeScript bindings under `apps/agent-gui/src/generated/` are produced by [tauri-specta](https://github.com/specta-rs/tauri-specta). **Never edit them by hand.**

After changing any `#[tauri::command]` signature, any `EventPayload` variant, or any domain type referenced in events:

1. Run `just gen-types`. This regenerates `commands.ts` and `events.ts`.
2. Run `just check-types`. CI also runs this in the `type-sync` job and will block merge if the generated output drifts.
3. If you added a new IPC command or event that the frontend listens to, update [`apps/agent-gui/e2e/tauri-mock.js`](https://github.com/Z-Only/kairox/blob/main/apps/agent-gui/e2e/tauri-mock.js) so Playwright E2E still runs against a complete mock.
4. New `#[tauri::command]` functions must be registered in **both** `tauri::generate_handler!` (in `apps/agent-gui/src-tauri/src/lib.rs`) **and** `collect_commands!` (in `apps/agent-gui/src-tauri/src/specta.rs`). Missing either causes runtime or type-gen failures.

## Commit messages

Conventional Commits with project-specific scopes. The full list:

```
core, runtime, models, tools, memory, store, config, mcp, skills, plugins, tui, gui, deps, ci
```

Examples that pass commitlint:

```
feat(runtime): add scheduler retry policy
fix(gui): handle empty trace state
feat(mcp): add SSE transport support
docs(readme): clarify local setup
chore(deps): bump tauri to 2.7
```

A bad commit message will fail the commit hook locally — commitlint runs via Husky. If you bypassed Husky (`bun install` not run after worktree creation), CI will reject the PR title check.

## Open the PR

```bash
git push -u origin <branch>
gh pr create --fill --base main
```

The PR template asks for:

- a one-paragraph summary of the change and the motivation;
- the verification you ran (which `just` recipes, what passed);
- screenshots or short clips for any GUI change;
- a note on platform-specific behavior if the change is OS-dependent.

Fill all of it. Reviewers triage PRs by template completeness first.

## Review and iteration

Reviewers will:

- check the change matches the spec (if one exists) or the discussion;
- check tests cover the new behavior (and would catch a regression);
- run the GUI surfaces and behaviorally test if the change is user-facing;
- request changes inline.

Push fixups to the same branch. After approval, your PR is squash-merged into `main` (auto-merge is enabled where the contributor flow supports it). Squash means one commit per PR on `main`; the message of that commit is the PR title, so titles matter.

## After merge

Your commit lands on `main` and is picked up by:

- [`git-cliff`](https://github.com/orhun/git-cliff) on the next release, which groups it into the changelog under the matching prefix (`feat:` → Features, `fix:` → Bug Fixes, etc.).
- The next desktop binary build (triggered on `v*` tag push by [`release-build.yml`](https://github.com/Z-Only/kairox/blob/main/.github/workflows/release-build.yml)).

See [Releases & Security](./releases-and-security) for the release model and how to verify built artifacts.

## When something breaks

If your PR breaks `main` after merge (rare but possible — flaky test, missing skip on a CI matrix entry):

- Open a follow-up `fix:` PR immediately.
- Do not force-push to `main`.
- The maintainer may temporarily revert your PR with `revert:` until the fix lands; that is not a judgment, it is how the project keeps `main` green.

## Code style

- **Rust**: `cargo fmt`, `cargo clippy -- -D warnings`. Follow the patterns in adjacent code; do not introduce a new dependency without justifying it in the PR description.
- **TypeScript / Vue**: `oxfmt` for format, `oxlint` for lint, `stylelint` for CSS. Use Pinia for state; prefer Composition API setup stores; do not bypass `vue-i18n` for user-facing strings.
- **Markdown / docs**: `oxfmt` formats Markdown too. Keep prose direct.

## Dependency updates

Dependabot is configured for Bun, Cargo, and GitHub Actions. Dependency PRs auto-merge when CI passes via the Dependabot auto-merge workflow. If you want to bump a dep manually, scope the PR narrowly — one ecosystem at a time.

## Getting help

- **[GitHub Discussions](https://github.com/Z-Only/kairox/discussions)** — design questions, integration questions, scope questions.
- **[GitHub Issues](https://github.com/Z-Only/kairox/issues)** — reproducible bugs with steps and environment.
- **[Crate Index](../reference/crate-index)** — the map from "I want to change behavior X" to "the code that defines behavior X."
- **[Architecture](../concepts/architecture)** — read this before any non-trivial PR; it explains the rules of the workspace.

## What this page does not cover

This page is the contribution workflow. It does not cover what to build ([Roadmap](./roadmap)), how to verify an artifact you downloaded ([Releases & Security](./releases-and-security)), or the conceptual model of the code you are about to change ([Architecture](../concepts/architecture), [Runtime & Sessions](../concepts/runtime-and-sessions)).
