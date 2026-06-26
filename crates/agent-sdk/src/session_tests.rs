use super::*;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, TaskId, WorkspaceId,
};

fn make_event(payload: EventPayload) -> DomainEvent {
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        payload,
    )
}

// ── StreamEvent::from_domain_event ──────────────────────────────────

#[test]
fn from_tool_invocation_started() {
    let event = make_event(EventPayload::ToolInvocationStarted {
        invocation_id: "inv-1".into(),
        tool_id: "fs.write".into(),
        input_preview: String::new(),
    });
    let stream_event = StreamEvent::from_domain_event(event);
    match stream_event {
        StreamEvent::ToolCall {
            tool_name,
            tool_input,
        } => {
            assert_eq!(tool_name, "fs.write");
            assert_eq!(tool_input, serde_json::Value::Null);
        }
        other => panic!("expected ToolCall, got: {other:?}"),
    }
}

#[test]
fn from_agent_task_failed() {
    let event = make_event(EventPayload::AgentTaskFailed {
        task_id: TaskId::new(),
        error: "out of memory".into(),
    });
    let stream_event = StreamEvent::from_domain_event(event);
    match stream_event {
        StreamEvent::Error(msg) => assert_eq!(msg, "out of memory"),
        other => panic!("expected Error, got: {other:?}"),
    }
}

#[test]
fn from_tool_invocation_failed() {
    let event = make_event(EventPayload::ToolInvocationFailed {
        invocation_id: "inv-2".into(),
        tool_id: "shell.exec".into(),
        error: "command not found".into(),
    });
    let stream_event = StreamEvent::from_domain_event(event);
    match stream_event {
        StreamEvent::Error(msg) => assert_eq!(msg, "command not found"),
        other => panic!("expected Error, got: {other:?}"),
    }
}

#[test]
fn from_unmatched_variant_maps_to_other() {
    let event = make_event(EventPayload::SessionInitialized {
        model_profile: "default".into(),
    });
    let stream_event = StreamEvent::from_domain_event(event);
    assert!(
        matches!(stream_event, StreamEvent::Other(_)),
        "expected Other, got: {stream_event:?}"
    );
}

#[test]
fn from_workspace_opened_maps_to_other() {
    let event = make_event(EventPayload::WorkspaceOpened {
        path: "/tmp".into(),
    });
    let stream_event = StreamEvent::from_domain_event(event);
    assert!(
        matches!(stream_event, StreamEvent::Other(_)),
        "expected Other, got: {stream_event:?}"
    );
}

// ── CollectedResponse ───────────────────────────────────────────────

#[test]
fn collected_response_debug() {
    let response = CollectedResponse {
        text: "hello".into(),
        events: vec![
            StreamEvent::Text("hello".into()),
            StreamEvent::TurnCompleted,
        ],
    };
    let debug = format!("{response:?}");
    assert!(debug.contains("hello"), "missing text: {debug}");
    assert!(debug.contains("TurnCompleted"), "missing event: {debug}");
}

#[test]
fn collected_response_clone() {
    let original = CollectedResponse {
        text: "result".into(),
        events: vec![StreamEvent::TurnCompleted],
    };
    let cloned = original.clone();
    assert_eq!(cloned.text, "result");
    assert_eq!(cloned.events.len(), 1);
    assert!(matches!(cloned.events[0], StreamEvent::TurnCompleted));
}

// ── MessageStream completed flag ────────────────────────────────────

#[tokio::test]
async fn message_stream_stops_after_turn_completed() {
    use futures::stream;

    let turn_completed_event = make_event(EventPayload::AssistantMessageCompleted {
        message_id: "msg-done".into(),
        content: "finished".into(),
    });
    let trailing_event = make_event(EventPayload::ModelTokenDelta {
        delta: "should not appear".into(),
    });

    let inner = stream::iter(vec![turn_completed_event, trailing_event]);
    let boxed: BoxStream<'static, DomainEvent> = Box::pin(inner);
    let mut message_stream = MessageStream::new(boxed, vec![]);

    // First poll: TurnCompleted
    let first = message_stream.next().await;
    assert!(
        matches!(first, Some(StreamEvent::TurnCompleted)),
        "expected TurnCompleted, got: {first:?}"
    );

    // Second poll: should be None because completed flag is set
    let second = message_stream.next().await;
    assert!(
        second.is_none(),
        "expected None after TurnCompleted, got: {second:?}"
    );
}

#[tokio::test]
async fn message_stream_stops_after_error() {
    use futures::stream;

    let error_event = make_event(EventPayload::AgentTaskFailed {
        task_id: TaskId::new(),
        error: "fatal".into(),
    });
    let trailing_event = make_event(EventPayload::ModelTokenDelta {
        delta: "nope".into(),
    });

    let inner = stream::iter(vec![error_event, trailing_event]);
    let boxed: BoxStream<'static, DomainEvent> = Box::pin(inner);
    let mut message_stream = MessageStream::new(boxed, vec![]);

    let first = message_stream.next().await;
    assert!(
        matches!(first, Some(StreamEvent::Error(ref e)) if e == "fatal"),
        "expected Error(fatal), got: {first:?}"
    );

    let second = message_stream.next().await;
    assert!(
        second.is_none(),
        "expected None after Error, got: {second:?}"
    );
}

#[tokio::test]
async fn message_stream_yields_text_events_before_completion() {
    use futures::stream;

    let events = vec![
        make_event(EventPayload::ModelTokenDelta {
            delta: "Hello".into(),
        }),
        make_event(EventPayload::ModelTokenDelta {
            delta: " world".into(),
        }),
        make_event(EventPayload::AssistantMessageCompleted {
            message_id: "msg-1".into(),
            content: "Hello world".into(),
        }),
    ];

    let inner = stream::iter(events);
    let boxed: BoxStream<'static, DomainEvent> = Box::pin(inner);
    let mut message_stream = MessageStream::new(boxed, vec![]);

    let first = message_stream.next().await;
    assert!(matches!(first, Some(StreamEvent::Text(ref t)) if t == "Hello"));

    let second = message_stream.next().await;
    assert!(matches!(second, Some(StreamEvent::Text(ref t)) if t == " world"));

    let third = message_stream.next().await;
    assert!(matches!(third, Some(StreamEvent::TurnCompleted)));

    let fourth = message_stream.next().await;
    assert!(fourth.is_none());
}

#[tokio::test]
async fn message_stream_empty_inner_returns_none() {
    use futures::stream;

    let inner = stream::iter(Vec::<DomainEvent>::new());
    let boxed: BoxStream<'static, DomainEvent> = Box::pin(inner);
    let mut message_stream = MessageStream::new(boxed, vec![]);

    let result = message_stream.next().await;
    assert!(result.is_none(), "expected None for empty stream");
}
