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
            permission_mode: "suggest".to_string(),
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
            permission_mode: "suggest".to_string(),
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
            permission_mode: "suggest".to_string(),
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
            permission_mode: "suggest".to_string(),
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
            permission_mode: "suggest".to_string(),
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
            permission_mode: "suggest".to_string(),
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
async fn cleanup_expired_also_deletes_project_session_metadata() {
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
            permission_mode: "suggest".to_string(),
            deleted_at: Some(old_deleted.to_rfc3339()),
            created_at: now.clone(),
            updated_at: now.clone(),
        })
        .await
        .unwrap();

    let repository = crate::ProjectMetaRepository::new(store.pool().clone());
    let project = repository
        .create_project("wrk_1", "Project", "/tmp/project", 0)
        .await
        .unwrap();
    repository
        .bind_session("ses_old", &project.project_id, "/tmp/project", None)
        .await
        .unwrap();
    repository
        .set_session_visibility("ses_old", "archived")
        .await
        .unwrap();

    let archived_before = repository.list_archived_sessions("wrk_1").await.unwrap();
    assert_eq!(archived_before.len(), 1);

    let deleted = store
        .cleanup_expired_sessions(std::time::Duration::from_secs(7 * 86400))
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    let archived_after = repository.list_archived_sessions("wrk_1").await.unwrap();
    assert!(archived_after.is_empty());
    assert!(repository
        .get_session_binding("ses_old")
        .await
        .unwrap()
        .is_none());
    assert!(repository
        .get_session_visibility("ses_old")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn cleanup_expired_deletes_only_sessions_selected_for_cleanup() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store
        .upsert_workspace("wrk_1", "/tmp/project")
        .await
        .unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let old_deleted = chrono::Utc::now() - chrono::Duration::days(10);
    let recent_deleted = chrono::Utc::now() - chrono::Duration::days(1);
    store
        .upsert_session(&SessionRow {
            session_id: "ses_selected".into(),
            workspace_id: "wrk_1".into(),
            title: "Selected for cleanup".into(),
            model_profile: "fake".into(),
            model_id: None,
            provider: None,
            permission_mode: "suggest".to_string(),
            deleted_at: Some(old_deleted.to_rfc3339()),
            created_at: now.clone(),
            updated_at: now.clone(),
        })
        .await
        .unwrap();
    store
        .upsert_session(&SessionRow {
            session_id: "ses_late".into(),
            workspace_id: "wrk_1".into(),
            title: "Becomes old during cleanup".into(),
            model_profile: "fake".into(),
            model_id: None,
            provider: None,
            permission_mode: "suggest".to_string(),
            deleted_at: Some(recent_deleted.to_rfc3339()),
            created_at: now.clone(),
            updated_at: now.clone(),
        })
        .await
        .unwrap();

    let repository = crate::ProjectMetaRepository::new(store.pool().clone());
    let project = repository
        .create_project("wrk_1", "Project", "/tmp/project", 0)
        .await
        .unwrap();
    repository
        .bind_session("ses_selected", &project.project_id, "/tmp/project", None)
        .await
        .unwrap();
    repository
        .set_session_visibility("ses_selected", "archived")
        .await
        .unwrap();
    repository
        .bind_session("ses_late", &project.project_id, "/tmp/project", None)
        .await
        .unwrap();
    repository
        .set_session_visibility("ses_late", "archived")
        .await
        .unwrap();

    sqlx::query(
        "CREATE TRIGGER mark_late_session_old_after_selected_cleanup
             AFTER DELETE ON kairox_project_sessions
             WHEN OLD.session_id = 'ses_selected'
             BEGIN
               UPDATE kairox_sessions
               SET deleted_at = '2000-01-01T00:00:00+00:00'
               WHERE session_id = 'ses_late';
             END",
    )
    .execute(store.pool())
    .await
    .unwrap();

    let deleted = store
        .cleanup_expired_sessions(std::time::Duration::from_secs(7 * 86400))
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    assert!(repository
        .get_session_binding("ses_selected")
        .await
        .unwrap()
        .is_none());
    assert!(repository
        .get_session_binding("ses_late")
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        repository.get_session_visibility("ses_late").await.unwrap(),
        Some("archived".to_string())
    );

    let late_session_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM kairox_sessions WHERE session_id = 'ses_late'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(late_session_count, 1);
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
            permission_mode: "suggest".to_string(),
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
            permission_mode: "suggest".to_string(),
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
            permission_mode: "suggest".to_string(),
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
                permission_mode: "suggest".to_string(),
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
            permission_mode: "suggest".to_string(),
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

#[tokio::test]
async fn save_and_get_draft() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store
        .upsert_workspace("wrk_1", "/tmp/project")
        .await
        .unwrap();
    store
        .upsert_session(&SessionRow {
            session_id: "ses_1".into(),
            workspace_id: "wrk_1".into(),
            title: "Test".into(),
            model_profile: "fast".into(),
            model_id: None,
            provider: None,
            permission_mode: "suggest".to_string(),
            deleted_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        })
        .await
        .unwrap();

    // Get draft for non-existent session returns empty
    let draft = store.get_draft("ses_nonexistent").await.unwrap();
    assert_eq!(draft, "");

    // Save draft
    store.save_draft("ses_1", "hello world").await.unwrap();

    // Get draft returns saved text
    let draft = store.get_draft("ses_1").await.unwrap();
    assert_eq!(draft, "hello world");

    // Overwrite draft
    store.save_draft("ses_1", "updated").await.unwrap();
    let draft = store.get_draft("ses_1").await.unwrap();
    assert_eq!(draft, "updated");

    // Clear draft
    store.save_draft("ses_1", "").await.unwrap();
    let draft = store.get_draft("ses_1").await.unwrap();
    assert_eq!(draft, "");
}

#[tokio::test]
async fn list_active_sessions_returns_most_recent_first() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store
        .upsert_workspace("wrk_1", "/tmp/project")
        .await
        .unwrap();

    let now = chrono::Utc::now();
    let old = (now - chrono::Duration::hours(1)).to_rfc3339();
    let recent = now.to_rfc3339();

    store
        .upsert_session(&SessionRow {
            session_id: "ses_old".into(),
            workspace_id: "wrk_1".into(),
            title: "Old".into(),
            model_profile: "fast".into(),
            model_id: None,
            provider: None,
            permission_mode: "suggest".to_string(),
            deleted_at: None,
            created_at: old.clone(),
            updated_at: old,
        })
        .await
        .unwrap();
    store
        .upsert_session(&SessionRow {
            session_id: "ses_recent".into(),
            workspace_id: "wrk_1".into(),
            title: "Recent".into(),
            model_profile: "fast".into(),
            model_id: None,
            provider: None,
            permission_mode: "suggest".to_string(),
            deleted_at: None,
            created_at: recent.clone(),
            updated_at: recent,
        })
        .await
        .unwrap();

    let sessions = store.list_active_sessions("wrk_1").await.unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].session_id, "ses_recent");
    assert_eq!(sessions[1].session_id, "ses_old");
}
