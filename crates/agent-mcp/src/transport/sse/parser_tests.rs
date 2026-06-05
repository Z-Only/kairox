use serde_json::{json, Value};

use super::parser::{parse_sse_response, SseResponse};

// ── Success parsing ──────────────────────────────────────────────────

#[test]
fn parse_valid_success_response() {
    let data = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    match resp {
        SseResponse::Success(r) => {
            assert_eq!(r.id, json!(1));
            assert_eq!(r.result, json!({"tools": []}));
        }
        SseResponse::Error { .. } => panic!("expected Success, got Error"),
    }
}

// ── Error parsing ────────────────────────────────────────────────────

#[test]
fn parse_valid_error_response() {
    let data = r#"{"jsonrpc":"2.0","id":42,"error":{"code":-32600,"message":"invalid request"}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    match resp {
        SseResponse::Error { id, code, message } => {
            assert_eq!(id, json!(42));
            assert_eq!(code, -32600);
            assert_eq!(message, "invalid request");
        }
        SseResponse::Success(_) => panic!("expected Error, got Success"),
    }
}

// ── None cases ───────────────────────────────────────────────────────

#[test]
fn parse_empty_string_returns_none() {
    assert!(parse_sse_response("").is_none());
}

#[test]
fn parse_whitespace_only_returns_none() {
    assert!(parse_sse_response("   \n\t  ").is_none());
}

#[test]
fn parse_invalid_json_returns_none() {
    assert!(parse_sse_response("{not json}").is_none());
}

#[test]
fn parse_json_array_returns_none() {
    assert!(parse_sse_response("[1, 2, 3]").is_none());
}

#[test]
fn parse_json_object_missing_id_returns_none() {
    let data = r#"{"jsonrpc":"2.0","result":{}}"#;
    assert!(parse_sse_response(data).is_none());
}

// ── Error field defaults ─────────────────────────────────────────────

#[test]
fn parse_error_missing_code_defaults_to_negative_one() {
    let data = r#"{"jsonrpc":"2.0","id":1,"error":{"message":"oops"}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    match resp {
        SseResponse::Error { code, .. } => assert_eq!(code, -1),
        SseResponse::Success(_) => panic!("expected Error"),
    }
}

#[test]
fn parse_error_missing_message_defaults_to_unknown() {
    let data = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    match resp {
        SseResponse::Error { message, .. } => assert_eq!(message, "unknown error"),
        SseResponse::Success(_) => panic!("expected Error"),
    }
}

// ── SseResponse::id() ────────────────────────────────────────────────

#[test]
fn id_returns_correct_value_for_success() {
    let data = r#"{"jsonrpc":"2.0","id":"req-abc","result":null}"#;
    let resp = parse_sse_response(data).expect("should parse");
    assert_eq!(resp.id(), &json!("req-abc"));
}

#[test]
fn id_returns_correct_value_for_error() {
    let data = r#"{"jsonrpc":"2.0","id":99,"error":{"code":-1,"message":"fail"}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    assert_eq!(resp.id(), &json!(99));
}

// ── Id type variations ───────────────────────────────────────────────

#[test]
fn parse_string_id() {
    let data = r#"{"jsonrpc":"2.0","id":"hello","result":{}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    assert_eq!(resp.id(), &Value::String("hello".into()));
}

#[test]
fn parse_numeric_id() {
    let data = r#"{"jsonrpc":"2.0","id":7,"result":{}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    assert_eq!(resp.id(), &json!(7));
}

#[test]
fn parse_null_id() {
    let data = r#"{"jsonrpc":"2.0","id":null,"result":{}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    assert_eq!(resp.id(), &Value::Null);
}

// ── Edge cases ───────────────────────────────────────────────────────

#[test]
fn parse_response_with_leading_trailing_whitespace() {
    let data = "  \n  {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":true}  \n  ";
    let resp = parse_sse_response(data).expect("should parse trimmed data");
    match resp {
        SseResponse::Success(r) => {
            assert_eq!(r.id, json!(1));
            assert_eq!(r.result, json!(true));
        }
        SseResponse::Error { .. } => panic!("expected Success"),
    }
}

#[test]
fn parse_error_with_non_integer_code_defaults() {
    // code is a string instead of a number — as_i64() returns None → defaults to -1
    let data = r#"{"jsonrpc":"2.0","id":1,"error":{"code":"NaN","message":"bad"}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    match resp {
        SseResponse::Error { code, message, .. } => {
            assert_eq!(code, -1);
            assert_eq!(message, "bad");
        }
        SseResponse::Success(_) => panic!("expected Error"),
    }
}

#[test]
fn parse_error_with_empty_error_object() {
    // error object exists but has neither code nor message
    let data = r#"{"jsonrpc":"2.0","id":1,"error":{}}"#;
    let resp = parse_sse_response(data).expect("should parse");
    match resp {
        SseResponse::Error { code, message, .. } => {
            assert_eq!(code, -1);
            assert_eq!(message, "unknown error");
        }
        SseResponse::Success(_) => panic!("expected Error"),
    }
}

#[test]
fn parse_object_with_id_but_no_result_or_error_returns_none() {
    // Has "id" but neither "result" nor "error" — not a valid JSON-RPC response
    let data = r#"{"jsonrpc":"2.0","id":1}"#;
    // error check: error field is absent so it won't be an Error variant.
    // success check: JsonRpcResponse requires "result", so from_str fails.
    assert!(parse_sse_response(data).is_none());
}
