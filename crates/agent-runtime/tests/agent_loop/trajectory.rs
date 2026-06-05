use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::EchoTool;

#[derive(Debug, Clone)]
struct ToolCallingModel {
    call_count: Arc<AtomicUsize>,
}

impl ToolCallingModel {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for ToolCallingModel {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let events: Vec<agent_models::Result<ModelEvent>> = if count == 0 {
            vec![
                Ok(ModelEvent::TokenDelta("Working".into())),
                Ok(ModelEvent::ToolCallRequested {
                    tool_call_id: "call_1".into(),
                    tool_id: "echo".into(),
                    arguments: serde_json::json!({"text": "hello"}),
                }),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        } else {
            vec![
                Ok(ModelEvent::TokenDelta("Done".into())),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

#[tokio::test]
async fn trajectory_events_emitted_on_tool_call_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = ToolCallingModel::new();
    let runtime = LocalRuntime::new(store, model)
        .with_approval_and_sandbox(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
        .with_trajectory_store_from_pool()
        .await;

    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(EchoTool));

    let workspace = runtime
        .open_workspace("/tmp/test-trajectory".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "do something".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        event_types.contains(&"TrajectoryStarted"),
        "missing TrajectoryStarted: {event_types:?}"
    );
    assert!(
        event_types.contains(&"TrajectoryStepRecorded"),
        "missing TrajectoryStepRecorded: {event_types:?}"
    );
    assert!(
        event_types.contains(&"TrajectoryCompleted"),
        "missing TrajectoryCompleted: {event_types:?}"
    );

    let trajectories = runtime.list_trajectories(session_id.clone()).await.unwrap();
    assert_eq!(trajectories.len(), 1, "should have one trajectory");
    let meta = &trajectories[0];
    assert_eq!(meta.step_count, 1);
    assert_eq!(
        meta.outcome,
        agent_core::TrajectoryOutcome::Success,
        "outcome should be success"
    );

    let steps = runtime
        .get_trajectory_steps(meta.trajectory_id.clone())
        .await
        .unwrap();
    assert_eq!(steps.len(), 1, "should have one step");
    assert_eq!(steps[0].action, "echo");
}

#[tokio::test]
async fn no_trajectory_when_store_not_configured() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = ToolCallingModel::new();
    let runtime = LocalRuntime::new(store, model).with_approval_and_sandbox(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );

    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(EchoTool));

    let workspace = runtime
        .open_workspace("/tmp/test-no-traj".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "do something".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        !event_types.contains(&"TrajectoryStarted"),
        "should not emit trajectory events without store: {event_types:?}"
    );
}
