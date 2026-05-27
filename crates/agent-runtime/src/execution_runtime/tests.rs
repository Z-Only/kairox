use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_core::{CoreError, SendMessageRequest, SessionId, TaskId, WorkspaceId};
use async_trait::async_trait;
use tokio::sync::{oneshot, Mutex};
use tokio_util::sync::CancellationToken;

use super::{ExecutionState, SessionExecutionRuntime, TaskControlExecutor, TurnExecutor};

fn request(session_id: SessionId, content: &str) -> SendMessageRequest {
    SendMessageRequest {
        workspace_id: WorkspaceId::from_string("wrk_execution_runtime".into()),
        session_id,
        content: content.into(),
        attachments: vec![],
    }
}

struct ImmediateExecutor {
    calls: AtomicUsize,
}

impl ImmediateExecutor {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl TurnExecutor for ImmediateExecutor {
    async fn execute_turn(
        &self,
        _request: SendMessageRequest,
        _cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct BlockingExecutor {
    started: Mutex<Option<oneshot::Sender<()>>>,
    release: Mutex<Option<oneshot::Receiver<()>>>,
    cancelled: Mutex<Option<oneshot::Sender<()>>>,
}

impl BlockingExecutor {
    fn new(
        started: oneshot::Sender<()>,
        release: oneshot::Receiver<()>,
        cancelled: oneshot::Sender<()>,
    ) -> Self {
        Self {
            started: Mutex::new(Some(started)),
            release: Mutex::new(Some(release)),
            cancelled: Mutex::new(Some(cancelled)),
        }
    }
}

#[async_trait]
impl TurnExecutor for BlockingExecutor {
    async fn execute_turn(
        &self,
        _request: SendMessageRequest,
        cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
        if let Some(started) = self.started.lock().await.take() {
            let _ = started.send(());
        }

        let release = self
            .release
            .lock()
            .await
            .take()
            .expect("blocking executor should be used once");

        tokio::select! {
            _ = release => Ok(()),
            _ = cancellation.cancelled() => {
                if let Some(cancelled) = self.cancelled.lock().await.take() {
                    let _ = cancelled.send(());
                }
                Err(CoreError::InvalidState("turn cancelled".into()))
            }
        }
    }
}

struct ConcurrentExecutor {
    current: AtomicUsize,
    max: AtomicUsize,
}

#[async_trait]
impl TurnExecutor for ConcurrentExecutor {
    async fn execute_turn(
        &self,
        _request: SendMessageRequest,
        _cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
        let active = self.current.fetch_add(1, Ordering::SeqCst) + 1;
        self.max.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(25)).await;
        self.current.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}

struct RecordingTaskControlExecutor {
    retries: AtomicUsize,
    cancels: AtomicUsize,
}

#[async_trait]
impl TaskControlExecutor for RecordingTaskControlExecutor {
    async fn retry_task(
        &self,
        _workspace_id: WorkspaceId,
        _session_id: SessionId,
        _task_id: TaskId,
    ) -> agent_core::Result<()> {
        self.retries.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn cancel_task(
        &self,
        _workspace_id: WorkspaceId,
        _session_id: SessionId,
        _task_id: TaskId,
    ) -> agent_core::Result<()> {
        self.cancels.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn run_turn_completes_and_returns_to_idle() {
    let executor = Arc::new(ImmediateExecutor::new());
    let runtime = SessionExecutionRuntime::new();
    let session_id = SessionId::new();

    runtime
        .run_turn(request(session_id.clone(), "hello"), executor.clone())
        .await
        .unwrap();

    assert_eq!(executor.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime.session_state(&session_id).await.unwrap(),
        ExecutionState::Idle
    );
}

#[tokio::test]
async fn run_turn_queues_when_session_is_busy() {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let (cancelled_tx, _cancelled_rx) = oneshot::channel();
    let first_executor = Arc::new(BlockingExecutor::new(started_tx, release_rx, cancelled_tx));
    let second_executor = Arc::new(ImmediateExecutor::new());
    let runtime = SessionExecutionRuntime::new();
    let session_id = SessionId::new();

    let runtime_for_first = runtime.clone();
    let executor_for_first = first_executor.clone();
    let first_session = session_id.clone();
    let first = tokio::spawn(async move {
        runtime_for_first
            .run_turn(request(first_session, "first"), executor_for_first)
            .await
    });
    started_rx.await.unwrap();

    let runtime_for_second = runtime.clone();
    let second_session = session_id.clone();
    let second_executor_for_turn = second_executor.clone();
    let second = tokio::spawn(async move {
        runtime_for_second
            .run_turn(request(second_session, "second"), second_executor_for_turn)
            .await
    });

    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(!second.is_finished());
    assert_eq!(second_executor.calls.load(Ordering::SeqCst), 0);

    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    assert_eq!(second_executor.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn run_operation_queues_when_session_is_busy() {
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let (cancelled_tx, _cancelled_rx) = oneshot::channel();
    let executor = Arc::new(BlockingExecutor::new(started_tx, release_rx, cancelled_tx));
    let operation_calls = Arc::new(AtomicUsize::new(0));
    let runtime = SessionExecutionRuntime::new();
    let session_id = SessionId::new();

    let runtime_for_turn = runtime.clone();
    let executor_for_turn = executor.clone();
    let turn_session = session_id.clone();
    let turn = tokio::spawn(async move {
        runtime_for_turn
            .run_turn(request(turn_session, "first"), executor_for_turn)
            .await
    });
    started_rx.await.unwrap();

    let runtime_for_operation = runtime.clone();
    let operation_session = session_id.clone();
    let calls_for_operation = operation_calls.clone();
    let operation = tokio::spawn(async move {
        runtime_for_operation
            .run_operation(&operation_session, async move {
                calls_for_operation.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
    });

    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(!operation.is_finished());
    assert_eq!(operation_calls.load(Ordering::SeqCst), 0);

    release_tx.send(()).unwrap();
    turn.await.unwrap().unwrap();
    operation.await.unwrap().unwrap();
    assert_eq!(operation_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancel_running_turn_triggers_cancellation_token() {
    let (started_tx, started_rx) = oneshot::channel();
    let (_release_tx, release_rx) = oneshot::channel();
    let (cancelled_tx, cancelled_rx) = oneshot::channel();
    let executor = Arc::new(BlockingExecutor::new(started_tx, release_rx, cancelled_tx));
    let runtime = SessionExecutionRuntime::new();
    let session_id = SessionId::new();

    let runtime_for_turn = runtime.clone();
    let executor_for_turn = executor.clone();
    let turn_session = session_id.clone();
    let turn = tokio::spawn(async move {
        runtime_for_turn
            .run_turn(request(turn_session, "cancel me"), executor_for_turn)
            .await
    });
    started_rx.await.unwrap();

    runtime
        .cancel_session(&session_id, "user requested".into())
        .await
        .unwrap();

    cancelled_rx.await.unwrap();
    assert!(turn.await.unwrap().is_err());
    assert_eq!(
        runtime.session_state(&session_id).await.unwrap(),
        ExecutionState::Idle
    );
}

#[tokio::test]
async fn cancel_running_turn_rejects_queued_turns() {
    let (started_tx, started_rx) = oneshot::channel();
    let (_release_tx, release_rx) = oneshot::channel();
    let (cancelled_tx, cancelled_rx) = oneshot::channel();
    let first_executor = Arc::new(BlockingExecutor::new(started_tx, release_rx, cancelled_tx));
    let second_executor = Arc::new(ImmediateExecutor::new());
    let runtime = SessionExecutionRuntime::new();
    let session_id = SessionId::new();

    let runtime_for_first = runtime.clone();
    let first_executor_for_turn = first_executor.clone();
    let first_session = session_id.clone();
    let first = tokio::spawn(async move {
        runtime_for_first
            .run_turn(request(first_session, "first"), first_executor_for_turn)
            .await
    });
    started_rx.await.unwrap();

    let runtime_for_second = runtime.clone();
    let second_session = session_id.clone();
    let second_executor_for_turn = second_executor.clone();
    let second = tokio::spawn(async move {
        runtime_for_second
            .run_turn(request(second_session, "second"), second_executor_for_turn)
            .await
    });

    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(!second.is_finished());
    assert_eq!(second_executor.calls.load(Ordering::SeqCst), 0);

    runtime
        .cancel_session(&session_id, "user requested".into())
        .await
        .unwrap();

    cancelled_rx.await.unwrap();
    assert!(first.await.unwrap().is_err());
    let second_result = second.await.unwrap();
    assert!(
        matches!(second_result, Err(CoreError::InvalidState(ref message)) if message.contains("session execution cancelled")),
        "expected queued turn cancellation error, got {second_result:?}"
    );
    assert_eq!(second_executor.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        runtime.session_state(&session_id).await.unwrap(),
        ExecutionState::Idle
    );
}

#[tokio::test]
async fn different_sessions_can_run_concurrently() {
    let executor = Arc::new(ConcurrentExecutor {
        current: AtomicUsize::new(0),
        max: AtomicUsize::new(0),
    });
    let runtime = SessionExecutionRuntime::new();

    let first = {
        let runtime = runtime.clone();
        let executor = executor.clone();
        tokio::spawn(async move {
            runtime
                .run_turn(request(SessionId::new(), "a"), executor)
                .await
        })
    };
    let second = {
        let runtime = runtime.clone();
        let executor = executor.clone();
        tokio::spawn(async move {
            runtime
                .run_turn(request(SessionId::new(), "b"), executor)
                .await
        })
    };

    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();

    assert_eq!(executor.max.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn same_session_reuses_one_actor() {
    let executor = Arc::new(ImmediateExecutor::new());
    let runtime = SessionExecutionRuntime::new();
    let session_id = SessionId::new();

    runtime
        .run_turn(request(session_id.clone(), "first"), executor.clone())
        .await
        .unwrap();
    runtime
        .run_turn(request(session_id, "second"), executor)
        .await
        .unwrap();

    assert_eq!(runtime.actor_count().await, 1);
}

#[tokio::test]
async fn task_control_commands_run_through_session_actor() {
    let executor = Arc::new(RecordingTaskControlExecutor {
        retries: AtomicUsize::new(0),
        cancels: AtomicUsize::new(0),
    });
    let runtime = SessionExecutionRuntime::new();
    let workspace_id = WorkspaceId::from_string("wrk_execution_runtime".into());
    let session_id = SessionId::new();

    runtime
        .retry_task(
            workspace_id.clone(),
            session_id.clone(),
            TaskId::from_string("task_retry".into()),
            executor.clone(),
        )
        .await
        .unwrap();
    runtime
        .cancel_task(
            workspace_id,
            session_id.clone(),
            TaskId::from_string("task_cancel".into()),
            executor.clone(),
        )
        .await
        .unwrap();

    assert_eq!(executor.retries.load(Ordering::SeqCst), 1);
    assert_eq!(executor.cancels.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime.session_state(&session_id).await.unwrap(),
        ExecutionState::Idle
    );
}

#[tokio::test]
async fn shutdown_stops_actor() {
    let executor = Arc::new(ImmediateExecutor::new());
    let runtime = SessionExecutionRuntime::new();
    let session_id = SessionId::new();

    runtime
        .run_turn(request(session_id.clone(), "first"), executor)
        .await
        .unwrap();
    runtime.shutdown_session(&session_id).await.unwrap();

    assert_eq!(runtime.session_state(&session_id).await, None);
    assert_eq!(runtime.actor_count().await, 0);
}
