use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use tokio::sync::oneshot;

use super::support::BlockingModelClient;
use crate::facade_runtime::LocalRuntime;

#[tokio::test]
async fn send_message_returns_session_busy_when_compacting() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello".into()]);
    let runtime = LocalRuntime::new(store, model);
    let rt = &runtime as &dyn AppFacade;

    let workspace = AppFacade::open_workspace(rt, "/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = AppFacade::start_session(
        rt,
        StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        },
    )
    .await
    .unwrap();

    // Force the session into compacting state.
    {
        let mut states = runtime.session_states.lock().await;
        states
            .entry(session_id.to_string())
            .or_insert_with(crate::session::SessionState::default)
            .compacting = true;
    }

    let result = AppFacade::send_message(
        rt,
        SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        },
    )
    .await;
    match result {
        Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
            assert_eq!(id, session_id.to_string());
        }
        other => panic!("expected SessionBusy, got {other:?}"),
    }
}

#[tokio::test]
async fn send_message_queues_same_session_turn_when_actor_turn_running() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let runtime = Arc::new(LocalRuntime::new(store, model));

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let first_runtime = runtime.clone();
    let first_workspace_id = workspace.workspace_id.clone();
    let first_session_id = session_id.clone();
    let first = tokio::spawn(async move {
        first_runtime
            .send_message(SendMessageRequest {
                workspace_id: first_workspace_id,
                session_id: first_session_id,
                content: "first".into(),
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    let second_runtime = runtime.clone();
    let second_workspace_id = workspace.workspace_id;
    let second_session_id = session_id.clone();
    let second = tokio::spawn(async move {
        second_runtime
            .send_message(SendMessageRequest {
                workspace_id: second_workspace_id,
                session_id: second_session_id,
                content: "second".into(),
                attachments: vec![],
            })
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert!(!second.is_finished());
    assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    assert_eq!(stream_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn send_message_returns_session_busy_when_compacting_during_actor_turn() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let stream_calls = Arc::new(AtomicUsize::new(0));
    let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
    let runtime = Arc::new(LocalRuntime::new(store, model));

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "blocking".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let first_runtime = runtime.clone();
    let first_workspace_id = workspace.workspace_id.clone();
    let first_session_id = session_id.clone();
    let first = tokio::spawn(async move {
        first_runtime
            .send_message(SendMessageRequest {
                workspace_id: first_workspace_id,
                session_id: first_session_id,
                content: "first".into(),
                attachments: vec![],
            })
            .await
    });
    started_rx.await.unwrap();

    {
        let mut states = runtime.session_states.lock().await;
        states
            .entry(session_id.to_string())
            .or_insert_with(crate::session::SessionState::default)
            .compacting = true;
    }

    let result = runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "second".into(),
            attachments: vec![],
        })
        .await;

    match result {
        Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
            assert_eq!(id, session_id.to_string());
        }
        other => panic!("expected SessionBusy, got {other:?}"),
    }
    assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
}
