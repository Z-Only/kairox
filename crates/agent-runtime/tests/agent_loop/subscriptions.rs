//! Live event subscriptions surfaced via `subscribe_session`.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use futures::StreamExt;

#[tokio::test]
async fn subscribe_session_receives_events() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-subscribe".into())
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

    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    // Collect events from the stream
    let mut received_types = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        match tokio::time::timeout(std::time::Duration::from_millis(100), event_stream.next()).await
        {
            Ok(Some(event)) => {
                received_types.push(event.event_type.to_string());
                if received_types.len() > 10 {
                    break;
                }
            }
            Ok(None) | Err(_) => {
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
            }
        }
    }

    assert!(
        received_types.iter().any(|t| t == "UserMessageAdded"),
        "subscribe_session should receive UserMessageAdded, got: {:?}",
        received_types
    );
}
