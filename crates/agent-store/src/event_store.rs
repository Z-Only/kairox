use agent_core::{DomainEvent, SessionId};
use async_trait::async_trait;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, event: &DomainEvent) -> crate::Result<()>;
    async fn load_session(&self, session_id: &SessionId) -> crate::Result<Vec<DomainEvent>>;
}

#[derive(Clone)]
pub struct SqliteEventStore {
    pool: SqlitePool,
}

impl SqliteEventStore {
    pub async fn in_memory() -> crate::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> crate::Result<()> {
        sqlx::query(include_str!("../migrations/0001_events.sql"))
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
        .bind(serde_json::to_string(&event.source_agent_id)?)
        .bind(serde_json::to_string(&event.privacy)?)
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

    #[tokio::test]
    async fn appends_and_replays_session_events_in_order() {
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
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].event_type, "UserMessageAdded");
        assert_eq!(replayed[1].event_type, "AssistantMessageCompleted");
    }
}
