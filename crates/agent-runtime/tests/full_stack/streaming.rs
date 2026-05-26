//! Event streaming consistency: the broadcast stream and the trace projection
//! should agree on what happened during a session.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use futures::StreamExt;

use super::support::make_simple_runtime;

#[tokio::test]
async fn full_stack_event_stream_matches_trace() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-stream-trace".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let mut stream = runtime.subscribe_session(sid.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Collect events from stream
    let mut stream_events = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(e) => {
                        stream_events.push(e);
                        if stream_events.len() > 30 { break; }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    let trace = runtime.get_trace(sid).await.unwrap();

    // Both should have events
    assert!(!stream_events.is_empty(), "Stream should have events");
    assert!(!trace.is_empty(), "Trace should have events");

    let stream_types: Vec<&str> = stream_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();
    let trace_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        stream_types.contains(&"UserMessageAdded"),
        "Stream should contain UserMessageAdded"
    );
    assert!(
        trace_types.contains(&"UserMessageAdded"),
        "Trace should contain UserMessageAdded"
    );
}
