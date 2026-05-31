use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;

use crate::error::Result;
use crate::transport::Transport;
use crate::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

use super::DapClient;

struct MockTransport {
    responses: Arc<StdMutex<VecDeque<JsonRpcResponse>>>,
    requests: Arc<StdMutex<Vec<JsonRpcRequest>>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            responses: Arc::new(StdMutex::new(VecDeque::new())),
            requests: Arc::new(StdMutex::new(Vec::new())),
        }
    }

    fn enqueue_result(&self, result: serde_json::Value) {
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

    async fn send_notification(&mut self, _: JsonRpcNotification) -> Result<()> {
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn initialize_sends_request() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_result(serde_json::json!({
        "seq": 1,
        "type": "response",
        "request_seq": 1,
        "command": "initialize",
        "success": true,
        "body": {"supportsConfigurationDoneRequest": true}
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    client.initialize().await.unwrap();

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs.len(), 1);
}

#[tokio::test]
async fn set_breakpoints_parses_response() {
    let mock = MockTransport::new();
    // Init response.
    mock.enqueue_result(serde_json::json!({
        "seq": 1, "type": "response", "request_seq": 1,
        "command": "initialize", "success": true, "body": {}
    }));
    // setBreakpoints response.
    mock.enqueue_result(serde_json::json!({
        "breakpoints": [
            {"id": 1, "verified": true, "line": 10},
            {"id": 2, "verified": true, "line": 20}
        ]
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    client.initialize().await.unwrap();

    let bps = client
        .set_breakpoints("/tmp/test.py", &[10, 20])
        .await
        .unwrap();
    assert_eq!(bps.len(), 2);
    assert!(bps[0].verified);
    assert_eq!(bps[0].line, Some(10));
}
