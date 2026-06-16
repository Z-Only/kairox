use super::*;
use agent_core::{SessionId, TaskConfirmationDecision, TaskConfirmationOption, WorkspaceId};
use agent_store::SqliteEventStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[test]
fn parse_tool_request_applies_defaults_and_options() {
    let request = parse_tool_request(
        "clarify_1",
        &serde_json::json!({
            "prompt": "Which scope should I use?",
            "options": [
                {
                    "id": "tests",
                    "label": "Tests only",
                    "description": "Add coverage without changing behavior"
                }
            ]
        }),
    )
    .unwrap();

    assert_eq!(request.request_id, "clarify_1");
    assert_eq!(request.prompt, "Which scope should I use?");
    assert_eq!(
        request.options,
        vec![TaskConfirmationOption {
            id: "tests".into(),
            label: "Tests only".into(),
            description: Some("Add coverage without changing behavior".into()),
        }]
    );
    assert!(!request.allow_multiple);
    assert!(request.allow_custom);
}

#[test]
fn parse_tool_request_rejects_blank_prompt() {
    let error = parse_tool_request(
        "clarify_1",
        &serde_json::json!({
            "prompt": "   ",
            "allow_custom": false
        }),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("task confirmation prompt cannot be empty"),
        "unexpected error: {error}"
    );
}

#[test]
fn parse_tool_request_rejects_prompt_with_no_response_path() {
    let error = parse_tool_request(
        "clarify_1",
        &serde_json::json!({
            "prompt": "Which scope should I use?",
            "allow_custom": false
        }),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("task confirmation requires at least one response path"),
        "unexpected error: {error}"
    );
}

#[test]
fn parse_tool_request_rejects_invalid_options() {
    for (arguments, expected) in [
        (
            serde_json::json!({
                "prompt": "Which scope should I use?",
                "options": [{ "id": "  ", "label": "Tests only" }]
            }),
            "task confirmation option id cannot be empty",
        ),
        (
            serde_json::json!({
                "prompt": "Which scope should I use?",
                "options": [{ "id": "tests", "label": "  " }]
            }),
            "task confirmation option label cannot be empty",
        ),
        (
            serde_json::json!({
                "prompt": "Which scope should I use?",
                "options": [
                    { "id": "tests", "label": "Tests only" },
                    { "id": "tests", "label": "Tests again" }
                ]
            }),
            "duplicate task confirmation option id",
        ),
    ] {
        let error = parse_tool_request("clarify_1", &arguments).unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?}, got {error}"
        );
    }
}

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
async fn request_task_confirmation_rejects_duplicate_pending_request_id() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, _) = tokio::sync::broadcast::channel(8);
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let request_id = "clarify_1".to_string();

    let first_request = TaskConfirmationRequest {
        request_id: request_id.clone(),
        prompt: "Which scope should I use?".into(),
        options: vec![TaskConfirmationOption {
            id: "tests".into(),
            label: "Tests only".into(),
            description: None,
        }],
        allow_multiple: false,
        allow_custom: true,
    };

    let pending_clone = pending.clone();
    let first_task = tokio::spawn(async move {
        request_task_confirmation(
            &store,
            &event_tx,
            &pending_clone,
            &workspace_id,
            &session_id,
            first_request,
        )
        .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(pending.lock().await.contains_key(&request_id));

    let duplicate_request = TaskConfirmationRequest {
        request_id: request_id.clone(),
        prompt: "Duplicate request id?".into(),
        options: vec![TaskConfirmationOption {
            id: "runtime".into(),
            label: "Runtime fix".into(),
            description: None,
        }],
        allow_multiple: false,
        allow_custom: true,
    };

    let duplicate_store = SqliteEventStore::in_memory().await.unwrap();
    let (duplicate_event_tx, _) = tokio::sync::broadcast::channel(8);
    let duplicate_workspace_id = WorkspaceId::new();
    let duplicate_session_id = SessionId::new();
    let error = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        request_task_confirmation(
            &duplicate_store,
            &duplicate_event_tx,
            &pending,
            &duplicate_workspace_id,
            &duplicate_session_id,
            duplicate_request,
        ),
    )
    .await
    .expect("duplicate request should return without waiting")
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("task confirmation request already pending"),
        "unexpected error: {error}"
    );
    assert!(
        pending.lock().await.contains_key(&request_id),
        "duplicate requests must not release the original pending request"
    );

    resolve_task_confirmation(
        &pending,
        TaskConfirmationDecision {
            request_id,
            selected_option_ids: vec!["tests".into()],
            custom_response: None,
        },
    )
    .await
    .unwrap();

    let output = first_task.await.unwrap().unwrap();
    assert!(output.contains("selected_option_ids=[\"tests\"]"));
}

#[tokio::test]
async fn resolve_task_confirmation_rejects_unknown_option_and_keeps_request_pending() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, _) = tokio::sync::broadcast::channel(8);
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let request_id = "clarify_1".to_string();

    let request = TaskConfirmationRequest {
        request_id: request_id.clone(),
        prompt: "Which scope should I use?".into(),
        options: vec![TaskConfirmationOption {
            id: "tests".into(),
            label: "Tests only".into(),
            description: None,
        }],
        allow_multiple: true,
        allow_custom: false,
    };

    let pending_clone = pending.clone();
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

    let error = resolve_task_confirmation(
        &pending,
        TaskConfirmationDecision {
            request_id: request_id.clone(),
            selected_option_ids: vec!["missing".into()],
            custom_response: None,
        },
    )
    .await
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unknown task confirmation option id"),
        "unexpected error: {error}"
    );
    assert!(
        pending.lock().await.contains_key(&request_id),
        "invalid decisions must not release the pending request"
    );

    resolve_task_confirmation(
        &pending,
        TaskConfirmationDecision {
            request_id,
            selected_option_ids: vec!["tests".into()],
            custom_response: None,
        },
    )
    .await
    .unwrap();

    let output = task.await.unwrap().unwrap();
    assert!(output.contains("selected_option_ids=[\"tests\"]"));
}

#[tokio::test]
async fn resolve_task_confirmation_enforces_single_select_and_custom_policy() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, _) = tokio::sync::broadcast::channel(8);
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let request_id = "clarify_1".to_string();

    let request = TaskConfirmationRequest {
        request_id: request_id.clone(),
        prompt: "Which scope should I use?".into(),
        options: vec![
            TaskConfirmationOption {
                id: "tests".into(),
                label: "Tests only".into(),
                description: None,
            },
            TaskConfirmationOption {
                id: "runtime".into(),
                label: "Runtime fix".into(),
                description: None,
            },
        ],
        allow_multiple: false,
        allow_custom: false,
    };

    let pending_clone = pending.clone();
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

    let multiple_error = resolve_task_confirmation(
        &pending,
        TaskConfirmationDecision {
            request_id: request_id.clone(),
            selected_option_ids: vec!["tests".into(), "runtime".into()],
            custom_response: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        multiple_error
            .to_string()
            .contains("does not allow multiple selected options"),
        "unexpected error: {multiple_error}"
    );

    let custom_error = resolve_task_confirmation(
        &pending,
        TaskConfirmationDecision {
            request_id: request_id.clone(),
            selected_option_ids: vec![],
            custom_response: Some("Do something else".into()),
        },
    )
    .await
    .unwrap_err();
    assert!(
        custom_error
            .to_string()
            .contains("does not allow custom response"),
        "unexpected error: {custom_error}"
    );

    assert!(
        pending.lock().await.contains_key(&request_id),
        "invalid decisions must not release the pending request"
    );

    resolve_task_confirmation(
        &pending,
        TaskConfirmationDecision {
            request_id,
            selected_option_ids: vec!["runtime".into()],
            custom_response: None,
        },
    )
    .await
    .unwrap();

    let output = task.await.unwrap().unwrap();
    assert!(output.contains("selected_option_ids=[\"runtime\"]"));
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

#[tokio::test]
async fn deny_pending_confirmations_for_session_denies_only_matching_session() {
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let target_session = SessionId::new();
    let other_session = SessionId::new();
    let (target_tx, target_rx) = tokio::sync::oneshot::channel();
    let (other_tx, mut other_rx) = tokio::sync::oneshot::channel();

    pending.lock().await.insert(
        "target".into(),
        PendingTaskConfirmation {
            session_id: target_session.clone(),
            options: vec![],
            allow_multiple: true,
            allow_custom: true,
            reply: target_tx,
        },
    );
    pending.lock().await.insert(
        "other".into(),
        PendingTaskConfirmation {
            session_id: other_session,
            options: vec![],
            allow_multiple: true,
            allow_custom: true,
            reply: other_tx,
        },
    );

    let denied = deny_pending_confirmations_for_session(
        &pending,
        &target_session,
        "session ended before the user responded",
    )
    .await
    .unwrap();

    assert_eq!(denied, vec!["target".to_string()]);
    assert!(pending.lock().await.contains_key("other"));
    let target_decision = target_rx.await.unwrap();
    assert_eq!(target_decision.request_id, "target");
    assert!(target_decision.selected_option_ids.is_empty());
    assert_eq!(
        target_decision.custom_response.as_deref(),
        Some("session ended before the user responded")
    );
    assert!(matches!(
        other_rx.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
}
