use agent_core::{SendMessageRequest, SessionId, WorkspaceId};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;

use crate::facade_runtime::{ExecutionMode, LocalRuntime};

#[tokio::test]
async fn default_execution_mode_is_single_step() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let request = SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: "hello".into(),
        attachments: vec![],
    };
    assert_eq!(runtime.execution_mode(&request), ExecutionMode::SingleStep);
}

#[tokio::test]
async fn plan_prefix_triggers_dag_mode() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_dag_execution().await;

    let request = SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: "/plan implement feature X".into(),
        attachments: vec![],
    };
    assert_eq!(
        runtime.execution_mode(&request),
        ExecutionMode::DagExecution
    );
}

#[tokio::test]
async fn no_plan_prefix_uses_single_step_even_with_dag() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_dag_execution().await;

    let request = SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: "just a question".into(),
        attachments: vec![],
    };
    assert_eq!(runtime.execution_mode(&request), ExecutionMode::SingleStep);
}
