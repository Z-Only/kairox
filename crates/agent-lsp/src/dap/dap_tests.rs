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

#[tokio::test]
async fn launch_sends_correct_command() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_result(serde_json::json!({
        "seq": 1, "type": "response", "request_seq": 1,
        "command": "launch", "success": true, "body": {}
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    client
        .launch("/tmp/test.py", &["--arg1".to_string()], Some("/tmp"), None)
        .await
        .unwrap();

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "launch");
    let params = reqs[0].params.as_ref().unwrap();
    let args = params.get("arguments").unwrap();
    assert_eq!(args["program"], "/tmp/test.py");
    assert_eq!(args["cwd"], "/tmp");
}

#[tokio::test]
async fn launch_with_env_passes_environment() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_result(serde_json::json!({
        "seq": 1, "type": "response", "request_seq": 1,
        "command": "launch", "success": true, "body": {}
    }));

    let mut env = std::collections::HashMap::new();
    env.insert("FOO".to_string(), "bar".to_string());

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    client
        .launch("/tmp/test.py", &[], None, Some(&env))
        .await
        .unwrap();

    let reqs = requests.lock().unwrap();
    let params = reqs[0].params.as_ref().unwrap();
    let args = params.get("arguments").unwrap();
    assert_eq!(args["env"]["FOO"], "bar");
}

#[tokio::test]
async fn stack_trace_parses_frames() {
    let mock = MockTransport::new();
    mock.enqueue_result(serde_json::json!({
        "stackFrames": [
            {"id": 1, "name": "main", "line": 10, "column": 0, "source": {"name": "test.py", "path": "/tmp/test.py"}},
            {"id": 2, "name": "helper", "line": 20, "column": 0, "source": {"name": "helper.py", "path": "/tmp/helper.py"}}
        ]
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    let frames = client.stack_trace(1).await.unwrap();

    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].name, "main");
    assert_eq!(frames[0].line, 10);
    assert_eq!(frames[1].name, "helper");
}

#[tokio::test]
async fn stack_trace_returns_empty_for_none() {
    let mock = MockTransport::new();
    mock.responses.lock().unwrap().push_back(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Null,
        result: None,
        error: None,
    });

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    let frames = client.stack_trace(1).await.unwrap();
    assert!(frames.is_empty());
}

#[tokio::test]
async fn variables_parses_response() {
    let mock = MockTransport::new();
    mock.enqueue_result(serde_json::json!({
        "variables": [
            {"name": "x", "value": "42", "variablesReference": 0},
            {"name": "y", "value": "hello", "variablesReference": 0}
        ]
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    let vars = client.variables(1).await.unwrap();

    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].name, "x");
    assert_eq!(vars[0].value, "42");
}

#[tokio::test]
async fn evaluate_returns_result_string() {
    let mock = MockTransport::new();
    mock.enqueue_result(serde_json::json!({
        "result": "42",
        "variablesReference": 0
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    let result = client.evaluate("1 + 41", Some(1)).await.unwrap();
    assert_eq!(result, "42");
}

#[tokio::test]
async fn evaluate_without_frame_id() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_result(serde_json::json!({
        "result": "hello",
        "variablesReference": 0
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    let result = client.evaluate("'hello'", None).await.unwrap();
    assert_eq!(result, "hello");

    let reqs = requests.lock().unwrap();
    let params = reqs[0].params.as_ref().unwrap();
    let args = params.get("arguments").unwrap();
    assert!(args.get("frameId").is_none());
}

#[tokio::test]
async fn evaluate_returns_empty_for_none_body() {
    let mock = MockTransport::new();
    mock.responses.lock().unwrap().push_back(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::Value::Null,
        result: None,
        error: None,
    });

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    let result = client.evaluate("expr", None).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn scopes_parses_response() {
    let mock = MockTransport::new();
    mock.enqueue_result(serde_json::json!({
        "scopes": [
            {"name": "Locals", "variablesReference": 1, "expensive": false},
            {"name": "Globals", "variablesReference": 2, "expensive": true}
        ]
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    let scopes = client.scopes(1).await.unwrap();

    assert_eq!(scopes.len(), 2);
    assert_eq!(scopes[0].name, "Locals");
    assert!(!scopes[0].expensive);
    assert!(scopes[1].expensive);
}

#[tokio::test]
async fn failed_dap_command_returns_error() {
    let mock = MockTransport::new();
    mock.enqueue_result(serde_json::json!({
        "seq": 1, "type": "response", "request_seq": 1,
        "command": "launch", "success": false,
        "message": "program not found"
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    let result = client.launch("/nonexistent", &[], None, None).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("program not found"), "got: {err_msg}");
}

#[tokio::test]
async fn continue_execution_sends_thread_id() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_result(serde_json::json!({
        "seq": 1, "type": "response", "request_seq": 1,
        "command": "continue", "success": true, "body": {}
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    client.continue_execution(42).await.unwrap();

    let reqs = requests.lock().unwrap();
    let params = reqs[0].params.as_ref().unwrap();
    let args = params.get("arguments").unwrap();
    assert_eq!(args["threadId"], 42);
}

#[tokio::test]
async fn step_over_sends_correct_command() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_result(serde_json::json!({
        "seq": 1, "type": "response", "request_seq": 1,
        "command": "next", "success": true, "body": {}
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    client.step_over(1).await.unwrap();

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "next");
}

#[tokio::test]
async fn step_into_sends_correct_command() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_result(serde_json::json!({
        "seq": 1, "type": "response", "request_seq": 1,
        "command": "stepIn", "success": true, "body": {}
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    client.step_into(1).await.unwrap();

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "stepIn");
}

#[tokio::test]
async fn disconnect_closes_transport() {
    let mock = MockTransport::new();
    let requests = mock.requests.clone();
    mock.enqueue_result(serde_json::json!({
        "seq": 1, "type": "response", "request_seq": 1,
        "command": "disconnect", "success": true, "body": {}
    }));

    let client = DapClient::new("test-dap".to_string(), Box::new(mock));
    client.disconnect().await.unwrap();

    let reqs = requests.lock().unwrap();
    assert_eq!(reqs[0].method, "disconnect");
}
