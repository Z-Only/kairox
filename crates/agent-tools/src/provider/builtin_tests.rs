use super::*;
use crate::registry::ToolInvocation;
use agent_core::events::EventPayload;
use agent_core::{SessionId, WorkspaceId};
use std::time::Duration;

#[tokio::test]
async fn builtin_provider_lists_all_tools() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tools = provider.list_tools().await;
    let tool_ids: Vec<&str> = tools.iter().map(|t| t.tool_id.as_str()).collect();
    assert!(
        tool_ids.contains(&"shell.exec"),
        "missing shell.exec, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"search.ripgrep"),
        "missing search.ripgrep, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"patch.apply"),
        "missing patch.apply, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"fs.read"),
        "missing fs.read, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"fs.write"),
        "missing fs.write, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"fs.list"),
        "missing fs.list, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"monitor.start"),
        "missing monitor.start, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"monitor.stop"),
        "missing monitor.stop, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"monitor.list"),
        "missing monitor.list, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"browser.action"),
        "missing browser.action, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"browser.batch"),
        "missing browser.batch, got: {:?}",
        tool_ids
    );
    assert!(
        tool_ids.contains(&"computer.use"),
        "missing computer.use, got: {:?}",
        tool_ids
    );
    assert_eq!(tools.len(), 12);
}

#[tokio::test]
async fn builtin_provider_gets_tool_by_id() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tool = provider.get_tool("shell.exec").await;
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().definition().tool_id, "shell.exec");
}

#[tokio::test]
async fn builtin_provider_returns_none_for_unknown() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tool = provider.get_tool("nonexistent").await;
    assert!(tool.is_none());
}

#[tokio::test]
async fn builtin_provider_gets_browser_action() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tool = provider.get_tool("browser.action").await;
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().definition().tool_id, "browser.action");
}

#[tokio::test]
async fn builtin_provider_gets_browser_batch() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tool = provider.get_tool("browser.batch").await;
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().definition().tool_id, "browser.batch");
}

#[tokio::test]
async fn builtin_provider_gets_computer_use() {
    let provider = BuiltinProvider::with_defaults(PathBuf::from("/tmp"));
    let tool = provider.get_tool("computer.use").await;
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().definition().tool_id, "computer.use");
}

#[tokio::test]
async fn workspace_scoped_builtin_tools_share_monitor_registry_per_root() {
    let temp = tempfile::tempdir().unwrap();
    let workspace_root = std::fs::canonicalize(temp.path()).unwrap();
    let (event_tx, mut event_rx) = tokio::sync::broadcast::channel(64);
    let tools = WorkspaceScopedBuiltinTools::new(event_tx);
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let start_tool = tools
        .tool("monitor.start", workspace_root.clone())
        .expect("monitor.start should be workspace-scoped");
    let start_output = start_tool
        .invoke(ToolInvocation {
            tool_id: "monitor.start".into(),
            arguments: serde_json::json!({
                "description": "workspace cwd",
                "command": "pwd > monitor-cwd.txt; printf 'ready\\n'; sleep 60",
                "persistent": true,
            }),
            workspace_id: workspace_id.to_string(),
            session_id: session_id.to_string(),
            preview: "monitor.start".into(),
            timeout_ms: 30_000,
            output_limit_bytes: 1024,
        })
        .await
        .unwrap();
    let monitor_id = start_output
        .text
        .strip_prefix("Monitor started: ")
        .unwrap()
        .trim()
        .to_string();

    let mut saw_ready = false;
    for _ in 0..8 {
        let event = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .unwrap()
            .unwrap();
        if matches!(
            event.payload,
            EventPayload::MonitorEvent { ref line, .. } if line == "ready"
        ) {
            saw_ready = true;
            break;
        }
    }
    assert!(saw_ready, "monitor command did not report readiness");

    let observed_cwd = std::fs::read_to_string(workspace_root.join("monitor-cwd.txt")).unwrap();
    assert_eq!(observed_cwd.trim(), workspace_root.display().to_string());

    let list_tool = tools
        .tool("monitor.list", workspace_root.clone())
        .expect("monitor.list should share the workspace registry");
    let list_output = list_tool
        .invoke(ToolInvocation {
            tool_id: "monitor.list".into(),
            arguments: serde_json::json!({}),
            workspace_id: workspace_id.to_string(),
            session_id: session_id.to_string(),
            preview: "monitor.list".into(),
            timeout_ms: 30_000,
            output_limit_bytes: 1024,
        })
        .await
        .unwrap();
    assert!(
        list_output.text.contains(&monitor_id),
        "monitor.list should include the monitor created by monitor.start"
    );

    let stop_tool = tools
        .tool("monitor.stop", workspace_root)
        .expect("monitor.stop should share the workspace registry");
    stop_tool
        .invoke(ToolInvocation {
            tool_id: "monitor.stop".into(),
            arguments: serde_json::json!({ "monitor_id": monitor_id }),
            workspace_id: workspace_id.to_string(),
            session_id: session_id.to_string(),
            preview: "monitor.stop".into(),
            timeout_ms: 30_000,
            output_limit_bytes: 1024,
        })
        .await
        .unwrap();
    tools.stop_all_monitors().await;
}
