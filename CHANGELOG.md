# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
