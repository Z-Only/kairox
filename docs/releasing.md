# Releasing Kairox

This document describes the recommended release flow for Kairox.

## Prerequisites

- `main` is green in CI
- `git-cliff` is installed (`cargo install git-cliff`)
- local verification has passed

## Recommended release flow

Prepare releases on a dedicated branch and merge them through PR review. Do not push release-prep commits directly to `main`.

1. Create a release branch, for example `chore/release-vX.Y.Z`
2. Bump versions and generate `CHANGELOG.md`
3. Run local verification
4. Open a PR and wait for the `ci-success` gate to pass
5. Merge the PR into `main`
6. Check out the merged `main` commit locally
7. Create and push the `vX.Y.Z` tag from that merged commit

`release-build.yml` runs from the pushed tag and publishes release notes, TUI binaries, and Tauri desktop bundles.

## Direct-push release script

`scripts/release.sh <version>` is a maintainer-only helper for repositories where direct pushes to `main` are explicitly allowed. It runs checks, generates the changelog, commits it, creates the tag, and pushes both `main` and the tag.

For protected-branch releases, prefer the PR-based flow above and use the manual steps below. Do not use `scripts/release.sh` when the release must pass through PR CI and merge before tagging.

## Manual release steps

If you need more control, prepare the release on a dedicated branch, open a PR, wait for the `ci-success` gate to pass, merge the PR, then create the release tag from the merged `main` commit.

Follow these steps:

### Bump version

```bash
just bump-version X.Y.Z
# This updates: Cargo.toml, Cargo.lock, package.json (root), apps/agent-gui/package.json, tauri.conf.json, docs/current-release.json, and current-release docs
bun run release-docs:check
git commit -m "chore(release): bump version to X.Y.Z"
```

### Local verification

```bash
just check
just check-types
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
git checkout main
git pull --ff-only origin main
git tag -fa vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z -f
```

## GitHub Actions behavior

- **CI** (`ci.yml`) runs on pushes to `main` and on pull requests, in parallel jobs: format check, clippy, web lint (oxlint + stylelint), cargo test, type-sync (specta), TUI build, GUI web build, Playwright E2E, tauri-pilot desktop E2E, and live model smoke tests
- **Release Build** (`release-build.yml`) runs on `v*` tags
- Release Build uses git-cliff to generate categorized release notes from conventional commits
- Release Build creates or updates the GitHub Release entry with git-cliff generated notes
- Release Build uploads TUI binaries (with SHA256 checksums) and Tauri desktop bundles for macOS, Linux, and Windows as release assets
- **Dependabot Auto Merge** automatically merges passing Dependabot PRs for Bun, Cargo, and GitHub Actions dependency updates

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
- [ ] `CHANGELOG.md` generated with `git cliff --tag vX.Y.Z -o CHANGELOG.md`
- [ ] local verification passed (`just check`, `just check-types`, and release-build smoke checks as needed)
- [ ] release branch pushed and PR opened
- [ ] PR `ci-success` gate passed
- [ ] PR merged into `main`
- [ ] local `main` updated with `git pull --ff-only origin main`
- [ ] tag created from the merged `main` commit
- [ ] tag pushed
- [ ] Release Build succeeded on all matrix jobs
- [ ] GitHub Release page shows categorized notes
- [ ] release assets include TUI packages and Tauri desktop bundles
