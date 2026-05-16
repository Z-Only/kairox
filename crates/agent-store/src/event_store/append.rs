use agent_core::{DomainEvent, SessionId};
use sqlx::Row;

use super::SqliteEventStore;

impl SqliteEventStore {
    pub(super) async fn append_event(&self, event: &DomainEvent) -> crate::Result<()> {
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

    pub(super) async fn load_session_events(
        &self,
        session_id: &SessionId,
    ) -> crate::Result<Vec<DomainEvent>> {
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
