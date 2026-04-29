# Releasing Kairox

This document describes the recommended release flow for Kairox.

## Prerequisites

- `main` is green in CI
- release-related docs are up to date
- `CHANGELOG.md` is updated
- local verification has passed

## Local verification

```bash
cd /Users/chanyu/AIProjects/kairox
npm run format:check
npm run lint
cargo test --workspace --all-targets
cargo build -p agent-tui --release
cd /Users/chanyu/AIProjects/kairox/apps/agent-gui
npm run build
npm run tauri:build
```

## Create or update a release tag

Example for a new version:

```bash
cd /Users/chanyu/AIProjects/kairox
git tag -a v0.1.1 -m "v0.1.1"
git push origin v0.1.1
```

If a tag must be corrected after fixing release workflow issues:

```bash
cd /Users/chanyu/AIProjects/kairox
git tag -fa v0.1.1 -m "v0.1.1"
git push origin v0.1.1 -f
```

## GitHub Actions behavior

- `CI` runs on pushes to `main` and on pull requests
- `Release Build` runs on `v*` tags
- `Release Publish` runs on `v*` tags and creates or updates the GitHub Release entry

## Release checklist

- [ ] `CHANGELOG.md` updated
- [ ] `README.md` version or release links checked
- [ ] local verification passed
- [ ] tag pushed
- [ ] Release Build succeeded on all matrix jobs
- [ ] GitHub Release page looks correct

## Helper script

A helper script is available:

```bash
/Users/chanyu/AIProjects/kairox/scripts/release.sh 0.1.1
```

This script runs checks, verifies builds, updates the tag, and pushes it.
