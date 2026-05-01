# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-05-02

### 🚀 Features

- **models**: add Anthropic Messages API client with SSE streaming support
- **models**: add OpenAI-compatible API client with streaming support
- **config**: add agent-config crate for model profile discovery, TOML parsing, and API key resolution
- **config**: auto-discover Anthropic API key from `~/.claude/settings.json` as fallback when env var is not set
- **models**: handle `max_tokens` stop reason in Anthropic SSE parser (proxy compatibility)
- **models**: sanitize tool names for Anthropic API compatibility (replace dots with underscores, e.g. `fs.read` → `fs_read`)
- **models**: detect proxy error responses in `{"error":{}}` format from Anthropic API
- **runtime**: broadcast `AgentTaskFailed` events on model errors for frontend error display
- **gui**: propagate send_message errors to frontend via Tauri events
- **gui**: handle `AgentTaskFailed` events in session store to show errors and reset streaming state

### 🐛 Bug Fixes

- **models**: fix Anthropic SSE parsing to handle `max_tokens` stop reason from proxies
- **config**: resolve Anthropic API key when `KAIROX_ANTHROPIC_KEY` env var is not set
- **runtime**: prevent frontend from permanently showing "streaming" when model API calls fail

## [0.4.0] - 2026-04-30

### 🚀 Features

- **ci**: add git-cliff for automated changelog and release notes (#21)
- **tools**: implement ToolProvider abstraction and builtin tools (#22)
- **deps**: migrate npm→pnpm, upgrade deps, fix security alerts (#23)
- **tui**: interactive ratatui TUI with three-panel layout (#31)

### 📚 Documentation

- update all docs for pnpm migration and improve README structure (#29)

### 🎨 Styling

- format markdown files with prettier

### 👷 CI

- add workflow smoke test for PRs that change workflow files (#30)
- **smoke-test**: bump actions to v6 to match release-build versions

### 📦 Dependencies

- **deps**: bump pnpm/action-setup from 4 to 6 (#24)
- **deps**: bump actions/checkout from 4 to 6 (#25)
- **deps**: bump actions/github-script from 7 to 9 (#26)
- **deps**: bump actions/setup-node from 4 to 6 (#27)
- **deps**: bump softprops/action-gh-release from 2 to 3 (#28)

### 🔧 Miscellaneous Tasks

- add .worktrees/ to gitignore for worktree isolation

## [0.2.0] - 2026-04-30

### 🚀 Features

- **agent-tools**: add ToolRegistry for tool dispatch with permission checks
- **models**: add tool call types and rich request builder
- **agent-models**: implement OpenAI-compatible streaming client
- **models**: implement Ollama NDJSON streaming client
- **models**: add ModelRouter for profile-based client routing
- **models**: add tool call support to FakeModelClient
- **runtime**: integrate tool dispatch, permissions, and event broadcast into agent loop
- add real model adapters and runtime agent loop
- **tui**: wire model profile detection, permission mode, and context limit
- **tui**: wire model profile detection, permission mode, and context limit

### 🧪 Testing

- **runtime**: add agent loop integration tests

### 🔧 Miscellaneous Tasks

- **models**: add reqwest and streaming dependencies
- fix clippy warnings and workspace verification

## [0.1.2] - 2026-04-29

### 🐛 Bug Fixes

- **actions**: upload release assets for TUI and Tauri builds

## [0.1.1] - 2026-04-29

### 🐛 Bug Fixes

- **dependabot**: support app actor identity
- **dependabot**: merge green dependency PRs directly

### 📚 Documentation

- **readme**: add badges and release link
- **repo**: add community health files
- **repo**: add release automation and dependency policies
- **repo**: add conduct, roadmap, and release guide
- **repo**: add architecture and label guidance
- **repo**: expand homepage and discussions guidance
- **repo**: add issue forms and release helper
- **readme**: refine landing page and repo metadata
- **readme**: add visuals and asset guidance
- **readme**: add logo banner and screenshot placeholders

### 👷 CI

- **dependabot**: enable safe auto-merge after green checks

### 📦 Dependencies

- **deps-dev**: bump typescript from 5.9.3 to 6.0.3 in /apps/agent-gui (#14)
- **deps-dev**: bump @commitlint/config-conventional (#13)
- **deps**: bump toml from 0.8.2 to 1.1.2+spec-1.1.0 (#11)
- **deps**: bump ratatui from 0.29.0 to 0.30.0 (#8)
- **deps-dev**: bump vitest from 2.1.9 to 4.1.5 (#7)
- **deps-dev**: bump globals from 15.15.0 to 17.5.0 (#2)
- **deps-dev**: bump vitest from 2.1.9 to 4.1.5 in /apps/agent-gui (#12)

## [0.1.0] - 2026-04-29

### 🚀 Features

- **core**: add event schema and session projection
- **core**: define app facade boundary
- **store**: persist append-only events in sqlite
- **workbench**: complete ai agent workbench baseline

### 🐛 Bug Fixes

- **core**: strengthen projection serialization and tests
- **ci**: stabilize release workflow and package hooks
- **actions**: make ci and tauri builds cross-platform
- **ci**: install tauri linux system libraries
- **ci**: reinstall gui deps for rollup optional packages
- **gui**: pin rollup native optional packages
- **gui**: sync rollup optional deps in lockfile
- **lockfile**: sync workspace rollup optional deps
- **actions**: use cross-platform node_modules cleanup

### 📚 Documentation

- add AI agent workbench design spec
- add AI agent workbench implementation plan

### 🎨 Styling

- **format**: apply prettier to updated files

### 🔧 Miscellaneous Tasks

- scaffold rust workspace
- commit rust lockfile
- update lockfile for facade dependencies
- **tooling**: add unified lint, format, and commit hooks
- **repo**: prepare open source docs and github workflows
<!-- generated by git-cliff -->
