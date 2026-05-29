use super::*;
use std::path::PathBuf;

fn test_registry() -> Arc<MonitorRegistry> {
    let (tx, _) = tokio::sync::broadcast::channel(64);
    Arc::new(MonitorRegistry::new(PathBuf::from("/tmp"), tx))
}

#[test]
fn monitor_start_tool_has_read_risk() {
    let tool = MonitorStartTool::new(test_registry());
    let invocation = ToolInvocation {
        tool_id: MONITOR_START_TOOL_ID.into(),
        arguments: serde_json::json!({"command": "echo hi", "description": "test"}),
        workspace_id: "wrk_test".into(),
        preview: "echo hi".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let risk = tool.risk(&invocation);
    assert_eq!(risk.tool_id, MONITOR_START_TOOL_ID);
}

#[test]
fn monitor_stop_tool_has_read_risk() {
    let tool = MonitorStopTool::new(test_registry());
    let invocation = ToolInvocation {
        tool_id: MONITOR_STOP_TOOL_ID.into(),
        arguments: serde_json::json!({"monitor_id": "mon_1"}),
        workspace_id: "wrk_test".into(),
        preview: "stop mon_1".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let risk = tool.risk(&invocation);
    assert_eq!(risk.tool_id, MONITOR_STOP_TOOL_ID);
}

#[test]
fn monitor_list_tool_definition() {
    let tool = MonitorListTool::new(test_registry());
    let def = tool.definition();
    assert_eq!(def.tool_id, MONITOR_LIST_TOOL_ID);
}

#[tokio::test]
async fn list_empty_returns_no_active() {
    let tool = MonitorListTool::new(test_registry());
    let invocation = ToolInvocation {
        tool_id: MONITOR_LIST_TOOL_ID.into(),
        arguments: serde_json::json!({}),
        workspace_id: "wrk_test".into(),
        preview: "list monitors".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let output = tool.invoke(invocation).await.unwrap();
    assert_eq!(output.text, "No active monitors.");
}

#[tokio::test]
async fn start_tool_invoke_returns_monitor_id() {
    let tool = MonitorStartTool::new(test_registry());
    let invocation = ToolInvocation {
        tool_id: MONITOR_START_TOOL_ID.into(),
        arguments: serde_json::json!({
            "command": "echo hello",
            "description": "test monitor"
        }),
        workspace_id: "wrk_test".into(),
        preview: "echo hello".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let output = tool.invoke(invocation).await.unwrap();
    assert!(
        output.text.starts_with("Monitor started: mon_"),
        "got: {}",
        output.text
    );
}

#[tokio::test]
async fn start_tool_missing_command_errors() {
    let tool = MonitorStartTool::new(test_registry());
    let invocation = ToolInvocation {
        tool_id: MONITOR_START_TOOL_ID.into(),
        arguments: serde_json::json!({"description": "no cmd"}),
        workspace_id: "wrk_test".into(),
        preview: "".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn stop_tool_unknown_monitor_errors() {
    let tool = MonitorStopTool::new(test_registry());
    let invocation = ToolInvocation {
        tool_id: MONITOR_STOP_TOOL_ID.into(),
        arguments: serde_json::json!({"monitor_id": "mon_999"}),
        workspace_id: "wrk_test".into(),
        preview: "stop mon_999".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn stop_tool_invoke_success() {
    let registry = test_registry();
    let start = MonitorStartTool::new(registry.clone());
    let inv = ToolInvocation {
        tool_id: MONITOR_START_TOOL_ID.into(),
        arguments: serde_json::json!({"command": "sleep 60", "description": "to stop"}),
        workspace_id: "wrk_test".into(),
        preview: "sleep 60".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let out = start.invoke(inv).await.unwrap();
    let monitor_id = out.text.strip_prefix("Monitor started: ").unwrap();

    let stop = MonitorStopTool::new(registry.clone());
    let inv = ToolInvocation {
        tool_id: MONITOR_STOP_TOOL_ID.into(),
        arguments: serde_json::json!({"monitor_id": monitor_id}),
        workspace_id: "wrk_test".into(),
        preview: format!("stop {monitor_id}"),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let out = stop.invoke(inv).await.unwrap();
    assert!(out.text.contains("Monitor stopped:"));
}

#[tokio::test]
async fn stop_tool_missing_monitor_id_errors() {
    let tool = MonitorStopTool::new(test_registry());
    let invocation = ToolInvocation {
        tool_id: MONITOR_STOP_TOOL_ID.into(),
        arguments: serde_json::json!({}),
        workspace_id: "wrk_test".into(),
        preview: "".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_with_active_monitor_shows_entries() {
    let registry = test_registry();
    let start = MonitorStartTool::new(registry.clone());
    let invocation = ToolInvocation {
        tool_id: MONITOR_START_TOOL_ID.into(),
        arguments: serde_json::json!({
            "command": "sleep 60",
            "description": "long running"
        }),
        workspace_id: "wrk_test".into(),
        preview: "sleep 60".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    start.invoke(invocation).await.unwrap();

    let list = MonitorListTool::new(registry.clone());
    let invocation = ToolInvocation {
        tool_id: MONITOR_LIST_TOOL_ID.into(),
        arguments: serde_json::json!({}),
        workspace_id: "wrk_test".into(),
        preview: "list".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let output = list.invoke(invocation).await.unwrap();
    assert!(output.text.contains("mon_"), "got: {}", output.text);
    assert!(output.text.contains("long running"));

    registry.stop_all().await;
}

#[tokio::test]
async fn start_tool_with_optional_params() {
    let tool = MonitorStartTool::new(test_registry());
    let invocation = ToolInvocation {
        tool_id: MONITOR_START_TOOL_ID.into(),
        arguments: serde_json::json!({
            "command": "echo test",
            "description": "persistent test",
            "persistent": true,
            "timeout_ms": 60000
        }),
        workspace_id: "wrk_test".into(),
        preview: "echo test".into(),
        timeout_ms: 0,
        output_limit_bytes: 0,
    };
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.starts_with("Monitor started: mon_"));
}
