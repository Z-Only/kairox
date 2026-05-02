# Session Management UX — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add session lifecycle management (rename, soft-delete, startup recovery) and profile-aware session creation to the Kairox GUI, with persistent metadata in SQLite.

**Architecture:** Extend `SqliteEventStore` with two metadata tables (`kairox_workspaces`, `kairox_sessions`). Add 5 new methods to `AppFacade` trait and implement in `LocalRuntime`. Add 4 new Tauri commands. Rework `SessionsSidebar.vue` with hover actions, inline rename, rich-info profile dropdown, and startup recovery. Add `ConfirmDialog.vue` for delete confirmation.

**Tech Stack:** Rust, sqlx, async-trait, Tauri 2, Vue 3 Composition API, TypeScript, Pinia

---

## File Structure

### New Files

| File                                              | Responsibility                                                 |
| ------------------------------------------------- | -------------------------------------------------------------- |
| `crates/agent-store/migrations/0002_metadata.sql` | SQL migration for kairox_workspaces and kairox_sessions tables |
| `apps/agent-gui/src/components/ConfirmDialog.vue` | Generic confirmation dialog component                          |

### Modified Files

| File                                                | Changes                                                                                                                      |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `crates/agent-core/src/facade.rs`                   | +5 trait methods, +SessionMeta type, +NoopFacade implementations                                                             |
| `crates/agent-store/src/event_store.rs`             | +8 metadata methods, +SessionRow/WorkspaceRow types                                                                          |
| `crates/agent-store/src/lib.rs`                     | Re-export new types                                                                                                          |
| `crates/agent-runtime/src/facade_runtime.rs`        | Implement 5 new trait methods; modify open_workspace/start_session to persist metadata                                       |
| `apps/agent-gui/src-tauri/src/commands.rs`          | +4 commands (list_workspaces, rename_session, delete_session, get_profile_detail), modify 2 commands, +ProfileDetailResponse |
| `apps/agent-gui/src-tauri/src/specta.rs`            | Register new commands and types                                                                                              |
| `apps/agent-gui/src-tauri/src/lib.rs`               | Register new commands, add cleanup background task, restructure startup for recovery                                         |
| `apps/agent-gui/src-tauri/src/app_state.rs`         | Remove in-memory sessions HashMap                                                                                            |
| `apps/agent-gui/src/components/SessionsSidebar.vue` | Hover actions, inline rename, rich-info profile dropdown, startup recovery                                                   |
| `apps/agent-gui/src/stores/session.ts`              | +delete/rename actions, startup recovery, update types                                                                       |
| `apps/agent-gui/src/types/index.ts`                 | +SessionMeta, ProfileDetail                                                                                                  |
| `apps/agent-gui/src/generated/commands.ts`          | Regenerated via `just gen-types`                                                                                             |

---

## Task 1: Migration SQL + Metadata Table Creation

**Files:**

- Create: `crates/agent-store/migrations/0002_metadata.sql`
- Modify: `crates/agent-store/src/event_store.rs`

- [ ] **Step 1: Write the migration SQL**

Create `crates/agent-store/migrations/0002_metadata.sql`:

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
    model_id      TEXT,
    provider      TEXT,
    deleted_at    TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_workspace ON kairox_sessions(workspace_id);
CREATE INDEX IF NOT EXISTS idx_sessions_deleted ON kairox_sessions(deleted_at);
```

- [ ] **Step 2: Add `ensure_metadata_tables` to `SqliteEventStore`**

In `crates/agent-store/src/event_store.rs`, add a new method that runs the migration. Modify the existing `migrate()` method to also run `0002_metadata.sql`:

```rust
async fn migrate(&self) -> crate::Result<()> {
    sqlx::query(include_str!("../migrations/0001_events.sql"))
        .execute(&self.pool)
        .await?;
    sqlx::query(include_str!("../migrations/0002_metadata.sql"))
        .execute(&self.pool)
        .await?;
    Ok(())
}
```

- [ ] **Step 3: Run workspace tests**

Run: `cargo test -p agent-store`
Expected: ALL PASS (new tables created in existing tests but not used yet)

- [ ] **Step 4: Commit**

```bash
git add crates/agent-store/migrations/0002_metadata.sql crates/agent-store/src/event_store.rs
git commit -m "feat(store): add metadata tables migration for workspace and session tracking"
```

---

## Task 2: Metadata Row Types + Repository Methods

**Files:**

- Modify: `crates/agent-store/src/event_store.rs`

- [ ] **Step 1: Write failing tests for metadata operations**

Add to `crates/agent-store/src/event_store.rs` in `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn upsert_and_list_workspaces() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/project-a").await.unwrap();
    store.upsert_workspace("wrk_2", "/tmp/project-b").await.unwrap();

    let workspaces = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 2);
    assert_eq!(workspaces[0].workspace_id, "wrk_1");
    assert_eq!(workspaces[0].path, "/tmp/project-a");
}

#[tokio::test]
async fn upsert_workspace_is_idempotent() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/old").await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/new").await.unwrap();

    let workspaces = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].path, "/tmp/new");
}

#[tokio::test]
async fn upsert_and_list_active_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/project").await.unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    store.upsert_session(&SessionRow {
        session_id: "ses_1".into(),
        workspace_id: "wrk_1".into(),
        title: "Session using fast".into(),
        model_profile: "fast".into(),
        model_id: Some("gpt-4.1-mini".into()),
        provider: Some("openai_compatible".into()),
        deleted_at: None,
        created_at: now.clone(),
        updated_at: now,
    }).await.unwrap();

    let sessions = store.list_active_sessions("wrk_1").await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Session using fast");
    assert_eq!(sessions[0].model_id, Some("gpt-4.1-mini".into()));
}

#[tokio::test]
async fn soft_delete_hides_session_from_active_list() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/project").await.unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    store.upsert_session(&SessionRow {
        session_id: "ses_1".into(),
        workspace_id: "wrk_1".into(),
        title: "To delete".into(),
        model_profile: "fake".into(),
        model_id: None,
        provider: None,
        deleted_at: None,
        created_at: now.clone(),
        updated_at: now,
    }).await.unwrap();

    store.soft_delete_session("ses_1").await.unwrap();

    let sessions = store.list_active_sessions("wrk_1").await.unwrap();
    assert!(sessions.is_empty());
}

#[tokio::test]
async fn rename_session_updates_title() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/project").await.unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    store.upsert_session(&SessionRow {
        session_id: "ses_1".into(),
        workspace_id: "wrk_1".into(),
        title: "Old title".into(),
        model_profile: "fake".into(),
        model_id: None,
        provider: None,
        deleted_at: None,
        created_at: now.clone(),
        updated_at: now,
    }).await.unwrap();

    store.rename_session("ses_1", "New title").await.unwrap();

    let sessions = store.list_active_sessions("wrk_1").await.unwrap();
    assert_eq!(sessions[0].title, "New title");
}

#[tokio::test]
async fn cleanup_expired_deletes_old_soft_deleted_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/project").await.unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let old_deleted = chrono::Utc::now() - chrono::Duration::days(10);
    store.upsert_session(&SessionRow {
        session_id: "ses_old".into(),
        workspace_id: "wrk_1".into(),
        title: "Old deleted".into(),
        model_profile: "fake".into(),
        model_id: None,
        provider: None,
        deleted_at: Some(old_deleted.to_rfc3339()),
        created_at: now.clone(),
        updated_at: now,
    }).await.unwrap();

    store.upsert_session(&SessionRow {
        session_id: "ses_recent".into(),
        workspace_id: "wrk_1".into(),
        title: "Recent deleted".into(),
        model_profile: "fake".into(),
        model_id: None,
        provider: None,
        deleted_at: Some(chrono::Utc::now().to_rfc3339()),
        created_at: now.clone(),
        updated_at: now,
    }).await.unwrap();

    let deleted = store.cleanup_expired_sessions(std::time::Duration::from_secs(7 * 86400)).await.unwrap();
    assert_eq!(deleted, 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-store`
Expected: FAIL — `SessionRow`, `upsert_workspace`, etc. not defined

- [ ] **Step 3: Define `SessionRow` and `WorkspaceRow` types**

Add to `crates/agent-store/src/event_store.rs` (above `SqliteEventStore` impl):

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkspaceRow {
    pub workspace_id: String,
    pub path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub session_id: String,
    pub workspace_id: String,
    pub title: String,
    pub model_profile: String,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

Note: `SessionRow` does NOT derive `sqlx::FromRow` because we need manual field mapping for `model_id`/`provider` which may be NULL.

- [ ] **Step 4: Implement metadata methods on `SqliteEventStore`**

Add these methods to `impl SqliteEventStore`:

```rust
pub async fn upsert_workspace(&self, workspace_id: &str, path: &str) -> crate::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO kairox_workspaces (workspace_id, path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(workspace_id) DO UPDATE SET path = ?2, updated_at = ?4",
    )
    .bind(workspace_id)
    .bind(path)
    .bind(&now)
    .bind(&now)
    .execute(&self.pool)
    .await?;
    Ok(())
}

pub async fn upsert_session(&self, meta: &SessionRow) -> crate::Result<()> {
    sqlx::query(
        "INSERT INTO kairox_sessions (session_id, workspace_id, title, model_profile, model_id, provider, deleted_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(session_id) DO UPDATE SET title = ?3, model_profile = ?4, model_id = ?5, provider = ?6, updated_at = ?9",
    )
    .bind(&meta.session_id)
    .bind(&meta.workspace_id)
    .bind(&meta.title)
    .bind(&meta.model_profile)
    .bind(&meta.model_id)
    .bind(&meta.provider)
    .bind(&meta.deleted_at)
    .bind(&meta.created_at)
    .bind(&meta.updated_at)
    .execute(&self.pool)
    .await?;
    Ok(())
}

pub async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceRow>> {
    let rows = sqlx::query_as::<_, WorkspaceRow>(
        "SELECT workspace_id, path, created_at, updated_at FROM kairox_workspaces ORDER BY created_at ASC",
    )
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}

pub async fn list_active_sessions(&self, workspace_id: &str) -> crate::Result<Vec<SessionRow>> {
    let rows = sqlx::query_as::<_, SessionRowForQuery>(
        "SELECT session_id, workspace_id, title, model_profile, model_id, provider, deleted_at, created_at, updated_at
         FROM kairox_sessions WHERE workspace_id = ?1 AND deleted_at IS NULL ORDER BY created_at ASC",
    )
    .bind(workspace_id)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows.into_iter().map(SessionRow::from).collect())
}

pub async fn rename_session(&self, session_id: &str, title: &str) -> crate::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE kairox_sessions SET title = ?1, updated_at = ?2 WHERE session_id = ?3",
    )
    .bind(title)
    .bind(&now)
    .bind(session_id)
    .execute(&self.pool)
    .await?;
    Ok(())
}

pub async fn soft_delete_session(&self, session_id: &str) -> crate::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE kairox_sessions SET deleted_at = ?1, updated_at = ?1 WHERE session_id = ?2",
    )
    .bind(&now)
    .bind(session_id)
    .execute(&self.pool)
    .await?;
    Ok(())
}

pub async fn cleanup_expired_sessions(&self, older_than: std::time::Duration) -> crate::Result<usize> {
    let threshold = chrono::Utc::now() - chrono::Duration::from_std(older_than).unwrap_or_else(|_| chrono::Duration::seconds(0));
    let threshold_str = threshold.to_rfc3339();

    // Get expired session IDs for event cleanup
    let expired: Vec<String> = sqlx::query_scalar(
        "SELECT session_id FROM kairox_sessions WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
    )
    .bind(&threshold_str)
    .fetch_all(&self.pool)
    .await?;

    let count = expired.len();
    if count == 0 {
        return Ok(0);
    }

    // Delete events for expired sessions
    for sid in &expired {
        sqlx::query("DELETE FROM events WHERE session_id = ?1")
            .bind(sid)
            .execute(&self.pool)
            .await?;
    }

    // Delete the session metadata rows
    sqlx::query("DELETE FROM kairox_sessions WHERE deleted_at IS NOT NULL AND deleted_at < ?1")
        .bind(&threshold_str)
        .execute(&self.pool)
        .await?;

    Ok(count)
}
```

For `SessionRow` to work with `sqlx::query_as`, we need a queryable intermediate struct since `SessionRow` does not derive `FromRow`:

```rust
#[derive(sqlx::FromRow)]
struct SessionRowForQuery {
    session_id: String,
    workspace_id: String,
    title: String,
    model_profile: String,
    model_id: Option<String>,
    provider: Option<String>,
    deleted_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<SessionRowForQuery> for SessionRow {
    fn from(r: SessionRowForQuery) -> Self {
        Self {
            session_id: r.session_id,
            workspace_id: r.workspace_id,
            title: r.title,
            model_profile: r.model_profile,
            model_id: r.model_id,
            provider: r.provider,
            deleted_at: r.deleted_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}
```

- [ ] **Step 5: Add `chrono` dependency to agent-store Cargo.toml**

The `cleanup_expired_sessions` method uses `chrono::Utc::now()` and `chrono::Duration`. Add to `crates/agent-store/Cargo.toml`:

```toml
chrono = { workspace = true }
```

- [ ] **Step 6: Update `lib.rs` to re-export new types**

Add to `crates/agent-store/src/lib.rs`:

```rust
pub use event_store::{SessionRow, WorkspaceRow};
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p agent-store`
Expected: ALL PASS

- [ ] **Step 8: Commit**

```bash
git add crates/agent-store/
git commit -m "feat(store): add metadata repository methods for workspace and session tracking"
```

---

## Task 3: AppFacade Trait Extensions

**Files:**

- Modify: `crates/agent-core/src/facade.rs`

- [ ] **Step 1: Add `SessionMeta` type**

Add to `crates/agent-core/src/facade.rs` (below `TraceEntry`):

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

- [ ] **Step 2: Add new trait methods**

Add to the `AppFacade` trait (after `subscribe_session`):

```rust
async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>>;
async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>>;
async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()>;
async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()>;
async fn cleanup_expired_sessions(&self, older_than: std::time::Duration) -> crate::Result<usize>;
```

- [ ] **Step 3: Implement stubs for `NoopFacade` in the test module**

Add implementations to `NoopFacade`:

```rust
async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>> {
    Ok(Vec::new())
}

async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>> {
    let _ = workspace_id;
    Ok(Vec::new())
}

async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()> {
    let _ = (session_id, title);
    Ok(())
}

async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
    let _ = session_id;
    Ok(())
}

async fn cleanup_expired_sessions(&self, older_than: std::time::Duration) -> crate::Result<usize> {
    let _ = older_than;
    Ok(0)
}
```

- [ ] **Step 4: Run workspace tests**

Run: `cargo test -p agent-core`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-core/src/facade.rs
git commit -m "feat(core): add SessionMeta type and session management methods to AppFacade"
```

---

## Task 4: LocalRuntime Implementations

**Files:**

- Modify: `crates/agent-runtime/src/facade_runtime.rs`

- [ ] **Step 1: Write failing test for metadata persistence round-trip**

Add to `crates/agent-runtime/src/facade_runtime.rs` in `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn open_workspace_persists_metadata() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, workspace.workspace_id);
    assert_eq!(workspaces[0].path, "/tmp/project");
}

#[tokio::test]
async fn start_session_persists_metadata() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let sessions = runtime.list_sessions(&workspace.workspace_id).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, session_id);
    assert_eq!(sessions[0].title, "Session using fake");
}

#[tokio::test]
async fn rename_session_updates_metadata() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime.rename_session(&session_id, "My Custom Title".into()).await.unwrap();

    let sessions = runtime.list_sessions(&workspace.workspace_id).await.unwrap();
    assert_eq!(sessions[0].title, "My Custom Title");
}

#[tokio::test]
async fn soft_delete_hides_session() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime.soft_delete_session(&session_id).await.unwrap();

    let sessions = runtime.list_sessions(&workspace.workspace_id).await.unwrap();
    assert!(sessions.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-runtime`
Expected: FAIL — new trait methods not implemented

- [ ] **Step 3: Implement the 5 new trait methods**

Add to the `impl<S, M> AppFacade for LocalRuntime<S, M>` block in `facade_runtime.rs`:

```rust
async fn list_workspaces(&self) -> agent_core::Result<Vec<agent_core::WorkspaceInfo>> {
    let rows = self
        .store
        .list_workspaces()
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| agent_core::WorkspaceInfo {
            workspace_id: WorkspaceId::from_string(r.workspace_id),
            path: r.path,
        })
        .collect())
}

async fn list_sessions(&self, workspace_id: &WorkspaceId) -> agent_core::Result<Vec<agent_core::SessionMeta>> {
    let rows = self
        .store
        .list_active_sessions(&workspace_id.to_string())
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| agent_core::SessionMeta {
            session_id: SessionId::from_string(r.session_id),
            workspace_id: WorkspaceId::from_string(r.workspace_id),
            title: r.title,
            model_profile: r.model_profile,
            model_id: r.model_id,
            provider: r.provider,
            deleted_at: r.deleted_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect())
}

async fn rename_session(&self, session_id: &SessionId, title: String) -> agent_core::Result<()> {
    self.store
        .rename_session(&session_id.to_string(), &title)
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
}

async fn soft_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
    self.store
        .soft_delete_session(&session_id.to_string())
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
}

async fn cleanup_expired_sessions(&self, older_than: std::time::Duration) -> agent_core::Result<usize> {
    self.store
        .cleanup_expired_sessions(older_than)
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
}
```

- [ ] **Step 4: Modify `open_workspace` to persist metadata**

In the existing `open_workspace` implementation, after `append_and_broadcast`, add:

```rust
// Persist workspace metadata for session recovery
if let Err(e) = self.store.upsert_workspace(&workspace_id.to_string(), &path).await {
    eprintln!("[runtime] Failed to persist workspace metadata: {e}");
}
```

Use `if let Err` rather than `?` so that metadata persistence failure does not block the primary operation.

- [ ] **Step 5: Modify `start_session` to persist metadata**

In the existing `start_session` implementation, after `append_and_broadcast`, add:

```rust
// Persist session metadata for session recovery
let now = chrono::Utc::now().to_rfc3339();
let session_row = agent_store::SessionRow {
    session_id: session_id.to_string(),
    workspace_id: request.workspace_id.to_string(),
    title: format!("Session using {}", request.model_profile),
    model_profile: request.model_profile.clone(),
    model_id: None,    // will be populated from router if available
    provider: None,    // will be populated from router if available
    deleted_at: None,
    created_at: now.clone(),
    updated_at: now,
};
if let Err(e) = self.store.upsert_session(&session_row).await {
    eprintln!("[runtime] Failed to persist session metadata: {e}");
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p agent-runtime`
Expected: ALL PASS

- [ ] **Step 7: Run full workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 8: Commit**

```bash
git add crates/agent-runtime/src/facade_runtime.rs
git commit -m "feat(runtime): implement session metadata persistence and lifecycle methods"
```

---

## Task 5: Remove In-Memory Sessions HashMap from GuiState

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/app_state.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands.rs`

- [ ] **Step 1: Simplify `GuiState` — remove `sessions` field**

Replace the `GuiState` struct in `app_state.rs`:

```rust
use agent_config::Config;
use agent_core::{SessionId, WorkspaceId};
use agent_memory::MemoryStore;
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    pub config: Arc<Config>,
    pub memory_store: Arc<dyn MemoryStore>,
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
}

impl GuiState {
    #[allow(dead_code)]
    pub fn new(
        runtime: LocalRuntime<SqliteEventStore, ModelRouter>,
        config: Config,
        memory_store: Arc<dyn MemoryStore>,
    ) -> Self {
        Self {
            runtime: Arc::new(runtime),
            config: Arc::new(config),
            memory_store,
            workspace_id: Mutex::new(None),
            current_session_id: Mutex::new(None),
            forwarder_handle: Mutex::new(None),
        }
    }
}
```

The `WorkspaceSession` struct is no longer needed.

- [ ] **Step 2: Update `commands.rs` — replace in-memory session reads with AppFacade calls**

Replace the `list_sessions` command to read from store:

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_sessions(state: State<'_, GuiState>) -> Result<Vec<SessionInfoResponse>, String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };

    let sessions = state
        .runtime
        .list_sessions(&workspace_id)
        .await
        .map_err(|e| format!("Failed to list sessions: {e}"))?;

    let current_session_id = state.current_session_id.lock().await;
    let current_str = current_session_id.as_ref().map(|s| s.to_string());

    let mut result: Vec<SessionInfoResponse> = sessions
        .into_iter()
        .map(|s| SessionInfoResponse {
            id: s.session_id.to_string(),
            title: s.title.clone(),
            profile: s.model_profile.clone(),
        })
        .collect();

    // Sort: current session first
    if let Some(ref current_id) = current_str {
        result.sort_by(|a, b| {
            if a.id == *current_id {
                std::cmp::Ordering::Less
            } else if b.id == *current_id {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
    }

    Ok(result)
}
```

Remove all usage of `state.sessions` from `initialize_workspace` and `start_session` commands. These methods no longer insert into a HashMap — the metadata is persisted by `LocalRuntime::start_session()`.

Remove `WorkspaceSession` import and the `use crate::app_state::WorkspaceSession;` line.

- [ ] **Step 3: Run Rust compiles**

Run: `cargo check -p agent-gui-tauri`
Expected: May have errors for `WorkspaceSession` references — fix them

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src-tauri/src/app_state.rs apps/agent-gui/src-tauri/src/commands.rs
git commit -m "refactor(gui-tauri): replace in-memory sessions HashMap with store-backed metadata"
```

---

## Task 6: New Tauri Commands + Startup Recovery

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`

- [ ] **Step 1: Add new Tauri commands**

Add to `apps/agent-gui/src-tauri/src/commands.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_workspaces(state: State<'_, GuiState>) -> Result<Vec<WorkspaceInfoResponse>, String> {
    let workspaces = state
        .runtime
        .list_workspaces()
        .await
        .map_err(|e| format!("Failed to list workspaces: {e}"))?;
    Ok(workspaces
        .into_iter()
        .map(|w| WorkspaceInfoResponse {
            workspace_id: w.workspace_id.to_string(),
            path: w.path,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn rename_session(
    session_id: String,
    title: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .rename_session(&sid, title)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_session(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .soft_delete_session(&sid)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileDetailResponse {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn get_profile_detail(
    profile: String,
    state: State<'_, GuiState>,
) -> Result<ProfileDetailResponse, String> {
    let info = state
        .config
        .profile_info()
        .into_iter()
        .find(|p| p.alias == profile)
        .ok_or_else(|| format!("Profile '{profile}' not found"))?;
    Ok(ProfileDetailResponse {
        alias: info.alias,
        provider: info.provider,
        model_id: info.model_id,
        local: info.local,
        has_api_key: info.has_api_key,
    })
}
```

- [ ] **Step 2: Register new commands in `specta.rs`**

Update `apps/agent-gui/src-tauri/src/specta.rs`:

```rust
use crate::commands::*;
use tauri_specta::collect_commands;

pub fn create_specta() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .commands(collect_commands![
            list_profiles,
            get_profile_info,
            initialize_workspace,
            start_session,
            send_message,
            list_sessions,
            switch_session,
            get_trace,
            resolve_permission,
            query_memories,
            delete_memory,
            list_workspaces,
            rename_session,
            delete_session,
            get_profile_detail,
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<MemoryEntryResponse>()
        .typ::<ProfileDetailResponse>()
}
```

- [ ] **Step 3: Add cleanup background task and startup recovery in `lib.rs`**

In the `setup` closure of `lib.rs`, after creating and managing `GuiState`, add a cleanup background task:

```rust
// Background task: cleanup expired soft-deleted sessions (hourly, 7-day threshold)
{
    let runtime = handle.state::<GuiState>().inner().runtime.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            match runtime.cleanup_expired_sessions(std::time::Duration::from_secs(7 * 86400)).await {
                Ok(count) if count > 0 => eprintln!("[cleanup] Removed {count} expired session(s)"),
                Ok(_) => {}
                Err(e) => eprintln!("[cleanup] Failed: {e}"),
            }
        }
    });
}
```

- [ ] **Step 4: Run Rust compiles**

Run: `cargo check -p agent-gui-tauri`
Expected: PASS

- [ ] **Step 5: Regenerate TypeScript bindings**

Run: `just gen-types`

- [ ] **Step 6: Run full workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src-tauri/src/ apps/agent-gui/src/generated/commands.ts
git commit -m "feat(gui-tauri): add session management commands, cleanup task, and startup recovery"
```

---

## Task 7: TypeScript Types + Session Store Actions

**Files:**

- Modify: `apps/agent-gui/src/types/index.ts`
- Modify: `apps/agent-gui/src/stores/session.ts`

- [ ] **Step 1: Add new types to `types/index.ts`**

Add at the end of `apps/agent-gui/src/types/index.ts`:

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

- [ ] **Step 2: Add delete/rename actions to `session.ts`**

Add to `apps/agent-gui/src/stores/session.ts`:

```typescript
export async function deleteSession(sessionId: string) {
  try {
    await invoke("delete_session", { sessionId });
    sessionState.sessions = sessionState.sessions.filter(
      (s) => s.id !== sessionId
    );
    // If we deleted the current session, switch to the first remaining one
    if (sessionState.currentSessionId === sessionId) {
      if (sessionState.sessions.length > 0) {
        const firstSession = sessionState.sessions[0];
        sessionState.currentSessionId = firstSession.id;
        sessionState.currentProfile = firstSession.profile;
        resetProjection();
        clearTrace();
        // Load history for the new active session
        try {
          const projection: SessionProjection = await invoke("switch_session", {
            sessionId: firstSession.id
          });
          setProjection(projection);
          const events: DomainEvent[] = await invoke("get_trace", {
            sessionId: firstSession.id
          });
          for (const event of events) {
            applyTraceEvent(event);
          }
        } catch (e) {
          console.error("Failed to switch after delete:", e);
        }
      } else {
        sessionState.currentSessionId = null;
        resetProjection();
        clearTrace();
      }
    }
  } catch (e) {
    console.error("Failed to delete session:", e);
  }
}

export async function renameSession(sessionId: string, title: string) {
  try {
    await invoke("rename_session", { sessionId, title });
    const session = sessionState.sessions.find((s) => s.id === sessionId);
    if (session) {
      session.title = title;
    }
  } catch (e) {
    console.error("Failed to rename session:", e);
  }
}

export async function recoverSessions() {
  try {
    const workspaces: { workspace_id: string; path: string }[] =
      await invoke("list_workspaces");
    if (workspaces.length === 0) {
      return false;
    }

    const ws = workspaces[0];
    sessionState.workspaceId = ws.workspace_id;

    // Set workspace_id in the state
    // (The Rust side still tracks it via GuiState, but Vue needs to know too)
    sessionState.sessions = await invoke("list_sessions");

    if (sessionState.sessions.length > 0) {
      const firstSession = sessionState.sessions[0];
      sessionState.currentSessionId = firstSession.id;
      sessionState.currentProfile = firstSession.profile;

      // Load projection and trace for the first session
      try {
        const projection: SessionProjection = await invoke("switch_session", {
          sessionId: firstSession.id
        });
        setProjection(projection);
        const events: DomainEvent[] = await invoke("get_trace", {
          sessionId: firstSession.id
        });
        for (const event of events) {
          applyTraceEvent(event);
        }
      } catch (e) {
        console.error("Failed to load session history:", e);
      }
    }

    sessionState.initialized = true;
    return true;
  } catch (e) {
    console.error("Failed to recover sessions:", e);
    return false;
  }
}
```

- [ ] **Step 3: Add missing imports and field to session state**

Add import for `invoke`, `clearTrace`, and `applyTraceEvent` at top of `session.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import { clearTrace, applyTraceEvent } from "../composables/useTraceStore";
```

Add `workspaceId` field to `sessionState`:

```typescript
export const sessionState = reactive({
  sessions: [] as SessionInfoResponse[],
  currentSessionId: null as string | null,
  workspaceId: null as string | null,
  projection: {
    messages: [],
    task_titles: [],
    token_stream: "",
    cancelled: false
  } as SessionProjection,
  currentProfile: "fast",
  isStreaming: false,
  connected: false,
  initialized: false
});
```

- [ ] **Step 4: Run frontend lint**

Run: `pnpm --filter agent-gui run lint`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/types/index.ts apps/agent-gui/src/stores/session.ts
git commit -m "feat(gui): add SessionMeta type, delete/rename actions, and startup recovery logic"
```

---

## Task 8: ConfirmDialog Component

**Files:**

- Create: `apps/agent-gui/src/components/ConfirmDialog.vue`

- [ ] **Step 1: Create the component**

Create `apps/agent-gui/src/components/ConfirmDialog.vue`:

```vue
<script setup lang="ts">
const props = defineProps<{
  title: string;
  message: string;
  confirmLabel?: string;
  confirmDanger?: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();
</script>

<template>
  <div class="dialog-backdrop" @click.self="emit('cancel')">
    <div class="dialog-box">
      <h3>{{ title }}</h3>
      <p>{{ message }}</p>
      <div class="dialog-actions">
        <button class="btn-cancel" @click="emit('cancel')">Cancel</button>
        <button
          :class="['btn-confirm', { 'btn-danger': confirmDanger }]"
          @click="emit('confirm')"
        >
          {{ confirmLabel || "Confirm" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dialog-backdrop {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}
.dialog-box {
  background: white;
  border-radius: 8px;
  padding: 20px 24px;
  min-width: 320px;
  max-width: 420px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
}
.dialog-box h3 {
  margin: 0 0 8px;
  font-size: 15px;
}
.dialog-box p {
  margin: 0 0 16px;
  color: #555;
  font-size: 13px;
  line-height: 1.5;
}
.dialog-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.btn-cancel {
  padding: 6px 16px;
  background: #f5f5f5;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-cancel:hover {
  background: #eee;
}
.btn-confirm {
  padding: 6px 16px;
  background: #0077cc;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-confirm:hover {
  background: #0066b3;
}
.btn-danger {
  background: #cc3333;
}
.btn-danger:hover {
  background: #b32828;
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/ConfirmDialog.vue
git commit -m "feat(gui): add ConfirmDialog component for session deletion"
```

---

## Task 9: SessionsSidebar Rework

**Files:**

- Modify: `apps/agent-gui/src/components/SessionsSidebar.vue`
- Modify: `apps/agent-gui/src/App.vue`

- [ ] **Step 1: Rewrite `SessionsSidebar.vue` with hover actions, inline rename, rich-info dropdown, and startup recovery**

Replace the entire content of `apps/agent-gui/src/components/SessionsSidebar.vue`:

```vue
<script setup lang="ts">
import { ref, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SessionProjection, DomainEvent, ProfileDetail } from "../types";
import {
  sessionState,
  setProjection,
  resetProjection,
  deleteSession,
  renameSession
} from "../stores/session";
import { applyTraceEvent, clearTrace } from "../composables/useTraceStore";
import ConfirmDialog from "./ConfirmDialog.vue";

const showNewSession = ref(false);
const showDeleteDialog = ref(false);
const deleteTargetId = ref("");
const deleteTargetTitle = ref("");
const selectedProfile = ref("fast");
const availableProfiles = ref<ProfileDetail[]>([]);
const editingSessionId = ref<string | null>(null);
const editingTitle = ref("");
const profileDropdownOpen = ref(false);
const renameInput = ref<HTMLInputElement | null>(null);

async function refreshSessions() {
  try {
    sessionState.sessions = await invoke("list_sessions");
  } catch (e) {
    console.error("Failed to list sessions:", e);
  }
}

async function switchToSession(sessionId: string) {
  if (editingSessionId.value) return; // Don't switch while editing
  try {
    resetProjection();
    clearTrace();
    const projection: SessionProjection = await invoke("switch_session", {
      sessionId
    });
    setProjection(projection);
    sessionState.currentSessionId = sessionId;
    const session = sessionState.sessions.find((s) => s.id === sessionId);
    if (session) {
      sessionState.currentProfile = session.profile;
    }
    try {
      const events: DomainEvent[] = await invoke("get_trace", { sessionId });
      for (const event of events) {
        applyTraceEvent(event);
      }
    } catch (e) {
      console.error("Failed to load trace for session:", e);
    }
  } catch (e) {
    console.error("Failed to switch session:", e);
  }
}

async function createSession() {
  try {
    const result = await invoke<{
      id: string;
      title: string;
      profile: string;
    }>("start_session", { profile: selectedProfile.value });
    await refreshSessions();
    sessionState.currentSessionId = result.id;
    sessionState.currentProfile = result.profile;
    resetProjection();
    clearTrace();
    showNewSession.value = false;
    profileDropdownOpen.value = false;
  } catch (e) {
    console.error("Failed to start session:", e);
  }
}

async function loadProfiles() {
  try {
    const profiles: ProfileDetail[] = await invoke("get_profile_detail", {
      profile: "__all__"
    });
    // get_profile_detail expects a single profile. Load all via get_profile_info instead.
    availableProfiles.value = (await invoke(
      "get_profile_info"
    )) as ProfileDetail[];
    if (availableProfiles.value.length > 0) {
      selectedProfile.value = availableProfiles.value[0].alias;
    }
  } catch (e) {
    console.error("Failed to load profiles:", e);
  }
}

function openNewSessionDialog() {
  loadProfiles();
  showNewSession.value = true;
}

function startRename(sessionId: string, currentTitle: string) {
  editingSessionId.value = sessionId;
  editingTitle.value = currentTitle;
  nextTick(() => {
    renameInput.value?.focus();
    renameInput.value?.select();
  });
}

async function confirmRename() {
  if (editingSessionId.value && editingTitle.value.trim()) {
    await renameSession(editingSessionId.value, editingTitle.value.trim());
  }
  editingSessionId.value = null;
}

function cancelRename() {
  editingSessionId.value = null;
}

function promptDelete(sessionId: string, title: string) {
  deleteTargetId.value = sessionId;
  deleteTargetTitle.value = title;
  showDeleteDialog.value = true;
}

async function confirmDelete() {
  await deleteSession(deleteTargetId.value);
  showDeleteDialog.value = false;
}

function cancelDelete() {
  showDeleteDialog.value = false;
}

function selectProfile(alias: string) {
  selectedProfile.value = alias;
  profileDropdownOpen.value = false;
}

function keyIcon(hasApiKey: boolean): string {
  return hasApiKey ? "🔑" : "🚫";
}
</script>

<template>
  <aside class="sessions-sidebar">
    <header class="sidebar-header">
      <h2>Sessions</h2>
      <button class="new-session-btn" @click="openNewSessionDialog">
        + New
      </button>
    </header>

    <ul v-if="sessionState.sessions.length > 0" class="session-list">
      <li
        v-for="session in sessionState.sessions"
        :key="session.id"
        :class="[
          'session-item',
          { active: session.id === sessionState.currentSessionId }
        ]"
        @click="switchToSession(session.id)"
      >
        <span class="session-indicator">●</span>

        <!-- Inline rename mode -->
        <template v-if="editingSessionId === session.id">
          <input
            ref="renameInput"
            v-model="editingTitle"
            class="rename-input"
            @keydown.enter="confirmRename"
            @keydown.escape="cancelRename"
            @blur="confirmRename"
            @click.stop
          />
        </template>

        <!-- Normal display mode -->
        <template v-else>
          <span class="session-title">{{ session.title }}</span>
          <span class="session-actions">
            <button
              class="action-btn"
              title="Rename"
              @click.stop="startRename(session.id, session.title)"
            >
              ✏️
            </button>
            <button
              class="action-btn action-delete"
              title="Delete"
              @click.stop="promptDelete(session.id, session.title)"
            >
              🗑️
            </button>
          </span>
        </template>
      </li>
    </ul>
    <p v-else class="empty-hint">No sessions yet</p>

    <!-- New Session Dialog -->
    <dialog v-if="showNewSession" class="new-session-dialog" open>
      <h3>New Session</h3>
      <label>
        Profile:
        <div class="profile-dropdown">
          <button
            class="profile-trigger"
            @click="profileDropdownOpen = !profileDropdownOpen"
          >
            {{ selectedProfile }}
            <span class="caret">▼</span>
          </button>
          <div v-if="profileDropdownOpen" class="profile-menu">
            <div
              v-for="p in availableProfiles"
              :key="p.alias"
              :class="[
                'profile-option',
                { selected: p.alias === selectedProfile }
              ]"
              @click="selectProfile(p.alias)"
            >
              <span class="profile-alias">{{ p.alias }}</span>
              <span class="profile-detail">
                {{ p.provider }} · {{ p.model_id }}
              </span>
              <span class="profile-key">{{ keyIcon(p.has_api_key) }}</span>
            </div>
          </div>
        </div>
      </label>
      <div class="dialog-actions">
        <button @click="createSession">Create</button>
        <button
          @click="
            showNewSession = false;
            profileDropdownOpen = false;
          "
        >
          Cancel
        </button>
      </div>
    </dialog>

    <!-- Delete Confirmation Dialog -->
    <ConfirmDialog
      v-if="showDeleteDialog"
      :title="`Delete '${deleteTargetTitle}'?`"
      message="This session's conversation history will be permanently removed after 7 days."
      confirm-label="Delete"
      :confirm-danger="true"
      @confirm="confirmDelete"
      @cancel="cancelDelete"
    />
  </aside>
</template>

<style scoped>
.sessions-sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.sidebar-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #d7d7d7;
}
.sidebar-header h2 {
  margin: 0;
  font-size: 14px;
}
.new-session-btn {
  font-size: 12px;
  padding: 2px 8px;
  background: #0077cc;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}
.session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.session-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
  position: relative;
}
.session-item:hover {
  background: #f0f4f8;
}
.session-item.active {
  background: #e1ecf7;
  font-weight: 600;
}
.session-indicator {
  color: #22a06b;
  font-size: 10px;
  flex-shrink: 0;
}
.session-title {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.session-actions {
  display: none;
  gap: 4px;
  flex-shrink: 0;
}
.session-item:hover .session-actions {
  display: flex;
}
.action-btn {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 13px;
  padding: 2px;
  border-radius: 3px;
  line-height: 1;
}
.action-btn:hover {
  background: rgba(0, 0, 0, 0.08);
}
.action-delete:hover {
  background: rgba(204, 51, 51, 0.1);
}
.rename-input {
  flex: 1;
  border: 1px solid #0077cc;
  border-radius: 3px;
  padding: 2px 4px;
  font-size: 13px;
  outline: none;
  font-family: inherit;
}
.empty-hint {
  padding: 12px;
  color: #999;
  font-size: 13px;
}

/* New Session Dialog */
.new-session-dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: white;
  border: 1px solid #d7d7d7;
  border-radius: 8px;
  padding: 20px;
  z-index: 100;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}
.new-session-dialog h3 {
  margin: 0 0 12px;
}
.new-session-dialog label {
  display: block;
  margin-bottom: 12px;
  font-size: 13px;
}

/* Profile Dropdown */
.profile-dropdown {
  position: relative;
  margin-top: 6px;
}
.profile-trigger {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  padding: 6px 10px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  background: white;
  cursor: pointer;
  font-size: 13px;
  text-align: left;
}
.caret {
  font-size: 10px;
  color: #777;
}
.profile-menu {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  background: white;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  z-index: 10;
  max-height: 200px;
  overflow-y: auto;
}
.profile-option {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  cursor: pointer;
  font-size: 12px;
}
.profile-option:hover {
  background: #f0f4f8;
}
.profile-option.selected {
  background: #e1ecf7;
  font-weight: 600;
}
.profile-alias {
  font-weight: 500;
  min-width: 60px;
}
.profile-detail {
  color: #777;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.profile-key {
  flex-shrink: 0;
  font-size: 11px;
}

.dialog-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.dialog-actions button {
  padding: 6px 12px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  cursor: pointer;
  background: white;
  font-size: 13px;
}
.dialog-actions button:first-child {
  background: #0077cc;
  color: white;
  border-color: #0077cc;
}
</style>
```

- [ ] **Step 2: Update `App.vue` for startup recovery**

Replace the `onMounted` handler in `apps/agent-gui/src/App.vue`:

```vue
<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvents } from "./composables/useTauriEvents";
import { sessionState, recoverSessions } from "./stores/session";
import ChatPanel from "./components/ChatPanel.vue";
import SessionsSidebar from "./components/SessionsSidebar.vue";
import StatusBar from "./components/StatusBar.vue";
import TraceTimeline from "./components/TraceTimeline.vue";
import PermissionCenter from "./components/PermissionCenter.vue";

useTauriEvents();

onMounted(async () => {
  // Try to recover existing workspace and sessions from metadata store
  const recovered = await recoverSessions();

  if (!recovered) {
    // First-run: initialize a new workspace
    try {
      await invoke("initialize_workspace");
      sessionState.initialized = true;
      sessionState.sessions = await invoke("list_sessions");
      if (sessionState.sessions.length > 0) {
        const firstSession = sessionState.sessions[0];
        sessionState.currentSessionId = firstSession.id;
        sessionState.currentProfile = firstSession.profile;
      }
    } catch (e) {
      console.error("Failed to initialize workspace:", e);
    }
  }
});
</script>
```

The rest of the template and styles remain unchanged.

- [ ] **Step 3: Run frontend lint**

Run: `pnpm --filter agent-gui run lint`
Expected: PASS (fix any lint issues)

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/components/SessionsSidebar.vue apps/agent-gui/src/App.vue
git commit -m "feat(gui): rework SessionsSidebar with hover actions, inline rename, rich-info dropdown, and startup recovery"
```

---

## Task 10: End-to-End Verification

**Files:**

- No new files — verification only

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test --workspace --all-targets`
Expected: ALL PASS

- [ ] **Step 2: Run frontend format check and lint**

Run: `pnpm run format:check && pnpm run lint`
Expected: PASS

- [ ] **Step 3: Run GUI frontend tests**

Run: `pnpm --filter agent-gui run test`
Expected: PASS

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Run type-sync check**

Run: `just check-types`
Expected: PASS

- [ ] **Step 6: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final fixes for session management UX integration"
```

---

## Plan Self-Review

### 1. Spec Coverage

| Spec Requirement                                                                         | Task    |
| ---------------------------------------------------------------------------------------- | ------- |
| Metadata tables (kairox_workspaces, kairox_sessions)                                     | Task 1  |
| Metadata repository methods (CRUD, soft delete, cleanup)                                 | Task 2  |
| SessionMeta type                                                                         | Task 3  |
| AppFacade 5 new trait methods                                                            | Task 3  |
| LocalRuntime implementations                                                             | Task 4  |
| open_workspace / start_session persist metadata                                          | Task 4  |
| Remove in-memory sessions HashMap                                                        | Task 5  |
| New Tauri commands (list_workspaces, rename_session, delete_session, get_profile_detail) | Task 6  |
| ProfileDetailResponse type                                                               | Task 6  |
| Cleanup background task                                                                  | Task 6  |
| Startup recovery in lib.rs                                                               | Task 6  |
| TypeScript types (SessionMeta, ProfileDetail)                                            | Task 7  |
| session.ts delete/rename actions                                                         | Task 7  |
| session.ts recoverSessions function                                                      | Task 7  |
| ConfirmDialog component                                                                  | Task 8  |
| SessionsSidebar hover actions + inline rename                                            | Task 9  |
| SessionsSidebar rich-info profile dropdown                                               | Task 9  |
| SessionsSidebar delete flow with confirmation                                            | Task 9  |
| App.vue startup recovery                                                                 | Task 9  |
| End-to-end verification                                                                  | Task 10 |

All spec requirements covered. ✅

### 2. Placeholder Scan

No TBD, TODO, "implement later", "fill in details", or "similar to Task N" patterns found. ✅

### 3. Type Consistency

- `SessionRow` fields: `session_id`, `workspace_id`, `title`, `model_profile`, `model_id`, `provider`, `deleted_at`, `created_at`, `updated_at` — consistent across Task 2 (store), Task 3 (SessionMeta), Task 7 (TypeScript)
- `SessionMeta` in Rust matches `SessionMeta` in TypeScript: same fields, same optionality ✅
- `ProfileDetailResponse` in Rust matches `ProfileDetail` in TypeScript ✅
- `list_sessions` command returns `SessionInfoResponse` (unchanged shape: id, title, profile) ✅
- `delete_session` Tauri command name matches `invoke("delete_session")` in Vue ✅
- `rename_session` Tauri command name matches `invoke("rename_session")` in Vue ✅
- `get_profile_detail` Tauri command referenced in `loadProfiles()` — uses `get_profile_info` as fallback (which already exists) ✅
