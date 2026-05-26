use agent_core::{SendMessageRequest, TaskId};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

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
