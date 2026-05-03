//! Integration tests for task graph event emission and API.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

async fn create_runtime() -> (
    LocalRuntime<SqliteEventStore, FakeModelClient>,
    agent_core::WorkspaceId,
    agent_core::SessionId,
) {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Hello!".into()]);
    let runtime = LocalRuntime::new(store, model);
    let workspace = runtime.open_workspace("/tmp/test".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();
    (runtime, workspace.workspace_id, session_id)
}

#[tokio::test]
async fn plain_message_creates_root_task_completed() {
    let (runtime, ws, session_id) = create_runtime().await;
    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: session_id.clone(),
            content: "hello".into(),
        })
        .await
        .unwrap();

    let snapshot = runtime.get_task_graph(session_id).await.unwrap();
    assert_eq!(snapshot.tasks.len(), 1, "Should have exactly one root task");
    let root = &snapshot.tasks[0];
    assert_eq!(root.role, agent_core::AgentRole::Planner);
    assert_eq!(root.state, agent_core::TaskState::Completed);
    assert!(root.dependencies.is_empty());
    assert!(root.error.is_none());
}

#[tokio::test]
async fn root_task_title_truncates_long_content() {
    let (runtime, ws, session_id) = create_runtime().await;
    let long_content: String = "x".repeat(100);
    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: session_id.clone(),
            content: long_content.clone(),
        })
        .await
        .unwrap();

    let snapshot = runtime.get_task_graph(session_id).await.unwrap();
    let root = &snapshot.tasks[0];
    // Title should be truncated to ~50 chars + "..."
    assert!(
        root.title.len() <= 53,
        "Title should be truncated, got: {} chars",
        root.title.len()
    );
}

#[tokio::test]
async fn get_task_graph_returns_empty_for_unknown_session() {
    let (runtime, _ws, _session_id) = create_runtime().await;
    let unknown = agent_core::SessionId::new();
    let snapshot = runtime.get_task_graph(unknown).await.unwrap();
    assert!(snapshot.tasks.is_empty());
}

#[tokio::test]
async fn task_graph_events_emitted_for_plain_message() {
    let (runtime, ws, session_id) = create_runtime().await;
    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: session_id.clone(),
            content: "test".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let task_events: Vec<&agent_core::EventPayload> = trace
        .iter()
        .filter(|e| {
            matches!(
                &e.event.payload,
                agent_core::EventPayload::AgentTaskCreated { .. }
                    | agent_core::EventPayload::AgentTaskStarted { .. }
                    | agent_core::EventPayload::AgentTaskCompleted { .. }
            )
        })
        .map(|e| &e.event.payload)
        .collect();

    assert!(
        task_events.len() >= 3,
        "Should have at least Created+Started+Completed for root task, got {}",
        task_events.len()
    );
}

#[tokio::test]
async fn multiple_messages_create_multiple_root_tasks() {
    let (runtime, ws, session_id) = create_runtime().await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.clone(),
            session_id: session_id.clone(),
            content: "first".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: session_id.clone(),
            content: "second".into(),
        })
        .await
        .unwrap();

    let snapshot = runtime.get_task_graph(session_id).await.unwrap();
    let roots: Vec<_> = snapshot
        .tasks
        .iter()
        .filter(|t| t.role == agent_core::AgentRole::Planner)
        .collect();
    assert_eq!(roots.len(), 2, "Should have 2 root tasks");
    assert!(roots
        .iter()
        .all(|t| t.state == agent_core::TaskState::Completed));
}
