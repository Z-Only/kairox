//! Messaging — text-only flows, tool-calling with permission grants, and
//! Agent permission mode auto-approval.

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};

use super::support::{make_simple_runtime, make_tool_runtime};

#[tokio::test]
async fn full_stack_send_message_text_only() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-msg".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "hello world".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert_eq!(proj.messages.len(), 2);
    assert_eq!(proj.messages[0].role, ProjectedRole::User);
    assert_eq!(proj.messages[0].content, "hello world");
    assert_eq!(proj.messages[1].role, ProjectedRole::Assistant);
    assert_eq!(proj.messages[1].content, "Simple response");
}

#[tokio::test]
async fn full_stack_tool_call_with_permission_grant() {
    let runtime = make_tool_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-tool".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "test".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "echo something".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(sid.clone()).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    // With PermissionMode::Agent, permission should be auto-granted
    assert!(
        event_types.contains(&"UserMessageAdded"),
        "Missing UserMessageAdded: {event_types:?}"
    );
    assert!(
        event_types.contains(&"ModelToolCallRequested"),
        "Missing ModelToolCallRequested: {event_types:?}"
    );
    assert!(
        event_types.contains(&"PermissionGranted"),
        "Missing PermissionGranted: {event_types:?}"
    );
    assert!(
        event_types.contains(&"ToolInvocationStarted"),
        "Missing ToolInvocationStarted: {event_types:?}"
    );
    assert!(
        event_types.contains(&"ToolInvocationCompleted"),
        "Missing ToolInvocationCompleted: {event_types:?}"
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted"),
        "Missing AssistantMessageCompleted: {event_types:?}"
    );

    // Final projection should include response after tool execution
    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert!(
        !proj.messages.is_empty(),
        "Should have messages after tool execution"
    );
}

#[tokio::test]
async fn full_stack_agent_mode_completes_without_prompt() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["response".into()]);
    let runtime = LocalRuntime::new(store, model).with_approval_and_sandbox(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );

    let ws = runtime
        .open_workspace("/tmp/test-agent-mode".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // With Agent mode, no permission prompt should block
    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert_eq!(
        proj.messages.len(),
        2,
        "Message should complete without permission prompt"
    );
}
