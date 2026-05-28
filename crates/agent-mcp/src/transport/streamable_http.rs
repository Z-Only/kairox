//! Streamable HTTP transport for MCP.

use std::collections::HashMap;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;

use crate::protocol::MCP_PROTOCOL_VERSION;
use crate::transport::Transport;
use crate::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpError, Result};

/// Transport that communicates with an MCP server over Streamable HTTP.
pub struct StreamableHttpTransport {
    client: Client,
    url: String,
    headers: HashMap<String, String>,
    session_id: Option<String>,
}

impl StreamableHttpTransport {
    /// Create a Streamable HTTP transport connected to the MCP endpoint URL.
    pub async fn new(
        url: &str,
        mut headers: HashMap<String, String>,
        api_key_env: Option<&str>,
    ) -> Result<Self> {
        if let Some(env_var) = api_key_env {
            match std::env::var(env_var) {
                Ok(key) => {
                    headers.insert("Authorization".to_string(), format!("Bearer {key}"));
                }
                Err(_) => {
                    tracing::warn!(
                        target: "mcp::streamable_http",
                        "API key environment variable '{}' is not set",
                        env_var
                    );
                }
            }
        }

        Ok(Self {
            client: Client::new(),
            url: url.to_string(),
            headers,
            session_id: None,
        })
    }

    fn apply_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut builder = builder
            .header("MCP-Protocol-Version", MCP_PROTOCOL_VERSION)
            .header("Accept", "application/json, text/event-stream");
        if let Some(session_id) = &self.session_id {
            builder = builder.header("MCP-Session-Id", session_id);
        }
        for (key, value) in &self.headers {
            if !value.is_empty() {
                builder = builder.header(key.as_str(), value.as_str());
            }
        }
        builder
    }

    fn capture_session_id(&mut self, headers: &reqwest::header::HeaderMap) {
        if let Some(value) = headers.get("mcp-session-id").and_then(|v| v.to_str().ok()) {
            self.session_id = Some(value.to_string());
        }
    }

    async fn response_from_json(value: Value) -> Result<JsonRpcResponse> {
        if let Some(error) = value.get("error").and_then(|e| e.as_object()) {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(McpError::Protocol(format!(
                "JSON-RPC error {code}: {message}"
            )));
        }
        serde_json::from_value(value)
            .map_err(|e| McpError::Protocol(format!("invalid JSON-RPC response: {e}")))
    }

    async fn response_from_sse(
        response: reqwest::Response,
        expected_id: Value,
    ) -> Result<JsonRpcResponse> {
        let stream = response.bytes_stream().eventsource();
        tokio::pin!(stream);

        while let Some(event_result) = stream.next().await {
            let event =
                event_result.map_err(|e| McpError::Transport(format!("SSE stream error: {e}")))?;
            let data = event.data.trim();
            if data.is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(data)
                .map_err(|e| McpError::Protocol(format!("invalid SSE JSON-RPC payload: {e}")))?;
            if value.get("id") == Some(&expected_id) {
                return Self::response_from_json(value).await;
            }
        }

        Err(McpError::Transport(
            "SSE stream ended before JSON-RPC response arrived".into(),
        ))
    }

    async fn response_from_sse_body(body: &str, expected_id: Value) -> Result<JsonRpcResponse> {
        let mut data = String::new();
        for line in body.lines() {
            let line = line.trim_start();
            if let Some(rest) = line.strip_prefix("data:") {
                data.push_str(rest.trim_start());
            }
            if line.is_empty() && !data.is_empty() {
                let value: Value = serde_json::from_str(data.trim()).map_err(|e| {
                    McpError::Protocol(format!("invalid SSE JSON-RPC payload: {e}"))
                })?;
                if value.get("id") == Some(&expected_id) {
                    return Self::response_from_json(value).await;
                }
                data.clear();
            }
        }
        if !data.is_empty() {
            let value: Value = serde_json::from_str(data.trim())
                .map_err(|e| McpError::Protocol(format!("invalid SSE JSON-RPC payload: {e}")))?;
            if value.get("id") == Some(&expected_id) {
                return Self::response_from_json(value).await;
            }
        }

        Err(McpError::Transport(
            "SSE body ended before JSON-RPC response arrived".into(),
        ))
    }
}

#[async_trait]
impl Transport for StreamableHttpTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let expected_id = request.id.clone();
        let response = self
            .apply_headers(self.client.post(&self.url))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| McpError::Transport(format!("POST request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(McpError::Transport(format!(
                "POST returned status {status}"
            )));
        }

        self.capture_session_id(response.headers());

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if content_type.contains("text/event-stream") {
            return Self::response_from_sse(response, expected_id).await;
        }

        let body = response
            .text()
            .await
            .map_err(|e| McpError::Transport(format!("failed to read response body: {e}")))?;
        let value: Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(json_error) => {
                return Self::response_from_sse_body(&body, expected_id)
                    .await
                    .map_err(|_| {
                        McpError::Protocol(format!("invalid JSON response: {json_error}"))
                    });
            }
        };
        Self::response_from_json(value).await
    }

    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        let response = self
            .apply_headers(self.client.post(&self.url))
            .header("Content-Type", "application/json")
            .json(&notification)
            .send()
            .await
            .map_err(|e| McpError::Transport(format!("POST notification failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(McpError::Transport(format!(
                "POST notification returned status {status}"
            )));
        }

        self.capture_session_id(response.headers());
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        if self.session_id.is_some() {
            let _ = self
                .apply_headers(self.client.delete(&self.url))
                .send()
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn streamable_http_posts_json_rpc_to_mcp_url_and_reuses_session_id() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(body_string_contains("\"method\":\"initialize\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .insert_header("mcp-session-id", "session-123")
                    .set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": {"serverInfo": {"name": "test", "version": "1.0.0"}}
                    })),
            )
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .and(header("mcp-session-id", "session-123"))
            .and(body_string_contains("\"method\":\"tools/list\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "result": {"tools": []}
                    })),
            )
            .mount(&mock_server)
            .await;

        let mut transport = StreamableHttpTransport::new(
            &format!("{}/mcp", mock_server.uri()),
            HashMap::new(),
            None,
        )
        .await
        .expect("transport should be created");

        let first = transport
            .send_request(JsonRpcRequest::new(1, "initialize", Some(json!({}))))
            .await
            .expect("initialize request should succeed");
        assert_eq!(first.id, json!(1));

        let second = transport
            .send_request(JsonRpcRequest::new(2, "tools/list", Some(json!({}))))
            .await
            .expect("tools/list request should reuse the session id");
        assert_eq!(second.result, json!({"tools": []}));
    }

    #[tokio::test]
    async fn streamable_http_preserves_exact_endpoint_url() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/mcp/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {}
            })))
            .mount(&mock_server)
            .await;

        let mut transport = StreamableHttpTransport::new(
            &format!("{}/mcp/", mock_server.uri()),
            HashMap::new(),
            None,
        )
        .await
        .expect("transport should be created");

        transport
            .send_request(JsonRpcRequest::new(1, "ping", Some(json!({}))))
            .await
            .expect("request should use the exact configured endpoint");
    }

    #[tokio::test]
    async fn streamable_http_parses_sse_json_rpc_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(
                        "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"tools\":[]}}\n\n",
                    ),
            )
            .mount(&mock_server)
            .await;

        let mut transport = StreamableHttpTransport::new(
            &format!("{}/mcp", mock_server.uri()),
            HashMap::new(),
            None,
        )
        .await
        .expect("transport should be created");

        let response = transport
            .send_request(JsonRpcRequest::new(7, "tools/list", Some(json!({}))))
            .await
            .expect("SSE response should be parsed");

        assert_eq!(response.result, json!({"tools": []}));
    }
}
