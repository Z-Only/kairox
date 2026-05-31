use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;

use crate::error::Result;
use crate::transport::Transport;
use crate::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

use super::LspClient;

struct MockTransport {
    responses: Arc<StdMutex<VecDeque<JsonRpcResponse>>>,
    requests: Arc<StdMutex<Vec<JsonRpcRequest>>>,
    notifications: Arc<StdMutex<Vec<JsonRpcNotification>>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            responses: Arc::new(StdMutex::new(VecDeque::new())),
            requests: Arc::new(StdMutex::new(Vec::new())),
            notifications: Arc::new(StdMutex::new(Vec::new())),
        }
    }

    fn enqueue_response(&self, result: serde_json::Value) {
        self.responses.lock().unwrap().push_back(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Null,
            result: Some(result),
            error: None,
        });
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| crate::error::LspError::Transport("no response queued".into()))
    }

    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        self.notifications.lock().unwrap().push(notification);
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn initialize_caches_capabilities() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    // Queue initialize response.
    mock.enqueue_response(serde_json::json!({
        "capabilities": {
            "textDocumentSync": 1,
            "hoverProvider": true,
            "definitionProvider": true,
        }
    }));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let caps = client.initialize("file:///tmp/test").await.unwrap();

    assert!(caps.hover_provider.is_some());
    assert!(caps.definition_provider.is_some());

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].method, "initialize");
}

#[tokio::test]
async fn goto_definition_sends_correct_method() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    // Queue init response then definition response.
    mock.enqueue_response(serde_json::json!({
        "capabilities": {"definitionProvider": true}
    }));
    mock.enqueue_response(serde_json::json!({
        "uri": "file:///tmp/test/src/main.rs",
        "range": {
            "start": {"line": 10, "character": 0},
            "end": {"line": 10, "character": 5}
        }
    }));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    client.initialize("file:///tmp/test").await.unwrap();

    let locs = client
        .goto_definition("file:///tmp/test/src/lib.rs", 5, 10)
        .await
        .unwrap();

    assert_eq!(locs.len(), 1);
    assert_eq!(locs[0].range.start.line, 10);

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[1].method, "textDocument/definition");
}

#[tokio::test]
async fn hover_returns_none_for_null_result() {
    let mock = MockTransport::new();
    mock.enqueue_response(serde_json::json!({
        "capabilities": {"hoverProvider": true}
    }));
    // Simulate null hover result.
    mock.responses.lock().unwrap().push_back(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Number(2.into()),
        result: None,
        error: None,
    });

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    client.initialize("file:///tmp/test").await.unwrap();

    let hover = client
        .hover("file:///tmp/test/src/lib.rs", 5, 10)
        .await
        .unwrap();
    assert!(hover.is_none());
}
