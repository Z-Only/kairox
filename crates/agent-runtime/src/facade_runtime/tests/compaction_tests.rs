use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_store::{EventStore, SqliteEventStore};
use tokio::sync::oneshot;

use super::support::{
    append_compaction_history, install_responding_dag_executor, test_config_with_threshold,
    wait_for_event, BlockingModelClient,
};
use crate::facade_runtime::LocalRuntime;

#[tokio::test]
async fn compact_session_queues_behind_active_actor_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let runtime = Arc::new(LocalRuntime::new(store, model));

    let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    append_compaction_history(
        runtime.event_store_for_test(),
        &workspace.workspace_id,
        &session_id,
        8,
    )
    .await;

    let turn_runtime = runtime.clone();
    let turn_workspace_id = workspace.workspace_id;
    let turn_session_id = session_id.clone();
    let turn = tokio::spawn(async move {
        turn_runtime
            .send_message(SendMessageRequest {
                workspace_id: turn_workspace_id,
                session_id: turn_session_id,
                content: "first".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    let compact_runtime = runtime.clone();
    let compact_session_id = session_id.clone();
    let compact = tokio::spawn(async move {
        compact_runtime
            .compact_session(
                compact_session_id,
                agent_core::CompactionReason::UserRequested,
            )
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert!(!compact.is_finished());
    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    assert!(!events.iter().any(|event| matches!(
        &event.payload,
        agent_core::EventPayload::ContextCompactionStarted { .. }
            | agent_core::EventPayload::CompactionSummary { .. }
            | agent_core::EventPayload::ContextCompactionCompleted { .. }
    )));

    release_tx.send(()).unwrap();
    turn.await.unwrap().unwrap();
    compact.await.unwrap().unwrap();

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                &event.payload,
                agent_core::EventPayload::ContextCompactionStarted { .. }
            ))
            .count(),
        1
    );
    assert_eq!(stream_calls.load(Ordering::SeqCst), 2);
}

// ------------------------------------------------------------------
// Turn-end auto-compaction (race-free) integration tests.
// Spec: docs/superpowers/specs/2026-05-27-race-free-auto-compaction-design.md
// ------------------------------------------------------------------

#[tokio::test]
async fn auto_compaction_queues_after_threshold_turn() {
    // SingleStep path: threshold=0.001 with FALLBACK_FAKE budget guarantees
    // the turn's `last_estimated_tokens` overshoots; tail of `execute_turn`
    // routes a `compact_session` through the actor and we observe its
    // `ContextCompactionCompleted` event arrive after the turn finishes.
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_threshold(0.001));

    let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    append_compaction_history(
        runtime.event_store_for_test(),
        &workspace.workspace_id,
        &session_id,
        8,
    )
    .await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "trigger".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    // Compaction is detached behind the actor — wait for the completion event.
    let saw_completed = wait_for_event(
        runtime.event_store_for_test(),
        &session_id,
        |p| {
            matches!(
                p,
                agent_core::EventPayload::ContextCompactionCompleted { .. }
            )
        },
        std::time::Duration::from_secs(5),
    )
    .await;
    assert!(saw_completed, "expected ContextCompactionCompleted event");

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    assert!(
        events.iter().any(|e| matches!(
            &e.payload,
            agent_core::EventPayload::ContextCompactionStarted { .. }
        )),
        "expected ContextCompactionStarted event"
    );
    assert!(
        !events.iter().any(|e| matches!(
            &e.payload,
            agent_core::EventPayload::ContextCompactionSkipped { .. }
        )),
        "no ContextCompactionSkipped expected on the trigger path"
    );
}

#[tokio::test]
async fn auto_compaction_emits_skipped_when_already_compacting() {
    // We bypass `send_message`'s busy gate by driving the executor
    // directly. Pre-setting `compacting = true` makes the scheduler take
    // the skip path and emit `ContextCompactionSkipped { AlreadyCompacting }`
    // inline.
    use crate::execution_runtime::TurnExecutor;
    use crate::facade_turn_executor::LocalRuntimeTurnExecutor;

    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_threshold(0.001));

    let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // Flip the busy flag on the entry that `initialize_session_limits`
    // already seeded with FALLBACK_FAKE limits.
    {
        let mut states = runtime.session_states_for_test().lock().await;
        states
            .get_mut(&session_id.to_string())
            .expect("session entry seeded by start_session")
            .compacting = true;
    }

    let executor = LocalRuntimeTurnExecutor::from_runtime(&runtime);
    executor
        .execute_turn(
            SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "trigger".into(),
                display_content: None,
                attachments: vec![],
            },
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let skipped = events.iter().find_map(|e| match &e.payload {
        agent_core::EventPayload::ContextCompactionSkipped { reason, ratio } => {
            Some((*reason, *ratio))
        }
        _ => None,
    });
    let (reason, _ratio) = skipped.expect("expected ContextCompactionSkipped event");
    assert!(matches!(
        reason,
        agent_core::CompactionSkipReason::AlreadyCompacting
    ));
    assert!(
        !events.iter().any(|e| matches!(
            &e.payload,
            agent_core::EventPayload::ContextCompactionStarted { .. }
        )),
        "compaction must not start when AlreadyCompacting"
    );
}

#[tokio::test]
async fn auto_compaction_skipped_when_threshold_disabled() {
    // threshold = 1.0 disables auto-compaction; SingleStep turn still
    // populates `last_estimated_tokens`, so the scheduler reaches the
    // skip branch and emits `ContextCompactionSkipped { ThresholdDisabled }`.
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_threshold(1.0));

    let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "trigger".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let skipped = events.iter().find_map(|e| match &e.payload {
        agent_core::EventPayload::ContextCompactionSkipped { reason, ratio } => {
            Some((*reason, *ratio))
        }
        _ => None,
    });
    let (reason, _ratio) = skipped.expect("expected ContextCompactionSkipped event");
    assert!(matches!(
        reason,
        agent_core::CompactionSkipReason::ThresholdDisabled
    ));
    assert!(
        !events.iter().any(|e| matches!(
            &e.payload,
            agent_core::EventPayload::ContextCompactionStarted { .. }
        )),
        "compaction must not start when ThresholdDisabled"
    );
}

#[tokio::test]
async fn dag_turn_also_triggers_auto_compaction() {
    // DAG path skips `prepare_turn_context`, so we pre-seed
    // `last_estimated_tokens` to prove the tail scheduler in
    // `LocalRuntimeTurnExecutor::execute_turn` fires on both branches.
    // RespondingPlannerStrategy keeps the DAG single-step: planner
    // emits AssistantMessageCompleted via Respond, no model call.
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let mut runtime = LocalRuntime::new(store, model)
        .with_config(test_config_with_threshold(0.001))
        .with_dag_execution()
        .await;
    install_responding_dag_executor(&mut runtime).await;

    let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    append_compaction_history(
        runtime.event_store_for_test(),
        &workspace.workspace_id,
        &session_id,
        8,
    )
    .await;

    // Pre-seed last_estimated_tokens so the DAG-tail scheduler sees usage.
    {
        let mut states = runtime.session_states_for_test().lock().await;
        let entry = states
            .get_mut(&session_id.to_string())
            .expect("session entry seeded by start_session");
        entry.last_estimated_tokens = 5_000;
    }

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "/plan run a tiny experiment".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let saw_completed = wait_for_event(
        runtime.event_store_for_test(),
        &session_id,
        |p| {
            matches!(
                p,
                agent_core::EventPayload::ContextCompactionCompleted { .. }
            )
        },
        std::time::Duration::from_secs(5),
    )
    .await;
    assert!(
        saw_completed,
        "expected ContextCompactionCompleted event on DAG path"
    );
}
