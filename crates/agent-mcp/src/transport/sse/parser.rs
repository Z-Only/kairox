//! Parsing of SSE event payloads into JSON-RPC responses.

use serde_json::Value;

use crate::JsonRpcResponse;

/// Internal response type that can represent either a success or an error
/// received over the SSE stream, correlated by request id.
#[derive(Debug)]
pub(super) enum SseResponse {
    Success(JsonRpcResponse),
    Error {
        id: Value,
        code: i64,
        message: String,
    },
}

impl SseResponse {
    /// Extract the request id from the response.
    pub(super) fn id(&self) -> &Value {
        match self {
            SseResponse::Success(r) => &r.id,
            SseResponse::Error { id, .. } => id,
        }
    }
}

/// Parse a JSON-RPC response or error from an SSE event data payload.
pub(super) fn parse_sse_response(data: &str) -> Option<SseResponse> {
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
