//! MCP client — high-level interface for interacting with an MCP server.
//!
//! Wraps a [`Transport`][crate::transport::Transport] and provides typed methods
//! for initialization, tool listing/invocation, resource access, and prompt rendering.

use crate::transport::Transport;
use crate::types::*;
use crate::{McpError, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

/// High-level MCP client for a single server connection.
///
/// Each `McpClient` wraps a [`Transport`] and provides typed methods that
/// correspond to the MCP protocol operations: handshake, tool discovery/invocation,
/// resource reading, and prompt rendering.
///
/// The client uses an internal request ID counter that increments atomically,
/// and a `OnceCell` for caching `ServerInfo` after the initial handshake.
/// Discovery methods (tools / resources / prompts) always query the server;
/// use [`DiscoveryCache`][crate::DiscoveryCache] for a caching layer on top.
pub struct McpClient {
    /// A friendly identifier for the server this client is connected to.
    server_id: String,
    /// The underlying transport, wrapped in `Arc<Mutex<...>>` because
    /// [`Transport::send_request`] takes `&mut self`.
    transport: Arc<Mutex<Box<dyn Transport>>>,
    /// Cached server info after a successful handshake.
    server_info: OnceCell<ServerInfo>,
    /// Monotonic request ID counter.
    next_id: AtomicU64,
}

impl McpClient {
    /// Create a new client for the given server, using the provided transport.
    pub fn new(server_id: impl Into<String>, transport: Box<dyn Transport>) -> Self {
        Self {
            server_id: server_id.into(),
            transport: Arc::new(Mutex::new(transport)),
            server_info: OnceCell::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Return the server identifier.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Return the cached `ServerInfo`, if a handshake has been completed.
    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.server_info.get()
    }

    // -- Handshake -----------------------------------------------------------

    /// Perform the MCP initialization handshake.
    ///
    /// Sends an `initialize` request, waits for the server's `ServerInfo`,
    /// then sends a `notifications/initialized` notification.
    ///
    /// The `ServerInfo` is cached; subsequent calls return the cached value.
    pub async fn handshake(&self) -> Result<&ServerInfo> {
        self.server_info
            .get_or_try_init(|| async {
                let id = self.next_id();
                let params = serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "kairox",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                });
                let request = JsonRpcRequest::new(id, "initialize", Some(params));
                let response = self.send_request(request).await?;

                let result = &response.result;
                let server_info: ServerInfo = result
                    .get("serverInfo")
                    .ok_or_else(|| {
                        McpError::Handshake("initialize response missing serverInfo".to_string())
                    })
                    .and_then(|v| {
                        serde_json::from_value(v.clone())
                            .map_err(|e| McpError::Handshake(format!("invalid serverInfo: {e}")))
                    })?;

                let notification = JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "notifications/initialized".to_string(),
                    params: None,
                };
                self.send_notification(notification).await?;

                Ok(server_info)
            })
            .await
    }

    // -- Discovery (always fresh from server) --------------------------------

    /// List tools available on the server.
    ///
    /// Always queries the server; use [`DiscoveryCache`][crate::DiscoveryCache] for caching.
    pub async fn discover_tools(&self) -> Result<Vec<McpToolDef>> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(id, "tools/list", Some(serde_json::json!({})));
        let response = self.send_request(request).await?;
        let tools: Vec<McpToolDef> = response
            .result
            .get("tools")
            .ok_or_else(|| McpError::Protocol("tools/list response missing tools".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid tools: {e}")))
            })?;
        Ok(tools)
    }

    /// List resources available on the server.
    ///
    /// Always queries the server; use [`DiscoveryCache`][crate::DiscoveryCache] for caching.
    pub async fn discover_resources(&self) -> Result<Vec<McpResourceDef>> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(id, "resources/list", Some(serde_json::json!({})));
        let response = self.send_request(request).await?;
        let resources: Vec<McpResourceDef> = response
            .result
            .get("resources")
            .ok_or_else(|| McpError::Protocol("resources/list response missing resources".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid resources: {e}")))
            })?;
        Ok(resources)
    }

    /// List prompts available on the server.
    ///
    /// Always queries the server; use [`DiscoveryCache`][crate::DiscoveryCache] for caching.
    pub async fn discover_prompts(&self) -> Result<Vec<McpPromptDef>> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(id, "prompts/list", Some(serde_json::json!({})));
        let response = self.send_request(request).await?;
        let prompts: Vec<McpPromptDef> = response
            .result
            .get("prompts")
            .ok_or_else(|| McpError::Protocol("prompts/list response missing prompts".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid prompts: {e}")))
            })?;
        Ok(prompts)
    }

    // -- Invocation ----------------------------------------------------------

    /// Call a tool on the server.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolResult> {
        let id = self.next_id();
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let request = JsonRpcRequest::new(id, "tools/call", Some(params));
        let response = self.send_request(request).await?;
        let result: McpToolResult = serde_json::from_value(response.result)
            .map_err(|e| McpError::Protocol(format!("invalid tool result: {e}")))?;
        Ok(result)
    }

    /// Read a resource from the server.
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<McpContentBlock>> {
        let id = self.next_id();
        let params = serde_json::json!({
            "uri": uri,
        });
        let request = JsonRpcRequest::new(id, "resources/read", Some(params));
        let response = self.send_request(request).await?;
        let contents: Vec<McpContentBlock> = response
            .result
            .get("contents")
            .ok_or_else(|| McpError::Protocol("resources/read response missing contents".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid contents: {e}")))
            })?;
        Ok(contents)
    }

    /// Get a prompt from the server.
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: std::collections::HashMap<String, String>,
    ) -> Result<Vec<McpContentBlock>> {
        let id = self.next_id();
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let request = JsonRpcRequest::new(id, "prompts/get", Some(params));
        let response = self.send_request(request).await?;
        let messages: Vec<McpContentBlock> = response
            .result
            .get("messages")
            .ok_or_else(|| McpError::Protocol("prompts/get response missing messages".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid messages: {e}")))
            })?;
        Ok(messages)
    }

    // -- Lifecycle -----------------------------------------------------------

    /// Send a shutdown notification and close the transport.
    pub async fn shutdown(&self) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/cancelled".to_string(),
            params: Some(serde_json::json!({ "reason": "shutdown" })),
        };
        self.send_notification(notification).await?;
        let mut transport = self.transport.lock().await;
        transport.close().await?;
        Ok(())
    }

    // -- Internal helpers ----------------------------------------------------

    /// Allocate the next request ID.
    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a JSON-RPC request through the transport.
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let mut transport = self.transport.lock().await;
        transport.send_request(request).await
    }

    /// Send a JSON-RPC notification through the transport.
    async fn send_notification(&self, notification: JsonRpcNotification) -> Result<()> {
        let mut transport = self.transport.lock().await;
        transport.send_notification(notification).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex as StdMutex;

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
                "protocolVersion": "2024-11-05",
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
                    "protocolVersion": "2024-11-05",
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
}
