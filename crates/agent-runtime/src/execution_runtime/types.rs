use agent_core::{SendMessageRequest, SessionId, TaskId, WorkspaceId};
use async_trait::async_trait;
use std::{future::Future, pin::Pin};
use tokio_util::sync::CancellationToken;

pub type SessionOperation = Pin<Box<dyn Future<Output = agent_core::Result<()>> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionState {
    Idle,
    Running { turn_id: String },
    Cancelling { turn_id: String },
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionCommand {
    RunTurn(SendMessageRequest),
    Cancel { reason: String },
    RetryTask { task_id: TaskId },
    CancelTask { task_id: TaskId },
    Shutdown,
}

#[async_trait]
pub trait TurnExecutor: Send + Sync + 'static {
    async fn execute_turn(
        &self,
        request: SendMessageRequest,
        cancellation: CancellationToken,
    ) -> agent_core::Result<()>;
}

#[async_trait]
pub trait TaskControlExecutor: Send + Sync + 'static {
    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()>;

    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()>;
}
