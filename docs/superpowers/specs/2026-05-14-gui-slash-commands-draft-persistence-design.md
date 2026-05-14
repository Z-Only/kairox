# GUI Slash Commands, File Mentions, and Draft Persistence

**Date**: 2026-05-14
**Status**: Approved

## Summary

Add `/` command palette and `@` file mention support to the chat input box, restore the most recently active session on app startup, and persist unsent draft text per session to SQLite.

---

## Feature 1: `/` Command Palette + `@` File Mention

### Behavior

- **`/` trigger**: When the textarea is empty or the character before cursor is whitespace/start-of-input, typing `/` opens a command palette popover above the input area.
- **`@` trigger**: Same whitespace/start-of-input condition, typing `@` opens a file search mention popover.
- **Closing**: Escape, clicking outside, or selecting an item closes the popover.
- **Filtering**: Typing after `/` or `@` filters the list. `@` searches file paths with fuzzy matching.

### Interaction Model (Hybrid)

| Command Type                                    | Behavior                                                                |
| ----------------------------------------------- | ----------------------------------------------------------------------- |
| No-arg commands (`/clear`, `/compact`, `/help`) | Execute immediately on selection                                        |
| Commands with args (`/model`)                   | Insert text token, user completes args, Enter sends                     |
| Skill commands (`/skill-name`)                  | Insert skill name as text token, send triggers skill                    |
| File mentions (`@path/to/file`)                 | Insert `@path/to/file` text token, send triggers file context injection |

### Command Registry

```ts
// composables/useCommandRegistry.ts
interface CommandDef {
  id: string;
  label: string;
  description: string;
  /** If set, command executes immediately without inserting text */
  handler?: () => Promise<void>;
  /** If set, text inserted into input when selected */
  insertText?: string;
  /** Context in which command is available */
  context?: "always" | "session-active" | "streaming";
}

// Built-in commands:
// /clear    — handler: reset projection, no text insert
// /compact  — handler: invoke compact_session
// /model    — insertText: "/model ", user types alias
// /help     — insertText: "/help"
// /skills   — dynamic: list available skills as sub-options
```

Skills are fetched from the existing `useSkillStore` and listed as sub-items under the `/` palette when the user types `/` or filters by name.

### Component Architecture

```
ChatPanel.vue (modified)
  ├── CommandPalette.vue        (new)
  │     — renders filtered command list + skill sub-items
  │     — keyboard nav: up/down/enter/escape
  │     — positioned above input area
  ├── FileMentionPalette.vue    (new)
  │     — fuzzy file search via preloaded file list
  │     — keyboard nav, shows path + icon
  └── composables/
        ├── useCommandRegistry.ts   (new) — command definitions + matching
        ├── useMentionSearch.ts     (new) — file fuzzy search logic
        └── useDraftStore.ts        (new) — draft persistence composable
```

### Trigger Detection

In `ChatPanel.vue` `@input` handler:

```
onInput():
  cursorPos = textarea.selectionStart
  textBeforeCursor = inputText.slice(0, cursorPos)

  if textBeforeCursor matches /^\s*\/[^\/\s]*$/:
    open CommandPalette with filter = text after "/"
  elif textBeforeCursor matches /^\s*@[^@\s]*$/:
    open FileMentionPalette with filter = text after "@"
  else:
    close any open palette
```

Key constraint: `/` or `@` is only a trigger at the START of input (or after whitespace). `/` or `@` in the middle of a sentence is treated as literal text.

### File Search

MVP: Preload workspace file list on session switch (cap at N files), filter client-side with fuzzy matching. No new Tauri command needed for this feature.

---

## Feature 2: Fix Session Recovery

### Root Cause

`SqliteEventStore::list_active_sessions` uses `ORDER BY created_at ASC` (oldest first). The Tauri `list_sessions` command sorts "current session first" but during recovery `current_session_id` is `None` (no session has been switched yet), so the original ASC order is preserved. `recoverSessions()` picks `sessions[0]`, loading the **oldest** session instead of the most recently active one.

### Fix

1. **Backend**: Change `list_active_sessions` SQL to `ORDER BY updated_at DESC`. The `updated_at` column already exists and is updated on session rename, model switch, and message send.

2. **Frontend**: Guard UI rendering with existing `initialized` flag to prevent empty-state flicker before recovery completes.

3. **Last active session tracking**: On `switchSession`, persist `last_active_session_id` via a Tauri command that writes to `~/.kairox/prefs.json`. On recovery, read this value and switch to the specified session (falling back to `sessions[0]` if the preference is missing or the session was deleted).

---

## Feature 3: Draft Input Persistence

### Storage

New SQLite table (migration `0005_session_drafts.sql`):

```sql
CREATE TABLE IF NOT EXISTS session_drafts (
    session_id TEXT PRIMARY KEY,
    draft_text TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES kairox_sessions(session_id)
);
```

### Tauri Commands

| Command                        | Description                       |
| ------------------------------ | --------------------------------- |
| `save_draft(session_id, text)` | UPSERT draft text for session     |
| `get_draft(session_id)`        | Return draft text or empty string |

### Frontend Integration

`useDraftStore` composable:

```ts
function useDraftStore() {
  // Load draft when switching sessions
  async function loadDraft(sessionId: string): Promise<string>;
  // Save draft on input change (debounced 500ms)
  async function saveDraft(sessionId: string, text: string): Promise<void>;
  // Clear draft after message sent
  async function clearDraft(sessionId: string): Promise<void>;
}
```

### Data Flow

1. **Session switch**: `switchToKnownSession()` calls `loadDraft(sessionId)` and sets `inputText`
2. **Typing**: `watchDebounced(inputText, 500ms)` calls `saveDraft(sessionId, text)`
3. **Send**: `sendMessage()` calls `clearDraft(sessionId)` after successful send

### ChatPanel.vue Changes

- `inputText` initialized from draft (loaded after session switch)
- `watchDebounced` on `inputText` auto-saves draft
- `watch` on `session.currentSessionId`: save current draft, load draft for new session

---

## Testing Plan

### Unit Tests (Vitest)

- `CommandPalette.test.ts`: render, filter, keyboard nav, selection
- `FileMentionPalette.test.ts`: render, fuzzy search, file path injection
- `useCommandRegistry.test.ts`: registration, matching, context filtering
- `useDraftStore.test.ts`: load/save/clear with mocked Tauri invoke

### Integration Tests (Rust)

- `draft_persistence.rs`: test `save_draft` / `get_draft` CRUD
- `list_sessions_ordering.rs`: verify `updated_at DESC` ordering

### E2E Tests (Playwright)

- Slash opens palette, select command executes or inserts text
- At mentions opens file search, select file injects path
- Draft text survives session switch and page reload
- Recovery loads last active session with correct model

---

## Files Changed

| File                                                    | Change                                                            |
| ------------------------------------------------------- | ----------------------------------------------------------------- |
| `apps/agent-gui/src/components/ChatPanel.vue`           | Add palette integration, draft loading, trigger detection         |
| `apps/agent-gui/src/components/CommandPalette.vue`      | New — command popover                                             |
| `apps/agent-gui/src/components/FileMentionPalette.vue`  | New — file mention popover                                        |
| `apps/agent-gui/src/composables/useCommandRegistry.ts`  | New — command definitions                                         |
| `apps/agent-gui/src/composables/useMentionSearch.ts`    | New — file search                                                 |
| `apps/agent-gui/src/composables/useDraftStore.ts`       | New — draft persistence                                           |
| `apps/agent-gui/src/stores/session.ts`                  | Add draft loading on switch, last_active_session tracking         |
| `apps/agent-gui/src/App.vue`                            | Guard rendering behind `initialized` flag                         |
| `apps/agent-gui/src/generated/commands.ts`              | Regenerated — new commands                                        |
| `crates/agent-store/migrations/0005_session_drafts.sql` | New — draft table                                                 |
| `crates/agent-store/src/event_store.rs`                 | Add `save_draft`, `get_draft`; sort sessions by `updated_at DESC` |
| `apps/agent-gui/src-tauri/src/commands.rs`              | Add `save_draft`, `get_draft` commands                            |
| `apps/agent-gui/src-tauri/src/lib.rs`                   | Register new commands                                             |
| `apps/agent-gui/src-tauri/src/specta.rs`                | Export new command types                                          |
| `apps/agent-gui/e2e/tauri-mock.js`                      | Mock new commands                                                 |
