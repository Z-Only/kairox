use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelMessage, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::{EventStore, SqliteEventStore};
use agent_tools::{PermissionMode, Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn hook_test_config(hooks: Vec<agent_config::HookConfig>) -> Arc<agent_config::Config> {
    Arc::new(agent_config::Config {
        profiles: vec![],
        mcp_servers: vec![],
        source: agent_config::ConfigSource::Defaults,
        context: agent_config::ContextPolicy::default(),
        disabled_mcp_servers: vec![],
        instructions: None,
        features: agent_config::FeatureFlags { hooks: true },
        hooks,
    })
}

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
            parameters: serde_json::json!({"type": "object"}),
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
async fn start_session_runs_session_start_hook() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["unused".into()]);
    let hook_dir = tempfile::TempDir::new().expect("temp dir");
    let hook_file = hook_dir.path().join("session-start-hook.txt");
    let runtime = LocalRuntime::new(store, model).with_config(hook_test_config(vec![
        agent_config::HookConfig {
            id: "session_start_capture".into(),
            event: agent_config::HookEvent::SessionStart,
            matcher: Some("*".into()),
            command: format!("printf session > {}", hook_file.display()),
            status_message: None,
            timeout_secs: Some(5),
            enabled: true,
        },
    ]));

    let workspace = runtime
        .open_workspace("/tmp/test-session-start-hook".into())
        .await
        .unwrap();
    runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id,
            model_profile: "fake".into(),
            permission_mode: None,
        })
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(hook_file).expect("session start hook should write file"),
        "session"
    );
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

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "read something".into(),
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

#[tokio::test]
async fn agent_loop_runs_pre_and_post_tool_hooks() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = ToolCallingModelClient::new();
    let hook_dir = tempfile::TempDir::new().expect("temp dir");
    let hook_file = hook_dir.path().join("tool-hooks.txt");
    let mut runtime = LocalRuntime::new(store, model).with_config(hook_test_config(vec![
        agent_config::HookConfig {
            id: "pre_echo".into(),
            event: agent_config::HookEvent::PreToolUse,
            matcher: Some("echo".into()),
            command: format!("printf pre > {}", hook_file.display()),
            status_message: None,
            timeout_secs: Some(5),
            enabled: true,
        },
        agent_config::HookConfig {
            id: "post_echo".into(),
            event: agent_config::HookEvent::PostToolUse,
            matcher: Some("echo".into()),
            command: format!("printf post >> {}", hook_file.display()),
            status_message: None,
            timeout_secs: Some(5),
            enabled: true,
        },
    ]));
    runtime = runtime.with_permission_mode(PermissionMode::Agent);
    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(EchoTool));

    let workspace = runtime
        .open_workspace("/tmp/test-tool-hooks".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id,
            content: "read something".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(hook_file).expect("tool hooks should write file"),
        "prepost"
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

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].content, "hello");
    assert_eq!(projection.messages[1].content, "Just a text response");
}

#[tokio::test]
async fn agent_loop_runs_stop_hook_after_text_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Just a text response".into()]);
    let hook_dir = tempfile::TempDir::new().expect("temp dir");
    let hook_file = hook_dir.path().join("stop-hook.txt");
    let runtime = LocalRuntime::new(store, model).with_config(hook_test_config(vec![
        agent_config::HookConfig {
            id: "stop_capture".into(),
            event: agent_config::HookEvent::Stop,
            matcher: Some("*".into()),
            command: format!("printf stop > {}", hook_file.display()),
            status_message: None,
            timeout_secs: Some(5),
            enabled: true,
        },
    ]));

    let workspace = runtime
        .open_workspace("/tmp/test-stop-hook".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id,
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(hook_file).expect("stop hook should write file"),
        "stop"
    );
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

            permission_mode: None,
        })
        .await
        .unwrap();

    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
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

// ---------------------------------------------------------------------------
// Task 7: additional agent-loop tests
// ---------------------------------------------------------------------------

/// Verify MAX_AGENT_LOOP_ITERATIONS is a reasonable value — the constant
/// guards against infinite loops, so it must be positive and bounded.
#[test]
#[allow(clippy::assertions_on_constants)]
fn max_agent_loop_iterations_is_reasonable() {
    use agent_runtime::agent_loop::MAX_AGENT_LOOP_ITERATIONS;
    assert!(MAX_AGENT_LOOP_ITERATIONS > 0);
    assert!(MAX_AGENT_LOOP_ITERATIONS <= 100);
}

/// Verify the exact event sequence for a simple (non-tool-call) completion.
/// Key events must appear in the expected relative order.
#[tokio::test]
async fn agent_loop_emits_completion_event_sequence() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Short reply".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-event-seq".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<String> = trace.iter().map(|e| e.event.event_type.clone()).collect();

    // Verify key events exist
    assert!(
        event_types.contains(&"UserMessageAdded".to_string()),
        "Missing UserMessageAdded: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"ModelTokenDelta".to_string()),
        "Missing ModelTokenDelta: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted".to_string()),
        "Missing AssistantMessageCompleted: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"AgentTaskCompleted".to_string()),
        "Missing AgentTaskCompleted: {:?}",
        event_types
    );

    // Verify expected relative order
    let user_pos = event_types
        .iter()
        .position(|t| t == "UserMessageAdded")
        .unwrap();
    let assistant_pos = event_types
        .iter()
        .position(|t| t == "AssistantMessageCompleted")
        .unwrap();
    let completed_pos = event_types
        .iter()
        .position(|t| t == "AgentTaskCompleted")
        .unwrap();

    assert!(
        user_pos < assistant_pos,
        "UserMessageAdded should come before AssistantMessageCompleted"
    );
    assert!(
        assistant_pos < completed_pos,
        "AssistantMessageCompleted should come before AgentTaskCompleted"
    );
}

/// A model client that always returns an error from `stream()`.
struct FailingModelClient;

#[async_trait]
impl ModelClient for FailingModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Err(agent_models::ModelError::Request("model failure".into()))
    }
}

/// When the model returns an error, the agent loop must emit failure events to
/// the store AND propagate the error to the caller via `InvalidState`.
#[tokio::test]
async fn agent_loop_handles_model_error_gracefully() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FailingModelClient;
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-model-error".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    let result = runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await;

    // The error MUST propagate to the caller.
    assert!(
        result.is_err(),
        "send_message should return Err when model fails"
    );
    match result {
        Err(agent_core::CoreError::InvalidState(msg)) => {
            assert!(
                msg.contains("model failure"),
                "Error message should mention the failure: {msg}"
            );
        }
        other => panic!("Expected InvalidState, got {other:?}"),
    }

    // Verify failure events were emitted to the store (events are appended
    // BEFORE the error is returned).
    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let has_user_msg = events.iter().any(|e| e.event_type == "UserMessageAdded");
    assert!(has_user_msg, "Store should contain UserMessageAdded");

    let has_failed = events.iter().any(|e| e.event_type == "AgentTaskFailed");
    assert!(has_failed, "Store should contain AgentTaskFailed event");
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
    runtime = runtime.with_permission_mode(PermissionMode::Agent);

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

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "echo something".into(),
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
