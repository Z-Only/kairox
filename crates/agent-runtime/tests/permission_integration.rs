// Integration tests for permission-mode behaviour in the agent loop.
use agent_core::{AppFacade, EventPayload, SendMessageRequest, StartSessionRequest};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::{EventStore, SqliteEventStore};
use agent_tools::{
    ApprovalPolicy, SandboxPolicy, Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolRisk,
};
use async_trait::async_trait;
use futures::stream::BoxStream;

// ---------------------------------------------------------------------------
// Helper: a model client that requests the "fs.write" tool on every call.
// ---------------------------------------------------------------------------
struct WriteToolCallingModelClient;

#[async_trait]
impl ModelClient for WriteToolCallingModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let events: Vec<agent_models::Result<ModelEvent>> = vec![
            Ok(ModelEvent::TokenDelta("Writing".into())),
            Ok(ModelEvent::ToolCallRequested {
                tool_call_id: "call_write_1".into(),
                tool_id: "fs.write".into(),
                arguments: serde_json::json!({"path": "/tmp/out.txt", "content": "data"}),
            }),
            Ok(ModelEvent::Completed { usage: None }),
        ];
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

// ---------------------------------------------------------------------------
// Helper: a write-classified tool registered for testing.
// ---------------------------------------------------------------------------
struct FsWriteTool;

#[async_trait]
impl Tool for FsWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "fs.write".into(),
            description: "Writes content to a file".into(),
            required_capability: "fs.write".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::write("fs.write")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        Ok(ToolOutput {
            text: format!("wrote: {}", invocation.arguments),
            truncated: false,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// In ReadOnly mode, the permission engine must deny the write-classified tool
/// and emit a PermissionDenied event in the trace.
#[tokio::test]
async fn permission_mode_restricts_write_tool() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = WriteToolCallingModelClient;
    let mut runtime = LocalRuntime::new(store, model);
    runtime = runtime.with_approval_and_sandbox(ApprovalPolicy::Never, SandboxPolicy::ReadOnly);

    let registry = runtime.tool_registry();
    registry.lock().await.register(Box::new(FsWriteTool));

    let workspace = runtime
        .open_workspace("/tmp/test-perm-write".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "write a file".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<String> = trace.iter().map(|e| e.event.event_type.clone()).collect();

    assert!(
        event_types.contains(&"PermissionDenied".to_string()),
        "Expected PermissionDenied in ReadOnly mode for write tool. Got: {:?}",
        event_types
    );

    // The write tool must NOT be invoked when permission is denied.
    assert!(
        !event_types.contains(&"ToolInvocationCompleted".to_string()),
        "Write tool should not be invoked in ReadOnly mode"
    );

    // The loop should still finish normally (no error propagated).
    assert!(
        event_types.contains(&"AssistantMessageCompleted".to_string()),
        "Loop should finish with AssistantMessageCompleted despite denied tool. Got: {:?}",
        event_types
    );
}

/// After sending a message, loading the session events must preserve the user
/// message content verbatim.
#[tokio::test]
async fn session_restore_preserves_messages() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = agent_models::FakeModelClient::new(vec!["reply".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-restore".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let content = "hello world from test";
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: content.into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Load the full event log and verify the user message content is intact.
    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();

    let user_msg = events
        .iter()
        .find(|e| e.event_type == "UserMessageAdded")
        .expect("UserMessageAdded event must be present");

    match &user_msg.payload {
        EventPayload::UserMessageAdded {
            content: msg_content,
            ..
        } => {
            assert_eq!(msg_content, "hello world from test");
        }
        other => panic!("Expected UserMessageAdded payload, got {other:?}"),
    }

    // Also verify the assistant reply is preserved.
    let has_assistant = events
        .iter()
        .any(|e| e.event_type == "AssistantMessageCompleted");
    assert!(has_assistant, "Session should contain an assistant reply");

    // The projection should also reflect both messages.
    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert!(
        !projection.messages.is_empty(),
        "Projection should have messages"
    );
}
