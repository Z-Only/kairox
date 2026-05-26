//! Task graph projection populated by the agent loop.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};

use super::support::make_tool_runtime;

#[tokio::test]
async fn full_stack_task_graph_populated() {
    let runtime = make_tool_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-task-graph".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "test".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "do something".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let snapshot = runtime.get_task_graph(sid).await.unwrap();
    assert!(
        !snapshot.tasks.is_empty(),
        "Task graph should have at least one task after a message"
    );

    // The root task should be completed or running
    let root = &snapshot.tasks[0];
    assert!(
        matches!(
            root.state,
            agent_core::TaskState::Completed | agent_core::TaskState::Running
        ),
        "Root task should be completed or running, got: {:?}",
        root.state
    );
}
