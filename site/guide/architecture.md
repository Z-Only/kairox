# Architecture

Kairox keeps UI code thin and routes application behavior through trait-based Rust boundaries. The `agent-core` facade defines the shared domain language, while runtime, models, tools, memory, store, MCP, skills, and plugins remain independently testable crates.

![Kairox architecture banner](/banner.svg)

## Core layers

- `agent-core` defines the facade, domain events, identifiers, projections, and build information shared by every surface.
- `agent-runtime` orchestrates sessions, model calls, task graphs, permissions, MCP lifecycle, and multi-agent strategies.
- `agent-models` abstracts OpenAI-compatible, Anthropic, Ollama, and fake providers behind one model client interface.
- `agent-tools` owns the built-in tool registry and permission engine.
- `agent-memory` and `agent-store` provide durable memory and append-only event persistence.
- `agent-skills` and `agent-plugins` discover reusable prompt, tool, workflow, and plugin capabilities.

## UI surfaces

The terminal UI and the desktop GUI both depend on the same core contracts. The Tauri app uses Rust commands for facade operations and streams events into Vue stores, while the TUI renders session, chat, trace, and permission state in a terminal layout.

## Local-first boundary

Kairox is designed around explicit local control. Tools are permissioned, sessions are event-sourced, memories are scoped, and model/provider configuration is loaded from local project and user profiles.
