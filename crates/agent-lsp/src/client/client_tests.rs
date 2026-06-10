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
async fn initialize_converts_absolute_root_path_to_file_uri() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_response(serde_json::json!({
        "capabilities": {}
    }));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    client.initialize("/tmp/test").await.unwrap();

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "initialize");
    assert_eq!(
        reqs[0].params.as_ref().unwrap()["rootUri"],
        "file:///tmp/test"
    );
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
async fn goto_definition_converts_absolute_path_to_file_uri() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_response(serde_json::json!([]));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let locs = client
        .goto_definition("/tmp/test/src/lib.rs", 5, 10)
        .await
        .unwrap();

    assert!(locs.is_empty());

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "textDocument/definition");
    assert_eq!(
        reqs[0].params.as_ref().unwrap()["textDocument"]["uri"],
        "file:///tmp/test/src/lib.rs"
    );
}

#[tokio::test]
async fn document_symbols_converts_absolute_path_to_file_uri() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_response(serde_json::json!([]));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let symbols = client
        .document_symbols("/tmp/test/src/lib.rs")
        .await
        .unwrap();

    assert!(symbols.is_empty());

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "textDocument/documentSymbol");
    assert_eq!(
        reqs[0].params.as_ref().unwrap()["textDocument"]["uri"],
        "file:///tmp/test/src/lib.rs"
    );
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

#[tokio::test]
async fn find_references_returns_locations() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_response(serde_json::json!([
        {
            "uri": "file:///tmp/test/src/lib.rs",
            "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 5}}
        },
        {
            "uri": "file:///tmp/test/src/main.rs",
            "range": {"start": {"line": 10, "character": 2}, "end": {"line": 10, "character": 7}}
        }
    ]));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let locs = client
        .find_references("file:///tmp/test/src/lib.rs", 1, 0)
        .await
        .unwrap();

    assert_eq!(locs.len(), 2);
    assert_eq!(locs[0].range.start.line, 1);
    assert_eq!(locs[1].range.start.line, 10);

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "textDocument/references");
}

#[tokio::test]
async fn find_references_returns_empty_for_null_result() {
    let mock = MockTransport::new();
    mock.responses.lock().unwrap().push_back(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Number(1.into()),
        result: None,
        error: None,
    });

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let locs = client
        .find_references("file:///tmp/test/src/lib.rs", 1, 0)
        .await
        .unwrap();
    assert!(locs.is_empty());
}

#[tokio::test]
async fn workspace_symbols_returns_symbols() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_response(serde_json::json!([
        {
            "name": "MyStruct",
            "kind": 23,
            "location": {
                "uri": "file:///tmp/test/src/lib.rs",
                "range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 8}}
            }
        }
    ]));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let symbols = client.workspace_symbols("MyStruct").await.unwrap();

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "MyStruct");

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "workspace/symbol");
}

#[tokio::test]
async fn workspace_symbols_returns_empty_for_null_result() {
    let mock = MockTransport::new();
    mock.responses.lock().unwrap().push_back(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Number(1.into()),
        result: None,
        error: None,
    });

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let symbols = client.workspace_symbols("anything").await.unwrap();
    assert!(symbols.is_empty());
}

#[tokio::test]
async fn completion_returns_items_from_array() {
    let mock = MockTransport::new();
    mock.enqueue_response(serde_json::json!([
        {"label": "println!", "kind": 3},
        {"label": "print!", "kind": 3}
    ]));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let items = client
        .completion("file:///tmp/test/src/lib.rs", 5, 10)
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].label, "println!");
}

#[tokio::test]
async fn completion_returns_items_from_completion_list() {
    let mock = MockTransport::new();
    mock.enqueue_response(serde_json::json!({
        "isIncomplete": false,
        "items": [{"label": "format!", "kind": 3}]
    }));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let items = client
        .completion("file:///tmp/test/src/lib.rs", 5, 10)
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "format!");
}

#[tokio::test]
async fn completion_returns_empty_for_null_result() {
    let mock = MockTransport::new();
    mock.responses.lock().unwrap().push_back(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Number(1.into()),
        result: None,
        error: None,
    });

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let items = client
        .completion("file:///tmp/test/src/lib.rs", 5, 10)
        .await
        .unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn did_open_sends_notification() {
    let mock = MockTransport::new();
    let notifications = mock.notifications.clone();

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    client
        .did_open("file:///tmp/test/src/lib.rs", "rust", "fn main() {}")
        .await
        .unwrap();

    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].method, "textDocument/didOpen");
}

#[tokio::test]
async fn shutdown_sends_request_then_exit() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    let notifications = mock.notifications.clone();
    // Queue shutdown response.
    mock.responses.lock().unwrap().push_back(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Number(1.into()),
        result: Some(serde_json::Value::Null),
        error: None,
    });

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    client.shutdown().await.unwrap();

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "shutdown");

    let notifs = notifications.lock().unwrap();
    assert_eq!(notifs[0].method, "exit");
}

#[tokio::test]
async fn document_symbols_parses_symbol_information_format() {
    let mock = MockTransport::new();
    // Return SymbolInformation[] format (legacy servers).
    #[allow(deprecated)]
    mock.enqueue_response(serde_json::json!([
        {
            "name": "main",
            "kind": 12,
            "location": {
                "uri": "file:///tmp/test/src/main.rs",
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 3, "character": 1}}
            }
        }
    ]));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let symbols = client
        .document_symbols("file:///tmp/test/src/main.rs")
        .await
        .unwrap();

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "main");
}

#[tokio::test]
async fn goto_definition_handles_location_link_response() {
    let mock = MockTransport::new();
    mock.enqueue_response(serde_json::json!([
        {
            "targetUri": "file:///tmp/test/src/lib.rs",
            "targetRange": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 0}},
            "targetSelectionRange": {"start": {"line": 1, "character": 4}, "end": {"line": 1, "character": 8}}
        }
    ]));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    let locs = client
        .goto_definition("file:///tmp/test/src/main.rs", 10, 5)
        .await
        .unwrap();

    assert_eq!(locs.len(), 1);
    assert_eq!(locs[0].range.start.line, 1);
    assert_eq!(locs[0].range.start.character, 4);
}

#[tokio::test]
async fn next_request_id_increments() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    // Queue two responses.
    mock.enqueue_response(serde_json::json!([]));
    mock.enqueue_response(serde_json::json!([]));

    let client = LspClient::new("test-server".to_string(), Box::new(mock));
    client
        .goto_definition("file:///tmp/test/src/lib.rs", 0, 0)
        .await
        .unwrap();
    client
        .goto_definition("file:///tmp/test/src/lib.rs", 1, 0)
        .await
        .unwrap();

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].id, serde_json::json!(1));
    assert_eq!(reqs[1].id, serde_json::json!(2));
}
