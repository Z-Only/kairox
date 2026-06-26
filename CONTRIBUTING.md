# Contributing to Kairox

Thanks for contributing to Kairox.

## Development setup

```bash
bun install
```

Optionally install [just](https://github.com/casey/just) for shortcut commands:

```bash
cargo install just    # or: brew install just
just --list           # show all available tasks
```

## Local verification

Run the full CI gate before opening a PR:

```bash
just check
```

This runs format check, lint, and tests — equivalent to CI.

Individual commands:

| Task                 | Command                                                          |
| -------------------- | ---------------------------------------------------------------- |
| Format check         | `just fmt-check` or `bun run format:check`                       |
| Lint                 | `just lint` or `bun run lint`                                    |
| Rust tests           | `just test` or `cargo test --workspace --all-targets`            |
| GUI tests            | `just test-gui`                                                  |
| TUI integration      | `just test-tui`                                                  |
| Full-stack tests     | `just test-fullstack`                                            |
| MCP tests            | `just test-mcp`                                                  |
| GUI E2E (Playwright) | `just test-e2e` (or `just test-e2e-headed` / `just test-e2e-ui`) |
| All test layers      | `just test-all`                                                  |
| Type sync check      | `just check-types`                                               |
| Regenerate types     | `just gen-types`                                                 |
| Build GUI web        | `just gui-build`                                                 |
| Build Tauri app      | `just tauri-build`                                               |

## Commit messages

This repository uses Conventional Commits. Allowed scopes: `core`, `runtime`, `models`, `tools`, `memory`, `store`, `config`, `mcp`, `tui`, `gui`, `deps`, `ci`. Examples:

- `feat(runtime): add scheduler retry policy`
- `fix(gui): handle empty trace state`
- `feat(mcp): add SSE transport support`
- `docs(readme): clarify local setup`

## Pull requests

- Keep PRs focused and reviewable
- Fill out the PR template
- Include screenshots for GUI changes
- Mention platform-specific behavior when relevant

## Code style

- Rust: `cargo fmt` and `cargo clippy`
- Frontend/docs: `oxfmt`, `oxlint`, `stylelint`
- Hooks: `husky`, `lint-staged`, `commitlint`

## Adding new event types or Tauri commands

TypeScript bindings under `apps/agent-gui/src/generated/` are **auto-generated** by [tauri-specta](https://github.com/specta-rs/tauri-specta) — never edit them by hand. After changing any `EventPayload` variant, domain type used in events, or `#[tauri::command]` signature:

1. Run `just gen-types` to regenerate `commands.ts` and `events.ts`
2. Run `just check-types` to verify the generated bindings are in sync (this is also enforced by the CI `type-sync` job)
3. If you added a new IPC command or event the frontend listens to, update the matching mock fragment under `apps/agent-gui/e2e/fixtures/tauri-mock/` and its registry entry so Playwright E2E tests keep passing
4. Make sure new `#[tauri::command]` functions are registered in **both** `generate_handler!` (in `lib.rs`) and `collect_commands!` (in `src/specta.rs`) — missing either one causes runtime or type-gen failures

For full architecture context and per-feature recipes, see [AGENTS.md](./AGENTS.md).

## Dependency updates

Dependabot is configured for the Bun workspace, Cargo dependencies, and GitHub Actions. Please keep dependency PRs scoped and green in CI.
