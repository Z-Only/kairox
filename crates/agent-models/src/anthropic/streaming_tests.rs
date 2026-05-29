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
