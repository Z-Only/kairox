use super::*;

#[test]
fn parses_content_block_delta_text() {
    let data =
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::Event(ModelEvent::TokenDelta(t)) => assert_eq!(t, "Hello"),
        _ => panic!("expected TokenDelta event"),
    }
}

#[test]
fn parses_content_block_start_tool_use() {
    let data = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01","name":"shell_exec"}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::ToolUseStarted { id, name } => {
            assert_eq!(id, "toolu_01");
            assert_eq!(name, "shell_exec");
        }
        _ => panic!("expected ToolUseStarted event"),
    }
}

#[test]
fn parses_content_block_delta_input_json() {
    let data = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"command\":"}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::ToolUseArgumentDelta { partial_json } => {
            assert_eq!(partial_json, "{\"command\":");
        }
        _ => panic!("expected ToolUseArgumentDelta event"),
    }
}

#[test]
fn parses_content_block_stop() {
    let data = r#"{"type":"content_block_stop","index":1}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::ToolUseFinished => {}
        _ => panic!("expected ToolUseFinished event"),
    }
}

#[test]
fn parses_message_delta_end_turn() {
    let data = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"input_tokens":10,"output_tokens":5}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::Event(ModelEvent::Completed { usage: Some(u) }) => {
            assert_eq!(u.input_tokens, 10);
            assert_eq!(u.output_tokens, 5);
        }
        _ => panic!("expected Completed event"),
    }
}

#[test]
fn parses_message_delta_tool_use_stop() {
    let data = r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"input_tokens":20,"output_tokens":10}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::Event(ModelEvent::Completed { usage }) => {
            assert_eq!(usage.as_ref().unwrap().output_tokens, 10);
        }
        _ => panic!("expected Completed event"),
    }
}

#[test]
fn parses_error_event() {
    let data = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::Event(ModelEvent::Failed { message }) => {
            assert_eq!(message, "Overloaded");
        }
        _ => panic!("expected Failed event"),
    }
}

#[test]
fn ignores_ping_and_start_events() {
    let data = r#"{"type":"ping"}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert!(events.is_empty());

    let data = r#"{"type":"message_start","message":{"id":"msg_123"}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert!(events.is_empty());
}

#[test]
fn parse_json_response_handles_tool_use() {
    let data = r#"{
        "id": "msg_01",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "text", "text": "I'll list the files."},
            {"type": "tool_use", "id": "toolu_01", "name": "shell_exec", "input": {"command": "ls"}}
        ],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 100, "output_tokens": 50}
    }"#;
    let events = parse_anthropic_json_response(data).unwrap();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t == "I'll list the files."));
    match &events[1] {
        ModelEvent::ToolCallRequested {
            tool_call_id,
            tool_id,
            arguments,
        } => {
            assert_eq!(tool_call_id, "toolu_01");
            assert_eq!(tool_id, "shell_exec");
            assert_eq!(arguments["command"], "ls");
        }
        _ => panic!("expected ToolCallRequested"),
    }
    assert!(
        matches!(&events[2], ModelEvent::Completed { usage: Some(u) } if u.input_tokens == 100 && u.output_tokens == 50)
    );
}

// ── Server-side tool streaming tests ───────────────────────────────────

#[test]
fn parses_server_tool_use_content_block() {
    let data = r#"{"type":"content_block_start","index":1,"content_block":{"type":"server_tool_use","id":"srvtoolu_01","name":"web_search"}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::Event(ModelEvent::TokenDelta(t)) => {
            assert!(t.contains("web_search"), "expected server tool name in delta: {t}");
        }
        other => panic!("expected TokenDelta for server_tool_use, got: {other:?}"),
    }
}

#[test]
fn parses_web_search_tool_result_content_block() {
    let data = r#"{"type":"content_block_start","index":2,"content_block":{"type":"web_search_tool_result","tool_use_id":"srvtoolu_01","content":[{"type":"web_search_result","title":"Example","url":"https://example.com","page_content":"Example page content"}]}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::Event(ModelEvent::TokenDelta(t)) => {
            assert!(t.contains("Example"), "expected title in delta: {t}");
            assert!(
                t.contains("https://example.com"),
                "expected url in delta: {t}"
            );
        }
        other => panic!("expected TokenDelta for web_search_tool_result, got: {other:?}"),
    }
}

#[test]
fn parses_code_execution_tool_result_content_block() {
    let data = r#"{"type":"content_block_start","index":2,"content_block":{"type":"code_execution_tool_result","tool_use_id":"srvtoolu_02","content":[{"type":"code_execution_output","stdout":"42\n","stderr":""},{"type":"code_execution_result","return_value":"42"}]}}"#;
    let events = parse_anthropic_raw_events(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AnthropicRawEvent::Event(ModelEvent::TokenDelta(t)) => {
            assert!(t.contains("42"), "expected stdout/return in delta: {t}");
        }
        other => panic!("expected TokenDelta for code_execution_tool_result, got: {other:?}"),
    }
}

#[test]
fn parse_json_response_handles_server_tool_use_and_results() {
    let data = r#"{
        "id": "msg_02",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "text", "text": "Let me search for that."},
            {"type": "server_tool_use", "id": "srvtoolu_01", "name": "web_search", "input": {"query": "Rust programming"}},
            {"type": "web_search_tool_result", "tool_use_id": "srvtoolu_01", "content": [
                {"type": "web_search_result", "title": "Rust Lang", "url": "https://rust-lang.org", "page_content": "A systems language"}
            ]},
            {"type": "text", "text": "Here's what I found."}
        ],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 200, "output_tokens": 100}
    }"#;
    let events = parse_anthropic_json_response(data).unwrap();
    // Expected: text, server_tool_use delta, web_search_result delta, text, completed
    assert!(events.len() >= 4, "expected at least 4 events, got: {events:?}");
    // First text
    assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t.contains("search")));
    // Server tool use
    assert!(matches!(&events[1], ModelEvent::TokenDelta(t) if t.contains("web_search")));
    // Web search results
    assert!(matches!(&events[2], ModelEvent::TokenDelta(t) if t.contains("Rust Lang")));
    // Second text
    assert!(matches!(&events[3], ModelEvent::TokenDelta(t) if t.contains("found")));
    // Completed
    assert!(events.iter().any(|e| matches!(e, ModelEvent::Completed { .. })));
}

#[test]
fn parse_json_response_handles_code_execution_result() {
    let data = r#"{
        "id": "msg_03",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "server_tool_use", "id": "srvtoolu_02", "name": "code_execution", "input": {"code": "print(1+1)"}},
            {"type": "code_execution_tool_result", "tool_use_id": "srvtoolu_02", "content": [
                {"type": "code_execution_output", "stdout": "2\n", "stderr": ""},
                {"type": "code_execution_result", "return_value": "2"}
            ]}
        ],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 50, "output_tokens": 30}
    }"#;
    let events = parse_anthropic_json_response(data).unwrap();
    // server_tool_use delta + code_execution_result delta + completed
    assert!(events.len() >= 3, "expected at least 3 events, got: {events:?}");
    assert!(events.iter().any(|e| matches!(e, ModelEvent::TokenDelta(t) if t.contains("code_execution"))));
    assert!(events.iter().any(|e| matches!(e, ModelEvent::TokenDelta(t) if t.contains("2"))));
    assert!(events.iter().any(|e| matches!(e, ModelEvent::Completed { .. })));
}
