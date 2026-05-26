use super::super::*;
use agent_core::{AgentId, EventPayload, PrivacyClassification, SessionId, WorkspaceId};

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
                permission_mode: "suggest".to_string(),
                approval_policy: None,
                sandbox_policy: None,
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
