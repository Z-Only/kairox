use super::*;

#[test]
fn builds_safe_to_original_tool_name_map() {
    let tools = vec![ToolDefinition {
        name: "shell.exec".into(),
        description: "Execute a shell command".into(),
        parameters: serde_json::json!({"type": "object"}),
    }];

    let name_map = anthropic_tool_name_map(&tools);

    assert_eq!(name_map.get("shell_exec"), Some(&"shell.exec".to_string()));
}

#[test]
fn accumulates_tool_call_across_chunks() {
    let name_map = HashMap::from([
        ("shell_exec".to_string(), "shell.exec".to_string()),
        ("fs_read".to_string(), "fs.read".to_string()),
    ]);
    let mut acc = AnthropicToolCallAccumulator::new(name_map);

    let events = acc.process(AnthropicRawEvent::Event(ModelEvent::TokenDelta(
        "I'll list files.".into(),
    )));
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t == "I'll list files."));

    let events = acc.process(AnthropicRawEvent::ToolUseStarted {
        id: "toolu_01".into(),
        name: "shell_exec".into(),
    });
    assert!(events.is_empty());

    let events = acc.process(AnthropicRawEvent::ToolUseArgumentDelta {
        partial_json: "{\"command\":".into(),
    });
    assert!(events.is_empty());
    let events = acc.process(AnthropicRawEvent::ToolUseArgumentDelta {
        partial_json: " \"ls\"}".into(),
    });
    assert!(events.is_empty());

    let events = acc.process(AnthropicRawEvent::ToolUseFinished);
    assert_eq!(events.len(), 1);
    match &events[0] {
        ModelEvent::ToolCallRequested {
            tool_call_id,
            tool_id,
            arguments,
        } => {
            assert_eq!(tool_call_id, "toolu_01");
            assert_eq!(tool_id, "shell.exec");
            assert_eq!(arguments["command"], "ls");
        }
        _ => panic!("expected ToolCallRequested"),
    }

    let events = acc.process(AnthropicRawEvent::ToolUseFinished);
    assert!(events.is_empty());
}

#[test]
fn handles_unknown_tool_name() {
    let mut acc = AnthropicToolCallAccumulator::new(HashMap::new());

    acc.process(AnthropicRawEvent::ToolUseStarted {
        id: "toolu_02".into(),
        name: "custom_tool".into(),
    });
    acc.process(AnthropicRawEvent::ToolUseArgumentDelta {
        partial_json: "{}".into(),
    });
    let events = acc.process(AnthropicRawEvent::ToolUseFinished);
    assert_eq!(events.len(), 1);
    match &events[0] {
        ModelEvent::ToolCallRequested { tool_id, .. } => {
            assert_eq!(tool_id, "custom_tool");
        }
        _ => panic!("expected ToolCallRequested"),
    }
}
