use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_core::{
    AgentRole, AppFacade, EventPayload, PermissionDecision, SendMessageRequest,
    StartSessionRequest, TaskState,
};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use agent_tools::{
    ApprovalPolicy, SandboxPolicy, Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolRisk,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio::sync::oneshot;

use agent_store::SqliteEventStore;

use super::support::{
    install_streaming_dag_executor, BlockingModelClient, BlockingStreamGate,
    MultiBlockingModelClient,
};
use crate::facade_runtime::LocalRuntime;
use crate::task_graph::TaskGraph;

// Full-workspace test runs can spend several seconds in context assembly and
// actor scheduling before a fake model reaches the tool call that asks for
// permission, especially after dependency cold starts.
const PERMISSION_PENDING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

struct ToolRequestModel;

#[async_trait]
impl ModelClient for ToolRequestModel {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Ok(Box::pin(futures::stream::iter(vec![
            Ok(ModelEvent::ToolCallRequested {
                tool_call_id: "pending-call".into(),
                tool_id: "write-risk".into(),
                arguments: serde_json::json!({}),
            }),
            Ok(ModelEvent::Completed { usage: None }),
        ])))
    }
}

#[derive(Debug, Clone)]
struct ToolThenTextModel {
    call_count: Arc<AtomicUsize>,
}

impl ToolThenTextModel {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for ToolThenTextModel {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let events = if count == 0 {
            vec![
                Ok(ModelEvent::ToolCallRequested {
                    tool_call_id: "pending-call".into(),
                    tool_id: "write-risk".into(),
                    arguments: serde_json::json!({}),
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

struct WriteRiskTool;

#[async_trait]
impl Tool for WriteRiskTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "write-risk".into(),
            description: "test write risk tool".into(),
            required_capability: "test".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::write(invocation.tool_id.clone())
    }

    async fn invoke(&self, _invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        Ok(ToolOutput {
            text: "write-risk executed".into(),
            truncated: false,
        })
    }
}

#[tokio::test]
async fn cancel_session_interrupts_running_single_step_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let runtime = Arc::new(LocalRuntime::new(store, model));

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let turn_runtime = runtime.clone();
    let turn_workspace_id = workspace.workspace_id.clone();
    let turn_session_id = session_id.clone();
    let mut turn = tokio::spawn(async move {
        turn_runtime
            .send_message(SendMessageRequest {
                workspace_id: turn_workspace_id,
                session_id: turn_session_id,
                content: "blocked single-step".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    let completed = tokio::time::timeout(std::time::Duration::from_millis(250), &mut turn).await;
    if completed.is_err() {
        drop(release_tx);
        let _ = turn.await;
        panic!("single-step turn should finish after session cancellation without stream release");
    }

    completed.unwrap().unwrap().unwrap();
    assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

    let graphs = runtime.task_graphs.lock().await;
    let graph = graphs.get(&session_id.to_string()).unwrap();
    let counts = graph.state_counts();
    assert_eq!(counts.running, 0);
    assert_eq!(counts.failed, 0);
    assert_eq!(counts.cancelled, 1);

    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(trace
        .iter()
        .any(|entry| matches!(entry.event.payload, EventPayload::TaskCancelled { .. })));
}

#[tokio::test]
async fn cancel_session_rejects_queued_same_session_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let runtime = Arc::new(LocalRuntime::new(store, model));

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let first_runtime = runtime.clone();
    let first_workspace_id = workspace.workspace_id.clone();
    let first_session_id = session_id.clone();
    let mut first = tokio::spawn(async move {
        first_runtime
            .send_message(SendMessageRequest {
                workspace_id: first_workspace_id,
                session_id: first_session_id,
                content: "first".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    let second_runtime = runtime.clone();
    let second_workspace_id = workspace.workspace_id.clone();
    let second_session_id = session_id.clone();
    let second = tokio::spawn(async move {
        second_runtime
            .send_message(SendMessageRequest {
                workspace_id: second_workspace_id,
                session_id: second_session_id,
                content: "second".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert!(!second.is_finished());
    assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    let first_completed =
        tokio::time::timeout(std::time::Duration::from_millis(250), &mut first).await;
    if first_completed.is_err() {
        drop(release_tx);
        let _ = first.await;
        let _ = second.await;
        panic!("first turn should finish after session cancellation");
    }

    first_completed.unwrap().unwrap().unwrap();
    let second_result = second.await.unwrap();
    assert!(
        matches!(second_result, Err(agent_core::CoreError::InvalidState(ref message)) if message.contains("session execution cancelled")),
        "expected queued turn cancellation error, got {second_result:?}"
    );
    assert_eq!(stream_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancel_session_denies_pending_permission_and_finishes_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = LocalRuntime::new(store, ToolRequestModel)
        .with_approval_and_sandbox(ApprovalPolicy::Always, SandboxPolicy::DangerFullAccess);
    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(WriteRiskTool));
    let runtime = Arc::new(runtime);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "tool-request".into(),
            approval_policy: Some("always".into()),
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let turn_runtime = runtime.clone();
    let turn_workspace_id = workspace.workspace_id.clone();
    let turn_session_id = session_id.clone();
    let mut turn = tokio::spawn(async move {
        turn_runtime
            .send_message(SendMessageRequest {
                workspace_id: turn_workspace_id,
                session_id: turn_session_id,
                content: "request write tool".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });

    let pending_observed = match tokio::time::timeout(PERMISSION_PENDING_TIMEOUT, async {
        loop {
            if runtime
                .pending_permissions
                .lock()
                .await
                .contains_key("pending-call")
            {
                break true;
            }
            if turn.is_finished() {
                break false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    {
        Ok(observed) => observed,
        Err(error) => {
            let trace = runtime.get_trace(session_id.clone()).await.unwrap();
            panic!("permission request should become pending: {error:?}, trace={trace:?}");
        }
    };
    if !pending_observed {
        let trace = runtime.get_trace(session_id.clone()).await.unwrap();
        let turn_result = tokio::time::timeout(std::time::Duration::from_millis(100), &mut turn)
            .await
            .expect("finished turn should be joinable");
        panic!(
            "turn finished before permission became pending: result={turn_result:?}, trace={trace:?}"
        );
    }

    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    let completed = tokio::time::timeout(std::time::Duration::from_millis(500), &mut turn).await;
    assert!(
        completed.is_ok(),
        "turn waiting on permission should finish after cancellation"
    );
    completed.unwrap().unwrap().unwrap();
    assert!(runtime.pending_permissions.lock().await.is_empty());

    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(trace.iter().any(|entry| matches!(
        &entry.event.payload,
        EventPayload::PermissionDenied { request_id, reason }
            if request_id == "pending-call" && reason == "cancelled by user"
    )));
    assert!(trace
        .iter()
        .any(|entry| matches!(entry.event.payload, EventPayload::SessionCancelled { .. })));
}

#[tokio::test]
async fn app_facade_decide_permission_resolves_pending_permission() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = LocalRuntime::new(store, ToolThenTextModel::new())
        .with_approval_and_sandbox(ApprovalPolicy::Always, SandboxPolicy::DangerFullAccess);
    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(WriteRiskTool));
    let runtime = Arc::new(runtime);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "tool-request".into(),
            approval_policy: Some("always".into()),
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let turn_runtime = runtime.clone();
    let turn_workspace_id = workspace.workspace_id.clone();
    let turn_session_id = session_id.clone();
    let mut turn = tokio::spawn(async move {
        turn_runtime
            .send_message(SendMessageRequest {
                workspace_id: turn_workspace_id,
                session_id: turn_session_id,
                content: "request write tool".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });

    tokio::time::timeout(PERMISSION_PENDING_TIMEOUT, async {
        loop {
            if runtime
                .pending_permissions
                .lock()
                .await
                .contains_key("pending-call")
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("permission request should become pending");

    AppFacade::decide_permission(
        runtime.as_ref(),
        PermissionDecision {
            request_id: "pending-call".into(),
            approve: true,
            reason: None,
        },
    )
    .await
    .unwrap();

    let completed = tokio::time::timeout(std::time::Duration::from_millis(500), &mut turn).await;
    assert!(
        completed.is_ok(),
        "turn waiting on permission should finish after facade decision"
    );
    completed.unwrap().unwrap().unwrap();
    assert!(runtime.pending_permissions.lock().await.is_empty());

    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(trace.iter().any(|entry| matches!(
        &entry.event.payload,
        EventPayload::PermissionGranted { request_id } if request_id == "pending-call"
    )));
    assert!(trace.iter().any(|entry| matches!(
        &entry.event.payload,
        EventPayload::ToolInvocationCompleted {
            invocation_id,
            output_preview,
            ..
        } if invocation_id == "pending-call" && output_preview == "write-risk executed"
    )));
}

#[tokio::test]
async fn cancel_session_does_not_cancel_other_running_session() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (first_started_tx, first_started_rx) = oneshot::channel();
    let (first_release_tx, first_release_rx) = oneshot::channel();
    let (second_started_tx, second_started_rx) = oneshot::channel();
    let (second_release_tx, second_release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = MultiBlockingModelClient::new(
        vec![
            BlockingStreamGate {
                started: first_started_tx,
                release: first_release_rx,
                token: "first".into(),
            },
            BlockingStreamGate {
                started: second_started_tx,
                release: second_release_rx,
                token: "second".into(),
            },
        ],
        stream_calls.clone(),
    );
    let runtime = Arc::new(LocalRuntime::new(store, model));

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let first_session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    let second_session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let first_runtime = runtime.clone();
    let first_workspace_id = workspace.workspace_id.clone();
    let first_turn_session = first_session_id.clone();
    let mut first = tokio::spawn(async move {
        first_runtime
            .send_message(SendMessageRequest {
                workspace_id: first_workspace_id,
                session_id: first_turn_session,
                content: "first blocked turn".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    first_started_rx.await.unwrap();

    let second_runtime = runtime.clone();
    let second_workspace_id = workspace.workspace_id.clone();
    let second_turn_session = second_session_id.clone();
    let second = tokio::spawn(async move {
        second_runtime
            .send_message(SendMessageRequest {
                workspace_id: second_workspace_id,
                session_id: second_turn_session,
                content: "second blocked turn".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    second_started_rx.await.unwrap();

    runtime
        .cancel_session(workspace.workspace_id, first_session_id)
        .await
        .unwrap();

    let completed = tokio::time::timeout(std::time::Duration::from_millis(250), &mut first).await;
    if completed.is_err() {
        drop(first_release_tx);
        drop(second_release_tx);
        let _ = first.await;
        let _ = second.await;
        panic!("cancelled session should finish without releasing its model stream");
    }
    completed.unwrap().unwrap().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        !second.is_finished(),
        "cancelling one session must not cancel another running session"
    );

    second_release_tx.send(()).unwrap();
    second.await.unwrap().unwrap();
    assert_eq!(stream_calls.load(Ordering::SeqCst), 2);
    drop(first_release_tx);
}

#[tokio::test]
async fn cancel_session_interrupts_running_dag_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let mut runtime = LocalRuntime::new(store, model);
    install_streaming_dag_executor(&mut runtime).await;
    let runtime = Arc::new(runtime);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let turn_runtime = runtime.clone();
    let turn_workspace_id = workspace.workspace_id.clone();
    let turn_session_id = session_id.clone();
    let mut turn = tokio::spawn(async move {
        turn_runtime
            .send_message(SendMessageRequest {
                workspace_id: turn_workspace_id,
                session_id: turn_session_id,
                content: "/plan blocked dag".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    let completed = tokio::time::timeout(std::time::Duration::from_millis(250), &mut turn).await;
    if completed.is_err() {
        drop(release_tx);
        let _ = turn.await;
        panic!("DAG turn should finish after session cancellation without stream release");
    }

    completed.unwrap().unwrap().unwrap();
    assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

    let graphs = runtime.task_graphs.lock().await;
    let graph = graphs.get(&session_id.to_string()).unwrap();
    let counts = graph.state_counts();
    assert_eq!(counts.running, 0);
    assert!(counts.cancelled > 0);
}

#[tokio::test]
async fn retry_task_queues_behind_active_actor_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let runtime = Arc::new(LocalRuntime::new(store, model).with_dag_execution().await);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let failed_task_id = {
        let mut graph = TaskGraph::default();
        let task_id = graph.add_task("failed task", AgentRole::Worker, vec![]);
        graph.mark_running(&task_id).unwrap();
        graph.mark_failed(&task_id, "boom".into()).unwrap();
        runtime
            .task_graphs
            .lock()
            .await
            .insert(session_id.to_string(), graph);
        task_id
    };

    let first_runtime = runtime.clone();
    let first_workspace_id = workspace.workspace_id.clone();
    let first_session_id = session_id.clone();
    let first = tokio::spawn(async move {
        first_runtime
            .send_message(SendMessageRequest {
                workspace_id: first_workspace_id,
                session_id: first_session_id,
                content: "first".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    let retry_runtime = runtime.clone();
    let retry_workspace_id = workspace.workspace_id;
    let retry_session_id = session_id.clone();
    let retry_task_id = failed_task_id.clone();
    let retry = tokio::spawn(async move {
        retry_runtime
            .retry_task(retry_workspace_id, retry_session_id, retry_task_id)
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert!(
        !retry.is_finished(),
        "retry_task should wait for the actor turn"
    );
    {
        let graphs = runtime.task_graphs.lock().await;
        let graph = graphs.get(&session_id.to_string()).unwrap();
        let task = graph.get_task(&failed_task_id).unwrap();
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.retry_count, 0);
    }

    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    retry.await.unwrap().unwrap();

    let graphs = runtime.task_graphs.lock().await;
    let graph = graphs.get(&session_id.to_string()).unwrap();
    let task = graph.get_task(&failed_task_id).unwrap();
    assert_eq!(task.state, TaskState::Pending);
    assert_eq!(task.retry_count, 1);
}

#[tokio::test]
async fn cancel_task_queues_behind_active_actor_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let runtime = Arc::new(LocalRuntime::new(store, model).with_dag_execution().await);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let pending_task_id = {
        let mut graph = TaskGraph::default();
        let task_id = graph.add_task("pending task", AgentRole::Worker, vec![]);
        runtime
            .task_graphs
            .lock()
            .await
            .insert(session_id.to_string(), graph);
        task_id
    };

    let first_runtime = runtime.clone();
    let first_workspace_id = workspace.workspace_id.clone();
    let first_session_id = session_id.clone();
    let first = tokio::spawn(async move {
        first_runtime
            .send_message(SendMessageRequest {
                workspace_id: first_workspace_id,
                session_id: first_session_id,
                content: "first".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    let cancel_runtime = runtime.clone();
    let cancel_workspace_id = workspace.workspace_id;
    let cancel_session_id = session_id.clone();
    let cancel_task_id = pending_task_id.clone();
    let cancel = tokio::spawn(async move {
        cancel_runtime
            .cancel_task(cancel_workspace_id, cancel_session_id, cancel_task_id)
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert!(
        !cancel.is_finished(),
        "cancel_task should wait for the actor turn"
    );
    {
        let graphs = runtime.task_graphs.lock().await;
        let graph = graphs.get(&session_id.to_string()).unwrap();
        let task = graph.get_task(&pending_task_id).unwrap();
        assert_eq!(task.state, TaskState::Pending);
    }

    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    cancel.await.unwrap().unwrap();

    let graphs = runtime.task_graphs.lock().await;
    let graph = graphs.get(&session_id.to_string()).unwrap();
    let task = graph.get_task(&pending_task_id).unwrap();
    assert_eq!(task.state, TaskState::Cancelled);
}
