use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

#[tokio::test]
async fn fake_model_completes_full_session_and_trace_replays() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["done".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/kairox-e2e".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "complete this".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    let event_types: Vec<_> = trace
        .iter()
        .map(|entry| entry.event.event_type.clone())
        .collect();
    assert!(event_types.contains(&"UserMessageAdded".to_string()));
    assert!(event_types.contains(&"ModelTokenDelta".to_string()));
    assert!(event_types.contains(&"AssistantMessageCompleted".to_string()));

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.last().unwrap().content, "done");
}
