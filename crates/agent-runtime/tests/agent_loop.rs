use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{PermissionMode, Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A fake model that returns a tool call on the first request,
/// then a text response on the second request with tool results.
#[derive(Debug, Clone)]
struct ToolCallingModelClient {
    call_count: Arc<AtomicUsize>,
}

impl ToolCallingModelClient {
    fn new() -> Self {
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

/// A simple echo tool for testing
struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "echo".into(),
            description: "Echoes input".into(),
            required_capability: "echo".into(),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> agent_tools::ToolRisk {
        agent_tools::ToolRisk::read("echo")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        Ok(ToolOutput {
            text: format!("echo: {}", invocation.arguments),
            truncated: false,
        })
    }
}

#[tokio::test]
async fn agent_loop_processes_tool_call_and_continues() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = ToolCallingModelClient::new();
    let mut runtime = LocalRuntime::new(store, model);
    runtime = runtime.with_permission_mode(PermissionMode::Agent);

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
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "read something".into(),
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

#[tokio::test]
async fn agent_loop_stops_when_no_tool_calls() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Just a text response".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-no-tools".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].content, "hello");
    assert_eq!(projection.messages[1].content, "Just a text response");
}

#[tokio::test]
async fn subscribe_session_receives_events() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-subscribe".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
        })
        .await
        .unwrap();

    // Collect events from the stream
    let mut received_types = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        match tokio::time::timeout(std::time::Duration::from_millis(100), event_stream.next()).await
        {
            Ok(Some(event)) => {
                received_types.push(event.event_type.to_string());
                if received_types.len() > 10 {
                    break;
                }
            }
            Ok(None) | Err(_) => {
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
            }
        }
    }

    assert!(
        received_types.iter().any(|t| t == "UserMessageAdded"),
        "subscribe_session should receive UserMessageAdded, got: {:?}",
        received_types
    );
}
