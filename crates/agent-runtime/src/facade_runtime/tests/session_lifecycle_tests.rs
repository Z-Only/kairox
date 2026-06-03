use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use agent_tools::MonitorRegistry;
use std::path::PathBuf;
use std::sync::Arc;

use crate::facade_runtime::LocalRuntime;

#[tokio::test]
async fn start_session_registers_idle_session_actor() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    assert_eq!(runtime.session_execution.actor_count().await, 0);

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id,
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    assert_eq!(runtime.session_execution.actor_count().await, 1);
    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        Some(crate::execution_runtime::ExecutionState::Idle)
    );
}

#[tokio::test]
async fn soft_delete_session_stops_session_actor() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["done".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
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
    assert_eq!(runtime.session_execution.actor_count().await, 1);

    runtime.soft_delete_session(&session_id).await.unwrap();

    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        None
    );
    assert_eq!(runtime.session_execution.actor_count().await, 0);
}

#[tokio::test]
async fn permanently_delete_session_stops_session_actor() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["done".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
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
    assert_eq!(runtime.session_execution.actor_count().await, 1);

    runtime
        .permanently_delete_session(&session_id)
        .await
        .unwrap();

    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        None
    );
    assert_eq!(runtime.session_execution.actor_count().await, 0);
}

#[tokio::test]
async fn restore_archived_session_restarts_session_actor() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["done".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id,
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    assert_eq!(runtime.session_execution.actor_count().await, 1);

    runtime.soft_delete_session(&session_id).await.unwrap();
    assert_eq!(runtime.session_execution.actor_count().await, 0);
    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        None
    );

    runtime.restore_archived_session(&session_id).await.unwrap();

    assert_eq!(runtime.session_execution.actor_count().await, 1);
    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        Some(crate::execution_runtime::ExecutionState::Idle)
    );
}

#[tokio::test]
async fn soft_delete_session_stops_all_monitors() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["done".into()]);
    let mut runtime = LocalRuntime::new(store, model);

    let registry = Arc::new(MonitorRegistry::new(
        PathBuf::from("/tmp"),
        runtime.event_tx.clone(),
    ));
    runtime.monitor_registry = Some(registry.clone());

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
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

    registry
        .start(
            "test monitor".into(),
            "sleep 60".into(),
            false,
            Some(60_000),
            workspace.workspace_id.clone(),
            session_id.clone(),
        )
        .await
        .unwrap();
    assert_eq!(registry.list().await.len(), 1);

    runtime.soft_delete_session(&session_id).await.unwrap();
    assert_eq!(registry.list().await.len(), 0);
}
