# Kairox

[![CI](https://github.com/Z-Only/kairox/actions/workflows/ci.yml/badge.svg)](https://github.com/Z-Only/kairox/actions/workflows/ci.yml)
[![Release Build](https://github.com/Z-Only/kairox/actions/workflows/release-build.yml/badge.svg)](https://github.com/Z-Only/kairox/actions/workflows/release-build.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://github.com/Z-Only/kairox/blob/main/LICENSE)
[![Release](https://img.shields.io/github/v/release/Z-Only/kairox)](https://github.com/Z-Only/kairox/releases)

Kairox is a local-first AI agent workbench built with a shared Rust core, a terminal UI, and a Tauri + Vue desktop GUI.

## Quick links

- [Latest release](https://github.com/Z-Only/kairox/releases/tag/v0.1.0)
- [Release notes](https://github.com/Z-Only/kairox/releases)
- [Roadmap](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md)
- [Contributing](https://github.com/Z-Only/kairox/blob/main/CONTRIBUTING.md)
- [Security policy](https://github.com/Z-Only/kairox/blob/main/SECURITY.md)
- [Discussions](https://github.com/Z-Only/kairox/discussions)

## Project navigation

- [Contributing guide](https://github.com/Z-Only/kairox/blob/main/CONTRIBUTING.md)
- [Security policy](https://github.com/Z-Only/kairox/blob/main/SECURITY.md)
- [Code of conduct](https://github.com/Z-Only/kairox/blob/main/CODE_OF_CONDUCT.md)
- [Release guide](https://github.com/Z-Only/kairox/blob/main/docs/releasing.md)
- [Roadmap](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md)
- [Discussions guide](https://github.com/Z-Only/kairox/blob/main/docs/github/discussions.md)

## Visuals

The repository is ready for screenshots and demo assets. Recommended future additions:

- `docs/assets/logo.svg`
- `docs/assets/banner.png`
- `docs/assets/gui-main.png`
- `docs/assets/demo.gif`

Asset guidance lives in [docs/assets/README.md](https://github.com/Z-Only/kairox/blob/main/docs/assets/README.md).

## Demo

> Demo assets are not added yet. Recommended next step: add GUI screenshots or a short animated capture of the Tauri app here.

Suggested assets:

- main GUI window screenshot
- trace / permission center screenshot
- short gif or mp4 showing a local-first workflow

## Why Kairox?

Kairox aims to provide a local-first foundation for AI agent workflows with explicit boundaries between shared core logic, runtime orchestration, model integration, and user interfaces.

## Architecture

```mermaid
graph TD
    UI["User Interfaces"]
    TUI["TUI (Rust)"]
    GUI["GUI (Tauri + Vue)"]
    CORE["agent-core"]
    RUNTIME["agent-runtime"]
    MODELS["agent-models"]
    TOOLS["agent-tools"]
    MEMORY["agent-memory"]
    STORE["agent-store"]

    UI --> TUI
    UI --> GUI
    TUI --> CORE
    GUI --> CORE
    CORE --> RUNTIME
    RUNTIME --> MODELS
    RUNTIME --> TOOLS
    RUNTIME --> MEMORY
    RUNTIME --> STORE
```

## Highlights

- Local-first architecture with a shared Rust core
- Two user surfaces: TUI and Tauri + Vue desktop GUI
- Structured runtime, memory, tools, and persistence layers
- Complete open-source repository baseline with CI, release automation, and community docs

## Features

- **Shared Rust core** for agent IDs, events, projections, manifests, memory, tools, and runtime orchestration
- **TUI application** for lightweight terminal-based interaction
- **GUI desktop shell** built with Tauri 2 + Vue 3
- **Local-first architecture** designed for offline-friendly workflows and explicit permission control
- **Unified quality gates** with Rust + frontend linting, formatting, commit hooks, and CI

## Repository layout

- `/Users/chanyu/AIProjects/kairox/crates/agent-core` — shared domain types and application facade
- `/Users/chanyu/AIProjects/kairox/crates/agent-runtime` — runtime orchestration and task graph
- `/Users/chanyu/AIProjects/kairox/crates/agent-models` — model profile and provider boundaries
- `/Users/chanyu/AIProjects/kairox/crates/agent-tools` — permission and tool abstractions
- `/Users/chanyu/AIProjects/kairox/crates/agent-memory` — memory and context assembly
- `/Users/chanyu/AIProjects/kairox/crates/agent-store` — SQLite-backed event store
- `/Users/chanyu/AIProjects/kairox/crates/agent-tui` — terminal UI app
- `/Users/chanyu/AIProjects/kairox/apps/agent-gui` — Vue frontend + Tauri desktop app

## Status

Kairox is in an early-stage but fully structured open-source state, with CI, release workflows, repository policies, and initial TUI/GUI packaging in place.

## Requirements

- Rust stable toolchain
- Node.js 20+
- npm 10+

For Tauri desktop packaging:

- macOS: Xcode Command Line Tools
- Linux: WebKitGTK and Tauri native dependencies
- Windows: WebView2 toolchain

## Getting started

If you want to try Kairox quickly, start with the local setup and quality gates below, then run either the TUI or the GUI shell.

## Quick start

### Install dependencies

```bash
cd /Users/chanyu/AIProjects/kairox
npm install
cd /Users/chanyu/AIProjects/kairox/apps/agent-gui
npm install
```

### Run quality gates

```bash
cd /Users/chanyu/AIProjects/kairox
npm run format:check
npm run lint
cargo test --workspace --all-targets
```

### Run TUI

```bash
cd /Users/chanyu/AIProjects/kairox
cargo run -p agent-tui
```

### Run GUI in development

```bash
cd /Users/chanyu/AIProjects/kairox/apps/agent-gui
npm run dev
```

### Build GUI web assets

```bash
cd /Users/chanyu/AIProjects/kairox/apps/agent-gui
npm run build
```

### Build Tauri desktop app

```bash
cd /Users/chanyu/AIProjects/kairox/apps/agent-gui
npm run tauri build
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

Useful commands:

```bash
cd /Users/chanyu/AIProjects/kairox
npm run format
npm run format:check
npm run lint
```

## Releases and packaging

GitHub Actions are configured to:

- run CI checks on pushes and pull requests
- build TUI binaries
- build GUI web assets
- build Tauri desktop bundles on macOS, Linux, and Windows

Current published release:

- [v0.1.0](https://github.com/Z-Only/kairox/releases/tag/v0.1.0)

## Contributing

1. Create a feature branch
2. Keep commits in Conventional Commit format
3. Run local checks before pushing
4. Open a pull request using the provided template

## Automation

This repository also includes:

- Dependabot for npm and Cargo dependency updates
- GitHub Release Notes configuration via `.github/release.yml`
- Automatic GitHub Release publishing on `v*` tags
- GitHub Discussions for questions and design discussion

## Discussions

Use [GitHub Discussions](https://github.com/Z-Only/kairox/discussions) for questions, design ideas, and broader product conversations. Use Issues for actionable bugs and feature work.

## License

Apache License 2.0. See `/Users/chanyu/AIProjects/kairox/LICENSE`.
