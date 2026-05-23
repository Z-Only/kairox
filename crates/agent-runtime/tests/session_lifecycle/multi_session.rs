//! Multi-session listing and selective soft-delete behavior in one workspace.

use agent_core::{AppFacade, StartSessionRequest};
use agent_store::SqliteEventStore;

use super::support::make_runtime;

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

            permission_mode: None,
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "claude".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    let s3 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "ollama".into(),

            permission_mode: None,
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
