//! Error propagation: when the model client fails, the loop must persist
//! failure events AND surface the error to the caller.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::{EventStore, SqliteEventStore};
use async_trait::async_trait;
use futures::{stream, stream::BoxStream};

/// A model client that always returns an error from `stream()`.
struct FailingModelClient;

#[async_trait]
impl ModelClient for FailingModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Err(agent_models::ModelError::Request("model failure".into()))
    }
}

/// A model client that returns a syntactically valid stream with no assistant
/// text and no tool calls.
struct EmptyModelClient;

#[async_trait]
impl ModelClient for EmptyModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Ok(Box::pin(stream::iter(vec![Ok(ModelEvent::Completed {
            usage: None,
        })])))
    }
}

/// When the model returns an error, the agent loop must emit failure events to
/// the store AND propagate the error to the caller via `InvalidState`.
#[tokio::test]
async fn agent_loop_handles_model_error_gracefully() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FailingModelClient;
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-model-error".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let result = runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await;

    // The error MUST propagate to the caller.
    assert!(
        result.is_err(),
        "send_message should return Err when model fails"
    );
    match result {
        Err(agent_core::CoreError::InvalidState(msg)) => {
            assert!(
                msg.contains("model failure"),
                "Error message should mention the failure: {msg}"
            );
        }
        other => panic!("Expected InvalidState, got {other:?}"),
    }

    // Verify failure events were emitted to the store (events are appended
    // BEFORE the error is returned).
    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    let has_user_msg = events.iter().any(|e| e.event_type == "UserMessageAdded");
    assert!(has_user_msg, "Store should contain UserMessageAdded");

    let has_failed = events.iter().any(|e| e.event_type == "AgentTaskFailed");
    assert!(has_failed, "Store should contain AgentTaskFailed event");
}

#[tokio::test]
async fn agent_loop_treats_empty_model_stream_as_failure() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = LocalRuntime::new(store, EmptyModelClient);

    let workspace = runtime
        .open_workspace("/tmp/test-empty-model-response".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let result = runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await;

    match result {
        Err(agent_core::CoreError::InvalidState(msg)) => {
            assert!(
                msg.contains("empty response"),
                "Error message should explain the empty model response: {msg}"
            );
        }
        other => panic!("Expected InvalidState for empty model response, got {other:?}"),
    }

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .unwrap();
    assert!(
        events.iter().any(|e| e.event_type == "AgentTaskFailed"),
        "Store should contain AgentTaskFailed for empty model response"
    );
    assert!(
        !events.iter().any(|e| e.event_type == "AgentTaskCompleted"),
        "Empty model response must not be marked completed"
    );
}
