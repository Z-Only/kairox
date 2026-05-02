# Contributing to Kairox

Thanks for contributing to Kairox.

## Development setup

```bash
pnpm install
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

| Task            | Command                                               |
| --------------- | ----------------------------------------------------- |
| Format check    | `just fmt-check` or `pnpm run format:check`           |
| Lint            | `just lint` or `pnpm run lint`                        |
| Rust tests      | `just test` or `cargo test --workspace --all-targets` |
| GUI tests       | `just test-gui`                                       |
| Type sync check | `just check-types`                                    |
| Build GUI web   | `just gui-build`                                      |
| Build Tauri app | `just tauri-build`                                    |

## Commit messages

This repository uses Conventional Commits. Examples:

- `feat(runtime): add scheduler retry policy`
- `fix(gui): handle empty trace state`
- `docs(readme): clarify local setup`

## Pull requests

- Keep PRs focused and reviewable
- Fill out the PR template
- Include screenshots for GUI changes
- Mention platform-specific behavior when relevant

## Code style

- Rust: `cargo fmt` and `cargo clippy`
- Frontend/docs: `prettier`, `eslint`, `stylelint`
- Hooks: `husky`, `lint-staged`, `commitlint`

## Adding new event types

When adding a new `EventPayload` variant in Rust, also update the TypeScript type in `apps/agent-gui/src/types/index.ts`. Run `just check-types` to verify both sides are in sync.

## Dependency updates

Dependabot is configured for the root npm workspace, the GUI workspace, Cargo dependencies, and GitHub Actions. Please keep dependency PRs scoped and green in CI.
