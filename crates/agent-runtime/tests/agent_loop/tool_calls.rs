//! Tool-call processing: permission flow, event sequence, and the loop's
//! ability to feed tool results back into the next model request.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{ModelClient, ModelEvent, ModelMessage, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{
    ApprovalPolicy, SandboxPolicy, Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolRisk,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio::sync::Notify;

use crate::EchoTool;

/// A fake model that returns a tool call on the first request,
/// then a text response on the second request with tool results.
#[derive(Debug, Clone)]
pub(crate) struct ToolCallingModelClient {
    pub(crate) call_count: Arc<AtomicUsize>,
}

impl ToolCallingModelClient {
    pub(crate) fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for ToolCallingModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let events: Vec<agent_models::Result<ModelEvent>> = if count == 0 {
            vec![
                Ok(ModelEvent::TokenDelta("Reading".into())),
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
async fn agent_loop_processes_tool_call_and_continues() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = ToolCallingModelClient::new();
    let mut runtime = LocalRuntime::new(store, model);
    runtime = runtime.with_approval_and_sandbox(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );

    // Register echo tool
    let registry = runtime.tool_registry();
    registry.lock().await.register(Box::new(EchoTool));

    let workspace = runtime
        .open_workspace("/tmp/test-agent-loop".into())
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
            content: "read something".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    let event_types: Vec<String> = trace
        .iter()
        .map(|e| e.event.event_type.to_string())
        .collect();

    assert!(
        event_types.iter().any(|t| t == "UserMessageAdded"),
        "Missing UserMessageAdded: {:?}",
        event_types
    );
    assert!(
        event_types.iter().any(|t| t == "ModelToolCallRequested"),
        "Missing ModelToolCallRequested: {:?}",
        event_types
    );
    assert!(
        event_types.iter().any(|t| t == "PermissionGranted")
            || event_types.iter().any(|t| t == "PermissionDenied"),
        "Missing permission decision: {:?}",
        event_types
    );
    assert!(
        event_types.iter().any(|t| t == "ToolInvocationStarted"),
        "Missing ToolInvocationStarted: {:?}",
        event_types
    );
    assert!(
        event_types.iter().any(|t| t == "ToolInvocationCompleted"),
        "Missing ToolInvocationCompleted: {:?}",
        event_types
    );

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert!(
        !projection.messages.is_empty(),
        "Should have chat messages after agent loop"
    );
}

/// A model client that records the request messages on the second call so we
/// can verify the agent loop feeds tool results into the next model request.
#[derive(Debug, Clone)]
struct TrackingModelClient {
    call_count: Arc<AtomicUsize>,
    second_request_messages: Arc<std::sync::Mutex<Vec<ModelMessage>>>,
}

impl TrackingModelClient {
    fn new() -> (Self, Arc<std::sync::Mutex<Vec<ModelMessage>>>) {
        let messages = Arc::new(std::sync::Mutex::new(Vec::new()));
        (
            Self {
                call_count: Arc::new(AtomicUsize::new(0)),
                second_request_messages: messages.clone(),
            },
            messages,
        )
    }
}

#[async_trait]
impl ModelClient for TrackingModelClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let events: Vec<agent_models::Result<ModelEvent>> = if count == 0 {
            // First call: return a tool call for the echo tool.
            vec![
                Ok(ModelEvent::ToolCallRequested {
                    tool_call_id: "call_1".into(),
                    tool_id: "echo".into(),
                    arguments: serde_json::json!({"text": "hello"}),
                }),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        } else {
            // Second call: capture the request messages so we can verify
            // they include the tool result fed back from the first iteration.
            if let Ok(mut msgs) = self.second_request_messages.lock() {
                *msgs = request.messages.clone();
            }
            vec![
                Ok(ModelEvent::TokenDelta("Done".into())),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

/// After a tool call executes, the next model request must include the tool
/// result so the model can incorporate it in its response.
#[tokio::test]
async fn agent_loop_feeds_tool_results_to_next_model_request() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (model, second_messages) = TrackingModelClient::new();
    // Clone the call-count Arc before model is moved into the runtime.
    let call_count = model.call_count.clone();
    let mut runtime = LocalRuntime::new(store, model);
    runtime = runtime.with_approval_and_sandbox(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );

    // Register the same EchoTool from the existing test above.
    let registry = runtime.tool_registry();
    registry.lock().await.register(Box::new(EchoTool));

    let workspace = runtime
        .open_workspace("/tmp/test-tool-feed".into())
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
            content: "echo something".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    // The model should have been called at least twice (first for tool call,
    // second with tool results).
    let count = call_count.load(Ordering::SeqCst);
    assert!(
        count >= 2,
        "Model should be called at least twice (got {count})"
    );

    // Verify the second request's messages include tool results.
    let has_tool_result = {
        let msgs = second_messages.lock().unwrap();
        let has = msgs.iter().any(|m| m.role == "tool");
        assert!(
            has,
            "Second model request should contain tool result messages. Got: {:?}",
            msgs
        );
        has
    };
    let _ = has_tool_result;

    // Also verify the trace contains the expected tool invocation events.
    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<String> = trace.iter().map(|e| e.event.event_type.clone()).collect();
    assert!(
        event_types.contains(&"ToolInvocationStarted".to_string()),
        "Missing ToolInvocationStarted: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"ToolInvocationCompleted".to_string()),
        "Missing ToolInvocationCompleted: {:?}",
        event_types
    );
}

#[derive(Clone)]
struct SlowToolCallModelClient {
    call_count: Arc<AtomicUsize>,
}

impl SlowToolCallModelClient {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for SlowToolCallModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let events: Vec<agent_models::Result<ModelEvent>> = if count == 0 {
            vec![
                Ok(ModelEvent::ToolCallRequested {
                    tool_call_id: "slow_call".into(),
                    tool_id: "slow".into(),
                    arguments: serde_json::json!({}),
                }),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        } else {
            vec![
                Ok(ModelEvent::TokenDelta(
                    "should not be requested after cancellation".into(),
                )),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

struct SlowTool {
    started: Arc<Notify>,
}

#[async_trait]
impl Tool for SlowTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "slow".into(),
            description: "Sleeps long enough for cancellation".into(),
            required_capability: "slow".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::read("slow")
    }

    async fn invoke(&self, _invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        self.started.notify_waiters();
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(ToolOutput {
            text: "slow completed".into(),
            truncated: false,
            images: vec![],
        })
    }
}

#[tokio::test]
async fn cancelling_turn_fails_running_tool_invocation_promptly() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = SlowToolCallModelClient::new();
    let call_count = model.call_count.clone();
    let mut runtime = LocalRuntime::new(store, model);
    runtime = runtime.with_approval_and_sandbox(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );

    let tool_started = Arc::new(Notify::new());
    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(SlowTool {
            started: tool_started.clone(),
        }));

    let workspace = runtime
        .open_workspace("/tmp/test-cancel-running-tool".into())
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
    let runtime = Arc::new(runtime);
    let send_runtime = runtime.clone();
    let send_workspace_id = workspace.workspace_id.clone();
    let send_session_id = session_id.clone();
    let mut send_task = tokio::spawn(async move {
        send_runtime
            .send_message(SendMessageRequest {
                workspace_id: send_workspace_id,
                session_id: send_session_id,
                content: "call slow tool".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });

    tokio::time::timeout(Duration::from_secs(2), tool_started.notified())
        .await
        .expect("slow tool should start");
    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    let send_result = tokio::time::timeout(Duration::from_millis(700), &mut send_task).await;
    if send_result.is_err() {
        send_task.abort();
        panic!("cancelled turn should not wait for the slow tool to finish");
    }
    send_result.unwrap().unwrap().unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<String> = trace.iter().map(|e| e.event.event_type.clone()).collect();
    assert!(
        event_types.contains(&"ToolInvocationFailed".to_string()),
        "cancelling a running tool should emit ToolInvocationFailed: {event_types:?}"
    );
    assert!(
        !event_types.contains(&"ToolInvocationCompleted".to_string()),
        "cancelled running tool must not complete: {event_types:?}"
    );
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "agent loop should not make a follow-up model call after cancellation"
    );
}
