use agent_core::{DomainEvent, SessionId};
use async_trait::async_trait;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use std::time::Duration;

#[async_trait]
/// Trait for the append-only event store with metadata support.
///
/// Events are stored in the order they are appended and can be replayed
/// per session. Metadata methods support workspace and session lifecycle
/// persistence for session recovery. The canonical implementation is
/// [`SqliteEventStore`].
pub trait EventStore: Send + Sync {
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
    /// Hard-delete sessions that were soft-deleted longer than the specified duration ago.
    async fn cleanup_expired_sessions(&self, older_than: Duration) -> crate::Result<usize>;
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
        sqlx::query(include_str!("../migrations/0001_events.sql"))
            .execute(&self.pool)
            .await?;
        sqlx::query(include_str!("../migrations/0002_metadata.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- Metadata repository methods ---

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
        sqlx::query("UPDATE kairox_sessions SET title = ?1, updated_at = ?2 WHERE session_id = ?3")
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

    pub async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> crate::Result<usize> {
        let threshold = chrono::Utc::now()
            - chrono::Duration::from_std(older_than)
                .unwrap_or_else(|_| chrono::Duration::seconds(0));
        let threshold_str = threshold.to_rfc3339();

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

        for sid in &expired {
            sqlx::query("DELETE FROM events WHERE session_id = ?1")
                .bind(sid)
                .execute(&self.pool)
                .await?;
        }

        sqlx::query("DELETE FROM kairox_sessions WHERE deleted_at IS NOT NULL AND deleted_at < ?1")
            .bind(&threshold_str)
            .execute(&self.pool)
            .await?;

        Ok(count)
    }
}

#[async_trait]
impl EventStore for SqliteEventStore {
    async fn append(&self, event: &DomainEvent) -> crate::Result<()> {
        let payload_json = serde_json::to_string(event)?;
        sqlx::query(
            "INSERT INTO events (schema_version, workspace_id, session_id, timestamp, source_agent_id, privacy, event_type, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(event.schema_version as i64)
        .bind(event.workspace_id.to_string())
        .bind(event.session_id.to_string())
        .bind(event.timestamp.to_rfc3339())
        .bind(event.source_agent_id.to_string())
        .bind(match event.privacy {
            agent_core::PrivacyClassification::MinimalTrace => "minimal_trace",
            agent_core::PrivacyClassification::FullTrace => "full_trace",
        })
        .bind(event.event_type.as_str())
        .bind(payload_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn load_session(&self, session_id: &SessionId) -> crate::Result<Vec<DomainEvent>> {
        let rows =
            sqlx::query("SELECT payload_json FROM events WHERE session_id = ?1 ORDER BY id ASC")
                .bind(session_id.to_string())
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|row| {
                let payload_json: String = row.try_get("payload_json")?;
                let event = serde_json::from_str(&payload_json)?;
                Ok(event)
            })
            .collect()
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

    async fn cleanup_expired_sessions(&self, older_than: Duration) -> crate::Result<usize> {
        SqliteEventStore::cleanup_expired_sessions(self, older_than).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AgentId, EventPayload, PrivacyClassification, SessionId, WorkspaceId};
    use sqlx::Row;

    #[tokio::test]
    async fn appends_and_replays_session_events_with_full_fidelity() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();

        let first = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: "m1".into(),
                content: "hello".into(),
            },
        );
        let second = DomainEvent::new(
            workspace_id,
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AssistantMessageCompleted {
                message_id: "m2".into(),
                content: "hi".into(),
            },
        );

        store.append(&first).await.unwrap();
        store.append(&second).await.unwrap();

        let replayed = store.load_session(&session_id).await.unwrap();
        assert_eq!(replayed, vec![first, second]);
    }

    #[tokio::test]
    async fn stores_queryable_scalar_envelope_columns() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let source_agent_id = AgentId::system();
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            source_agent_id.clone(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: "m1".into(),
                content: "hello".into(),
            },
        );

        store.append(&event).await.unwrap();

        let row = sqlx::query(
            "SELECT workspace_id, session_id, source_agent_id, privacy, event_type FROM events",
        )
        .fetch_one(&store.pool)
        .await
        .unwrap();

        assert_eq!(row.get::<String, _>("workspace_id"), workspace_id.as_str());
        assert_eq!(row.get::<String, _>("session_id"), session_id.as_str());
        assert_eq!(
            row.get::<String, _>("source_agent_id"),
            source_agent_id.as_str()
        );
        assert_eq!(row.get::<String, _>("privacy"), "full_trace");
        assert_eq!(row.get::<String, _>("event_type"), "UserMessageAdded");
    }

    #[tokio::test]
    async fn connects_to_file_backed_sqlite_for_persisted_replay() {
        let db_path = std::env::temp_dir().join(format!(
            "kairox-agent-store-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let database_url = format!("sqlite://{}", db_path.display());
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let event = DomainEvent::new(
            workspace_id,
            session_id.clone(),
            AgentId::planner(),
            PrivacyClassification::MinimalTrace,
            EventPayload::WorkspaceOpened {
                path: "/tmp/kairox".into(),
            },
        );

        {
            let store = SqliteEventStore::connect(&database_url).await.unwrap();
            store.append(&event).await.unwrap();
        }

        let reopened = SqliteEventStore::connect(&database_url).await.unwrap();
        let replayed = reopened.load_session(&session_id).await.unwrap();

        assert_eq!(replayed, vec![event]);

        std::fs::remove_file(db_path).unwrap();
    }

    // --- Metadata repository tests ---

    #[tokio::test]
    async fn upsert_and_list_workspaces() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project-a")
            .await
            .unwrap();
        store
            .upsert_workspace("wrk_2", "/tmp/project-b")
            .await
            .unwrap();

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
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        store
            .upsert_session(&SessionRow {
                session_id: "ses_1".into(),
                workspace_id: "wrk_1".into(),
                title: "Session using fast".into(),
                model_profile: "fast".into(),
                model_id: Some("gpt-4.1-mini".into()),
                provider: Some("openai_compatible".into()),
                deleted_at: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await
            .unwrap();

        let sessions = store.list_active_sessions("wrk_1").await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title, "Session using fast");
        assert_eq!(sessions[0].model_id, Some("gpt-4.1-mini".into()));
    }

    #[tokio::test]
    async fn soft_delete_hides_session_from_active_list() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        store
            .upsert_session(&SessionRow {
                session_id: "ses_1".into(),
                workspace_id: "wrk_1".into(),
                title: "To delete".into(),
                model_profile: "fake".into(),
                model_id: None,
                provider: None,
                deleted_at: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await
            .unwrap();

        store.soft_delete_session("ses_1").await.unwrap();

        let sessions = store.list_active_sessions("wrk_1").await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn rename_session_updates_title() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        store
            .upsert_session(&SessionRow {
                session_id: "ses_1".into(),
                workspace_id: "wrk_1".into(),
                title: "Old title".into(),
                model_profile: "fake".into(),
                model_id: None,
                provider: None,
                deleted_at: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await
            .unwrap();

        store.rename_session("ses_1", "New title").await.unwrap();

        let sessions = store.list_active_sessions("wrk_1").await.unwrap();
        assert_eq!(sessions[0].title, "New title");
    }

    #[tokio::test]
    async fn cleanup_expired_deletes_old_soft_deleted_sessions() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        let old_deleted = chrono::Utc::now() - chrono::Duration::days(10);
        store
            .upsert_session(&SessionRow {
                session_id: "ses_old".into(),
                workspace_id: "wrk_1".into(),
                title: "Old deleted".into(),
                model_profile: "fake".into(),
                model_id: None,
                provider: None,
                deleted_at: Some(old_deleted.to_rfc3339()),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await
            .unwrap();

        store
            .upsert_session(&SessionRow {
                session_id: "ses_recent".into(),
                workspace_id: "wrk_1".into(),
                title: "Recent deleted".into(),
                model_profile: "fake".into(),
                model_id: None,
                provider: None,
                deleted_at: Some(chrono::Utc::now().to_rfc3339()),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await
            .unwrap();

        let deleted = store
            .cleanup_expired_sessions(std::time::Duration::from_secs(7 * 86400))
            .await
            .unwrap();
        assert_eq!(deleted, 1);
    }

    #[tokio::test]
    async fn list_active_sessions_returns_empty_for_unknown_workspace() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let sessions = store.list_active_sessions("wrk_nonexistent").await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn cleanup_expired_also_deletes_associated_events() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        let old_deleted = chrono::Utc::now() - chrono::Duration::days(10);
        store
            .upsert_session(&SessionRow {
                session_id: "ses_old".into(),
                workspace_id: "wrk_1".into(),
                title: "Old deleted".into(),
                model_profile: "fake".into(),
                model_id: None,
                provider: None,
                deleted_at: Some(old_deleted.to_rfc3339()),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await
            .unwrap();

        let workspace_id = WorkspaceId::from_string("wrk_1".into());
        let session_id = SessionId::from_string("ses_old".into());
        let event = DomainEvent::new(
            workspace_id,
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: "m1".into(),
                content: "hello".into(),
            },
        );
        store.append(&event).await.unwrap();

        let events_before = store.load_session(&session_id).await.unwrap();
        assert_eq!(events_before.len(), 1);

        let deleted = store
            .cleanup_expired_sessions(std::time::Duration::from_secs(7 * 86400))
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        let events_after = store.load_session(&session_id).await.unwrap();
        assert!(events_after.is_empty());
    }

    #[tokio::test]
    async fn cleanup_expired_skips_recently_deleted() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        let recent_deleted = chrono::Utc::now() - chrono::Duration::days(1);
        store
            .upsert_session(&SessionRow {
                session_id: "ses_recent".into(),
                workspace_id: "wrk_1".into(),
                title: "Recently deleted".into(),
                model_profile: "fake".into(),
                model_id: None,
                provider: None,
                deleted_at: Some(recent_deleted.to_rfc3339()),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await
            .unwrap();

        let deleted = store
            .cleanup_expired_sessions(std::time::Duration::from_secs(7 * 86400))
            .await
            .unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn upsert_session_updates_existing_record() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        store
            .upsert_session(&SessionRow {
                session_id: "ses_1".into(),
                workspace_id: "wrk_1".into(),
                title: "Original".into(),
                model_profile: "fake".into(),
                model_id: None,
                provider: None,
                deleted_at: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            })
            .await
            .unwrap();

        store
            .upsert_session(&SessionRow {
                session_id: "ses_1".into(),
                workspace_id: "wrk_1".into(),
                title: "Updated title".into(),
                model_profile: "fast".into(),
                model_id: Some("gpt-4.1-mini".into()),
                provider: Some("openai_compatible".into()),
                deleted_at: None,
                created_at: now.clone(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            })
            .await
            .unwrap();

        let sessions = store.list_active_sessions("wrk_1").await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title, "Updated title");
        assert_eq!(sessions[0].model_profile, "fast");
        assert_eq!(sessions[0].model_id, Some("gpt-4.1-mini".into()));
        assert_eq!(sessions[0].provider, Some("openai_compatible".into()));
    }

    #[tokio::test]
    async fn metadata_survives_across_reopen() {
        let db_path = std::env::temp_dir().join(format!(
            "kairox-store-metadata-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let database_url = format!("sqlite://{}", db_path.display());

        {
            let store = SqliteEventStore::connect(&database_url).await.unwrap();
            store
                .upsert_workspace("wrk_1", "/tmp/project")
                .await
                .unwrap();
            let now = chrono::Utc::now().to_rfc3339();
            store
                .upsert_session(&SessionRow {
                    session_id: "ses_1".into(),
                    workspace_id: "wrk_1".into(),
                    title: "Persistent session".into(),
                    model_profile: "fast".into(),
                    model_id: Some("gpt-4.1-mini".into()),
                    provider: Some("openai_compatible".into()),
                    deleted_at: None,
                    created_at: now.clone(),
                    updated_at: now,
                })
                .await
                .unwrap();
        }

        let reopened = SqliteEventStore::connect(&database_url).await.unwrap();
        let workspaces = reopened.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].workspace_id, "wrk_1");

        let sessions = reopened.list_active_sessions("wrk_1").await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title, "Persistent session");
        assert_eq!(sessions[0].model_id, Some("gpt-4.1-mini".into()));

        std::fs::remove_file(db_path).unwrap();
    }

    #[tokio::test]
    async fn soft_deleted_session_still_exists_in_table() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_1", "/tmp/project")
            .await
            .unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        store
            .upsert_session(&SessionRow {
                session_id: "ses_1".into(),
                workspace_id: "wrk_1".into(),
                title: "To delete".into(),
                model_profile: "fake".into(),
                model_id: None,
                provider: None,
                deleted_at: None,
                created_at: now.clone(),
                updated_at: now,
            })
            .await
            .unwrap();

        store.soft_delete_session("ses_1").await.unwrap();

        let active = store.list_active_sessions("wrk_1").await.unwrap();
        assert!(active.is_empty());

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM kairox_sessions WHERE session_id = 'ses_1'")
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
    }
}
