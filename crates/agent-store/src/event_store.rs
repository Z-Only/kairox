use agent_core::{DomainEvent, SessionId};
use async_trait::async_trait;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use std::time::Duration;

#[async_trait]
/// Trait for the append-only event store.
///
/// Events are stored in the order they are appended and can be replayed
/// per session. The canonical implementation is [`SqliteEventStore`].
pub trait EventStore: Send + Sync {
    /// Append a domain event to the store.
    async fn append(&self, event: &DomainEvent) -> crate::Result<()>;
    /// Load all events for a session in append order.
    async fn load_session(&self, session_id: &SessionId) -> crate::Result<Vec<DomainEvent>>;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AgentId, EventPayload, PrivacyClassification, WorkspaceId};
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
}
