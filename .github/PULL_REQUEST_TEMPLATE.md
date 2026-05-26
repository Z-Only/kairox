## Summary

- What problem does this PR solve?
- What is the main implementation approach?

## Changes

- [ ] Rust core / runtime changes
- [ ] TUI changes
- [ ] GUI / Vue / Tauri changes
- [ ] Tooling / CI / docs changes

## Validation

Run `just check` and verify all jobs pass. Check applicable items:

- [ ] `just fmt-check` — format check
- [ ] `just lint` — clippy + eslint + stylelint
- [ ] `just test` — cargo test (workspace, all targets)
- [ ] `just test-gui` — Vue / Vitest
- [ ] `just test-tui` — deterministic TUI unit/integration layers (if TUI changed)
- [ ] `just test-tui-pty` — real PTY smoke for terminal integration changes
- [ ] `just test-fullstack` — full-stack runtime integration tests (if runtime changed)
- [ ] `just test-mcp` — MCP integration tests (if MCP changed)
- [ ] `just test-e2e` — Playwright E2E (if GUI/IPC changed)
- [ ] `bun run coverage:rust` — Rust risk-tier coverage gates (`scripts/check-rust-coverage.mjs`)
- [ ] `bun run coverage:web` — Vitest V8 coverage thresholds (`apps/agent-gui/vitest.config.ts`)
- [ ] `just gen-types` — regenerate `apps/agent-gui/src/generated/{commands,events}.ts` if any `#[tauri::command]` or `EventPayload` / domain type changed
- [ ] `just check-types` — assert generated TypeScript bindings are in sync
- [ ] Updated `apps/agent-gui/e2e/tauri-mock.js` if new IPC commands or events were added
- [ ] `just tauri-build` — verify Tauri desktop bundle builds

## Screenshots / recordings

If GUI behavior changed, attach screenshots or a short recording.

## Risks

- Any known follow-up work?
- Any packaging or platform-specific caveats?
