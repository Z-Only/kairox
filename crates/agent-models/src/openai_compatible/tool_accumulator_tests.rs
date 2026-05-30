use super::*;

#[test]
fn accumulates_tool_call_across_chunks() {
    let mut acc = OpenAiToolCallAccumulator::new();

    let events = acc.process(OpenAiChunkEvent::Event(ModelEvent::TokenDelta(
        "Reading file...".into(),
    )));
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t == "Reading file..."));

    let events = acc.process(OpenAiChunkEvent::ToolCallStarted {
        index: 0,
        id: "call_abc".into(),
        name: "fs.read".into(),
    });
    assert!(events.is_empty());

    let events = acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
        index: 0,
        partial_arguments: "{\"pa".into(),
    });
    assert!(events.is_empty());

    let events = acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
        index: 0,
        partial_arguments: "th\": \"README.md\"}".into(),
    });
    assert!(events.is_empty());

    let events = acc.process(OpenAiChunkEvent::Event(ModelEvent::Completed {
        usage: None,
    }));
    assert_eq!(events.len(), 1);

    let events = acc.flush();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ModelEvent::ToolCallRequested {
            tool_call_id,
            tool_id,
            arguments,
        } => {
            assert_eq!(tool_call_id, "call_abc");
            assert_eq!(tool_id, "fs.read");
            assert_eq!(arguments["path"], "README.md");
        }
        _ => panic!("expected ToolCallRequested"),
    }
}

#[test]
fn handles_multiple_tool_calls() {
    let mut acc = OpenAiToolCallAccumulator::new();

    acc.process(OpenAiChunkEvent::ToolCallStarted {
        index: 0,
        id: "call_1".into(),
        name: "fs.read".into(),
    });
    acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
        index: 0,
        partial_arguments: "{\"path\":\"README.md\"}".into(),
    });

    acc.process(OpenAiChunkEvent::ToolCallStarted {
        index: 1,
        id: "call_2".into(),
        name: "shell.exec".into(),
    });
    acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
        index: 1,
        partial_arguments: "{\"command\":\"ls\"}".into(),
    });

    let mut events = acc.flush();
    assert_eq!(events.len(), 2);

    events.sort_by_key(|e| match e {
        ModelEvent::ToolCallRequested { tool_call_id, .. } => tool_call_id.clone(),
        _ => String::new(),
    });

    match &events[0] {
        ModelEvent::ToolCallRequested {
            tool_call_id,
            tool_id,
            arguments,
        } => {
            assert_eq!(tool_call_id, "call_1");
            assert_eq!(tool_id, "fs.read");
            assert_eq!(arguments["path"], "README.md");
        }
        _ => panic!("expected ToolCallRequested"),
    }
    match &events[1] {
        ModelEvent::ToolCallRequested {
            tool_call_id,
            tool_id,
            arguments,
        } => {
            assert_eq!(tool_call_id, "call_2");
            assert_eq!(tool_id, "shell.exec");
            assert_eq!(arguments["command"], "ls");
        }
        _ => panic!("expected ToolCallRequested"),
    }
}
