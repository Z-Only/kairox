use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_core::{AppFacade, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_store::{EventStore, SqliteEventStore};
use tokio::sync::oneshot;

use super::support::{test_config_with_two_profiles, BlockingModelClient};
use crate::facade_runtime::LocalRuntime;

// ------------------------------------------------------------------
// P4: mid-session model switch
// ------------------------------------------------------------------

#[tokio::test]
async fn switch_model_appends_event_and_updates_session_limits() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
    let rt = &runtime as &dyn AppFacade;

    let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
        .await
        .unwrap();
    let session_id = AppFacade::start_session(
        rt,
        StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
            approval_policy: None,
            sandbox_policy: None,
        },
    )
    .await
    .unwrap();

    runtime
        .switch_model(session_id.clone(), "opus".into(), None)
        .await
        .expect("switch should succeed");

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let switched = events
        .iter()
        .find(|e| {
            matches!(
                &e.payload,
                agent_core::EventPayload::ModelProfileSwitched { .. }
            )
        })
        .expect("ModelProfileSwitched event present");
    match &switched.payload {
        agent_core::EventPayload::ModelProfileSwitched {
            from_profile,
            to_profile,
            ..
        } => {
            assert_eq!(from_profile, "fast");
            assert_eq!(to_profile, "opus");
        }
        _ => unreachable!(),
    }

    let states = runtime.session_states_for_test().lock().await;
    let entry = states.get(session_id.as_str()).unwrap();
    let limits = entry
        .model_limits
        .as_ref()
        .expect("limits set after switch");
    assert!(limits.context_window > 0);
}

#[tokio::test]
async fn switch_model_updates_session_metadata_profile() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
    let rt = &runtime as &dyn AppFacade;

    let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
        .await
        .unwrap();
    let session_id = AppFacade::start_session(
        rt,
        StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
            approval_policy: None,
            sandbox_policy: None,
        },
    )
    .await
    .unwrap();

    runtime
        .switch_model(session_id.clone(), "opus".into(), None)
        .await
        .expect("switch should succeed");

    let sessions = AppFacade::list_sessions(rt, &workspace.workspace_id)
        .await
        .unwrap();
    let switched = sessions
        .into_iter()
        .find(|session| session.session_id == session_id)
        .expect("switched session should remain listed");
    assert_eq!(switched.model_profile, "opus");
    assert_eq!(switched.model_id.as_deref(), Some("fake-opus"));
    assert_eq!(switched.provider.as_deref(), Some("fake"));
}

#[tokio::test]
async fn switch_model_rejects_unknown_alias() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
    let rt = &runtime as &dyn AppFacade;

    let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
        .await
        .unwrap();
    let session_id = AppFacade::start_session(
        rt,
        StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
            approval_policy: None,
            sandbox_policy: None,
        },
    )
    .await
    .unwrap();

    let result = runtime
        .switch_model(session_id, "nonexistent".into(), None)
        .await;
    assert!(matches!(
        result,
        Err(agent_core::CoreError::InvalidState(ref msg)) if msg.contains("nonexistent")
    ));
}

#[tokio::test]
async fn switch_model_appends_event_for_reasoning_only_change() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
    let rt = &runtime as &dyn AppFacade;

    let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
        .await
        .unwrap();
    let session_id = AppFacade::start_session(
        rt,
        StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "opus".into(),
            approval_policy: None,
            sandbox_policy: None,
        },
    )
    .await
    .unwrap();

    runtime
        .switch_model(session_id.clone(), "opus".into(), Some("xhigh".into()))
        .await
        .expect("reasoning-only switch should succeed");

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let switched = events
        .iter()
        .find_map(|event| match &event.payload {
            agent_core::EventPayload::ModelProfileSwitched {
                reasoning_effort, ..
            } => reasoning_effort.as_deref(),
            _ => None,
        })
        .expect("reasoning switch event present");
    assert_eq!(switched, "xhigh");
}

#[tokio::test]
async fn switch_model_is_noop_when_alias_matches_current_profile() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
    let rt = &runtime as &dyn AppFacade;

    let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
        .await
        .unwrap();
    let session_id = AppFacade::start_session(
        rt,
        StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
            approval_policy: None,
            sandbox_policy: None,
        },
    )
    .await
    .unwrap();

    runtime
        .switch_model(session_id.clone(), "fast".into(), None)
        .await
        .expect("same-profile switch is a no-op, not an error");

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let count = events
        .iter()
        .filter(|e| {
            matches!(
                &e.payload,
                agent_core::EventPayload::ModelProfileSwitched { .. }
            )
        })
        .count();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn switch_model_returns_session_busy_when_compacting() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
    let rt = &runtime as &dyn AppFacade;

    let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
        .await
        .unwrap();
    let session_id = AppFacade::start_session(
        rt,
        StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
            approval_policy: None,
            sandbox_policy: None,
        },
    )
    .await
    .unwrap();

    {
        let mut states = runtime.session_states.lock().await;
        states
            .entry(session_id.to_string())
            .or_insert_with(crate::session::SessionState::default)
            .compacting = true;
    }

    let result = runtime
        .switch_model(session_id.clone(), "opus".into(), None)
        .await;
    match result {
        Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
            assert_eq!(id, session_id.to_string());
        }
        other => panic!("expected SessionBusy, got {other:?}"),
    }
}

#[tokio::test]
async fn switch_model_queues_behind_active_actor_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let runtime =
        Arc::new(LocalRuntime::new(store, model).with_config(test_config_with_two_profiles()));

    let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fast".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let turn_runtime = runtime.clone();
    let turn_workspace_id = workspace.workspace_id;
    let turn_session_id = session_id.clone();
    let turn = tokio::spawn(async move {
        turn_runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: turn_workspace_id,
                session_id: turn_session_id,
                content: "first".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    let switch_runtime = runtime.clone();
    let switch_session_id = session_id.clone();
    let switch = tokio::spawn(async move {
        switch_runtime
            .switch_model(switch_session_id, "opus".into(), None)
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert!(!switch.is_finished());
    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    assert!(!events.iter().any(|event| matches!(
        &event.payload,
        agent_core::EventPayload::ModelProfileSwitched { .. }
    )));

    release_tx.send(()).unwrap();
    turn.await.unwrap().unwrap();
    switch.await.unwrap().unwrap();

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
                agent_core::EventPayload::ModelProfileSwitched { .. }
            ))
            .count(),
        1
    );
    assert_eq!(stream_calls.load(Ordering::SeqCst), 1);
}
