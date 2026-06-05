use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use serde_json::json;

use agent_lsp::transport::Transport;
use agent_lsp::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use agent_lsp::DapClient;

use crate::permission::ToolEffect;
use crate::registry::{Tool, ToolInvocation, ToolProvider};

use super::DapToolProvider;

// ---------------------------------------------------------------------------
// Mock transport (mirrors agent-lsp dap_tests)
// ---------------------------------------------------------------------------

struct MockTransport {
    responses: Arc<StdMutex<VecDeque<JsonRpcResponse>>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            responses: Arc::new(StdMutex::new(VecDeque::new())),
        }
    }

    /// Enqueue a raw JSON-RPC result value (used for non-DAP-wrapped responses).
    fn enqueue(&self, result: serde_json::Value) {
        self.responses.lock().unwrap().push_back(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: serde_json::Value::Null,
            result: Some(result),
            error: None,
        });
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send_request(&mut self, _request: JsonRpcRequest) -> agent_lsp::Result<JsonRpcResponse> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| agent_lsp::LspError::Transport("no response queued".into()))
    }

    async fn send_notification(&mut self, _notification: JsonRpcNotification) -> agent_lsp::Result<()> {
        Ok(())
    }

    async fn close(&mut self) -> agent_lsp::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_provider(mock: MockTransport) -> DapToolProvider {
    let client = Arc::new(DapClient::new("test-dbg".into(), Box::new(mock)));
    DapToolProvider::new("test-dbg".into(), client)
}

fn invocation(tool_id: &str, arguments: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        tool_id: tool_id.into(),
        arguments,
        workspace_id: "ws".into(),
        session_id: "sess".into(),
        preview: String::new(),
        timeout_ms: 30_000,
        output_limit_bytes: 1_000_000,
    }
}

/// DAP responses go through the `DapClient` which first tries to interpret the
/// JSON-RPC result as a `DapResponse` envelope. If it matches, it checks
/// `success` and extracts `body`. Otherwise it passes the raw value through.
/// Most tool-level tests just need the body content, so we enqueue the raw
/// body value directly (the client falls through to the raw-result path).
fn dap_success(body: serde_json::Value) -> serde_json::Value {
    json!({
        "seq": 1,
        "type": "response",
        "request_seq": 1,
        "command": "test",
        "success": true,
        "body": body
    })
}

fn dap_success_no_body() -> serde_json::Value {
    json!({
        "seq": 1,
        "type": "response",
        "request_seq": 1,
        "command": "test",
        "success": true
    })
}

// ---------------------------------------------------------------------------
// ToolProvider basics
// ---------------------------------------------------------------------------

#[test]
fn name_uses_dap_prefix_and_server_id() {
    let provider = make_provider(MockTransport::new());
    assert_eq!(provider.name(), "dap:test-dbg");
}

#[tokio::test]
async fn list_tools_returns_nine_tools() {
    let provider = make_provider(MockTransport::new());
    let tools = provider.list_tools().await;
    assert_eq!(tools.len(), 9);
    let ids: Vec<&str> = tools.iter().map(|t| t.tool_id.as_str()).collect();
    assert!(ids.contains(&"debug.test-dbg.launch"));
    assert!(ids.contains(&"debug.test-dbg.set_breakpoints"));
    assert!(ids.contains(&"debug.test-dbg.continue"));
    assert!(ids.contains(&"debug.test-dbg.step_over"));
    assert!(ids.contains(&"debug.test-dbg.step_into"));
    assert!(ids.contains(&"debug.test-dbg.stacktrace"));
    assert!(ids.contains(&"debug.test-dbg.variables"));
    assert!(ids.contains(&"debug.test-dbg.evaluate"));
    assert!(ids.contains(&"debug.test-dbg.disconnect"));
}

#[tokio::test]
async fn list_tools_all_require_debug_invoke_capability() {
    let provider = make_provider(MockTransport::new());
    for td in provider.list_tools().await {
        assert_eq!(td.required_capability, "debug.invoke");
    }
}

#[tokio::test]
async fn get_tool_returns_some_for_valid_operation() {
    let provider = make_provider(MockTransport::new());
    assert!(provider.get_tool("debug.test-dbg.launch").await.is_some());
    assert!(provider.get_tool("debug.test-dbg.continue").await.is_some());
    assert!(provider.get_tool("debug.test-dbg.disconnect").await.is_some());
}

#[tokio::test]
async fn get_tool_returns_none_for_unknown_operation() {
    let provider = make_provider(MockTransport::new());
    assert!(provider.get_tool("debug.test-dbg.unknown_op").await.is_none());
}

#[tokio::test]
async fn get_tool_returns_none_for_wrong_prefix() {
    let provider = make_provider(MockTransport::new());
    assert!(provider.get_tool("debug.other.launch").await.is_none());
}

// ---------------------------------------------------------------------------
// Tool instance basics (definition, risk)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tool_definition_contains_operation_name() {
    let provider = make_provider(MockTransport::new());
    let tool = provider.get_tool("debug.test-dbg.launch").await.unwrap();
    let def = tool.definition();
    assert_eq!(def.tool_id, "debug.test-dbg.launch");
    assert!(def.description.contains("launch"));
}

#[tokio::test]
async fn tool_risk_is_debug_invoke() {
    let provider = make_provider(MockTransport::new());
    let tool = provider.get_tool("debug.test-dbg.step_over").await.unwrap();
    let inv = invocation("debug.test-dbg.step_over", json!({}));
    let risk = tool.risk(&inv);
    assert_eq!(risk.tool_id, "debug.test-dbg.step_over");
    assert_eq!(risk.effect, ToolEffect::DebugInvoke);
}

// ---------------------------------------------------------------------------
// invoke — launch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_launch_returns_success_message() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success_no_body());

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.launch").await.unwrap();
    let inv = invocation(
        "debug.test-dbg.launch",
        json!({"program": "/usr/bin/test", "args": ["--verbose"], "cwd": "/tmp"}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("/usr/bin/test"));
    assert!(output.text.contains("launched"));
}

#[tokio::test]
async fn invoke_launch_missing_program() {
    let mock = MockTransport::new();
    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.launch").await.unwrap();
    let inv = invocation("debug.test-dbg.launch", json!({}));
    let err = tool.invoke(inv).await.unwrap_err();
    assert!(err.to_string().contains("program"));
}

// ---------------------------------------------------------------------------
// invoke — set_breakpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_set_breakpoints_formats_output() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success(json!({
        "breakpoints": [
            {"id": 1, "verified": true, "line": 10},
            {"id": 2, "verified": false, "line": 20}
        ]
    })));

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.set_breakpoints").await.unwrap();
    let inv = invocation(
        "debug.test-dbg.set_breakpoints",
        json!({"file": "/tmp/test.py", "lines": [10, 20]}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("/tmp/test.py"));
    assert!(output.text.contains("L10"));
    assert!(output.text.contains("L20"));
    assert!(output.text.contains("unverified"));
}

// ---------------------------------------------------------------------------
// invoke — continue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_continue_default_thread() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success_no_body());

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.continue").await.unwrap();
    let inv = invocation("debug.test-dbg.continue", json!({}));
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "Execution continued");
}

#[tokio::test]
async fn invoke_continue_custom_thread() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success_no_body());

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.continue").await.unwrap();
    let inv = invocation("debug.test-dbg.continue", json!({"thread_id": 42}));
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "Execution continued");
}

// ---------------------------------------------------------------------------
// invoke — step_over / step_into
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_step_over() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success_no_body());

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.step_over").await.unwrap();
    let inv = invocation("debug.test-dbg.step_over", json!({}));
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "Stepped over");
}

#[tokio::test]
async fn invoke_step_into() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success_no_body());

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.step_into").await.unwrap();
    let inv = invocation("debug.test-dbg.step_into", json!({}));
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "Stepped into");
}

// ---------------------------------------------------------------------------
// invoke — stacktrace
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_stacktrace_formats_frames() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success(json!({
        "stackFrames": [
            {
                "id": 0,
                "name": "main",
                "source": {"path": "/src/main.rs"},
                "line": 15,
                "column": 1
            },
            {
                "id": 1,
                "name": "run",
                "source": {"path": "/src/lib.rs"},
                "line": 42,
                "column": 5
            }
        ]
    })));

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.stacktrace").await.unwrap();
    let inv = invocation("debug.test-dbg.stacktrace", json!({}));
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("#0 main"));
    assert!(output.text.contains("/src/main.rs:15"));
    assert!(output.text.contains("#1 run"));
    assert!(output.text.contains("/src/lib.rs:42"));
}

#[tokio::test]
async fn invoke_stacktrace_empty() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success(json!({"stackFrames": []})));

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.stacktrace").await.unwrap();
    let inv = invocation("debug.test-dbg.stacktrace", json!({}));
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "No stack frames");
}

#[tokio::test]
async fn invoke_stacktrace_missing_source() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success(json!({
        "stackFrames": [
            {"id": 0, "name": "unknown", "line": 0, "column": 0}
        ]
    })));

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.stacktrace").await.unwrap();
    let inv = invocation("debug.test-dbg.stacktrace", json!({}));
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("??"));
}

// ---------------------------------------------------------------------------
// invoke — variables
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_variables_formats_scopes_and_vars() {
    let mock = MockTransport::new();
    // scopes response
    mock.enqueue(dap_success(json!({
        "scopes": [
            {"name": "Locals", "variablesReference": 100, "expensive": false}
        ]
    })));
    // variables response for scope ref 100
    mock.enqueue(dap_success(json!({
        "variables": [
            {"name": "x", "value": "42", "type": "i32"},
            {"name": "msg", "value": "\"hello\""}
        ]
    })));

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.variables").await.unwrap();
    let inv = invocation("debug.test-dbg.variables", json!({"frame_id": 0}));
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("--- Locals ---"));
    assert!(output.text.contains("x : i32 = 42"));
    assert!(output.text.contains("msg = \"hello\""));
}

#[tokio::test]
async fn invoke_variables_empty_scopes() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success(json!({"scopes": []})));

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.variables").await.unwrap();
    let inv = invocation("debug.test-dbg.variables", json!({}));
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "No variables available");
}

// ---------------------------------------------------------------------------
// invoke — evaluate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_evaluate_returns_result() {
    let mock = MockTransport::new();
    mock.enqueue(dap_success(json!({"result": "42"})));

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.evaluate").await.unwrap();
    let inv = invocation(
        "debug.test-dbg.evaluate",
        json!({"expression": "1 + 1", "frame_id": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "42");
}

#[tokio::test]
async fn invoke_evaluate_missing_expression() {
    let mock = MockTransport::new();
    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.evaluate").await.unwrap();
    let inv = invocation("debug.test-dbg.evaluate", json!({}));
    let err = tool.invoke(inv).await.unwrap_err();
    assert!(err.to_string().contains("expression"));
}

// ---------------------------------------------------------------------------
// invoke — disconnect
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_disconnect() {
    let mock = MockTransport::new();
    // disconnect ignores errors, but the transport close() will be called
    mock.enqueue(dap_success_no_body());

    let provider = make_provider(mock);
    let tool = provider.get_tool("debug.test-dbg.disconnect").await.unwrap();
    let inv = invocation("debug.test-dbg.disconnect", json!({}));
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("disconnected"));
}

// ---------------------------------------------------------------------------
// get_tool rejects unknown operations
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_tool_rejects_nonexistent() {
    let provider = make_provider(MockTransport::new());
    assert!(provider.get_tool("debug.test-dbg.nonexistent").await.is_none());
}
