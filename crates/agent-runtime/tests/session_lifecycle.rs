//! Session lifecycle integration tests for CRUD, persistence, and cleanup.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

/// Helper: create an in-memory runtime for quick tests.
fn make_runtime(store: SqliteEventStore) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    LocalRuntime::new(store, FakeModelClient::new(vec!["response".into()]))
}

/// Helper: create a file-backed SQLite store for persistence tests.
async fn make_file_backed_store() -> (SqliteEventStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!(
        "kairox-session-lifecycle-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let database_url = format!("sqlite://{}", db_path.display());
    let store = SqliteEventStore::connect(&database_url)
        .await
        .expect("failed to connect to file-backed SQLite");
    (store, db_path)
}

// ---------------------------------------------------------------------------
// Test 1: Full workspace → session → message → projection → cancel → trace
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_workspace_session_round_trip() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);

    // Open workspace
    let workspace = runtime
        .open_workspace("/tmp/kairox-round-trip".into())
        .await
        .unwrap();

    // Start session
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Send message
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello agent".into(),
        })
        .await
        .unwrap();

    // Get projection — should have 2 messages (user + assistant)
    let projection = runtime
        .get_session_projection(session_id.clone())
        .await
        .unwrap();
    assert_eq!(
        projection.messages.len(),
        2,
        "expected 2 messages (user + assistant), got {:?}",
        projection
            .messages
            .iter()
            .map(|m| format!("{:?}: {}", m.role, m.content))
            .collect::<Vec<_>>()
    );
    assert_eq!(projection.messages[0].content, "hello agent");
    assert_eq!(projection.messages[1].content, "response");

    // Cancel session
    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    // Verify cancelled flag
    let projection_after_cancel = runtime
        .get_session_projection(session_id.clone())
        .await
        .unwrap();
    assert!(
        projection_after_cancel.cancelled,
        "session should be marked as cancelled"
    );

    // Get trace — should be non-empty
    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(
        !trace.is_empty(),
        "trace should contain events after the round trip"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Session metadata persists across store reconnection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_metadata_persists_across_reopen() {
    let (store, db_path) = make_file_backed_store().await;
    let runtime = make_runtime(store);

    // Open workspace + start session
    let workspace = runtime
        .open_workspace("/tmp/kairox-persist".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Keep copies of IDs for verification after reconnection
    let original_workspace_id = workspace.workspace_id.to_string();
    let original_session_id = session_id.to_string();

    // Drop the store (and runtime) — data is on disk
    drop(runtime);

    // Reconnect to the same database
    let database_url = format!("sqlite://{}", db_path.display());
    let store2 = SqliteEventStore::connect(&database_url)
        .await
        .expect("failed to reconnect to file-backed SQLite");
    let runtime2 = make_runtime(store2);

    // Verify workspace data recovered
    let workspaces = runtime2.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1, "should recover 1 workspace");
    assert_eq!(workspaces[0].workspace_id.as_str(), original_workspace_id);

    // Verify session data recovered
    let wid = agent_core::WorkspaceId::from_string(original_workspace_id);
    let sessions = runtime2.list_sessions(&wid).await.unwrap();
    assert_eq!(sessions.len(), 1, "should recover 1 session");
    assert_eq!(sessions[0].session_id.as_str(), original_session_id);

    // Clean up temp file
    let _ = std::fs::remove_file(&db_path);
}

// ---------------------------------------------------------------------------
// Test 3: Rename and soft-delete flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rename_and_soft_delete_flow() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);

    // Open workspace → start session
    let workspace = runtime
        .open_workspace("/tmp/kairox-rename-delete".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Rename
    runtime
        .rename_session(&session_id, "Renamed Session".into())
        .await
        .unwrap();

    // Verify new title in list
    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Renamed Session");

    // Soft-delete
    runtime.soft_delete_session(&session_id).await.unwrap();

    // Verify list is empty (soft-deleted sessions are excluded from active list)
    let sessions_after_delete = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert!(
        sessions_after_delete.is_empty(),
        "soft-deleted session should not appear in active list"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Multiple sessions in the same workspace
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_sessions_in_same_workspace() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);

    // Open workspace
    let workspace = runtime
        .open_workspace("/tmp/kairox-multi-session".into())
        .await
        .unwrap();

    // Create 3 sessions with different profiles
    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "gpt-4".into(),
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "claude".into(),
        })
        .await
        .unwrap();
    let s3 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "ollama".into(),
        })
        .await
        .unwrap();

    // List returns 3
    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 3, "should have 3 sessions");

    // Delete one
    runtime.soft_delete_session(&s2).await.unwrap();

    // List returns 2
    let sessions_after = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(
        sessions_after.len(),
        2,
        "should have 2 sessions after delete"
    );

    // Verify remaining IDs are correct
    let remaining_ids: Vec<String> = sessions_after
        .iter()
        .map(|s| s.session_id.as_str().to_string())
        .collect();
    assert!(
        remaining_ids.contains(&s1.to_string()),
        "s1 should still be present"
    );
    assert!(
        remaining_ids.contains(&s3.to_string()),
        "s3 should still be present"
    );
    assert!(
        !remaining_ids.contains(&s2.to_string()),
        "s2 should not be present after soft delete"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Cleanup expired removes old sessions and events
// ---------------------------------------------------------------------------

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
        })
        .await
        .unwrap();

    // Send a message (creates events)
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello".into(),
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
