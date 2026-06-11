use super::*;

#[test]
fn parses_token_delta_from_chunk() {
    let data = r#"{"choices":[{"delta":{"content":"Hello"},"index":0}]}"#;
    let events = parse_openai_chunk(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        OpenAiChunkEvent::Event(ModelEvent::TokenDelta(t)) => assert_eq!(t, "Hello"),
        _ => panic!("expected TokenDelta event"),
    }
}

#[test]
fn parses_tool_call_start_from_chunk() {
    let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"fs.read","arguments":"{\"pa"}}]},"index":0}]}"#;
    let events = parse_openai_chunk(data).unwrap();
    assert_eq!(events.len(), 2);
    match &events[0] {
        OpenAiChunkEvent::ToolCallStarted { index, id, name } => {
            assert_eq!(*index, 0);
            assert_eq!(id, "call_1");
            assert_eq!(name, "fs.read");
        }
        _ => panic!("expected ToolCallStarted"),
    }
    match &events[1] {
        OpenAiChunkEvent::ToolCallArgumentDelta {
            index,
            partial_arguments,
        } => {
            assert_eq!(*index, 0);
            assert_eq!(partial_arguments, "{\"pa");
        }
        _ => panic!("expected ToolCallArgumentDelta"),
    }
}

#[test]
fn parses_tool_call_argument_delta_chunk() {
    let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\": \"README.md\"}"}}]},"index":0}]}"#;
    let events = parse_openai_chunk(data).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        OpenAiChunkEvent::ToolCallArgumentDelta {
            index,
            partial_arguments,
        } => {
            assert_eq!(*index, 0);
            assert_eq!(partial_arguments, "th\": \"README.md\"}");
        }
        _ => panic!("expected ToolCallArgumentDelta"),
    }
}

#[test]
fn parses_completion_event() {
    let data = r#"{"choices":[{"finish_reason":"stop","index":0}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
    let events = parse_openai_chunk(data).unwrap();
    assert!(matches!(
        &events[0],
        OpenAiChunkEvent::Event(ModelEvent::Completed { usage: Some(u) })
        if u.input_tokens == 10 && u.output_tokens == 5
    ));
}

#[test]
fn maps_top_level_error_chunk_to_failed_event() {
    let data = r#"{"error":{"message":"rate limit exceeded","type":"rate_limit_error","code":"rate_limit_exceeded"},"type":"error"}"#;

    let events = parse_openai_chunk(data).unwrap();

    assert_eq!(events.len(), 1);
    match &events[0] {
        OpenAiChunkEvent::Event(ModelEvent::Failed { message }) => {
            assert!(message.contains("rate limit exceeded"), "{message}");
            assert!(message.contains("rate_limit_error"), "{message}");
            assert!(message.contains("rate_limit_exceeded"), "{message}");
        }
        _ => panic!("expected Failed event"),
    }
}
