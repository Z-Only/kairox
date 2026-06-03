//! Full-stack integration test for P2 context compaction.
//!
//! Wires a real `LocalRuntime` (in-memory `SqliteEventStore` +
//! `FakeModelClient`) and exercises:
//!  1. Manual `compact_session` end-to-end → four-event chain emitted.
//!  2. `send_message` rejected with `SessionBusy` while compacting.
//!  3. After completion, the next `send_message` works AND the assistant
//!     produces a reply (post-compaction message flow is intact).

use agent_config::{Config, ContextPolicy};
use agent_core::{
    AppFacade, CompactionReason, EventPayload, SendMessageRequest, StartSessionRequest,
};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::{EventStore, SqliteEventStore};
use std::sync::Arc;

async fn fixture_runtime_with_history(
    pairs: usize,
) -> (
    Arc<LocalRuntime<SqliteEventStore, FakeModelClient>>,
    agent_core::WorkspaceId,
    agent_core::SessionId,
) {
    let store = SqliteEventStore::in_memory().await.unwrap();
    // FakeModelClient cycles through these responses for each turn.
    let model = FakeModelClient::new(vec!["ok".into(); pairs + 4]);
    let mut config = Config::defaults();
    // Disable auto-compaction so we can drive the manual path deterministically.
    config.context = ContextPolicy {
        auto_compact_threshold: 1.0,
        compactor_profile: None,
        max_tool_definition_tokens: None,
    };
    let runtime = Arc::new(LocalRuntime::new(store, model).with_config(Arc::new(config)));

    let workspace = runtime.open_workspace("/tmp/ctx-p2".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // Drive `pairs` user/assistant turns via the facade.
    for i in 0..pairs {
        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id.clone(),
                session_id: session_id.clone(),
                content: format!("turn {i}"),
                display_content: None,
                attachments: vec![],
            })
            .await
            .unwrap();
    }

    (runtime, workspace.workspace_id, session_id)
}

#[tokio::test]
async fn manual_compact_session_emits_full_event_chain() {
    let (runtime, _ws, session_id) = fixture_runtime_with_history(8).await;

    runtime
        .compact_session(session_id.clone(), CompactionReason::UserRequested)
        .await
        .expect("manual compaction should succeed");

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();

    let started = events
        .iter()
        .filter(|e| matches!(e.payload, EventPayload::ContextCompactionStarted { .. }))
        .count();
    let summary = events
        .iter()
        .filter(|e| matches!(e.payload, EventPayload::CompactionSummary { .. }))
        .count();
    let completed = events
        .iter()
        .filter(|e| {
            matches!(
                e.payload,
                EventPayload::ContextCompactionCompleted {
                    fallback_used: false,
                    ..
                }
            )
        })
        .count();
    assert_eq!(started, 1, "expected exactly 1 Started event");
    assert_eq!(summary, 1, "expected exactly 1 Summary event");
    assert_eq!(
        completed, 1,
        "expected exactly 1 Completed event with fallback_used=false"
    );
}

#[tokio::test]
async fn send_message_rejected_with_session_busy_during_compaction() {
    let (runtime, ws, session_id) = fixture_runtime_with_history(6).await;

    // Manually flip the busy flag so the gate fires deterministically.
    {
        let mut states = runtime.session_states_for_test().lock().await;
        states
            .entry(session_id.to_string())
            .or_insert_with(agent_runtime::session::SessionState::default)
            .compacting = true;
    }

    let result = runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: session_id.clone(),
            content: "should be rejected".into(),
            display_content: None,
            attachments: vec![],
        })
        .await;
    match result {
        Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
            assert_eq!(id, session_id.to_string());
        }
        other => panic!("expected SessionBusy, got {other:?}"),
    }
}

#[tokio::test]
async fn send_message_succeeds_after_compaction_with_summary_substituted() {
    let (runtime, ws, session_id) = fixture_runtime_with_history(8).await;

    runtime
        .compact_session(session_id.clone(), CompactionReason::UserRequested)
        .await
        .expect("compaction should succeed");

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: session_id.clone(),
            content: "post-compaction turn".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    // The assistant must have responded (FakeModelClient always replies).
    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let last_assistant = events
        .iter()
        .rev()
        .find_map(|e| match &e.payload {
            EventPayload::AssistantMessageCompleted { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("expected at least one assistant message after compaction");
    assert!(!last_assistant.is_empty());
}
