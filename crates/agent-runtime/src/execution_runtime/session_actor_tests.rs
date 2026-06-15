use super::*;
use agent_core::{CoreError, SendMessageRequest, SessionId, TaskId, WorkspaceId};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Mock executors
// ---------------------------------------------------------------------------

struct ImmediateTurnExecutor;

#[async_trait::async_trait]
impl TurnExecutor for ImmediateTurnExecutor {
    async fn execute_turn(
        &self,
        _request: SendMessageRequest,
        _cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
        Ok(())
    }
}

struct BlockingTurnExecutor {
    started: Arc<tokio::sync::Notify>,
}

impl BlockingTurnExecutor {
    fn new() -> (Self, Arc<tokio::sync::Notify>) {
        let started = Arc::new(tokio::sync::Notify::new());
        (
            Self {
                started: started.clone(),
            },
            started,
        )
    }
}

#[async_trait::async_trait]
impl TurnExecutor for BlockingTurnExecutor {
    async fn execute_turn(
        &self,
        _request: SendMessageRequest,
        cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
        self.started.notify_one();
        cancellation.cancelled().await;
        Ok(())
    }
}

struct FailingTurnExecutor;

#[async_trait::async_trait]
impl TurnExecutor for FailingTurnExecutor {
    async fn execute_turn(
        &self,
        _request: SendMessageRequest,
        _cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
        Err(CoreError::InvalidState("test failure".into()))
    }
}

struct StubbornTurnExecutor {
    started: Arc<tokio::sync::Notify>,
}

#[async_trait::async_trait]
impl TurnExecutor for StubbornTurnExecutor {
    async fn execute_turn(
        &self,
        _request: SendMessageRequest,
        _cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
        self.started.notify_one();
        futures::future::pending::<()>().await;
        Ok(())
    }
}

struct MockTaskControlExecutor;

#[async_trait::async_trait]
impl TaskControlExecutor for MockTaskControlExecutor {
    async fn retry_task(
        &self,
        _workspace_id: WorkspaceId,
        _session_id: SessionId,
        _task_id: TaskId,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn cancel_task(
        &self,
        _workspace_id: WorkspaceId,
        _session_id: SessionId,
        _task_id: TaskId,
    ) -> agent_core::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_request() -> SendMessageRequest {
    SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: "test".into(),
        display_content: None,
        attachments: vec![],
    }
}

fn immediate_executor() -> Arc<dyn TurnExecutor> {
    Arc::new(ImmediateTurnExecutor)
}

fn failing_executor() -> Arc<dyn TurnExecutor> {
    Arc::new(FailingTurnExecutor)
}

fn task_control_executor() -> Arc<dyn TaskControlExecutor> {
    Arc::new(MockTaskControlExecutor)
}

/// Wait until the actor reports the expected state (with a timeout).
async fn wait_for_state(handle: &SessionActorHandle, expected: &ExecutionState) {
    for _ in 0..50 {
        if let Some(state) = handle.state().await {
            if &state == expected {
                return;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!(
        "timed out waiting for state {:?}, got {:?}",
        expected,
        handle.state().await
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn spawn_initial_state_is_idle() {
    let handle = SessionActorHandle::spawn();
    let state = handle.state().await;
    assert_eq!(state, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn run_turn_success_returns_to_idle() {
    let handle = SessionActorHandle::spawn();
    handle
        .run_turn(make_request(), immediate_executor())
        .await
        .unwrap();
    assert_eq!(handle.state().await, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn run_turn_failure_returns_to_idle() {
    let handle = SessionActorHandle::spawn();
    let result = handle.run_turn(make_request(), failing_executor()).await;
    assert!(result.is_err());
    assert_eq!(handle.state().await, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn two_sequential_turns_succeed() {
    let handle = SessionActorHandle::spawn();
    handle
        .run_turn(make_request(), immediate_executor())
        .await
        .unwrap();
    handle
        .run_turn(make_request(), immediate_executor())
        .await
        .unwrap();
    assert_eq!(handle.state().await, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancel_running_turn() {
    let handle = SessionActorHandle::spawn();
    let (executor, started) = BlockingTurnExecutor::new();
    let executor: Arc<dyn TurnExecutor> = Arc::new(executor);

    let turn_handle = {
        let handle = handle.clone();
        let executor = executor.clone();
        tokio::spawn(async move { handle.run_turn(make_request(), executor).await })
    };

    // Wait until the turn has actually started executing.
    started.notified().await;

    handle.cancel("user request".into()).await.unwrap();

    // The turn future should resolve (cancelled executor returns Ok).
    let result = turn_handle.await.unwrap();
    assert!(result.is_ok());

    // After the turn finishes the actor should go back to Idle.
    wait_for_state(&handle, &ExecutionState::Idle).await;
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancel_force_aborts_stubborn_turn() {
    let handle = SessionActorHandle::spawn();
    let started = Arc::new(tokio::sync::Notify::new());
    let executor: Arc<dyn TurnExecutor> = Arc::new(StubbornTurnExecutor {
        started: started.clone(),
    });

    let turn_handle = {
        let handle = handle.clone();
        let executor = executor.clone();
        tokio::spawn(async move { handle.run_turn(make_request(), executor).await })
    };

    started.notified().await;
    handle.cancel("user request".into()).await.unwrap();

    let result = tokio::time::timeout(std::time::Duration::from_millis(500), turn_handle)
        .await
        .expect("stubborn turn should be force-aborted after cancellation")
        .unwrap();
    assert!(result.is_err());
    wait_for_state(&handle, &ExecutionState::Idle).await;
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancel_idle_actor_is_ok() {
    let handle = SessionActorHandle::spawn();
    handle.cancel("no-op".into()).await.unwrap();
    assert_eq!(handle.state().await, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_sets_stopped_state() {
    let handle = SessionActorHandle::spawn();
    handle.shutdown().await.unwrap();
    // After shutdown the actor loop exits, so the channel is closed.
    // state() returns None because the receiver is gone.
    assert_eq!(handle.state().await, None);
}

#[tokio::test]
async fn run_turn_after_shutdown_returns_error() {
    let handle = SessionActorHandle::spawn();
    handle.shutdown().await.unwrap();
    let result = handle.run_turn(make_request(), immediate_executor()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn shutdown_aborts_running_turn() {
    let handle = SessionActorHandle::spawn();
    let (executor, started) = BlockingTurnExecutor::new();
    let executor: Arc<dyn TurnExecutor> = Arc::new(executor);

    let turn_handle = {
        let handle = handle.clone();
        let executor = executor.clone();
        tokio::spawn(async move { handle.run_turn(make_request(), executor).await })
    };

    started.notified().await;

    handle.shutdown().await.unwrap();

    // The turn should receive an error because shutdown aborts it.
    let result = turn_handle.await.unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn pending_turn_rejected_on_cancel() {
    let handle = SessionActorHandle::spawn();
    let (executor, started) = BlockingTurnExecutor::new();
    let executor: Arc<dyn TurnExecutor> = Arc::new(executor);

    // Start a blocking turn.
    let first = {
        let handle = handle.clone();
        let executor = executor.clone();
        tokio::spawn(async move { handle.run_turn(make_request(), executor).await })
    };
    started.notified().await;

    // Queue a second turn (will be pending).
    let second = {
        let handle = handle.clone();
        tokio::spawn(async move {
            handle
                .run_turn(make_request(), Arc::new(ImmediateTurnExecutor) as _)
                .await
        })
    };

    // Give the pending message time to arrive.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Cancel — should reject the pending turn.
    handle.cancel("abort".into()).await.unwrap();

    let second_result = second.await.unwrap();
    assert!(second_result.is_err());

    // First turn completes normally (BlockingTurnExecutor returns Ok on cancel).
    let first_result = first.await.unwrap();
    assert!(first_result.is_ok());

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn pending_turn_rejected_on_shutdown() {
    let handle = SessionActorHandle::spawn();
    let (executor, started) = BlockingTurnExecutor::new();
    let executor: Arc<dyn TurnExecutor> = Arc::new(executor);

    let first = {
        let handle = handle.clone();
        let executor = executor.clone();
        tokio::spawn(async move { handle.run_turn(make_request(), executor).await })
    };
    started.notified().await;

    let second = {
        let handle = handle.clone();
        tokio::spawn(async move {
            handle
                .run_turn(make_request(), Arc::new(ImmediateTurnExecutor) as _)
                .await
        })
    };

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    handle.shutdown().await.unwrap();

    // Both should error out.
    assert!(first.await.unwrap().is_err());
    assert!(second.await.unwrap().is_err());
}

#[tokio::test]
async fn state_is_running_during_turn() {
    let handle = SessionActorHandle::spawn();
    let (executor, started) = BlockingTurnExecutor::new();
    let executor: Arc<dyn TurnExecutor> = Arc::new(executor);

    let turn_handle = {
        let handle = handle.clone();
        let executor = executor.clone();
        tokio::spawn(async move { handle.run_turn(make_request(), executor).await })
    };

    started.notified().await;

    let state = handle.state().await.unwrap();
    assert!(
        matches!(state, ExecutionState::Running { .. }),
        "expected Running, got {:?}",
        state,
    );

    handle.cancel("done".into()).await.unwrap();
    let _ = turn_handle.await;
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn run_operation_success() {
    let handle = SessionActorHandle::spawn();
    let operation: SessionOperation = Box::pin(async { Ok(()) });
    handle.run_operation(operation).await.unwrap();
    assert_eq!(handle.state().await, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn run_operation_after_shutdown_returns_error() {
    let handle = SessionActorHandle::spawn();
    handle.shutdown().await.unwrap();
    let operation: SessionOperation = Box::pin(async { Ok(()) });
    let result = handle.run_operation(operation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn actor_stopped_error_message() {
    let error = actor_stopped();
    let message = error.to_string();
    assert!(
        message.contains("stopped"),
        "expected 'stopped' in: {message}",
    );
}

#[tokio::test]
async fn actor_cancelled_error_message() {
    let error = actor_cancelled("user abort");
    let message = error.to_string();
    assert!(
        message.contains("user abort"),
        "expected 'user abort' in: {message}",
    );
}

#[tokio::test]
async fn retry_task_success() {
    let handle = SessionActorHandle::spawn();
    handle
        .retry_task(
            WorkspaceId::new(),
            SessionId::new(),
            TaskId::new(),
            task_control_executor(),
        )
        .await
        .unwrap();
    assert_eq!(handle.state().await, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancel_task_success() {
    let handle = SessionActorHandle::spawn();
    handle
        .cancel_task(
            WorkspaceId::new(),
            SessionId::new(),
            TaskId::new(),
            task_control_executor(),
        )
        .await
        .unwrap();
    assert_eq!(handle.state().await, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn state_is_cancelling_during_cancel() {
    let handle = SessionActorHandle::spawn();
    let (executor, started) = BlockingTurnExecutor::new();
    // This executor does NOT return on cancel — it stays blocked so we can observe Cancelling.
    let slow_executor = Arc::new(SlowCancelTurnExecutor {
        started: executor.started.clone(),
    });

    let turn_handle = {
        let handle = handle.clone();
        let executor = slow_executor.clone() as Arc<dyn TurnExecutor>;
        tokio::spawn(async move { handle.run_turn(make_request(), executor).await })
    };

    started.notified().await;

    handle.cancel("check state".into()).await.unwrap();

    let state = handle.state().await.unwrap();
    assert!(
        matches!(state, ExecutionState::Cancelling { .. }),
        "expected Cancelling, got {:?}",
        state,
    );

    // Clean up: drop the handle so the actor shuts down.
    drop(handle);
    let _ = turn_handle.await;
}

/// An executor that acknowledges start but does not exit on cancellation quickly —
/// it sleeps a bit after being cancelled to keep the actor in `Cancelling` state.
struct SlowCancelTurnExecutor {
    started: Arc<tokio::sync::Notify>,
}

#[async_trait::async_trait]
impl TurnExecutor for SlowCancelTurnExecutor {
    async fn execute_turn(
        &self,
        _request: SendMessageRequest,
        cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
        self.started.notify_one();
        cancellation.cancelled().await;
        // Stay alive briefly so the test can observe Cancelling state.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        Ok(())
    }
}

#[tokio::test]
async fn run_operation_failure_propagated() {
    let handle = SessionActorHandle::spawn();
    let operation: SessionOperation =
        Box::pin(async { Err(CoreError::InvalidState("op failed".into())) });
    let result = handle.run_operation(operation).await;
    assert!(result.is_err());
    assert_eq!(handle.state().await, Some(ExecutionState::Idle));
    handle.shutdown().await.unwrap();
}
