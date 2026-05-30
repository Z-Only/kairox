use super::*;
use serde_json::json;

#[test]
fn test_json_rpc_request_construction() {
    let req = JsonRpcRequest::new(
        1,
        "initialize",
        Some(json!({"protocolVersion": MCP_PROTOCOL_VERSION})),
    );
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, json!(1));
    assert_eq!(req.method, "initialize");

    let req_str = JsonRpcRequest::new_string_id("abc", "tools/list", None);
    assert_eq!(req_str.id, json!("abc"));
}

#[test]
fn test_server_capabilities_default() {
    let caps = ServerCapabilities::default();
    assert!(caps.tools.is_none());
    assert!(caps.resources.is_none());
    assert!(caps.prompts.is_none());
}

#[test]
fn test_server_capabilities_with_tools() {
    let json = json!({
        "tools": { "list_changed": true }
    });
    let caps: ServerCapabilities = serde_json::from_value(json).unwrap();
    assert!(caps.tools.is_some());
    assert_eq!(caps.tools.as_ref().unwrap().list_changed, Some(true));
}
