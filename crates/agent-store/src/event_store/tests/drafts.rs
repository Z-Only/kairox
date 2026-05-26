use super::super::*;

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
            approval_policy: None,
            sandbox_policy: None,
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
