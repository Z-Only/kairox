//! SSE (Server-Sent Events) transport for MCP.
//!
//! Communicates with an MCP server by listening for events on an SSE endpoint
//! and sending requests via HTTP POST.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::transport::Transport;
use crate::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpError, Result};

/// Internal response type that can represent either a success or an error
/// received over the SSE stream, correlated by request id.
#[derive(Debug)]
enum SseResponse {
    Success(JsonRpcResponse),
    Error {
        id: Value,
        code: i64,
        message: String,
    },
}

impl SseResponse {
    /// Extract the request id from the response.
    fn id(&self) -> &Value {
        match self {
            SseResponse::Success(r) => &r.id,
            SseResponse::Error { id, .. } => id,
        }
    }
}

/// Transport that communicates with an MCP server over SSE + HTTP POST.
///
/// The client:
/// - Sends JSON-RPC requests and notifications via HTTP POST to the message
///   endpoint.
/// - Receives JSON-RPC responses via a background SSE listener that connects
///   to the SSE endpoint.
///
/// The MCP SSE transport protocol works as follows:
/// 1. Client connects to `{base_url}/sse` to receive a stream of events.
/// 2. Client sends JSON-RPC messages via HTTP POST to `{base_url}/message`.
/// 3. The server pushes JSON-RPC responses back over the SSE stream.
pub struct SseTransport {
    /// HTTP client used for POST requests and the SSE connection.
    client: Client,
    /// Base URL of the MCP server (e.g. `http://localhost:8080`).
    base_url: String,
    /// HTTP headers applied to every request (including auth).
    headers: HashMap<String, String>,
    /// Pending response senders, keyed by JSON-RPC request id.
    pending_responses: Arc<Mutex<HashMap<Value, tokio::sync::oneshot::Sender<SseResponse>>>>,
    /// Handle for the background SSE listener task.
    sse_task: Option<JoinHandle<()>>,
}

impl SseTransport {
    /// Create a new SSE transport connected to the given URL.
    ///
    /// The `url` parameter should be the base URL of the MCP server (e.g.
    /// `http://localhost:8080`). The transport will connect to `{url}/sse`
    /// for the event stream and POST to `{url}/message` for sending messages.
    ///
    /// If `api_key_env` is set, the environment variable with that name is
    /// resolved and added as an `Authorization: Bearer {key}` header.
    pub async fn new(
        url: &str,
        headers: HashMap<String, String>,
        api_key_env: Option<&str>,
    ) -> Result<Self> {
        let mut headers = headers;

        // Resolve API key from environment variable if specified.
        if let Some(env_var) = api_key_env {
            match std::env::var(env_var) {
                Ok(key) => {
                    headers.insert("Authorization".to_string(), format!("Bearer {key}"));
                }
                Err(_) => {
                    tracing::warn!(
                        target: "mcp::sse",
                        "API key environment variable '{}' is not set",
                        env_var
                    );
                }
            }
        }

        let client = Client::new();
        let pending_responses: Arc<
            Mutex<HashMap<Value, tokio::sync::oneshot::Sender<SseResponse>>>,
        > = Arc::new(Mutex::new(HashMap::new()));

        let sse_url = format!("{}/sse", url.trim_end_matches('/'));

        // Start the background SSE listener.
        let sse_client = client.clone();
        let sse_headers = headers.clone();
        let pending = pending_responses.clone();
        let sse_task = tokio::spawn(async move {
            sse_listener(sse_url, sse_client, sse_headers, pending).await;
        });

        Ok(Self {
            client,
            base_url: url.trim_end_matches('/').to_string(),
            headers,
            pending_responses,
            sse_task: Some(sse_task),
        })
    }

    /// Build the message endpoint URL.
    fn message_url(&self) -> String {
        format!("{}/message", self.base_url)
    }

    /// Apply configured headers to a reqwest request builder.
    fn apply_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut builder = builder;
        for (key, value) in &self.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }
        builder
    }
}

/// Parse a JSON-RPC response or error from an SSE event data payload.
fn parse_sse_response(data: &str) -> Option<SseResponse> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try to parse as a generic JSON value first.
    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(target: "mcp::sse", "Failed to parse SSE data as JSON: {e}");
            return None;
        }
    };

    // Check if it looks like a JSON-RPC response (has "id" and either "result" or "error").
    let obj = match value.as_object() {
        Some(o) => o,
        None => {
            tracing::debug!(target: "mcp::sse", "SSE data is not a JSON object");
            return None;
        }
    };

    // Must have jsonrpc version field and an id.
    if !obj.contains_key("id") {
        return None;
    }

    let id = obj.get("id").cloned().unwrap_or(Value::Null);

    // Check for error response first.
    if let Some(error) = obj.get("error").and_then(|e| e.as_object()) {
        let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error")
            .to_string();
        return Some(SseResponse::Error { id, code, message });
    }

    // Try to parse as a success response.
    if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed) {
        return Some(SseResponse::Success(resp));
    }

    None
}

/// Background task that listens to the SSE endpoint and routes responses
/// to the correct pending request channels.
async fn sse_listener(
    sse_url: String,
    client: Client,
    headers: HashMap<String, String>,
    pending: Arc<Mutex<HashMap<Value, tokio::sync::oneshot::Sender<SseResponse>>>>,
) {
    tracing::info!(target: "mcp::sse", "SSE listener starting, connecting to {sse_url}");

    loop {
        match connect_sse(&sse_url, &client, &headers, &pending).await {
            Ok(()) => {
                tracing::info!(target: "mcp::sse", "SSE stream ended, reconnecting...");
            }
            Err(e) => {
                tracing::warn!(target: "mcp::sse", "SSE connection error: {e}, reconnecting in 1s...");
            }
        }

        // Brief delay before reconnecting to avoid tight loops.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

/// Connect to the SSE endpoint and process events until the stream ends
/// or an error occurs.
async fn connect_sse(
    sse_url: &str,
    client: &Client,
    headers: &HashMap<String, String>,
    pending: &Arc<Mutex<HashMap<Value, tokio::sync::oneshot::Sender<SseResponse>>>>,
) -> Result<()> {
    let mut builder = client.get(sse_url);
    for (key, value) in headers {
        builder = builder.header(key.as_str(), value.as_str());
    }
    builder = builder.header("Accept", "text/event-stream");

    let response = builder
        .send()
        .await
        .map_err(|e| McpError::Transport(format!("SSE connection failed: {e}")))?;

    if !response.status().is_success() {
        return Err(McpError::Transport(format!(
            "SSE endpoint returned status {}",
            response.status()
        )));
    }

    let stream = response.bytes_stream().eventsource();

    tokio::pin!(stream);

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                tracing::debug!(
                    target: "mcp::sse",
                    "SSE event: type={}, data={}",
                    event.event,
                    event.data
                );

                // Handle endpoint discovery event (MCP SSE spec).
                // The server may send an "endpoint" event with the URL for posting messages.
                // For now we parse all data as potential JSON-RPC responses.
                if let Some(sse_response) = parse_sse_response(&event.data) {
                    let id = sse_response.id().clone();
                    let mut map = pending.lock().await;
                    if let Some(sender) = map.remove(&id) {
                        let _ = sender.send(sse_response);
                    } else {
                        tracing::debug!(
                            target: "mcp::sse",
                            "No pending request for id {:?}, dropping response",
                            id
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(target: "mcp::sse", "SSE stream error: {e}");
                // Continue processing — the stream may recover.
            }
        }
    }

    Ok(())
}

#[async_trait]
impl Transport for SseTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let id = request.id.clone();

        // Create a oneshot channel to receive the response from the SSE listener.
        let (tx, rx) = tokio::sync::oneshot::channel::<SseResponse>();
        {
            let mut map = self.pending_responses.lock().await;
            map.insert(id.clone(), tx);
        }

        // POST the request to the message endpoint.
        let body = serde_json::to_string(&request)?;
        let post_result = self
            .apply_headers(self.client.post(self.message_url()))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await;

        match post_result {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    // Clean up the pending entry.
                    let mut map = self.pending_responses.lock().await;
                    map.remove(&id);
                    return Err(McpError::Transport(format!(
                        "POST /message returned status {status}"
                    )));
                }
            }
            Err(e) => {
                // Clean up the pending entry.
                let mut map = self.pending_responses.lock().await;
                map.remove(&id);
                return Err(McpError::Transport(format!(
                    "POST /message request failed: {e}"
                )));
            }
        }

        // Wait for the response to arrive via the SSE listener.
        match rx.await {
            Ok(SseResponse::Success(response)) => Ok(response),
            Ok(SseResponse::Error {
                code,
                message,
                id: _,
            }) => Err(McpError::Protocol(format!(
                "JSON-RPC error {code}: {message}"
            ))),
            Err(_) => {
                // The sender was dropped (SSE listener terminated).
                Err(McpError::Transport(
                    "SSE listener dropped before response arrived".into(),
                ))
            }
        }
    }

    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        let body = serde_json::to_string(&notification)?;
        let resp = self
            .apply_headers(self.client.post(self.message_url()))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| {
                McpError::Transport(format!("POST /message (notification) failed: {e}"))
            })?;

        if !resp.status().is_success() {
            return Err(McpError::Transport(format!(
                "POST /message (notification) returned status {}",
                resp.status()
            )));
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        if let Some(handle) = self.sse_task.take() {
            handle.abort();
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::JsonRpcRequest;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: build an SSE-formatted response body from lines of `data: ...`.
    fn sse_body(events: &[&str]) -> String {
        events.iter().map(|e| format!("data: {e}\n\n")).collect()
    }

    #[tokio::test]
    async fn sse_transport_connects_and_sends_request() {
        let mock_server = MockServer::start().await;

        // The JSON-RPC response that the server will push over SSE.
        let rpc_response = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;

        let sse_events = sse_body(&[rpc_response]);

        // Mount mock for GET /sse that returns the SSE stream.
        Mock::given(method("GET"))
            .and(path("/sse"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_events),
            )
            .mount(&mock_server)
            .await;

        // Mount mock for POST /message that returns 202 Accepted.
        Mock::given(method("POST"))
            .and(path("/message"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&mock_server)
            .await;

        let url = mock_server.uri();
        let mut transport = SseTransport::new(&url, HashMap::new(), None)
            .await
            .expect("failed to create SseTransport");

        // Give the SSE listener a moment to connect and process events.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let request = JsonRpcRequest::new(1, "tools/list", Some(json!({})));
        let response = transport
            .send_request(request)
            .await
            .expect("send_request failed");

        assert_eq!(response.id, json!(1));
        assert_eq!(response.result, json!({"tools": []}));

        transport.close().await.expect("close failed");
    }

    #[tokio::test]
    async fn sse_transport_handles_error_response() {
        let mock_server = MockServer::start().await;

        // The JSON-RPC error response that the server will push over SSE.
        let rpc_error =
            r#"{"jsonrpc":"2.0","id":5,"error":{"code":-32600,"message":"Invalid Request"}}"#;

        let sse_events = sse_body(&[rpc_error]);

        Mock::given(method("GET"))
            .and(path("/sse"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_events),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/message"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&mock_server)
            .await;

        let url = mock_server.uri();
        let mut transport = SseTransport::new(&url, HashMap::new(), None)
            .await
            .expect("failed to create SseTransport");

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let request = JsonRpcRequest::new(5, "bad_method", None);
        let result = transport.send_request(request).await;

        assert!(
            result.is_err(),
            "expected error for JSON-RPC error response"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Invalid Request"),
            "error should contain 'Invalid Request', got: {err_msg}"
        );

        transport.close().await.expect("close failed");
    }

    #[tokio::test]
    async fn sse_transport_handles_connection_error() {
        // Use a port that's not listening — should fail when trying to POST.
        let url = "http://127.0.0.1:1";

        let mut transport = SseTransport::new(url, HashMap::new(), None)
            .await
            .expect("SseTransport::new should succeed even if SSE can't connect");

        let request = JsonRpcRequest::new(1, "test", None);
        let result = transport.send_request(request).await;
        assert!(
            result.is_err(),
            "send_request to non-existent server should fail"
        );

        transport.close().await.ok();
    }

    #[tokio::test]
    async fn sse_transport_api_key_from_env() {
        let mock_server = MockServer::start().await;

        let rpc_response = r#"{"jsonrpc":"2.0","id":10,"result":{"ok":true}}"#;
        let sse_events = sse_body(&[rpc_response]);

        Mock::given(method("GET"))
            .and(path("/sse"))
            .and(header("authorization", "Bearer test-secret-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_events),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/message"))
            .and(header("authorization", "Bearer test-secret-key"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&mock_server)
            .await;

        // Set the environment variable.
        let env_key = "KAIROX_SSE_TEST_API_KEY";
        std::env::set_var(env_key, "test-secret-key");

        let url = mock_server.uri();
        let mut transport = SseTransport::new(&url, HashMap::new(), Some(env_key))
            .await
            .expect("failed to create SseTransport");

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let request = JsonRpcRequest::new(10, "test", None);
        let response = transport
            .send_request(request)
            .await
            .expect("send_request failed");

        assert_eq!(response.id, json!(10));
        assert_eq!(response.result, json!({"ok": true}));

        transport.close().await.expect("close failed");

        // Clean up env var.
        std::env::remove_var(env_key);
    }

    #[tokio::test]
    async fn sse_transport_send_notification() {
        let mock_server = MockServer::start().await;

        // SSE endpoint returns an empty stream (no events expected for notifications).
        Mock::given(method("GET"))
            .and(path("/sse"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(""),
            )
            .mount(&mock_server)
            .await;

        // POST /message returns 202.
        Mock::given(method("POST"))
            .and(path("/message"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&mock_server)
            .await;

        let url = mock_server.uri();
        let mut transport = SseTransport::new(&url, HashMap::new(), None)
            .await
            .expect("failed to create SseTransport");

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/cancelled".to_string(),
            params: Some(json!({"reason": "test"})),
        };

        // send_notification should complete immediately without waiting for a response.
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            transport.send_notification(notification),
        )
        .await
        .expect("send_notification timed out")
        .expect("send_notification failed");

        // Verify the POST was actually received by the mock server.
        // wiremock verifies that the mock was hit when we check via the mock server.
        // We can verify by checking that the POST mock was hit at least once.
        let requests = mock_server.received_requests().await.unwrap_or_default();
        let post_hits = requests
            .iter()
            .filter(|r| r.method == wiremock::http::Method::POST)
            .count();
        assert!(
            post_hits >= 1,
            "expected at least one POST request, got {post_hits}"
        );

        transport.close().await.expect("close failed");
    }

    #[tokio::test]
    async fn sse_transport_custom_headers_sent() {
        let mock_server = MockServer::start().await;

        let rpc_response = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        let sse_events = sse_body(&[rpc_response]);

        Mock::given(method("GET"))
            .and(path("/sse"))
            .and(header("x-custom-header", "custom-value"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_events),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/message"))
            .and(header("x-custom-header", "custom-value"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&mock_server)
            .await;

        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());

        let url = mock_server.uri();
        let mut transport = SseTransport::new(&url, headers, None)
            .await
            .expect("failed to create SseTransport");

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let request = JsonRpcRequest::new(1, "test", None);
        let response = transport
            .send_request(request)
            .await
            .expect("send_request failed");
        assert_eq!(response.id, json!(1));

        transport.close().await.expect("close failed");
    }

    #[test]
    fn parse_sse_response_success() {
        let data = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let response = parse_sse_response(data).expect("should parse");
        match response {
            SseResponse::Success(r) => {
                assert_eq!(r.id, json!(1));
                assert_eq!(r.result, json!({"tools": []}));
            }
            SseResponse::Error { .. } => panic!("expected success response"),
        }
    }

    #[test]
    fn parse_sse_response_error() {
        let data =
            r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let response = parse_sse_response(data).expect("should parse");
        match response {
            SseResponse::Success(_) => panic!("expected error response"),
            SseResponse::Error { id, code, message } => {
                assert_eq!(id, json!(2));
                assert_eq!(code, -32600);
                assert_eq!(message, "Invalid Request");
            }
        }
    }

    #[test]
    fn parse_sse_response_empty_data() {
        assert!(parse_sse_response("").is_none());
        assert!(parse_sse_response("  ").is_none());
    }

    #[test]
    fn parse_sse_response_non_object() {
        assert!(parse_sse_response("\"hello\"").is_none());
        assert!(parse_sse_response("42").is_none());
    }

    #[test]
    fn parse_sse_response_no_id_field() {
        // A notification has no id, should not be treated as a response.
        let data = r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{}}"#;
        assert!(parse_sse_response(data).is_none());
    }
}
