# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-04-29

### Changed

- Added release helper documentation, repository metadata, community health files, issue forms, and README visuals.
- Added a catch-all category so GitHub generated release notes include uncategorized pull requests.
- Enabled Dependabot automation for safe auto-merge after green checks.
- Updated Rust dependencies including `ratatui` and `toml`.
- Updated JavaScript tooling dependencies including `@commitlint/config-conventional`, `globals`, `typescript`, and `vitest`.

### Verified

- GitHub CI passed on `main` after the merged dependency updates.

## [0.1.0] - 2026-04-29

### Added

- Initial Rust workspace with shared agent core, runtime, models, tools, memory, store, and TUI crates.
- Initial Tauri + Vue GUI shell in `/Users/chanyu/AIProjects/kairox/apps/agent-gui`.
- Repository-wide lint and format toolchain with Prettier, ESLint, Stylelint, cargo fmt, and Clippy.
- Git hooks with Husky + lint-staged and Conventional Commit enforcement with commitlint.
- Open source repository assets including README, Apache-2.0 license, PR template, CI workflow, and release build workflow.

### Verified

- Local `cargo test --workspace --all-targets` passes.
- Local TUI release build passes.
- Local GUI web build passes.
- Local Tauri desktop build passes.
