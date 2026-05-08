//! Verifies that the agent loop emits `ContextAssembled { usage }` with
//! `total_tokens <= budget_tokens` and that the `usage.by_source`
//! breakdown contains the expected categories.

use agent_core::{
    AppFacade, EventPayload, SendMessageRequest, SessionId, StartSessionRequest, WorkspaceId,
};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;

async fn build_test_runtime() -> Arc<LocalRuntime<SqliteEventStore, FakeModelClient>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let _ = pool; // pool no longer needed; use the canonical helper.
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["ok".into(); 16]);
    Arc::new(LocalRuntime::new(store, model))
}

async fn load_events(
    runtime: &LocalRuntime<SqliteEventStore, FakeModelClient>,
    session_id: &SessionId,
) -> Vec<agent_core::DomainEvent> {
    use agent_store::EventStore;
    runtime
        .event_store_for_test()
        .load_session(session_id)
        .await
        .unwrap()
}

#[tokio::test]
async fn context_assembled_event_emitted_with_budget_respected() {
    let runtime = build_test_runtime().await;

    let ws_info = runtime.open_workspace(".".into()).await.unwrap();
    let workspace_id: WorkspaceId = ws_info.workspace_id;
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    for i in 0..5 {
        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                content: format!("turn {} please", i),
            })
            .await
            .unwrap();
    }

    let events = load_events(&runtime, &session_id).await;
    let assembled: Vec<_> = events
        .iter()
        .filter_map(|e| match &e.payload {
            EventPayload::ContextAssembled { usage } => Some(usage),
            _ => None,
        })
        .collect();

    assert!(
        !assembled.is_empty(),
        "expected at least one ContextAssembled event"
    );
    for usage in &assembled {
        assert!(
            usage.total_tokens <= usage.budget_tokens,
            "ContextAssembled.total_tokens ({}) exceeded budget_tokens ({})",
            usage.total_tokens,
            usage.budget_tokens
        );
        assert_eq!(usage.estimator, "cl100k_base");
        assert_eq!(usage.context_window, 4_096); // FALLBACK_FAKE from model_registry
        assert!(usage
            .by_source
            .iter()
            .any(|(s, n)| matches!(s, agent_core::ContextSource::System) && *n > 0));
    }
}
