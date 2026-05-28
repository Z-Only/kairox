//! Unit tests for [`McpClient`][super::McpClient] using a mock transport.

use super::McpClient;
use crate::protocol::*;
use crate::transport::Transport;
use crate::types::*;
use crate::{McpError, Result};
use async_trait::async_trait;
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

/// Shared inner state for the mock transport, accessible both from within
/// the `Transport` implementation and from test assertions.
#[derive(Debug, Default)]
struct MockState {
    responses: VecDeque<JsonRpcResponse>,
    notifications: Vec<JsonRpcNotification>,
    requests: Vec<JsonRpcRequest>,
}

/// A mock transport that returns enqueued responses and records
/// requests/notifications. Uses `Arc<StdMutex<...>>` so the state can be
/// inspected from outside the `McpClient`.
struct MockTransport {
    state: Arc<StdMutex<MockState>>,
}

impl MockTransport {
    fn new(state: Arc<StdMutex<MockState>>) -> Self {
        Self { state }
    }

    fn enqueue_response(state: &Arc<StdMutex<MockState>>, response: JsonRpcResponse) {
        state.lock().unwrap().responses.push_back(response);
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        self.state.lock().unwrap().requests.push(request);
        self.state
            .lock()
            .unwrap()
            .responses
            .pop_front()
            .ok_or_else(|| McpError::Transport("no response queued".into()))
    }

    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        self.state.lock().unwrap().notifications.push(notification);
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Helper: build an initialize response with the given server info.
fn make_init_response(id: u64, server_name: &str, server_version: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(id),
        result: json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "serverInfo": {
                "name": server_name,
                "version": server_version,
            }
        }),
    }
}

#[tokio::test]
async fn handshake_sends_initialize_and_initialized() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(&state, make_init_response(1, "test-server", "1.0.0"));

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let info = client.handshake().await.unwrap();

    assert_eq!(info.name, "test-server");
    assert_eq!(info.version, "1.0.0");

    // Verify the initialize request was sent
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 1);
    assert_eq!(s.requests[0].method, "initialize");
    assert_eq!(s.requests[0].id, json!(1));
    assert_eq!(
        s.requests[0].params.as_ref().unwrap()["protocolVersion"],
        MCP_PROTOCOL_VERSION
    );

    // Verify the initialized notification was sent
    assert_eq!(s.notifications.len(), 1);
    assert_eq!(s.notifications[0].method, "notifications/initialized");
}

#[tokio::test]
async fn handshake_is_cached_on_second_call() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(&state, make_init_response(1, "cached-srv", "2.0.0"));

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));

    // First call
    let info1 = client.handshake().await.unwrap();
    assert_eq!(info1.name, "cached-srv");

    // Second call should return the same cached data (no new request sent)
    let info2 = client.handshake().await.unwrap();
    assert_eq!(info2.name, "cached-srv");

    // Only one request should have been sent during the first handshake
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 1);
}

#[tokio::test]
async fn discover_tools_calls_tools_list() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({
                "tools": [
                    {
                        "name": "echo",
                        "description": "Echo back the input",
                    },
                    {
                        "name": "add",
                        "description": "Add two numbers",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "a": { "type": "number" },
                                "b": { "type": "number" }
                            }
                        }
                    }
                ]
            }),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let tools = client.discover_tools().await.unwrap();

    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "echo");
    assert_eq!(
        tools[0].description,
        Some("Echo back the input".to_string())
    );
    assert_eq!(tools[1].name, "add");
    assert!(tools[1].input_schema.is_some());

    // Verify the request
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 1);
    assert_eq!(s.requests[0].method, "tools/list");
}

#[tokio::test]
async fn discover_resources_calls_resources_list() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({
                "resources": [
                    {
                        "uri": "file:///tmp/readme.md",
                        "name": "readme",
                        "description": "Project readme",
                        "mime_type": "text/markdown"
                    }
                ]
            }),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let resources = client.discover_resources().await.unwrap();

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "file:///tmp/readme.md");
    assert_eq!(resources[0].name, "readme");
}

#[tokio::test]
async fn discover_prompts_calls_prompts_list() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({
                "prompts": [
                    {
                        "name": "greeting",
                        "description": "A greeting prompt",
                        "arguments": [
                            { "name": "name", "required": true }
                        ]
                    }
                ]
            }),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let prompts = client.discover_prompts().await.unwrap();

    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "greeting");
    assert_eq!(prompts[0].arguments.len(), 1);
    assert_eq!(prompts[0].arguments[0].name, "name");
}

#[tokio::test]
async fn call_tool_sends_tools_call_with_arguments() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({
                "content": [
                    { "type": "text", "text": "Hello, world!" }
                ],
                "is_error": false
            }),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let result = client
        .call_tool("echo", json!({"message": "Hello, world!"}))
        .await
        .unwrap();

    assert_eq!(result.content.len(), 1);
    assert_eq!(result.is_error, Some(false));
    match &result.content[0] {
        McpContentBlock::Text { text } => assert_eq!(text, "Hello, world!"),
        other => panic!("Expected Text block, got {:?}", other),
    }

    // Verify the request
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 1);
    assert_eq!(s.requests[0].method, "tools/call");
    assert_eq!(s.requests[0].params.as_ref().unwrap()["name"], "echo");
    assert_eq!(
        s.requests[0].params.as_ref().unwrap()["arguments"]["message"],
        "Hello, world!"
    );
}

#[tokio::test]
async fn call_tool_returns_error_result() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({
                "content": [
                    { "type": "text", "text": "Tool not found" }
                ],
                "is_error": true
            }),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let result = client.call_tool("missing", json!({})).await.unwrap();

    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn read_resource_sends_resources_read() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({
                "contents": [
                    {
                        "type": "text",
                        "text": "file contents here"
                    }
                ]
            }),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let contents = client.read_resource("file:///tmp/readme.md").await.unwrap();

    assert_eq!(contents.len(), 1);
    match &contents[0] {
        McpContentBlock::Text { text } => assert_eq!(text, "file contents here"),
        other => panic!("Expected Text block, got {:?}", other),
    }

    // Verify the request
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 1);
    assert_eq!(s.requests[0].method, "resources/read");
    assert_eq!(
        s.requests[0].params.as_ref().unwrap()["uri"],
        "file:///tmp/readme.md"
    );
}

#[tokio::test]
async fn get_prompt_sends_prompts_get() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({
                "messages": [
                    {
                        "type": "text",
                        "text": "Hello, Alice!"
                    }
                ]
            }),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let mut args = std::collections::HashMap::new();
    args.insert("name".to_string(), "Alice".to_string());
    let messages = client.get_prompt("greeting", args).await.unwrap();

    assert_eq!(messages.len(), 1);
    match &messages[0] {
        McpContentBlock::Text { text } => assert_eq!(text, "Hello, Alice!"),
        other => panic!("Expected Text block, got {:?}", other),
    }

    // Verify the request
    let s = state.lock().unwrap();
    assert_eq!(s.requests.len(), 1);
    assert_eq!(s.requests[0].method, "prompts/get");
    assert_eq!(s.requests[0].params.as_ref().unwrap()["name"], "greeting");
}

#[tokio::test]
async fn shutdown_sends_notification_and_closes() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));

    client.shutdown().await.unwrap();

    let s = state.lock().unwrap();
    assert_eq!(s.notifications.len(), 1);
    assert_eq!(s.notifications[0].method, "notifications/cancelled");
    assert_eq!(
        s.notifications[0].params.as_ref().unwrap()["reason"],
        "shutdown"
    );
}

#[tokio::test]
async fn handshake_fails_without_server_info() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    // Return a response that is missing `serverInfo`
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {}
                // no serverInfo!
            }),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let err = client.handshake().await.unwrap_err();
    assert!(
        err.to_string().contains("serverInfo"),
        "expected serverInfo error, got: {err}"
    );
}

#[tokio::test]
async fn discover_tools_missing_tools_field_returns_error() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    MockTransport::enqueue_response(
        &state,
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: json!({}),
        },
    );

    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let err = client.discover_tools().await.unwrap_err();
    assert!(
        err.to_string().contains("tools"),
        "expected tools error, got: {err}"
    );
}

#[tokio::test]
async fn transport_error_propagates() {
    let state = Arc::new(StdMutex::new(MockState::default()));
    // Don't enqueue any response — transport will return error
    let client = McpClient::new("test", Box::new(MockTransport::new(state.clone())));
    let err = client.discover_tools().await.unwrap_err();
    assert!(
        err.to_string().contains("no response queued"),
        "expected transport error, got: {err}"
    );
}
