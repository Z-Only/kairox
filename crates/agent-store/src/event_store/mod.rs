use agent_core::{DomainEvent, SessionId};
use async_trait::async_trait;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::time::Duration;

mod append;
mod metadata;
mod migrations;

#[async_trait]
/// Trait for the append-only event store with metadata support.
///
/// Events are stored in the order they are appended and can be replayed
/// per session. Metadata methods support workspace and session lifecycle
/// persistence for session recovery. The canonical implementation is
/// [`SqliteEventStore`].
pub trait EventStore: Send + Sync {
    /// Return the underlying SQLite pool when this store is backed by SQLite.
    fn sqlite_pool(&self) -> Option<SqlitePool> {
        None
    }

    /// Append a domain event to the store.
    async fn append(&self, event: &DomainEvent) -> crate::Result<()>;
    /// Load all events for a session in append order.
    async fn load_session(&self, session_id: &SessionId) -> crate::Result<Vec<DomainEvent>>;

    // --- Metadata methods ---

    /// Insert or update a workspace record.
    async fn upsert_workspace(&self, workspace_id: &str, path: &str) -> crate::Result<()>;
    /// Insert or update a session metadata record.
    async fn upsert_session(&self, meta: &SessionRow) -> crate::Result<()>;
    /// List all known workspaces.
    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceRow>>;
    /// List all active (non-deleted) sessions for a workspace.
    async fn list_active_sessions(&self, workspace_id: &str) -> crate::Result<Vec<SessionRow>>;
    /// Rename a session by updating its title.
    async fn rename_session(&self, session_id: &str, title: &str) -> crate::Result<()>;
    /// Soft-delete a session by setting deleted_at.
    async fn soft_delete_session(&self, session_id: &str) -> crate::Result<()>;
    /// Permanently hard-delete a session and all associated data.
    async fn permanently_delete_session(&self, session_id: &str) -> crate::Result<()>;
    /// Restore an archived session back to visible status.
    async fn restore_archived_session(&self, session_id: &str) -> crate::Result<()>;
    /// Hard-delete sessions that were soft-deleted longer than the specified duration ago.
    async fn cleanup_expired_sessions(&self, older_than: Duration) -> crate::Result<usize>;
    /// List visible project-bound sessions.
    async fn list_visible_project_sessions(
        &self,
        project_id: &str,
    ) -> crate::Result<Vec<ProjectSessionMetaRow>>;
    /// List archived project-bound sessions for a workspace.
    async fn list_archived_project_session_metas(
        &self,
        workspace_id: &str,
    ) -> crate::Result<Vec<ProjectSessionMetaRow>>;

    /// Save draft text for a session (upsert).
    async fn save_draft(&self, session_id: &str, draft_text: &str) -> crate::Result<()>;
    /// Get draft text for a session, returning empty string if none exists.
    async fn get_draft(&self, session_id: &str) -> crate::Result<String>;
    /// Update the permission mode for a session.
    async fn update_permission_mode(&self, session_id: &str, mode: &str) -> crate::Result<()>;
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectSessionMetaRow {
    pub session_id: String,
    pub workspace_id: String,
    pub title: String,
    pub model_profile: String,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub project_id: String,
    pub worktree_path: String,
    pub branch: Option<String>,
    pub visibility: String,
    pub permission_mode: String,
}

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
    pub permission_mode: String,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow)]
struct SessionRowForQuery {
    session_id: String,
    workspace_id: String,
    title: String,
    model_profile: String,
    model_id: Option<String>,
    provider: Option<String>,
    permission_mode: String,
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
            permission_mode: r.permission_mode,
            deleted_at: r.deleted_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Clone)]
pub struct SqliteEventStore {
    pool: SqlitePool,
}

impl SqliteEventStore {
    pub async fn connect(database_url: &str) -> crate::Result<Self> {
        if let Some(path) = database_url.strip_prefix("sqlite://") {
            if !path.is_empty() && path != ":memory:" {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    if !parent.as_os_str().is_empty() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                }
                if !std::path::Path::new(path).exists() {
                    let _ = tokio::fs::File::create(path).await?;
                }
            }
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .connect(database_url)
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    pub async fn in_memory() -> crate::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .connect("sqlite::memory:")
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn migrate(&self) -> crate::Result<()> {
        migrations::run(&self.pool).await
    }
}

#[async_trait]
impl EventStore for SqliteEventStore {
    fn sqlite_pool(&self) -> Option<SqlitePool> {
        Some(self.pool.clone())
    }

    async fn append(&self, event: &DomainEvent) -> crate::Result<()> {
        self.append_event(event).await
    }

    async fn load_session(&self, session_id: &SessionId) -> crate::Result<Vec<DomainEvent>> {
        self.load_session_events(session_id).await
    }

    // --- Metadata methods (delegate to inherent implementations) ---

    async fn upsert_workspace(&self, workspace_id: &str, path: &str) -> crate::Result<()> {
        SqliteEventStore::upsert_workspace(self, workspace_id, path).await
    }

    async fn upsert_session(&self, meta: &SessionRow) -> crate::Result<()> {
        SqliteEventStore::upsert_session(self, meta).await
    }

    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceRow>> {
        SqliteEventStore::list_workspaces(self).await
    }

    async fn list_active_sessions(&self, workspace_id: &str) -> crate::Result<Vec<SessionRow>> {
        SqliteEventStore::list_active_sessions(self, workspace_id).await
    }

    async fn rename_session(&self, session_id: &str, title: &str) -> crate::Result<()> {
        SqliteEventStore::rename_session(self, session_id, title).await
    }

    async fn soft_delete_session(&self, session_id: &str) -> crate::Result<()> {
        SqliteEventStore::soft_delete_session(self, session_id).await
    }

    async fn permanently_delete_session(&self, session_id: &str) -> crate::Result<()> {
        SqliteEventStore::permanently_delete_session(self, session_id).await
    }

    async fn restore_archived_session(&self, session_id: &str) -> crate::Result<()> {
        SqliteEventStore::restore_archived_session(self, session_id).await
    }

    async fn cleanup_expired_sessions(&self, older_than: Duration) -> crate::Result<usize> {
        SqliteEventStore::cleanup_expired_sessions(self, older_than).await
    }

    async fn list_visible_project_sessions(
        &self,
        project_id: &str,
    ) -> crate::Result<Vec<ProjectSessionMetaRow>> {
        self.list_visible_project_session_rows(project_id).await
    }

    async fn list_archived_project_session_metas(
        &self,
        workspace_id: &str,
    ) -> crate::Result<Vec<ProjectSessionMetaRow>> {
        self.list_archived_project_session_rows(workspace_id).await
    }

    async fn save_draft(&self, session_id: &str, draft_text: &str) -> crate::Result<()> {
        SqliteEventStore::save_draft(self, session_id, draft_text).await
    }

    async fn get_draft(&self, session_id: &str) -> crate::Result<String> {
        SqliteEventStore::get_draft(self, session_id).await
    }

    async fn update_permission_mode(&self, session_id: &str, mode: &str) -> crate::Result<()> {
        SqliteEventStore::update_permission_mode(self, session_id, mode).await
    }
}

#[cfg(test)]
mod tests;
