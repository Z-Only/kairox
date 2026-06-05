//! Messaging — text-only flows, tool-calling with permission grants, and
//! Agent permission mode auto-approval.

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::fs;

use super::support::{make_simple_runtime, make_tool_runtime, PatchThenTextModel};

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
            display_content: None,
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
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(sid.clone()).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    // With OnRequest + WorkspaceWrite, this non-destructive tool should be auto-granted.
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
async fn project_session_patch_apply_uses_project_worktree_root() {
    let gui_root = tempfile::tempdir().expect("gui root");
    let project_root = tempfile::tempdir().expect("project root");
    let project_file = project_root.path().join("selftest.txt");
    fs::write(&project_file, "alpha\ntarget\nomega\n").expect("write project file");

    let patch = "\
--- a/selftest.txt
+++ b/selftest.txt
@@ -1,3 +1,3 @@
 alpha
-target
+target-edited
 omega
";

    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = PatchThenTextModel::new(patch);
    let runtime = LocalRuntime::new(store, model)
        .with_approval_and_sandbox(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
        .with_builtin_tools(gui_root.path().to_path_buf())
        .await;

    let workspace = runtime
        .open_workspace(gui_root.path().display().to_string())
        .await
        .unwrap();
    let project = runtime
        .add_existing_project(
            workspace.workspace_id.clone(),
            project_root.path().display().to_string(),
        )
        .await
        .unwrap();
    let session_id = runtime
        .create_project_draft_session(project.project_id)
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "patch the project file".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    assert_eq!(
        fs::read_to_string(project_file).expect("read patched project file"),
        "alpha\ntarget-edited\nomega\n"
    );
    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<&str> = trace
        .iter()
        .map(|entry| entry.event.event_type.as_str())
        .collect();
    assert!(
        event_types.contains(&"ToolInvocationCompleted"),
        "expected patch.apply to complete, got {event_types:?}"
    );
    assert!(
        !event_types.contains(&"ToolInvocationFailed"),
        "patch.apply should not resolve against the GUI root"
    );
}

#[tokio::test]
async fn project_session_monitor_start_uses_project_worktree_root() {
    let gui_root = tempfile::tempdir().expect("gui root");
    let project_root = tempfile::tempdir().expect("project root");
    let gui_root = std::fs::canonicalize(gui_root.path()).expect("canonical gui root");
    let project_root = std::fs::canonicalize(project_root.path()).expect("canonical project root");
    let cwd_file = project_root.join("monitor-cwd.txt");

    let model = FakeModelClient::new(vec!["Monitor started".into()]).with_tool_call_for(
        "monitor.start",
        serde_json::json!({
            "description": "project monitor cwd",
            "command": "pwd > monitor-cwd.txt; printf 'ready\\n'; sleep 60",
            "persistent": true,
        }),
    );
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = LocalRuntime::new(store, model)
        .with_approval_and_sandbox(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
        .with_builtin_tools(gui_root.clone())
        .await;

    let workspace = runtime
        .open_workspace(gui_root.display().to_string())
        .await
        .unwrap();
    let project = runtime
        .add_existing_project(
            workspace.workspace_id.clone(),
            project_root.display().to_string(),
        )
        .await
        .unwrap();
    let session_id = runtime
        .create_project_draft_session(project.project_id)
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "start the project monitor".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    for _ in 0..20 {
        if cwd_file.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(
        fs::read_to_string(&cwd_file)
            .expect("read monitor cwd marker")
            .trim(),
        project_root.display().to_string()
    );
    assert!(
        !gui_root.join("monitor-cwd.txt").exists(),
        "monitor.start should not resolve relative paths against the GUI root"
    );

    let registry = runtime
        .monitor_registry()
        .expect("runtime should expose the shared monitor registry");
    let monitors = registry.list().await;
    assert!(
        monitors
            .iter()
            .any(|monitor| monitor.description == "project monitor cwd"),
        "shared monitor registry should list the workspace-scoped monitor"
    );
    let monitor_id = monitors
        .iter()
        .find(|monitor| monitor.description == "project monitor cwd")
        .expect("project monitor should be listed")
        .monitor_id
        .clone();

    // Wait for stdout (`ready\n`) to be captured and persisted as MonitorEvent
    // before stopping. Under instrumented code (coverage) the persistence lag
    // can exceed the CWD-file poll window, causing a race.
    for _ in 0..40 {
        let trace = runtime.get_trace(session_id.clone()).await.unwrap();
        if trace.iter().any(|e| e.event.event_type == "MonitorEvent") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    registry.stop(&monitor_id).await.unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<&str> = trace
        .iter()
        .map(|entry| entry.event.event_type.as_str())
        .collect();
    assert!(
        event_types.contains(&"ToolInvocationCompleted"),
        "expected monitor.start to complete, got {event_types:?}"
    );
    assert!(
        !event_types.contains(&"ToolInvocationFailed"),
        "monitor.start should not fail for project sessions"
    );
    assert!(
        event_types.contains(&"MonitorStarted"),
        "monitor.start should persist MonitorStarted, got {event_types:?}"
    );
    assert!(
        event_types.contains(&"MonitorEvent"),
        "monitor stdout should persist MonitorEvent, got {event_types:?}"
    );
    assert!(
        event_types.contains(&"MonitorStopped"),
        "monitor.stop should persist MonitorStopped, got {event_types:?}"
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
            display_content: None,
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
