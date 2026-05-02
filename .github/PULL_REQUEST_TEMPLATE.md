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
- [ ] `just test` — cargo test
- [ ] `just test-gui` — vitest
- [ ] `just check-types` — Rust ↔ TypeScript EventPayload sync
- [ ] `just gen-types` — run if any `#[tauri::command]` signature changed
- [ ] `just tauri-build` — verify GUI builds

## Screenshots / recordings

If GUI behavior changed, attach screenshots or a short recording.

## Risks

- Any known follow-up work?
- Any packaging or platform-specific caveats?
