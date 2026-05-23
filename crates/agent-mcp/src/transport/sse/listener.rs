//! Background SSE listener that maintains the event stream and routes
//! responses to the correct pending request channels.

use std::collections::HashMap;
use std::sync::Arc;

use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{McpError, Result};

use super::parser::{parse_sse_response, SseResponse};

/// Background task that listens to the SSE endpoint and routes responses
/// to the correct pending request channels.
pub(super) async fn sse_listener(
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
