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

    let mut transport =
        StreamableHttpTransport::new(&format!("{}/mcp", mock_server.uri()), HashMap::new(), None)
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

    let mut transport =
        StreamableHttpTransport::new(&format!("{}/mcp/", mock_server.uri()), HashMap::new(), None)
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

    let mut transport =
        StreamableHttpTransport::new(&format!("{}/mcp", mock_server.uri()), HashMap::new(), None)
            .await
            .expect("transport should be created");

    let response = transport
        .send_request(JsonRpcRequest::new(7, "tools/list", Some(json!({}))))
        .await
        .expect("SSE response should be parsed");

    assert_eq!(response.result, json!({"tools": []}));
}
