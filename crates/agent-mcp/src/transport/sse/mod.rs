//! SSE (Server-Sent Events) transport for MCP.
//!
//! Communicates with an MCP server by listening for events on an SSE endpoint
//! and sending requests via HTTP POST.

mod listener;
mod parser;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "parser_tests.rs"]
mod parser_tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::transport::Transport;
use crate::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpError, Result};

use self::listener::sse_listener;
use self::parser::SseResponse;

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
    /// Maximum time to wait for an SSE response before timing out.
    request_timeout: Duration,
}

/// Default timeout for SSE request-response round-trips.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        })
    }

    /// Set the maximum time to wait for an SSE response before timing out.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
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

        // Wait for the response to arrive via the SSE listener, with timeout.
        match tokio::time::timeout(self.request_timeout, rx).await {
            Ok(Ok(SseResponse::Success(response))) => Ok(response),
            Ok(Ok(SseResponse::Error {
                code,
                message,
                id: _,
            })) => Err(McpError::Protocol(format!(
                "JSON-RPC error {code}: {message}"
            ))),
            Ok(Err(_)) => {
                // The sender was dropped (SSE listener terminated).
                Err(McpError::Transport(
                    "SSE listener dropped before response arrived".into(),
                ))
            }
            Err(_elapsed) => {
                // Timeout: clean up the pending entry so it doesn't leak.
                let mut map = self.pending_responses.lock().await;
                map.remove(&id);
                Err(McpError::Transport(format!(
                    "request timed out after {}s waiting for SSE response",
                    self.request_timeout.as_secs()
                )))
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
