use std::collections::HashMap;
use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::types::JsonRpcRequest;
use crate::{transport::Transport, JsonRpcNotification};

use super::parser::{parse_sse_response, SseResponse};
use super::SseTransport;

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

#[tokio::test]
async fn sse_transport_request_timeout_when_no_matching_response() {
    let mock_server = MockServer::start().await;

    // SSE returns events for a different id — id=1 will never get a response.
    let wrong_response = r#"{"jsonrpc":"2.0","id":999,"result":{"wrong":true}}"#;
    let sse_events = sse_body(&[wrong_response]);

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
        .expect("failed to create SseTransport")
        .with_request_timeout(Duration::from_millis(100));

    // Give the SSE listener time to connect and consume the (non-matching) events.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let request = JsonRpcRequest::new(1, "tools/list", Some(json!({})));
    let result = transport.send_request(request).await;

    assert!(
        result.is_err(),
        "send_request should time out when SSE delivers no matching response"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.to_lowercase().contains("timed out"),
        "error should mention timeout, got: {err_msg}"
    );

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
    let data = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"Invalid Request"}}"#;
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

#[tokio::test]
async fn sse_url_validation() {
    let mock_server = MockServer::start().await;

    // Mock the SSE endpoint so the background listener doesn't fail.
    Mock::given(method("GET"))
        .and(path("/sse"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(""),
        )
        .mount(&mock_server)
        .await;

    // Test 1: URL without trailing slash produces correct message URL.
    let url_no_slash = mock_server.uri();
    let mut transport = SseTransport::new(&url_no_slash, HashMap::new(), None)
        .await
        .expect("creating SseTransport with valid URL should succeed");
    let expected = format!("{}/message", url_no_slash);
    assert_eq!(
        transport.message_url(),
        expected,
        "message_url() should be {{base}}/message"
    );
    transport.close().await.ok();

    // Test 2: URL with trailing slash — trailing slash should be stripped.
    let url_with_slash = format!("{}/", mock_server.uri());
    let mut transport2 = SseTransport::new(&url_with_slash, HashMap::new(), None)
        .await
        .expect("creating SseTransport with trailing-slash URL should succeed");
    // The trailing slash should be stripped, so message_url() matches the
    // same as without the trailing slash.
    assert_eq!(
        transport2.message_url(),
        format!("{}/message", url_no_slash),
        "URL with trailing slash should produce same message_url() as without"
    );
    transport2.close().await.ok();
}

#[tokio::test]
async fn sse_url_invalid_rejected_at_connection() {
    // An obviously malformed URL (missing scheme) should fail when the
    // background SSE listener attempts to connect.
    let result = SseTransport::new("not-a-valid-url", HashMap::new(), None).await;
    // Construction itself succeeds (it spawns a background task), but the
    // SSE listener will fail to connect and log warnings.
    // We verify that new() itself does not panic on invalid input.
    assert!(
        result.is_ok(),
        "SseTransport::new should not panic on invalid URL"
    );
    let mut transport = result.unwrap();
    transport.close().await.ok();
}
