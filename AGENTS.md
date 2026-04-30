# Kairox — AI Agent Instructions

This file provides project-specific guidance for AI coding assistants (Codex, Claude Code, Cursor, etc.).

## Project structure

- **Rust workspace** at the root: `crates/agent-core`, `agent-runtime`, `agent-models`, `agent-tools`, `agent-memory`, `agent-store`, `agent-tui`
- **Tauri + Vue GUI**: `apps/agent-gui/`
- **Tooling**: Prettier, ESLint, Stylelint, cargo fmt, clippy, husky, lint-staged, commitlint

## Commit conventions

Conventional Commits are enforced via commitlint. Use these scopes:

- `runtime`, `models`, `tools`, `memory`, `store`, `tui`, `gui`, `deps`, `ci`

Examples:

- `feat(runtime): add scheduler retry policy`
- `fix(gui): handle empty trace state`
- `chore(deps): bump reqwest to 0.12`

## Git worktrees

This project uses git worktrees for isolated branch development. After creating a worktree, always run `npm install` to set up husky hooks (the `prepare` script auto-links hooks for worktrees):

```bash
git worktree add ../kairox-<branch> -b <branch> main
cd ../kairox-<branch>
npm install   # triggers prepare.cjs which links husky hooks
```

The `prepare.cjs` script detects worktrees and creates a symlink from `GIT_DIR/.husky` to the worktree's `.husky` directory so that pre-commit and commit-msg hooks fire correctly.

## Local verification

Run before opening a PR or pushing to main:

```bash
npm run format:check
npm run lint
cargo test --workspace --all-targets
```

Pre-commit hooks (husky + lint-staged) automatically run on staged files:

- `*.{json,md}` → `prettier --write`
- `apps/agent-gui/**/*.{ts,tsx,js,jsx,vue}` → `prettier --write` + `eslint --fix`
- `apps/agent-gui/src/**/*.{vue,css,scss,sass,less}` → `prettier --write` + `stylelint --fix`
- `*.rs` → `cargo fmt --all`

## Release flow

Use `scripts/release.sh <version>` to publish a release. Example:

```bash
scripts/release.sh 0.3.0
```

The script runs checks, verifies the GUI build, generates `CHANGELOG.md` with git-cliff, commits it, creates the tag, and pushes.

### Manual release steps (if not using the script)

If you need to release without the script:

1. Run `git cliff --tag vX.Y.Z -o CHANGELOG.md` to regenerate the changelog
2. Commit the changelog: `git add CHANGELOG.md && git commit -m "chore(release): update CHANGELOG for vX.Y.Z"`
3. Create and push the tag: `git tag -fa vX.Y.Z -m "vX.Y.Z" && git push origin main && git push origin vX.Y.Z -f`

**Always commit an updated `CHANGELOG.md` before pushing the release tag.** The tag should point to a commit that includes the changelog update.

### How git-cliff works

- `cliff.toml` at the repo root configures the changelog format and commit grouping
- Commits are grouped into Features, Bug Fixes, Performance, Documentation, Testing, Refactor, Dependencies, and Miscellaneous
- `chore(release):` commits are automatically excluded from the changelog
- GitHub Actions also runs git-cliff to generate Release Notes on the GitHub Release page

## CI

- **CI** runs on push to `main` and on pull requests
- **Release Build** runs on `v*` tags: builds TUI + Tauri binaries for all platforms, publishes a GitHub Release with git-cliff generated notes
