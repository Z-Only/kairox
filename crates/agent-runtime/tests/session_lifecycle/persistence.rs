//! Verifies workspace and session metadata survives a store reconnection.

use agent_core::{AppFacade, StartSessionRequest};
use agent_store::SqliteEventStore;

use super::support::{make_file_backed_store, make_runtime};

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
            approval_policy: None,
            sandbox_policy: None,
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
