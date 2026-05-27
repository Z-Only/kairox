# Getting Started

Kairox is a local-first AI agent workbench. The repository includes the Rust crates that power the runtime, a terminal UI, and a Tauri + Vue desktop GUI.

## Requirements

- Rust stable toolchain
- Node.js 22 or newer
- Bun 1.3 or newer
- Platform dependencies required by Tauri when building desktop bundles

## Install

```bash
bun install
```

## Run the terminal UI

```bash
just tui
```

The TUI is built with ratatui and is useful for fast terminal-based agent sessions.

## Run the desktop GUI

```bash
just tauri-dev
```

This starts the Vite frontend and the native Tauri window together. For web-only frontend development, use:

```bash
just gui-dev
```

## Verify changes

```bash
just check
```

This runs the repository format, lint, and Rust test gates. GUI-focused work may also require Vitest, Playwright, or tauri-pilot checks depending on the changed surface.
