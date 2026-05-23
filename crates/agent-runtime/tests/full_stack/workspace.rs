//! Workspace management.

use agent_core::AppFacade;

use super::support::make_simple_runtime;

#[tokio::test]
async fn full_stack_open_workspace_returns_info() {
    let runtime = make_simple_runtime().await;
    let ws = runtime.open_workspace("/tmp/test-ws".into()).await.unwrap();

    assert!(
        ws.workspace_id.as_str().starts_with("wrk_"),
        "Workspace ID should have wrk_ prefix, got: {}",
        ws.workspace_id.as_str()
    );
    assert_eq!(ws.path, "/tmp/test-ws");

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, ws.workspace_id);
}

#[tokio::test]
async fn full_stack_multiple_workspaces_are_independent() {
    let runtime = make_simple_runtime().await;

    let ws1 = runtime.open_workspace("/tmp/ws1".into()).await.unwrap();
    let ws2 = runtime.open_workspace("/tmp/ws2".into()).await.unwrap();

    assert_ne!(ws1.workspace_id, ws2.workspace_id);

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 2);
}
