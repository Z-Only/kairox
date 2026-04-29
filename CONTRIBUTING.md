# Contributing to Kairox

Thanks for contributing to Kairox.

## Development setup

```bash
cd /Users/chanyu/AIProjects/kairox
npm install
cd /Users/chanyu/AIProjects/kairox/apps/agent-gui
npm install
```

## Local verification

Run these before opening a pull request:

```bash
cd /Users/chanyu/AIProjects/kairox
npm run format:check
npm run lint
cargo test --workspace --all-targets
cd /Users/chanyu/AIProjects/kairox/apps/agent-gui
npm run build
npm run tauri:build
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
