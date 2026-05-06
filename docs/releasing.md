# Releasing Kairox

This document describes the recommended release flow for Kairox.

## Prerequisites

- `main` is green in CI
- `git-cliff` is installed (`cargo install git-cliff`)
- local verification has passed

## Quick release

Use the helper script to run checks, generate the changelog, tag, and push:

```bash
scripts/release.sh <version>
# Example: scripts/release.sh 0.8.0
```

The script performs these steps automatically:

1. Run format checks, lint, and tests
2. Verify the GUI build
3. Generate `CHANGELOG.md` with git-cliff
4. Commit the changelog update
5. Create (or update) the `vX.Y.Z` tag
6. Push `main` and the tag to origin

## Manual release steps

If you need more control, follow these steps:

### Bump version

```bash
just bump-version X.Y.Z
# This updates: Cargo.toml, Cargo.lock, package.json (root), apps/agent-gui/package.json, tauri.conf.json
git commit -m "chore(release): bump version to X.Y.Z"
```

### Local verification

```bash
just check
just tauri-build
```

### Generate the changelog

```bash
git cliff --tag vX.Y.Z -o CHANGELOG.md
git add CHANGELOG.md
git commit -m "chore(release): update CHANGELOG for vX.Y.Z"
```

Always commit the updated `CHANGELOG.md` **before** creating the tag, so the tagged commit includes the changelog.

### Create or update a release tag

```bash
git tag -fa vX.Y.Z -m "vX.Y.Z"
git push origin main
git push origin vX.Y.Z -f
```

## GitHub Actions behavior

- **CI** (`ci.yml`) runs on pushes to `main` and on pull requests, in parallel jobs: format check, clippy, web lint (eslint + stylelint), cargo test, type-sync (specta), TUI build, GUI web build, and Playwright E2E test job
- **Release Build** (`release-build.yml`) runs on `v*` tags
- Release Build uses git-cliff to generate categorized release notes from conventional commits
- Release Build creates or updates the GitHub Release entry with git-cliff generated notes
- Release Build uploads TUI binaries (with SHA256 checksums) and Tauri desktop bundles for macOS, Linux, and Windows as release assets
- **Dependabot Auto Merge** automatically merges passing Dependabot PRs for npm, Cargo, and GitHub Actions dependency updates

## git-cliff configuration

The changelog format is defined in `cliff.toml` at the repo root. Commits are grouped into:

| Group         | Commit prefix   |
| ------------- | --------------- |
| Features      | `feat`          |
| Bug Fixes     | `fix`           |
| Performance   | `perf`          |
| Documentation | `docs`          |
| Testing       | `test`          |
| Refactor      | `refactor`      |
| Styling       | `style`         |
| CI            | `ci`            |
| Dependencies  | `chore(deps)`   |
| Miscellaneous | `chore` (other) |

`chore(release):` commits are excluded from the changelog automatically.

## Release checklist

- [ ] version bumped in all config files (`just bump-version X.Y.Z`)
- [ ] local verification passed (`just check`)
- [ ] `CHANGELOG.md` generated with `git cliff --tag vX.Y.Z -o CHANGELOG.md`
- [ ] changelog committed before tagging
- [ ] tag pushed
- [ ] Release Build succeeded on all matrix jobs
- [ ] GitHub Release page shows categorized notes
- [ ] release assets include TUI packages and Tauri desktop bundles
