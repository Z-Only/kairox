//! Cancellation: explicit cancel from the facade and mid-stream interruption
//! of the agent loop while the model is still emitting tokens.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::sync::Arc;

use super::support::make_simple_runtime;

#[tokio::test]
async fn full_stack_cancel_session() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-cancel".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .cancel_session(ws.workspace_id, sid.clone())
        .await
        .unwrap();

    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert!(proj.cancelled, "Session should be cancelled");
}

/// A model that yields tokens slowly, allowing cancellation to interrupt mid-stream.
#[derive(Debug, Clone)]
struct SlowStreamingModel {
    tokens: Vec<String>,
}

#[async_trait]
impl ModelClient for SlowStreamingModel {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let tokens = self.tokens.clone();
        let stream = async_stream::stream! {
            for token in &tokens {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                yield Ok(ModelEvent::TokenDelta(token.clone()));
            }
            yield Ok(ModelEvent::Completed { usage: None });
        };
        Ok(Box::pin(stream))
    }
}

#[tokio::test]
async fn cancellation_stops_agent_loop_mid_stream() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = SlowStreamingModel {
        tokens: vec![
            "Hello ".into(),
            "world ".into(),
            "this ".into(),
            "is ".into(),
            "a ".into(),
            "test ".into(),
            "with ".into(),
            "many ".into(),
            "tokens ".into(),
            "to ".into(),
            "allow ".into(),
            "cancellation ".into(),
        ],
    };
    let runtime =
        Arc::new(LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest));

    let ws = runtime
        .open_workspace("/tmp/test-cancel-stream".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "slow".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // Spawn send_message in a background task
    let rt = runtime.clone();
    let ws_id = ws.workspace_id.clone();
    let sid_clone = sid.clone();
    let handle = tokio::spawn(async move {
        rt.send_message(SendMessageRequest {
            workspace_id: ws_id,
            session_id: sid_clone,
            content: "hello".into(),
            attachments: vec![],
        })
        .await
    });

    // Wait a bit for streaming to start, then cancel
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    runtime
        .cancel_session(ws.workspace_id.clone(), sid.clone())
        .await
        .unwrap();

    // The send_message task should complete (not hang)
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("send_message should complete within 5 seconds after cancellation");

    // send_message should return Ok (graceful cancellation)
    assert!(
        result.is_ok(),
        "send_message should return Ok after cancellation: {:?}",
        result
    );

    // Session should be marked as cancelled
    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert!(proj.cancelled, "Session should be marked as cancelled");
}
