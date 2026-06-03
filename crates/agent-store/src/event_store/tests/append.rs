use super::super::*;
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
            display_content: None,
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
            display_content: None,
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
