//! Lifecycle hook execution: session start, pre/post tool, and stop hooks.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};

use crate::tool_calls::ToolCallingModelClient;
use crate::{hook_test_config, EchoTool};

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
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    assert_eq!(
        std::fs::read_to_string(hook_file).expect("session start hook should write file"),
        "session"
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
    runtime = runtime.with_approval_and_sandbox(
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
        .open_workspace("/tmp/test-tool-hooks".into())
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
            approval_policy: None,
            sandbox_policy: None,
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
