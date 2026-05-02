# Session Management UX — Design Spec

**Date:** 2026-05-03
**Status:** Approved
**Scope:** GUI session lifecycle management, persistence, and interaction polish

---

## Problem

Kairox GUI has a functional but incomplete session management experience:

1. **Session list lost on restart** — workspace→session mapping lives in `GuiState` memory (`HashMap<String, WorkspaceSession>`), so all sessions disappear when the app restarts
2. **No session lifecycle operations** — users cannot rename or delete sessions
3. **Profile selection is opaque** — the new-session dialog shows only profile aliases (e.g., "fast"), with no indication of provider, model, or API key availability
4. **No startup recovery** — after restart, the GUI initializes a fresh workspace instead of resuming previous work

While all session events persist in SQLite via `SqliteEventStore`, there is no metadata table indexing which workspaces and sessions exist.

## Design Decisions

| Decision              | Choice                                 | Rationale                                                                          |
| --------------------- | -------------------------------------- | ---------------------------------------------------------------------------------- |
| Persistence scope     | List restore + lazy history load       | Core UX need without loading all events eagerly                                    |
| Management operations | Delete + Rename + Profile details      | Covers the most common lifecycle operations; YAGNI for search/sort/context-menu    |
| Interaction pattern   | Hover actions + inline editing         | Desktop app standard (VS Code, Slack); most intuitive, no discoverability issues   |
| Profile selection     | Rich-info dropdown                     | One-glance decision: alias + provider + model_id + key status                      |
| Delete data policy    | Soft delete + expired cleanup          | Regret window + automatic disk cleanup                                             |
| Architecture          | Extend EventStore with metadata tables | Single data source, transactional consistency, shared across TUI/GUI via AppFacade |

## Data Model

Two new metadata tables in the existing SQLite database:

```sql
CREATE TABLE IF NOT EXISTS kairox_workspaces (
    workspace_id  TEXT PRIMARY KEY,
    path          TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kairox_sessions (
    session_id    TEXT PRIMARY KEY,
    workspace_id  TEXT NOT NULL REFERENCES kairox_workspaces(workspace_id),
    title         TEXT NOT NULL,
    model_profile TEXT NOT NULL,
    model_id      TEXT,           -- display: "gpt-4o-mini"
    provider      TEXT,           -- display: "openai"
    deleted_at    TEXT,           -- NULL = active, non-null = soft-deleted
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_workspace ON kairox_sessions(workspace_id);
CREATE INDEX IF NOT EXISTS idx_sessions_deleted ON kairox_sessions(deleted_at);
```

**Key points:**

- `deleted_at` NULL = active session; non-null = soft-deleted (hidden from list)
- `model_id` / `provider` stored at creation time for sidebar display
- Workspace:Session is 1:N
- Shares the SQLite connection pool with events and memories (via `store.pool()`)

## AppFacade Extensions

### New Trait Methods (`agent-core/src/facade.rs`)

```rust
async fn list_workspaces(&self) -> Result<Vec<WorkspaceInfo>>;
async fn list_sessions(&self, workspace_id: &WorkspaceId) -> Result<Vec<SessionMeta>>;
async fn rename_session(&self, session_id: &SessionId, title: String) -> Result<()>;
async fn soft_delete_session(&self, session_id: &SessionId) -> Result<()>;
async fn cleanup_expired_sessions(&self, older_than: Duration) -> Result<usize>;
```

### New Type: `SessionMeta`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: SessionId,
    pub workspace_id: WorkspaceId,
    pub title: String,
    pub model_profile: String,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

## EventStore Extensions

New methods on `EventStore` trait and `SqliteEventStore`:

```rust
async fn ensure_metadata_tables(&self) -> store::Result<()>;
async fn upsert_workspace(&self, workspace_id: &str, path: &str) -> store::Result<()>;
async fn upsert_session(&self, meta: &SessionMeta) -> store::Result<()>;
async fn list_workspaces(&self) -> store::Result<Vec<WorkspaceRow>>;
async fn list_active_sessions(&self, workspace_id: &str) -> store::Result<Vec<SessionRow>>;
async fn rename_session(&self, session_id: &str, title: &str) -> store::Result<()>;
async fn soft_delete_session(&self, session_id: &str) -> store::Result<()>;
async fn cleanup_expired_sessions(&self, older_than: Duration) -> store::Result<usize>;
```

Row types mirror `SessionMeta` fields but use `String` for IDs (no newtype wrappers at the storage layer).

## LocalRuntime Behavior Changes

| Method                       | Change                                                                                                                   |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `open_workspace()`           | Additionally calls `store.upsert_workspace()` to persist workspace metadata                                              |
| `start_session()`            | Additionally calls `store.upsert_session()` to persist session metadata; resolves `provider`/`model_id` from ModelRouter |
| `list_workspaces()`          | New: reads from metadata table                                                                                           |
| `list_sessions()`            | New: reads active sessions (WHERE deleted_at IS NULL) from metadata table                                                |
| `rename_session()`           | New: updates title + updated_at in metadata table                                                                        |
| `soft_delete_session()`      | New: sets deleted_at = now() in metadata table                                                                           |
| `cleanup_expired_sessions()` | New: deletes session rows + their event rows where deleted_at < now() - duration                                         |

The `start_session()` method resolves provider/model_id by querying the ModelRouter for the given profile. If the profile is not found (e.g., "fake"), `provider` and `model_id` are stored as `None`.

## Tauri Commands

### New Commands

```rust
#[tauri::command]
#[specta::specta]
async fn list_workspaces(state: State<'_, GuiState>) -> Result<Vec<WorkspaceInfoResponse>, String>

#[tauri::command]
#[specta::specta]
async fn rename_session(
    session_id: String,
    title: String,
    state: State<'_, GuiState>
) -> Result<(), String>

#[tauri::command]
#[specta::specta]
async fn delete_session(
    session_id: String,
    state: State<'_, GuiState>
) -> Result<(), String>

#[tauri::command]
#[specta::specta]
async fn get_profile_detail(
    profile: String,
    state: State<'_, GuiState>
) -> Result<ProfileDetailResponse, String>
```

### Modified Commands

- `list_sessions`: reads from store instead of in-memory `HashMap`
- `start_session`: writes session metadata to store after creation

### New Response Type

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileDetailResponse {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
}
```

This reuses the existing `ProfileInfo` from `agent-config` but as a Tauri command response for clarity.

## Vue Frontend Changes

### ConfirmDialog.vue (New)

A generic confirmation dialog component:

- Props: `title`, `message`, `confirmLabel`, `confirmDanger` (boolean — red button when true)
- Emits: `confirm`, `cancel`
- Rendered as a centered modal with backdrop
- Used by the delete-session flow

### SessionsSidebar.vue (Major Rework)

**Hover actions:**

- Each session item shows ✏️ (rename) and 🗑️ (delete) icons on hover (CSS `opacity: 0` → `1` on `:hover`)
- Icons are positioned at the right edge of the session row
- Clicking ✏️ enters inline-edit mode
- Clicking 🗑️ opens ConfirmDialog

**Inline rename:**

- Clicking ✏️ replaces the session title `<span>` with an `<input>`
- Enter confirms (calls `invoke("rename_session")`), Escape cancels
- Clicking outside the input also cancels (blur handler)
- On success, the local session list title is updated reactively

**Rich-info profile dropdown:**

- Replaces the simple `<select>` with a custom `<div>`-based dropdown
- Each option displays: `alias · provider · model_id · 🔑/🚫`
- Keyboard navigation (up/down arrows, Enter to select)
- Uses `invoke("get_profile_detail")` to fetch per-profile details

**Delete flow:**

1. Click 🗑️ → opens ConfirmDialog with "Delete [session title]?" message
2. Confirm → calls `invoke("delete_session", { sessionId })`
3. On success: remove session from `sessionState.sessions`
4. If the deleted session was the current one: auto-switch to the first remaining session
5. If no sessions remain: show "No sessions yet" empty state

### App.vue (Startup Recovery)

The `onMounted` hook is restructured:

```
1. invoke("list_workspaces") → get workspace list
2. If workspaces exist:
   a. Set workspace_id from first workspace
   b. invoke("list_sessions") → restore session list
   c. If sessions exist → auto-select first, invoke("switch_session") to load history
   d. Spawn event forwarder for selected session
3. If no workspaces:
   a. invoke("initialize_workspace") → first-run flow (unchanged)
```

### session.ts Store Changes

- Add `SessionMeta` interface to the store's type imports
- Add `deleteSession(sessionId: string)` action — calls Tauri command + removes from local list
- Add `renameSession(sessionId: string, title: string)` action — calls Tauri command + updates local list
- Update `SessionInfoResponse` to include `model_id` and `provider` fields
- Startup recovery replaces the current `initialize_workspace`-only flow

### StatusBar.vue (Minor)

- Show current permission mode from store instead of hardcoded "interactive"

## Startup Recovery Flow

```
App mounted
  │
  ├── list_workspaces() ─── empty? ──→ initialize_workspace() (first-run)
  │                                    │
  │                                    ├── open_workspace()
  │                                    ├── start_session(default_profile)
  │                                    └── done → show chat
  │
  └── has workspaces
       │
       ├── set workspace_id
       ├── list_sessions(workspace_id)
       │
       ├── has sessions? ──→ switch to first session
       │                     ├── get session projection (from events)
       │                     ├── get_trace (for trace panel)
       │                     └── spawn event forwarder
       │
       └── no sessions? ──→ show "No sessions yet" state
                            (user can create via + New)
```

## Expired Session Cleanup

A background task registered in `lib.rs` during Tauri `setup`:

```rust
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(3600)); // hourly
    loop {
        interval.tick().await;
        if let Err(e) = runtime.cleanup_expired_sessions(Duration::from_secs(7 * 86400)).await {
            eprintln!("[cleanup] Failed: {e}");
        }
    }
});
```

- Deletes sessions where `deleted_at < now() - 7 days`
- Also deletes the corresponding event rows from the `events` table
- Runs hourly; configurable in future via config file

## TypeScript Types

`apps/agent-gui/src/types/index.ts` additions:

```typescript
export interface SessionMeta {
  session_id: string;
  workspace_id: string;
  title: string;
  model_profile: string;
  model_id: string | null;
  provider: string | null;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ProfileDetail {
  alias: string;
  provider: string;
  model_id: string;
  local: boolean;
  has_api_key: boolean;
}
```

## File Changes Summary

| Layer    | File                                                | Change                                                                     |
| -------- | --------------------------------------------------- | -------------------------------------------------------------------------- |
| core     | `crates/agent-core/src/facade.rs`                   | +5 trait methods, +SessionMeta type, +NoopFacade implementations           |
| store    | `crates/agent-store/src/event_store.rs`             | +8 metadata methods, +ensure_metadata_tables, +SessionRow, +WorkspaceRow   |
| store    | `crates/agent-store/src/lib.rs`                     | Re-export new types                                                        |
| runtime  | `crates/agent-runtime/src/facade_runtime.rs`        | Implement 5 new trait methods; modify open_workspace/start_session         |
| gui-rust | `apps/agent-gui/src-tauri/src/commands.rs`          | +4 commands, modify 2 commands, +ProfileDetailResponse                     |
| gui-rust | `apps/agent-gui/src-tauri/src/lib.rs`               | Register new commands, add cleanup background task, restructure startup    |
| gui-rust | `apps/agent-gui/src-tauri/src/app_state.rs`         | Remove in-memory sessions HashMap (migration to store)                     |
| gui-vue  | `apps/agent-gui/src/components/SessionsSidebar.vue` | Hover actions, inline rename, rich-info dropdown, startup recovery         |
| gui-vue  | `apps/agent-gui/src/components/ConfirmDialog.vue`   | New generic confirmation dialog                                            |
| gui-vue  | `apps/agent-gui/src/stores/session.ts`              | +delete/rename actions, startup recovery logic, update SessionInfoResponse |
| gui-vue  | `apps/agent-gui/src/types/index.ts`                 | +SessionMeta, ProfileDetail                                                |
| gui-ts   | `apps/agent-gui/src/generated/commands.ts`          | Regenerate via `just gen-types`                                            |

## Testing Strategy

| Layer            | Test                                | What it verifies                                                         |
| ---------------- | ----------------------------------- | ------------------------------------------------------------------------ |
| agent-store      | Unit: metadata table CRUD           | upsert_workspace, upsert_session, list, rename, soft_delete              |
| agent-store      | Unit: soft delete filtering         | list_active_sessions excludes deleted_at IS NOT NULL                     |
| agent-store      | Unit: expired cleanup               | cleanup deletes sessions + events past threshold                         |
| agent-runtime    | Integration: persistence round-trip | open_workspace + start_session → restart simulation → list recovers both |
| agent-runtime    | Integration: rename flow            | rename_session updates title, list_sessions reflects change              |
| agent-runtime    | Integration: delete flow            | soft_delete_session hides from list, cleanup removes data                |
| agent-gui (Rust) | Integration: command invocation     | Tauri commands call AppFacade correctly                                  |
| agent-gui (Vue)  | Component: SessionsSidebar          | Hover → icons visible; rename → input appears; delete → dialog opens     |

## Non-Goals

- Session search/filter (deferred to next iteration)
- Drag-and-drop session reordering
- Right-click context menus
- Workspace switching (single workspace for now; multi-workspace is a mid-term roadmap item)
- Session export/import
- Auto-generate EventPayload TypeScript types (separate roadmap item)
