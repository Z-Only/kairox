// Integration tests for the event emitter / broadcast channel.
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use futures::StreamExt;

/// subscribe_session() must forward at least UserMessageAdded, ModelTokenDelta,
/// and AssistantMessageCompleted events to the broadcast stream.
#[tokio::test]
async fn event_emitter_forwards_key_payload_types() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    // Two tokens so we get at least two ModelTokenDelta events in the stream.
    let model = FakeModelClient::new(vec!["Hello".into(), " world".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-emitter".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // Subscribe before sending so we don't miss early events.
    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello emitter test".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    // Drain the in-flight events (the agent loop has already completed, so
    // all events are buffered in the channel).
    let mut received_types: Vec<String> = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(1000);
    loop {
        match tokio::time::timeout(std::time::Duration::from_millis(50), event_stream.next()).await
        {
            Ok(Some(event)) => {
                received_types.push(event.event_type.clone());
                // Cap to avoid spinning forever if the channel gets noisy.
                if received_types.len() > 30 {
                    break;
                }
            }
            Ok(None) | Err(_) => {
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
                // Give the channel a moment to flush.
                tokio::task::yield_now().await;
            }
        }
    }

    assert!(
        received_types.iter().any(|t| t == "UserMessageAdded"),
        "subscribe_session should receive UserMessageAdded. Got: {:?}",
        received_types
    );
    assert!(
        received_types.iter().any(|t| t == "ModelTokenDelta"),
        "subscribe_session should receive ModelTokenDelta. Got: {:?}",
        received_types
    );
    assert!(
        received_types
            .iter()
            .any(|t| t == "AssistantMessageCompleted"),
        "subscribe_session should receive AssistantMessageCompleted. Got: {:?}",
        received_types
    );
    assert!(
        received_types.iter().any(|t| t == "AgentTaskCompleted"),
        "subscribe_session should receive AgentTaskCompleted. Got: {:?}",
        received_types
    );

    // We should receive at least these 4 distinct event types.
    assert!(
        received_types.len() >= 4,
        "Should receive at least 4 events. Got {}: {:?}",
        received_types.len(),
        received_types
    );
}
