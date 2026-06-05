use serde_json::json;

use super::*;

// ── ServerStatus ──

#[test]
fn server_status_variants_are_distinct() {
    let stopped = ServerStatus::Stopped;
    let starting = ServerStatus::Starting;
    let running = ServerStatus::Running;
    let error = ServerStatus::Error("boom".into());

    assert_ne!(stopped, starting);
    assert_ne!(stopped, running);
    assert_ne!(stopped, error);
    assert_ne!(starting, running);
    assert_ne!(running, error);
}

#[test]
fn server_status_equality() {
    assert_eq!(ServerStatus::Stopped, ServerStatus::Stopped);
    assert_eq!(ServerStatus::Starting, ServerStatus::Starting);
    assert_eq!(ServerStatus::Running, ServerStatus::Running);
    assert_eq!(
        ServerStatus::Error("x".into()),
        ServerStatus::Error("x".into())
    );
    assert_ne!(
        ServerStatus::Error("a".into()),
        ServerStatus::Error("b".into())
    );
}

#[test]
fn server_status_debug_format() {
    let dbg = format!("{:?}", ServerStatus::Error("fail".into()));
    assert!(dbg.contains("Error"));
    assert!(dbg.contains("fail"));
}

#[test]
fn server_status_clone() {
    let original = ServerStatus::Error("cloned".into());
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

// ── JsonRpcRequest ──

#[test]
fn json_rpc_request_serializes_with_params() {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        method: "initialize".to_string(),
        params: Some(json!({"rootUri": "file:///tmp"})),
    };
    let serialized = serde_json::to_value(&req).unwrap();
    assert_eq!(serialized["jsonrpc"], "2.0");
    assert_eq!(serialized["id"], 1);
    assert_eq!(serialized["method"], "initialize");
    assert_eq!(serialized["params"]["rootUri"], "file:///tmp");
}

#[test]
fn json_rpc_request_omits_null_params() {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(42),
        method: "shutdown".to_string(),
        params: None,
    };
    let serialized = serde_json::to_value(&req).unwrap();
    assert!(serialized.get("params").is_none());
}

// ── JsonRpcResponse ──

#[test]
fn json_rpc_response_deserializes_result() {
    let input = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {"capabilities": {}}
    });
    let resp: JsonRpcResponse = serde_json::from_value(input).unwrap();
    assert_eq!(resp.jsonrpc, "2.0");
    assert!(resp.result.is_some());
    assert!(resp.error.is_none());
}

#[test]
fn json_rpc_response_deserializes_error() {
    let input = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {"code": -32600, "message": "Invalid Request"}
    });
    let resp: JsonRpcResponse = serde_json::from_value(input).unwrap();
    assert!(resp.result.is_none());
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32600);
    assert_eq!(err.message, "Invalid Request");
    assert!(err.data.is_none());
}

#[test]
fn json_rpc_response_defaults_missing_fields() {
    let input = json!({"jsonrpc": "2.0", "id": 1});
    let resp: JsonRpcResponse = serde_json::from_value(input).unwrap();
    assert!(resp.result.is_none());
    assert!(resp.error.is_none());
}

// ── JsonRpcErrorObject ──

#[test]
fn json_rpc_error_object_with_data() {
    let input = json!({
        "code": -32601,
        "message": "Method not found",
        "data": {"detail": "unknown method"}
    });
    let err: JsonRpcErrorObject = serde_json::from_value(input).unwrap();
    assert_eq!(err.code, -32601);
    assert_eq!(err.message, "Method not found");
    assert!(err.data.is_some());
}

// ── JsonRpcNotification ──

#[test]
fn json_rpc_notification_serializes_with_params() {
    let notif = JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didOpen".to_string(),
        params: Some(json!({"uri": "file:///tmp/test.rs"})),
    };
    let serialized = serde_json::to_value(&notif).unwrap();
    assert_eq!(serialized["method"], "textDocument/didOpen");
    assert!(serialized.get("params").is_some());
}

#[test]
fn json_rpc_notification_omits_null_params() {
    let notif = JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "exit".to_string(),
        params: None,
    };
    let serialized = serde_json::to_value(&notif).unwrap();
    assert!(serialized.get("params").is_none());
}

// ── DAP protocol types ──

#[test]
fn dap_request_serializes() {
    let req = dap::DapRequest {
        seq: 1,
        msg_type: "request".to_string(),
        command: "initialize".to_string(),
        arguments: Some(json!({"clientID": "test"})),
    };
    let serialized = serde_json::to_value(&req).unwrap();
    assert_eq!(serialized["seq"], 1);
    assert_eq!(serialized["type"], "request");
    assert_eq!(serialized["command"], "initialize");
}

#[test]
fn dap_request_omits_null_arguments() {
    let req = dap::DapRequest {
        seq: 2,
        msg_type: "request".to_string(),
        command: "disconnect".to_string(),
        arguments: None,
    };
    let serialized = serde_json::to_value(&req).unwrap();
    assert!(serialized.get("arguments").is_none());
}

#[test]
fn dap_response_deserializes() {
    let input = json!({
        "seq": 1,
        "type": "response",
        "request_seq": 1,
        "command": "initialize",
        "success": true,
        "body": {"supportsConfigurationDoneRequest": true}
    });
    let resp: dap::DapResponse = serde_json::from_value(input).unwrap();
    assert_eq!(resp.seq, 1);
    assert_eq!(resp.msg_type, "response");
    assert_eq!(resp.request_seq, 1);
    assert!(resp.success);
    assert!(resp.body.is_some());
    assert!(resp.message.is_none());
}

#[test]
fn dap_response_failure() {
    let input = json!({
        "seq": 2,
        "type": "response",
        "request_seq": 1,
        "command": "launch",
        "success": false,
        "message": "program not found"
    });
    let resp: dap::DapResponse = serde_json::from_value(input).unwrap();
    assert!(!resp.success);
    assert_eq!(resp.message.as_deref(), Some("program not found"));
}

#[test]
fn dap_event_deserializes() {
    let input = json!({
        "seq": 5,
        "type": "event",
        "event": "stopped",
        "body": {"reason": "breakpoint", "threadId": 1}
    });
    let event: dap::DapEvent = serde_json::from_value(input).unwrap();
    assert_eq!(event.seq, 5);
    assert_eq!(event.event, "stopped");
    assert!(event.body.is_some());
}

#[test]
fn dap_event_without_body() {
    let input = json!({
        "seq": 3,
        "type": "event",
        "event": "initialized"
    });
    let event: dap::DapEvent = serde_json::from_value(input).unwrap();
    assert_eq!(event.event, "initialized");
    assert!(event.body.is_none());
}

#[test]
fn breakpoint_round_trips() {
    let bp = dap::Breakpoint {
        id: Some(1),
        verified: true,
        line: Some(42),
        message: None,
    };
    let json = serde_json::to_value(&bp).unwrap();
    let round_tripped: dap::Breakpoint = serde_json::from_value(json).unwrap();
    assert_eq!(round_tripped.id, Some(1));
    assert!(round_tripped.verified);
    assert_eq!(round_tripped.line, Some(42));
}

#[test]
fn breakpoint_defaults() {
    let input = json!({"verified": false});
    let bp: dap::Breakpoint = serde_json::from_value(input).unwrap();
    assert!(bp.id.is_none());
    assert!(!bp.verified);
    assert!(bp.line.is_none());
    assert!(bp.message.is_none());
}

#[test]
fn stack_frame_round_trips() {
    let frame = dap::StackFrame {
        id: 1,
        name: "main".to_string(),
        source: Some(dap::Source {
            name: Some("main.py".to_string()),
            path: Some("/tmp/main.py".to_string()),
        }),
        line: 10,
        column: 1,
    };
    let json = serde_json::to_value(&frame).unwrap();
    let round_tripped: dap::StackFrame = serde_json::from_value(json).unwrap();
    assert_eq!(round_tripped.id, 1);
    assert_eq!(round_tripped.name, "main");
    assert_eq!(round_tripped.line, 10);
    assert!(round_tripped.source.is_some());
}

#[test]
fn stack_frame_without_source() {
    let input = json!({
        "id": 2,
        "name": "<unknown>",
        "line": 0,
        "column": 0
    });
    let frame: dap::StackFrame = serde_json::from_value(input).unwrap();
    assert!(frame.source.is_none());
}

#[test]
fn scope_round_trips() {
    let scope = dap::Scope {
        name: "Local".to_string(),
        variables_reference: 1000,
        expensive: false,
    };
    let json = serde_json::to_value(&scope).unwrap();
    let round_tripped: dap::Scope = serde_json::from_value(json).unwrap();
    assert_eq!(round_tripped.name, "Local");
    assert_eq!(round_tripped.variables_reference, 1000);
    assert!(!round_tripped.expensive);
}

#[test]
fn variable_round_trips() {
    let var = dap::Variable {
        name: "x".to_string(),
        value: "42".to_string(),
        var_type: Some("int".to_string()),
        variables_reference: Some(0),
    };
    let json = serde_json::to_value(&var).unwrap();
    let round_tripped: dap::Variable = serde_json::from_value(json).unwrap();
    assert_eq!(round_tripped.name, "x");
    assert_eq!(round_tripped.value, "42");
    assert_eq!(round_tripped.var_type.as_deref(), Some("int"));
}

#[test]
fn variable_defaults() {
    let input = json!({"name": "y", "value": "hello"});
    let var: dap::Variable = serde_json::from_value(input).unwrap();
    assert_eq!(var.name, "y");
    assert!(var.var_type.is_none());
    assert!(var.variables_reference.is_none());
}

#[test]
fn source_defaults() {
    let input = json!({});
    let src: dap::Source = serde_json::from_value(input).unwrap();
    assert!(src.name.is_none());
    assert!(src.path.is_none());
}
