# Kairox

![Kairox banner](https://github.com/Z-Only/kairox/blob/main/docs/assets/banner.svg)

[![CI](https://github.com/Z-Only/kairox/actions/workflows/ci.yml/badge.svg)](https://github.com/Z-Only/kairox/actions/workflows/ci.yml)
[![Release Build](https://github.com/Z-Only/kairox/actions/workflows/release-build.yml/badge.svg)](https://github.com/Z-Only/kairox/actions/workflows/release-build.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://github.com/Z-Only/kairox/blob/main/LICENSE)
[![Release](https://img.shields.io/github/v/release/Z-Only/kairox)](https://github.com/Z-Only/kairox/releases)

Kairox is a local-first AI agent workbench built with a shared Rust core, a terminal UI, and a Tauri + Vue desktop GUI.

![Kairox logo](https://github.com/Z-Only/kairox/blob/main/docs/assets/logo.svg)

## Quick links

- [Latest release](https://github.com/Z-Only/kairox/releases/latest)
- [Roadmap](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md)
- [Contributing](https://github.com/Z-Only/kairox/blob/main/CONTRIBUTING.md)
- [Security policy](https://github.com/Z-Only/kairox/blob/main/SECURITY.md)
- [Discussions](https://github.com/Z-Only/kairox/discussions)
- [Code of conduct](https://github.com/Z-Only/kairox/blob/main/CODE_OF_CONDUCT.md)
- [Release guide](https://github.com/Z-Only/kairox/blob/main/docs/releasing.md)

## Architecture

```mermaid
graph TD
    UI["User Interfaces"]
    TUI["TUI (ratatui)"]
    GUI["GUI (Tauri + Vue)"]
    CORE["agent-core"]
    RUNTIME["agent-runtime"]
    MODELS["agent-models"]
    TOOLS["agent-tools"]
    MEMORY["agent-memory"]
    STORE["agent-store"]
    CONFIG["agent-config"]

    UI --> TUI
    UI --> GUI
    TUI --> CORE
    GUI --> CORE
    CORE --> RUNTIME
    RUNTIME --> MODELS
    RUNTIME --> TOOLS
    RUNTIME --> MEMORY
    RUNTIME --> STORE
    RUNTIME --> CONFIG
```

## Highlights

- Local-first architecture with a shared Rust core
- Two user surfaces: TUI and Tauri + Vue desktop GUI
- Structured runtime, memory, tools, and persistence layers
- Complete open-source repository baseline with CI, release automation, and community docs

## Features

- **Shared Rust core** — domain types, event-sourced runtime, facade trait, typed IDs
- **Memory system** — durable session/user/workspace-scoped memory with `<memory>` marker protocol and keyword retrieval
- **Model adapters** — OpenAI, Anthropic, Ollama, and fake provider for testing
- **Tool system** — built-in tools (shell, search, patch, fs) with 5-level permission control
- **Config discovery** — TOML config with profile management and env-variable API keys
- **TUI application** — three-panel ratatui terminal UI with streaming chat, trace, and permission prompts
- **GUI desktop app** — Tauri 2 + Vue 3 with session management, trace visualization, memory browser, and permission center
- **Local-first architecture** — designed for offline-friendly workflows and explicit permission control
- **Quality gates** — parallel CI, type-sync checks, cargo clippy, ESLint, Stylelint, Prettier, commitlint

## Repository layout

- `crates/agent-core` — shared domain types and application facade
- `crates/agent-runtime` — runtime orchestration and task graph
- `crates/agent-models` — model profile and provider boundaries
- `crates/agent-tools` — permission and tool abstractions
- `crates/agent-memory` — memory and context assembly with tiktoken
- `crates/agent-store` — SQLite-backed event store
- `crates/agent-config` — TOML config loading, model profile discovery, API key resolution
- `crates/agent-tui` — interactive ratatui terminal UI app
- `apps/agent-gui` — Vue 3 frontend + Tauri 2 desktop app

## Status

Kairox v0.7.0 is in active development with a fully interactive TUI and a functional GUI featuring session management, trace visualization, memory browsing, and permission control. Real model adapters (OpenAI, Anthropic, Ollama), built-in tools, event-sourced runtime, and memory are in place. CI runs 7 parallel jobs with type-sync checks.

## Requirements

- Rust stable toolchain
- Node.js 22+
- pnpm 10+

For Tauri desktop packaging:

- macOS: Xcode Command Line Tools
- Linux: WebKitGTK and Tauri native dependencies (see `ci.yml` for the full list)
- Windows: WebView2 toolchain

## Demo

> Run `cargo run -p agent-tui` for a live demo of the interactive TUI with streaming chat, tool trace, and sidebar controls.

## Why Kairox?

Kairox aims to provide a local-first foundation for AI agent workflows with explicit boundaries between shared core logic, runtime orchestration, model integration, and user interfaces.

## Getting started

If you want to try Kairox quickly, start with the local setup and quality gates below, then run either the TUI or the GUI shell.

### Install dependencies

```bash
pnpm install
```

### Run quality gates

```bash
just check
```

Or individually:

```bash
just fmt-check      # format check
just lint           # clippy + eslint + stylelint
just test           # cargo test
just check-types    # Rust ↔ TypeScript type sync
```

> Install [just](https://github.com/casey/just) with `cargo install just` or `brew install just`.

### Run TUI

```bash
just tui
```

### Run GUI (Vite dev server)

```bash
just gui-dev
```

### Run Tauri desktop app in development

```bash
just tauri-dev
```

This starts the Vite dev server and the native Tauri window together, providing hot-reload for both the frontend and the Rust backend.

### Build GUI web assets

```bash
just gui-build
```

### Build Tauri desktop app

```bash
just tauri-build
```

## Tooling

Repository-level quality tooling includes:

- **Prettier** for frontend/docs formatting
- **ESLint** for Vue/TS linting
- **Stylelint** for styles and Vue style blocks
- **cargo fmt** for Rust formatting
- **cargo clippy** for Rust linting
- **Husky + lint-staged** for pre-commit enforcement
- **commitlint** for Conventional Commits on `commit-msg`

Useful commands (with [just](https://github.com/casey/just)):

```bash
just check        # full CI gate: format + lint + test
just fmt          # auto-format all code
just tui          # run the TUI app
just gui-dev      # run the GUI dev server
just bump-version X.Y.Z  # bump version in all config files
just check-types  # verify Rust ↔ TypeScript EventPayload sync
just gen-types    # regenerate Tauri command TypeScript bindings
just worktree <name>    # create isolated git worktree
```

Or the underlying pnpm/cargo commands:

```bash
pnpm run format
pnpm run format:check
pnpm run lint
```

## Releases and packaging

GitHub Actions are configured to:

- run CI checks on pushes and pull requests
- build TUI binaries
- build GUI web assets
- build Tauri desktop bundles on macOS, Linux, and Windows

See the [latest release](https://github.com/Z-Only/kairox/releases/latest) for downloadable assets.

## Contributing

1. Create a feature branch
2. Keep commits in Conventional Commit format
3. Run local checks before pushing
4. Open a pull request using the provided template

## Automation

This repository also includes:

- Dependabot for npm, Cargo, and GitHub Actions dependency updates
- GitHub Release Notes configuration via `.github/release.yml`
- Automatic GitHub Release publishing on `v*` tags
- GitHub Discussions for questions and design discussion

## Discussions

Use [GitHub Discussions](https://github.com/Z-Only/kairox/discussions) for questions, design ideas, and broader product conversations. Use Issues for actionable bugs and feature work.

## License

Apache License 2.0. See [LICENSE](https://github.com/Z-Only/kairox/blob/main/LICENSE).
