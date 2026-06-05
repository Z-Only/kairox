use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use serde_json::json;

use agent_lsp::transport::Transport;
use agent_lsp::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use agent_lsp::LspClient;

use crate::permission::ToolEffect;
use crate::registry::{Tool, ToolInvocation, ToolProvider};

use super::LspToolProvider;

// ---------------------------------------------------------------------------
// Mock transport (mirrors the one in agent-lsp client_tests)
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

    fn enqueue(&self, result: serde_json::Value) {
        self.responses.lock().unwrap().push_back(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: serde_json::Value::Null,
            result: Some(result),
            error: None,
        });
    }

    fn enqueue_none(&self) {
        self.responses.lock().unwrap().push_back(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: serde_json::Value::Null,
            result: None,
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

fn make_provider(mock: MockTransport) -> LspToolProvider {
    let client = Arc::new(LspClient::new("test-lsp".into(), Box::new(mock)));
    LspToolProvider::new("test-lsp".into(), client)
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

// ---------------------------------------------------------------------------
// ToolProvider basics
// ---------------------------------------------------------------------------

#[test]
fn name_uses_lsp_prefix_and_server_id() {
    let provider = make_provider(MockTransport::new());
    assert_eq!(provider.name(), "lsp:test-lsp");
}

#[tokio::test]
async fn list_tools_returns_six_tools() {
    let provider = make_provider(MockTransport::new());
    let tools = provider.list_tools().await;
    assert_eq!(tools.len(), 6);
    let ids: Vec<&str> = tools.iter().map(|t| t.tool_id.as_str()).collect();
    assert!(ids.contains(&"lsp.test-lsp.goto_definition"));
    assert!(ids.contains(&"lsp.test-lsp.find_references"));
    assert!(ids.contains(&"lsp.test-lsp.hover"));
    assert!(ids.contains(&"lsp.test-lsp.document_symbols"));
    assert!(ids.contains(&"lsp.test-lsp.workspace_symbols"));
    assert!(ids.contains(&"lsp.test-lsp.diagnostics"));
}

#[tokio::test]
async fn list_tools_all_require_lsp_query_capability() {
    let provider = make_provider(MockTransport::new());
    for td in provider.list_tools().await {
        assert_eq!(td.required_capability, "lsp.query");
    }
}

#[tokio::test]
async fn get_tool_returns_some_for_valid_operation() {
    let provider = make_provider(MockTransport::new());
    let tool = provider.get_tool("lsp.test-lsp.hover").await;
    assert!(tool.is_some());
}

#[tokio::test]
async fn get_tool_returns_none_for_unknown_operation() {
    let provider = make_provider(MockTransport::new());
    assert!(provider.get_tool("lsp.test-lsp.unknown_op").await.is_none());
}

#[tokio::test]
async fn get_tool_returns_none_for_wrong_prefix() {
    let provider = make_provider(MockTransport::new());
    assert!(provider.get_tool("lsp.other.hover").await.is_none());
}

// ---------------------------------------------------------------------------
// Tool instance basics (definition, risk)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tool_definition_contains_operation_name() {
    let provider = make_provider(MockTransport::new());
    let tool = provider.get_tool("lsp.test-lsp.hover").await.unwrap();
    let def = tool.definition();
    assert_eq!(def.tool_id, "lsp.test-lsp.hover");
    assert!(def.description.contains("hover"));
}

#[tokio::test]
async fn tool_risk_is_lsp_query() {
    let provider = make_provider(MockTransport::new());
    let tool = provider.get_tool("lsp.test-lsp.goto_definition").await.unwrap();
    let inv = invocation("lsp.test-lsp.goto_definition", json!({}));
    let risk = tool.risk(&inv);
    assert_eq!(risk.tool_id, "lsp.test-lsp.goto_definition");
    assert_eq!(risk.effect, ToolEffect::LspQuery);
}

// ---------------------------------------------------------------------------
// invoke — goto_definition
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_goto_definition_formats_locations() {
    let mock = MockTransport::new();
    mock.enqueue(json!({
        "uri": "file:///src/main.rs",
        "range": {
            "start": {"line": 9, "character": 4},
            "end": {"line": 9, "character": 10}
        }
    }));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.goto_definition").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.goto_definition",
        json!({"file": "file:///src/lib.rs", "line": 5, "character": 10}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("file:///src/main.rs"));
    assert!(output.text.contains("10:5")); // line+1 : char+1
    assert!(!output.truncated);
}

#[tokio::test]
async fn invoke_goto_definition_empty() {
    let mock = MockTransport::new();
    mock.enqueue(json!([]));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.goto_definition").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.goto_definition",
        json!({"file": "f.rs", "line": 0, "character": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "No locations found");
}

#[tokio::test]
async fn invoke_goto_definition_missing_param() {
    let mock = MockTransport::new();
    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.goto_definition").await.unwrap();
    let inv = invocation("lsp.test-lsp.goto_definition", json!({"file": "f.rs"}));
    let err = tool.invoke(inv).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("line"), "error should mention missing param: {msg}");
}

// ---------------------------------------------------------------------------
// invoke — find_references
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_find_references_formats_locations() {
    let mock = MockTransport::new();
    mock.enqueue(json!([
        {
            "uri": "file:///a.rs",
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}}
        },
        {
            "uri": "file:///b.rs",
            "range": {"start": {"line": 3, "character": 2}, "end": {"line": 3, "character": 7}}
        }
    ]));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.find_references").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.find_references",
        json!({"file": "f.rs", "line": 0, "character": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("file:///a.rs"));
    assert!(output.text.contains("file:///b.rs"));
}

// ---------------------------------------------------------------------------
// invoke — hover
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_hover_markup_content() {
    let mock = MockTransport::new();
    mock.enqueue(json!({
        "contents": {"kind": "markdown", "value": "fn main()"}
    }));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.hover").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.hover",
        json!({"file": "f.rs", "line": 0, "character": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "fn main()");
}

#[tokio::test]
async fn invoke_hover_none() {
    let mock = MockTransport::new();
    mock.enqueue_none();

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.hover").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.hover",
        json!({"file": "f.rs", "line": 0, "character": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "No hover information available");
}

#[tokio::test]
async fn invoke_hover_scalar_string() {
    let mock = MockTransport::new();
    mock.enqueue(json!({
        "contents": "plain hover text"
    }));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.hover").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.hover",
        json!({"file": "f.rs", "line": 0, "character": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "plain hover text");
}

#[tokio::test]
async fn invoke_hover_language_string() {
    let mock = MockTransport::new();
    mock.enqueue(json!({
        "contents": {"language": "rust", "value": "let x = 1;"}
    }));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.hover").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.hover",
        json!({"file": "f.rs", "line": 0, "character": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("```rust"));
    assert!(output.text.contains("let x = 1;"));
}

#[tokio::test]
async fn invoke_hover_array_of_marked_strings() {
    let mock = MockTransport::new();
    mock.enqueue(json!({
        "contents": [
            "first section",
            {"language": "python", "value": "x = 1"}
        ]
    }));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.hover").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.hover",
        json!({"file": "f.rs", "line": 0, "character": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("first section"));
    assert!(output.text.contains("```python"));
    assert!(output.text.contains("---")); // separator between array elements
}

// ---------------------------------------------------------------------------
// invoke — document_symbols
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_document_symbols_formats_tree() {
    let mock = MockTransport::new();
    mock.enqueue(json!([
        {
            "name": "MyStruct",
            "kind": 23,
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 10, "character": 0}},
            "selectionRange": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 8}},
            "children": [
                {
                    "name": "new",
                    "kind": 12,
                    "range": {"start": {"line": 2, "character": 4}, "end": {"line": 5, "character": 4}},
                    "selectionRange": {"start": {"line": 2, "character": 8}, "end": {"line": 2, "character": 11}}
                }
            ]
        }
    ]));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.document_symbols").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.document_symbols",
        json!({"file": "f.rs"}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("MyStruct"));
    assert!(output.text.contains("new"));
    // Child should be indented
    assert!(output.text.contains("  new"));
}

#[tokio::test]
async fn invoke_document_symbols_empty() {
    let mock = MockTransport::new();
    mock.enqueue(json!([]));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.document_symbols").await.unwrap();
    let inv = invocation("lsp.test-lsp.document_symbols", json!({"file": "f.rs"}));
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "No symbols found");
}

// ---------------------------------------------------------------------------
// invoke — workspace_symbols
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_workspace_symbols_formats_results() {
    let mock = MockTransport::new();
    mock.enqueue(json!([
        {
            "name": "Config",
            "kind": 23,
            "location": {
                "uri": "file:///config.rs",
                "range": {"start": {"line": 5, "character": 0}, "end": {"line": 20, "character": 0}}
            }
        }
    ]));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.workspace_symbols").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.workspace_symbols",
        json!({"query": "Config"}),
    );
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("Config"));
    assert!(output.text.contains("file:///config.rs"));
}

#[tokio::test]
async fn invoke_workspace_symbols_empty() {
    let mock = MockTransport::new();
    mock.enqueue(json!([]));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.workspace_symbols").await.unwrap();
    let inv = invocation("lsp.test-lsp.workspace_symbols", json!({"query": "Foo"}));
    let output = tool.invoke(inv).await.unwrap();
    assert_eq!(output.text, "No symbols found");
}

// ---------------------------------------------------------------------------
// invoke — diagnostics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_diagnostics_returns_limitation_message() {
    let mock = MockTransport::new();
    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.diagnostics").await.unwrap();
    let inv = invocation("lsp.test-lsp.diagnostics", json!({"file": "f.rs"}));
    let output = tool.invoke(inv).await.unwrap();
    assert!(output.text.contains("notifications"));
}

// ---------------------------------------------------------------------------
// invoke — unknown operation triggers NotFound
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invoke_unknown_operation_returns_error() {
    // Manually construct an LspToolInstance via get_tool with a known-good op,
    // then test the fallback by checking that unknown IDs fail at get_tool.
    let provider = make_provider(MockTransport::new());
    assert!(provider.get_tool("lsp.test-lsp.nonexistent").await.is_none());
}

// ---------------------------------------------------------------------------
// Formatting helpers (exercised indirectly above, but verify edge cases)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn goto_definition_multiple_locations() {
    let mock = MockTransport::new();
    mock.enqueue(json!([
        {
            "uri": "file:///a.rs",
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}
        },
        {
            "uri": "file:///b.rs",
            "range": {"start": {"line": 5, "character": 2}, "end": {"line": 5, "character": 8}}
        }
    ]));

    let provider = make_provider(mock);
    let tool = provider.get_tool("lsp.test-lsp.goto_definition").await.unwrap();
    let inv = invocation(
        "lsp.test-lsp.goto_definition",
        json!({"file": "f.rs", "line": 0, "character": 0}),
    );
    let output = tool.invoke(inv).await.unwrap();
    let lines: Vec<&str> = output.text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("file:///a.rs"));
    assert!(lines[1].starts_with("file:///b.rs"));
}
