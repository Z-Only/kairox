# Contributing to Kairox

Thanks for contributing to Kairox.

## Development setup

```bash
pnpm install
```

## Local verification

Run these before opening a pull request:

```bash
pnpm run format:check
pnpm run lint
cargo test --workspace --all-targets
pnpm --filter agent-gui run build
pnpm --filter agent-gui run tauri:build
```

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

## Dependency updates

Dependabot is configured for the root npm workspace, the GUI workspace, Cargo dependencies, and GitHub Actions. Please keep dependency PRs scoped and green in CI.
