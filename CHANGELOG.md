# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-04-30

### Added

- OpenAI-compatible streaming model client with SSE parsing, tool call support, and wiremock integration tests.
- Ollama NDJSON streaming model client with wiremock integration tests.
- ModelRouter for profile-based client routing (register profiles, resolve by alias, list sorted).
- ToolCall and ToolDefinition types for structured tool call handling.
- Extended ModelRequest with system_prompt, tools, and add_message builder methods.
- FakeModelClient with_tool_call() mode for testing agent loop tool call flows.
- ModelError variants: Http, StreamParse, Api for structured HTTP error handling.
- ToolRegistry with permission-aware dispatch (register, get, invoke_with_permission).
- ToolError::NotFound variant.
- LocalRuntime agent loop: model -> tool call detection -> permission check -> tool execution -> result feedback -> model continuation.
- Broadcast event channel for subscribe_session (real-time event streaming).
- LocalRuntime constructors: with_permission_mode(), with_context_limit(), tool_registry().
- RuntimeError variants: MaxIterationsExceeded, PermissionRequired.
- Agent loop integration tests: tool call processing, no-tool termination, event subscription.
- TUI model profile auto-detection (OpenAI, Ollama, fake) with permission mode and context limit.

### Changed

- Replaced stub OpenAI config with full streaming client implementation.
- Replaced stub Ollama config with full NDJSON streaming client implementation.
- LocalRuntime now drives a complete agent loop instead of single model call.
- subscribe_session returns real broadcast stream instead of empty stream.
- TUI uses PermissionMode::Suggest and ContextAssembler with 100K token limit.
- Removed unused TuiApp.input field.

### Dependencies

- Added reqwest 0.12 (json, stream, rustls-tls) for HTTP model client support.
- Added eventsource-stream 0.2 for SSE parsing.
- Added wiremock 0.6 for mock HTTP server integration tests.
- Added async-stream 0.3 for broadcast event stream generation.
- Added tracing 0.1 for structured logging.

## [0.1.2] - 2026-04-30

### Fixed

- Release workflow now uploads TUI and Tauri build artifacts as GitHub Release assets.
- Added `permissions: contents: write` so the GITHUB_TOKEN can upload release assets.
- Added `tagName` and `assetNamePattern` inputs to `tauri-action@v0` so Tauri desktop bundles are published.
- Added Package and Upload steps for TUI binaries (tar.gz on Linux/macOS, zip on Windows).
- Merged the separate `release-publish.yml` into `release-build.yml` for a single workflow.

### Added

- Tauri bundle configuration with proper app icons for all platforms.

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
