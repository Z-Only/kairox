use super::super::*;
use agent_core::{AgentId, EventPayload, PrivacyClassification, SessionId, WorkspaceId};

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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
