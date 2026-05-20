//! SSE integration tests using wiremock to mock an MCP server endpoint.

use agent_mcp::transport::sse::SseTransport;
use agent_mcp::transport::Transport;
use agent_mcp::types::JsonRpcRequest;
use std::collections::HashMap;
use std::time::Duration;

/// Helper: create a minimal JSON-RPC success response as SSE event.
fn sse_event(data: &str) -> String {
    format!("event: message\ndata: {data}\n\n")
}

#[tokio::test]
async fn sse_connects_and_sends_request() {
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    let response_json =
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {"name": "test", "version": "1.0"}})
            .to_string();

    Mock::given(method("GET"))
        .and(path("/sse"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_delay(Duration::from_millis(50))
                .set_body_string(sse_event(&response_json)),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/message"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        })))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut transport = SseTransport::new(&base_url, HashMap::new(), None)
        .await
        .expect("Failed to create SseTransport");

    let request = JsonRpcRequest::new(1, "initialize", Some(serde_json::json!({})));
    let resp = transport
        .send_request(request)
        .await
        .expect("SSE request should return the matching response");

    assert_eq!(resp.id, serde_json::json!(1));
    assert_eq!(
        resp.result,
        serde_json::json!({"name": "test", "version": "1.0"})
    );

    transport.close().await.ok();
}

#[tokio::test]
async fn sse_handles_connection_error() {
    // SseTransport::new starts an SSE listener in the background that connects
    // to the /sse endpoint. If the server is unreachable, the connection may
    // still succeed (the SSE listener retries in the background), but actual
    // requests should fail. Test that sending a request to a dead server fails.
    let mut transport = SseTransport::new("http://127.0.0.1:1/mcp", HashMap::new(), None)
        .await
        .expect("Transport creation should not block on connection");

    // Sending a request should fail because the POST endpoint is unreachable
    let request = JsonRpcRequest::new(1, "test", None);
    let result = transport.send_request(request).await;
    assert!(
        result.is_err(),
        "Expected error when sending to dead server"
    );

    transport.close().await.ok();
}

#[tokio::test]
async fn sse_custom_headers_sent() {
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    let response_json =
        serde_json::json!({"jsonrpc": "2.0", "id": 7, "result": {"ok": true}}).to_string();

    // Mount SSE endpoint with header expectation
    Mock::given(method("GET"))
        .and(path("/sse"))
        .and(header("x-custom-header", "custom-value"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_delay(Duration::from_millis(50))
                .set_body_string(sse_event(&response_json)),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/message"))
        .and(header("x-custom-header", "custom-value"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "test"
        })))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut transport = SseTransport::new(
        &base_url,
        HashMap::from([("x-custom-header".into(), "custom-value".into())]),
        None,
    )
    .await
    .expect("Failed to create SseTransport with custom headers")
    .with_request_timeout(Duration::from_millis(500));

    let request = JsonRpcRequest::new(7, "test", None);
    let resp = transport
        .send_request(request)
        .await
        .expect("SSE request with custom headers should return the matching response");

    assert_eq!(resp.id, serde_json::json!(7));
    assert_eq!(resp.result, serde_json::json!({"ok": true}));

    transport.close().await.ok();
}

#[tokio::test]
async fn sse_send_notification() {
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/sse"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(""),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/message"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut transport = SseTransport::new(&base_url, HashMap::new(), None)
        .await
        .expect("Failed to create SseTransport");

    let notification = agent_mcp::types::JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "notifications/initialized".to_string(),
        params: None,
    };

    // Notification should complete without hanging
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        transport.send_notification(notification),
    )
    .await;

    result
        .expect("send_notification timed out")
        .expect("send_notification should POST to /message successfully");

    transport.close().await.ok();
}

#[tokio::test]
async fn sse_request_timeout_when_no_matching_response() {
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    // SSE stream returns events for a different id — id=1 times out.
    let other_response =
        json!({"jsonrpc": "2.0", "id": 999, "result": {"ignored": true}}).to_string();

    Mock::given(method("GET"))
        .and(path("/sse"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_event(&other_response)),
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
        .expect("Failed to create SseTransport")
        .with_request_timeout(Duration::from_millis(100));

    let request = JsonRpcRequest::new(1, "initialize", Some(json!({})));
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

    transport.close().await.ok();
}
