# GUI Bug Fixes — 2026-05-13

## Overview

Four targeted bug fixes for the Kairox GUI frontend (Vue 3 / TypeScript).

---

## Task 1: Default session name — "New Session" with auto-numbering

### Current behavior

- `session.ts:temporaryTitleFromFirstMessage()` returns `"New conversation"` for empty first messages.
- `project.ts:createDraftSessionPlaceholder()` hardcodes `"New conversation"`.
- No dedup logic exists — two empty sessions both get the same title.

### Target behavior

- Base name: `"New Session"`.
- If a session (or project session) with that title already exists, append ` 1`, ` 2`, etc. until unique.
- Regular sessions dedup globally against all non-project sessions.
- Project sessions dedup within their own project only.

### Implementation

- Add a `uniqueSessionTitle(base, existingTitles)` helper (shared or local).
- `session.ts`: use it when creating regular sessions.
- `project.ts`: use it in `createDraftSessionPlaceholder()`, scoped to `sessionsByProject.get(projectId)`.

---

## Task 2: Fix "unknown model profile" error message

### Current behavior

- Rust backend emits `"unknown model profile: '{alias}'"` from two locations (`router.rs:50`, `facade_runtime.rs:517`).
- Frontend `ChatPanel.vue:218` displays the raw Rust error via `String(e)`.
- i18n keys `errors.profileNotFound` and `context.switchModelFailed` exist but are never used.

### Target behavior

- Backend error says `"unknown model: '{alias}'"` (terminology matches the current model-list UI).
- Frontend catches the error and shows a friendlier localized message.

### Implementation

- `crates/agent-models/src/router.rs:50`: `s/unknown model profile/unknown model/`
- `crates/agent-runtime/src/facade_runtime.rs:517`: `s/unknown model profile/unknown model/`
- `ChatPanel.vue:216-218`: wrap error in `t('errors.modelNotFound', { model: alias })`.
- Add/update i18n: `errors.modelNotFound` in `en.json` and `zh-CN.json`.
- Update tests in `agent-models/tests/integration.rs:82-83` to match new message text.
- Also wire up `lastSendError` in the session store so the error banner (`ChatPanel.vue:357-361`) actually renders.

---

## Task 3: Fix image thumbnails in attachment chips

### Current behavior

- `ChatPanel.vue` uses `convertFileSrc(att.path)` from `@tauri-apps/api/core` as `<img>` src.
- In Tauri v2, this requires the asset protocol to be properly scoped; without it, thumbnails silently fail.
- On error, `onThumbnailError` hides the `<img>` and shows a generic file-type icon instead.

### Target behavior

- Image attachments show a working thumbnail preview.
- Non-image attachments keep the current file-icon + filename display.

### Implementation

- Replace `convertFileSrc()` with reading the file as bytes and creating an object URL.
- Use `invoke("read_attachment_thumbnail", { path })` or `@tauri-apps/plugin-fs` `readFile` → `URL.createObjectURL(new Blob([bytes]))`.
- Store thumbnail URLs in a reactive map keyed by attachment id.
- Keep the existing `onThumbnailError` fallback for unsupported image formats.

---

## Task 4: Fix archive confirmation icon mismatch

### Current behavior

| Context                 | Default icon | Armed (confirm) icon         |
| ----------------------- | ------------ | ---------------------------- |
| Regular session archive | Archive box  | Crossed-circle (weird shape) |
| Project session archive | Archive box  | Waveform-like shape          |
| Project delete          | Trash can    | Checkmark (correct)          |

The two archive confirmation icons differ from each other AND from the project-delete checkmark pattern. They also appear to overlap visually.

### Target behavior

- Both archive confirmation icons use the same checkmark SVG path as project delete.
- Icon replaces in-place (archive icon → checkmark icon) on first click.
- Consistent red styling via existing `.confirm-action` CSS class.

### Implementation

- `SessionsSidebar.vue`: replace the armed SVG paths at lines 711-713 (regular session) and 572-574 (project session) with the project-delete checkmark path:
  ```
  d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z"
  ```

---

## Testing

- **Task 1**: Unit test `uniqueSessionTitle` dedup logic. Vitest for session/project store creation.
- **Task 2**: Update Rust integration test in `agent-models/tests/integration.rs`. Verify i18n key presence.
- **Task 3**: Manually verify thumbnail display in dev mode (visual).
- **Task 4**: Visual verification in sidebar.

## Scope boundaries

- No backend API changes beyond error message text (no new IPC commands).
- No new dependencies.
- No changes to the Tauri asset protocol configuration.
- Task 3 uses existing Tauri file-read capability (no new permissions needed).
