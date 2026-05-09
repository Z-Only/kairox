# tauri-pilot v0.5.1 Upgrade Design

## Scope

Upgrade Kairox's tauri-pilot integration to the upstream v0.5.1 release that fixes GitHub issue #85, then restore the skipped textarea-driven chat pilot scenario.

## Requirements

- Pin `tauri-plugin-pilot` to the upstream `v0.5.1` tag so builds are reproducible.
- Update `Cargo.lock` away from the old `624c9c32` v0.5.0 commit.
- Restore `apps/agent-gui/e2e-pilot/chat-flow.toml` so it fills the chat `<textarea>` and sends a message.
- Update developer-facing and CI CLI install hints to install `tauri-pilot-cli` from the same `v0.5.1` tag.
- Verify the pilot-enabled Tauri crate builds, and run the pilot scenario when the local CLI/runtime is available.

## Design

The Rust dependency remains git-based because upstream `tauri-plugin-pilot` is consumed from the GitHub repository. The dependency is fixed to `tag = "v0.5.1"`, matching the upstream release PR that includes the textarea fix. The TOML pilot scenario becomes the regression test: it uses `fill` on `textarea[data-test='message-input']`, clicks `[data-test='send-button']`, and asserts the submitted user message is visible in the chat message list.

## Verification

- `cargo update -p tauri-plugin-pilot`
- `cargo check -p agent-gui-tauri --features pilot`
- `just test-pilot` when `tauri-pilot` CLI and the local desktop runtime are available
