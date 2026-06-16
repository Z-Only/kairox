use super::*;
use agent_core::{SessionId, TaskConfirmationDecision, TaskConfirmationOption, WorkspaceId};
use agent_store::SqliteEventStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn request_task_confirmation_emits_event_and_waits_for_decision() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, _) = tokio::sync::broadcast::channel(8);
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let request = TaskConfirmationRequest {
        request_id: "clarify_1".into(),
        prompt: "Which scope should I use?".into(),
        options: vec![TaskConfirmationOption {
            id: "tests".into(),
            label: "Tests only".into(),
            description: Some("Add failing tests first".into()),
        }],
        allow_multiple: true,
        allow_custom: true,
    };

    let pending_clone = pending.clone();
    let request_id = request.request_id.clone();
    let task = tokio::spawn(async move {
        request_task_confirmation(
            &store,
            &event_tx,
            &pending_clone,
            &workspace_id,
            &session_id,
            request,
        )
        .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(
        pending.lock().await.contains_key(&request_id),
        "request should remain pending until the user responds"
    );

    resolve_task_confirmation(
        &pending,
        TaskConfirmationDecision {
            request_id,
            selected_option_ids: vec!["tests".into()],
            custom_response: Some("Also update TUI".into()),
        },
    )
    .await
    .unwrap();

    let output = task.await.unwrap().unwrap();
    assert!(output.contains("selected_option_ids=[\"tests\"]"));
    assert!(output.contains("custom_response=Also update TUI"));
}

#[tokio::test]
async fn resolve_task_confirmation_is_noop_for_unknown_request() {
    let pending = Arc::new(Mutex::new(HashMap::new()));
    resolve_task_confirmation(
        &pending,
        TaskConfirmationDecision {
            request_id: "missing".into(),
            selected_option_ids: vec![],
            custom_response: Some("free form".into()),
        },
    )
    .await
    .unwrap();
}
