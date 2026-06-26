use super::super::*;

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
            title: "New conversation".into(),
            model_profile: "fast".into(),
            model_id: Some("gpt-4.1-mini".into()),
            approval_policy: None,
            sandbox_policy: None,
            provider: Some("openai_compatible".into()),
            deleted_at: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        })
        .await
        .unwrap();

    let sessions = store.list_active_sessions("wrk_1").await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "New conversation");
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
async fn list_active_sessions_returns_empty_for_unknown_workspace() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let sessions = store.list_active_sessions("wrk_nonexistent").await.unwrap();
    assert!(sessions.is_empty());
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
async fn list_archived_sessions_returns_soft_deleted_ordinary_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store
        .upsert_workspace("wrk_1", "/tmp/project")
        .await
        .unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    store
        .upsert_session(&SessionRow {
            session_id: "ses_archived".into(),
            workspace_id: "wrk_1".into(),
            title: "Archived ordinary session".into(),
            model_profile: "ali-mo-claude".into(),
            model_id: Some("claude-opus-4-6".into()),
            provider: Some("ali-mo".into()),
            approval_policy: Some("on_request".into()),
            sandbox_policy: Some("{\"kind\":\"workspace_write\"}".into()),
            deleted_at: None,
            created_at: now.clone(),
            updated_at: now,
        })
        .await
        .unwrap();

    store.soft_delete_session("ses_archived").await.unwrap();

    let archived = store.list_archived_sessions("wrk_1").await.unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].session_id, "ses_archived");
    assert_eq!(archived[0].title, "Archived ordinary session");
    assert_eq!(archived[0].model_profile, "ali-mo-claude");
    assert!(archived[0].deleted_at.is_some());
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
