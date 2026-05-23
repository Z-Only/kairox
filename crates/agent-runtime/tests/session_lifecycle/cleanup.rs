//! Cleanup of soft-deleted sessions and their event history.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_store::SqliteEventStore;

use super::support::{make_file_backed_store, make_runtime};

#[tokio::test]
async fn cleanup_expired_removes_old_sessions_and_events() {
    let (store, db_path) = make_file_backed_store().await;
    let runtime = make_runtime(store);

    // Open workspace + start session
    let workspace = runtime
        .open_workspace("/tmp/kairox-cleanup".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    // Send a message (creates events)
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Verify trace is non-empty before delete
    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    assert!(!trace.is_empty(), "trace should have events before cleanup");

    // Soft-delete the session
    runtime.soft_delete_session(&session_id).await.unwrap();

    // Drop the runtime, reopen the store to call cleanup
    drop(runtime);

    let database_url = format!("sqlite://{}", db_path.display());
    let store2 = SqliteEventStore::connect(&database_url)
        .await
        .expect("failed to reconnect for cleanup");
    let runtime2 = make_runtime(store2);

    // cleanup_expired_sessions with Duration::from_secs(0) forces cleanup
    // of all soft-deleted sessions (deleted_at is always in the past)
    let deleted = runtime2
        .cleanup_expired_sessions(std::time::Duration::from_secs(0))
        .await
        .unwrap();
    assert!(
        deleted >= 1,
        "should have cleaned up at least 1 session, got {}",
        deleted
    );

    // Verify the session is gone from the store
    let sessions = runtime2
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert!(
        sessions.is_empty(),
        "session should be fully removed after cleanup"
    );

    // Clean up temp file
    let _ = std::fs::remove_file(&db_path);
}
