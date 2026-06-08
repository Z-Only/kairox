use super::LspStdioTransport;
use crate::error::LspError;
use crate::transport::Transport;
use crate::types::{JsonRpcNotification, JsonRpcRequest};
use std::collections::HashMap;

fn empty_env() -> HashMap<String, String> {
    HashMap::new()
}

#[tokio::test]
async fn spawn_nonexistent_command_returns_transport_error() {
    let result =
        LspStdioTransport::spawn("__nonexistent_binary_4f9a__", &[], empty_env(), None).await;
    match result {
        Err(LspError::Transport(msg)) => {
            assert!(
                msg.contains("__nonexistent_binary_4f9a__"),
                "error should mention the command name, got: {msg}"
            );
        }
        Err(other) => panic!("expected Transport error, got: {other}"),
        Ok(_) => panic!("spawning a nonexistent binary should fail"),
    }
}

#[tokio::test]
async fn spawn_and_close_succeeds() {
    let mut transport = LspStdioTransport::spawn("cat", &[], empty_env(), None)
        .await
        .expect("cat should spawn successfully");
    transport.close().await.expect("closing cat should succeed");
}

#[tokio::test]
async fn send_notification_to_cat_succeeds() {
    let mut transport = LspStdioTransport::spawn("cat", &[], empty_env(), None)
        .await
        .expect("cat should spawn");

    let notification = JsonRpcNotification {
        jsonrpc: "2.0".into(),
        method: "initialized".into(),
        params: None,
    };

    // `cat` will echo the framed message to stdout, but send_notification
    // does not read a response, so this should succeed without blocking.
    transport
        .send_notification(notification)
        .await
        .expect("sending notification to cat should not fail");

    transport.close().await.ok();
}

#[tokio::test]
async fn send_request_to_cat_echoes_back_as_valid_response() {
    // `cat` echoes stdin → stdout byte-for-byte, so the framed JSON-RPC
    // request we write will come back through read_message. Because
    // the echoed JSON has an "id" field, read_message will try to
    // deserialize it as a JsonRpcResponse.
    let mut transport = LspStdioTransport::spawn("cat", &[], empty_env(), None)
        .await
        .expect("cat should spawn");

    let request = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: serde_json::json!(1),
        method: "test/echo".into(),
        params: Some(serde_json::json!({"hello": "world"})),
    };

    // The echoed payload is a request shape (has "method"), but also has "id",
    // so read_message will parse it. JsonRpcResponse uses `#[serde(default)]`
    // for `result` and `error`, so deserialization should succeed with both None.
    let response = transport.send_request(request).await;
    match response {
        Ok(resp) => {
            assert_eq!(resp.jsonrpc, "2.0");
            assert_eq!(resp.id, serde_json::json!(1));
        }
        Err(e) => panic!("expected successful echo response, got error: {e}"),
    }

    transport.close().await.ok();
}

#[tokio::test]
async fn send_request_to_closed_process_returns_transport_error() {
    // Spawn `true` which exits immediately with code 0.
    let mut transport = LspStdioTransport::spawn("true", &[], empty_env(), None)
        .await
        .expect("`true` should spawn");

    // Give the process a moment to exit.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: serde_json::json!(1),
        method: "test/noop".into(),
        params: None,
    };

    let result = transport.send_request(request).await;
    assert!(
        result.is_err(),
        "sending a request to an exited process should fail"
    );
}

#[tokio::test]
async fn content_length_framing_roundtrip_via_cat() {
    // Verify that write_message produces correct Content-Length framing
    // by reading back through read_message (both go through cat).
    // We send two notifications then one request. The request triggers
    // a read, and cat will have queued all three framed messages.
    // read_message in send_request skips messages without "id" (notifications),
    // so it should skip the first two and return the request-echo as response.
    let mut transport = LspStdioTransport::spawn("cat", &[], empty_env(), None)
        .await
        .expect("cat should spawn");

    // Send two notifications (no "id" — will be skipped by send_request's loop).
    for i in 0..2 {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".into(),
            method: format!("test/notify{i}"),
            params: None,
        };
        transport
            .send_notification(notification)
            .await
            .expect("notification should succeed");
    }

    // Send a request (has "id" — will be matched).
    let request = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: serde_json::json!(42),
        method: "test/roundtrip".into(),
        params: Some(serde_json::json!({"key": "value"})),
    };

    let response = transport
        .send_request(request)
        .await
        .expect("roundtrip request should succeed");

    assert_eq!(response.id, serde_json::json!(42));
    assert_eq!(response.jsonrpc, "2.0");

    transport.close().await.ok();
}
