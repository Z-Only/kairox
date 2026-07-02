# Session Diagnostics Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a redacted GUI diagnostics bundle command that can be safely attached to bug reports without changing the existing raw `export_session_diagnostics` command used by eval and pilot assertions.

**Scope:** Tauri backend DTOs/command registration, focused Rust tests, and generated TypeScript command bindings. No dashboard UI, no replay engine, no trace file writer.

---

## Task 1: Define Redacted Bundle DTOs

**Files:**

- Modify `apps/agent-gui/src-tauri/src/commands.rs`

- [x] Add `SessionDiagnosticsBundleResponse` with `schema_version`, `generated_at`, `redaction`, and `summary`.
- [x] Add `SessionDiagnosticsRedactionResponse` with `applied`, `strategy`, `redacted_fields`, and `max_message_preview_chars`.

## Task 2: Build Redacted Bundle

**Files:**

- Modify `apps/agent-gui/src-tauri/src/commands/session.rs`

- [x] Add a RED test proving the bundle keeps counts but redacts message content, stream status messages, and local DB path.
- [x] Implement `build_redacted_diagnostics_bundle(trace, data_dir)`.
- [x] Add `redact_diagnostics_summary` and fixed marker helper so no original content is retained in the bundle.
- [x] Add Tauri command `export_session_diagnostics_bundle`.

## Task 3: Register Command And Types

**Files:**

- Modify `apps/agent-gui/src-tauri/src/lib.rs`
- Modify `apps/agent-gui/src-tauri/src/specta.rs`
- Modify `apps/agent-gui/src-tauri/src/bin/export_specta.rs`
- Generated `apps/agent-gui/src/generated/commands.ts`

- [x] Register the command in the runtime invoke handler.
- [x] Register the command and DTOs in Specta/type export.
- [x] Regenerate command TypeScript bindings.

## Verification

- [x] `cargo test -p agent-gui-tauri redacted_session_diagnostics_bundle_redacts_sensitive_fields`
- [x] `cargo test -p agent-gui-tauri session_diagnostics`
- [x] `cargo clippy -p agent-gui-tauri --all-targets -- -D warnings`
- [x] `cargo fmt --all --check`
- [x] `bun run format:check`
- [x] `bun run lint:web`
- [ ] Create commit `feat(gui): add redacted diagnostics bundle`
- [ ] Push `feat/session-diagnostics-bundle` and open a ready PR.

## Notes

- Existing `export_session_diagnostics` intentionally remains unredacted for deterministic local assertions.
- The new bundle is a compact support artifact, not a full trace export.
