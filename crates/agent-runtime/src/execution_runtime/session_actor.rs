use std::collections::VecDeque;
use std::sync::Arc;

use agent_core::{CoreError, SendMessageRequest, SessionId, TaskId, WorkspaceId};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::types::{ExecutionState, SessionOperation, TaskControlExecutor, TurnExecutor};

const ACTOR_CHANNEL_CAPACITY: usize = 32;

pub(crate) enum ActorMessage {
    RunTurn {
        request: SendMessageRequest,
        executor: Arc<dyn TurnExecutor>,
        reply: oneshot::Sender<agent_core::Result<()>>,
    },
    RunTurnIfIdle {
        request: SendMessageRequest,
        executor: Arc<dyn TurnExecutor>,
        reply: oneshot::Sender<agent_core::Result<()>>,
    },
    Cancel {
        reason: String,
        reply: oneshot::Sender<agent_core::Result<()>>,
    },
    ForceAbort {
        turn_id: String,
    },
    RetryTask {
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
        executor: Arc<dyn TaskControlExecutor>,
        reply: oneshot::Sender<agent_core::Result<()>>,
    },
    CancelTask {
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
        executor: Arc<dyn TaskControlExecutor>,
        reply: oneshot::Sender<agent_core::Result<()>>,
    },
    RunOperation {
        operation: SessionOperation,
        reply: oneshot::Sender<agent_core::Result<()>>,
    },
    State {
        reply: oneshot::Sender<ExecutionState>,
    },
    Shutdown {
        reply: oneshot::Sender<agent_core::Result<()>>,
    },
}

struct RunningTurn {
    turn_id: String,
    cancellation: CancellationToken,
    reply: oneshot::Sender<agent_core::Result<()>>,
    join: tokio::task::JoinHandle<agent_core::Result<()>>,
}

struct PendingTurn {
    request: SendMessageRequest,
    executor: Arc<dyn TurnExecutor>,
    reply: oneshot::Sender<agent_core::Result<()>>,
}

struct PendingTaskControl {
    workspace_id: WorkspaceId,
    session_id: SessionId,
    task_id: TaskId,
    executor: Arc<dyn TaskControlExecutor>,
    reply: oneshot::Sender<agent_core::Result<()>>,
}

struct PendingOperation {
    operation: SessionOperation,
    reply: oneshot::Sender<agent_core::Result<()>>,
}

enum PendingCommand {
    RunTurn(PendingTurn),
    RetryTask(PendingTaskControl),
    CancelTask(PendingTaskControl),
    RunOperation(PendingOperation),
}

impl PendingCommand {
    fn reject_stopped(self) {
        match self {
            Self::RunTurn(pending) => {
                let _ = pending.reply.send(Err(actor_stopped()));
            }
            Self::RetryTask(pending) | Self::CancelTask(pending) => {
                let _ = pending.reply.send(Err(actor_stopped()));
            }
            Self::RunOperation(pending) => {
                let _ = pending.reply.send(Err(actor_stopped()));
            }
        }
    }

    fn reject_cancelled(self, reason: &str) {
        match self {
            Self::RunTurn(pending) => {
                let _ = pending.reply.send(Err(actor_cancelled(reason)));
            }
            Self::RetryTask(pending) | Self::CancelTask(pending) => {
                let _ = pending.reply.send(Err(actor_cancelled(reason)));
            }
            Self::RunOperation(pending) => {
                let _ = pending.reply.send(Err(actor_cancelled(reason)));
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct SessionActorHandle {
    sender: mpsc::Sender<ActorMessage>,
}

impl SessionActorHandle {
    pub(crate) fn spawn() -> Self {
        let (sender, receiver) = mpsc::channel(ACTOR_CHANNEL_CAPACITY);
        tokio::spawn(SessionExecutionActor::new(receiver, sender.clone()).run());
        Self { sender }
    }

    pub(crate) async fn run_turn(
        &self,
        request: SendMessageRequest,
        executor: Arc<dyn TurnExecutor>,
    ) -> agent_core::Result<()> {
        let (reply, result) = oneshot::channel();
        self.sender
            .send(ActorMessage::RunTurn {
                request,
                executor,
                reply,
            })
            .await
            .map_err(|_| actor_stopped())?;
        result.await.map_err(|_| actor_stopped())?
    }

    pub(crate) async fn run_turn_if_idle(
        &self,
        request: SendMessageRequest,
        executor: Arc<dyn TurnExecutor>,
    ) -> agent_core::Result<()> {
        let (reply, result) = oneshot::channel();
        self.sender
            .send(ActorMessage::RunTurnIfIdle {
                request,
                executor,
                reply,
            })
            .await
            .map_err(|_| actor_stopped())?;
        result.await.map_err(|_| actor_stopped())?
    }

    pub(crate) async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
        executor: Arc<dyn TaskControlExecutor>,
    ) -> agent_core::Result<()> {
        let (reply, result) = oneshot::channel();
        self.sender
            .send(ActorMessage::RetryTask {
                workspace_id,
                session_id,
                task_id,
                executor,
                reply,
            })
            .await
            .map_err(|_| actor_stopped())?;
        result.await.map_err(|_| actor_stopped())?
    }

    pub(crate) async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
        executor: Arc<dyn TaskControlExecutor>,
    ) -> agent_core::Result<()> {
        let (reply, result) = oneshot::channel();
        self.sender
            .send(ActorMessage::CancelTask {
                workspace_id,
                session_id,
                task_id,
                executor,
                reply,
            })
            .await
            .map_err(|_| actor_stopped())?;
        result.await.map_err(|_| actor_stopped())?
    }

    pub(crate) async fn run_operation(
        &self,
        operation: SessionOperation,
    ) -> agent_core::Result<()> {
        let (reply, result) = oneshot::channel();
        self.sender
            .send(ActorMessage::RunOperation { operation, reply })
            .await
            .map_err(|_| actor_stopped())?;
        result.await.map_err(|_| actor_stopped())?
    }

    pub(crate) async fn cancel(&self, reason: String) -> agent_core::Result<()> {
        let (reply, result) = oneshot::channel();
        self.sender
            .send(ActorMessage::Cancel { reason, reply })
            .await
            .map_err(|_| actor_stopped())?;
        result.await.map_err(|_| actor_stopped())?
    }

    pub(crate) async fn state(&self) -> Option<ExecutionState> {
        let (reply, result) = oneshot::channel();
        self.sender.send(ActorMessage::State { reply }).await.ok()?;
        result.await.ok()
    }

    pub(crate) async fn shutdown(&self) -> agent_core::Result<()> {
        let (reply, result) = oneshot::channel();
        self.sender
            .send(ActorMessage::Shutdown { reply })
            .await
            .map_err(|_| actor_stopped())?;
        result.await.map_err(|_| actor_stopped())?
    }
}

struct SessionExecutionActor {
    receiver: mpsc::Receiver<ActorMessage>,
    sender: mpsc::Sender<ActorMessage>,
    state: ExecutionState,
    running: Option<RunningTurn>,
    pending: VecDeque<PendingCommand>,
}

impl SessionExecutionActor {
    fn new(receiver: mpsc::Receiver<ActorMessage>, sender: mpsc::Sender<ActorMessage>) -> Self {
        Self {
            receiver,
            sender,
            state: ExecutionState::Idle,
            running: None,
            pending: VecDeque::new(),
        }
    }

    async fn run(mut self) {
        loop {
            if self.running.is_some() {
                let active = self.running.as_mut().expect("running turn exists");
                tokio::select! {
                    join_result = &mut active.join => {
                        self.finish_running_turn(join_result).await;
                    }
                    message = self.receiver.recv() => {
                        if !self.handle_message(message).await {
                            break;
                        }
                    }
                }
            } else {
                let message = self.receiver.recv().await;
                if !self.handle_message(message).await {
                    break;
                }
            }
        }
    }

    async fn handle_message(&mut self, message: Option<ActorMessage>) -> bool {
        let Some(message) = message else {
            return false;
        };

        match message {
            ActorMessage::RunTurn {
                request,
                executor,
                reply,
            } => {
                self.handle_command(PendingCommand::RunTurn(PendingTurn {
                    request,
                    executor,
                    reply,
                }))
                .await;
                true
            }
            ActorMessage::RunTurnIfIdle {
                request,
                executor,
                reply,
            } => {
                self.handle_run_turn_if_idle(request, executor, reply).await;
                true
            }
            ActorMessage::Cancel { reason, reply } => {
                self.handle_cancel(reason, reply);
                true
            }
            ActorMessage::ForceAbort { turn_id } => {
                self.handle_force_abort(turn_id).await;
                true
            }
            ActorMessage::RetryTask {
                workspace_id,
                session_id,
                task_id,
                executor,
                reply,
            } => {
                self.handle_command(PendingCommand::RetryTask(PendingTaskControl {
                    workspace_id,
                    session_id,
                    task_id,
                    executor,
                    reply,
                }))
                .await;
                true
            }
            ActorMessage::CancelTask {
                workspace_id,
                session_id,
                task_id,
                executor,
                reply,
            } => {
                self.handle_command(PendingCommand::CancelTask(PendingTaskControl {
                    workspace_id,
                    session_id,
                    task_id,
                    executor,
                    reply,
                }))
                .await;
                true
            }
            ActorMessage::RunOperation { operation, reply } => {
                self.handle_command(PendingCommand::RunOperation(PendingOperation {
                    operation,
                    reply,
                }))
                .await;
                true
            }
            ActorMessage::State { reply } => {
                let _ = reply.send(self.state.clone());
                true
            }
            ActorMessage::Shutdown { reply } => {
                self.handle_shutdown(reply);
                false
            }
        }
    }

    async fn handle_command(&mut self, command: PendingCommand) {
        if self.state == ExecutionState::Stopped {
            command.reject_stopped();
            return;
        }

        if matches!(self.state, ExecutionState::Cancelling { .. }) {
            command.reject_cancelled("cancellation in progress");
            return;
        }

        if self.running.is_some() {
            self.pending.push_back(command);
            return;
        }

        self.execute_or_start_command(command).await;
        self.drain_pending_until_running().await;
    }

    async fn handle_run_turn_if_idle(
        &mut self,
        request: SendMessageRequest,
        executor: Arc<dyn TurnExecutor>,
        reply: oneshot::Sender<agent_core::Result<()>>,
    ) {
        match &self.state {
            ExecutionState::Running { turn_id } => {
                let _ = reply.send(Err(actor_busy(
                    &request.session_id,
                    format!("session execution running ({turn_id})"),
                )));
                return;
            }
            ExecutionState::Cancelling { turn_id } => {
                let _ = reply.send(Err(actor_busy(
                    &request.session_id,
                    format!("session execution cancelling ({turn_id})"),
                )));
                return;
            }
            ExecutionState::Stopped => {
                let _ = reply.send(Err(actor_stopped()));
                return;
            }
            ExecutionState::Idle => {}
        }

        if self.running.is_some() {
            let _ = reply.send(Err(actor_busy(
                &request.session_id,
                "session execution running".to_string(),
            )));
            return;
        }

        self.handle_command(PendingCommand::RunTurn(PendingTurn {
            request,
            executor,
            reply,
        }))
        .await;
    }

    async fn execute_or_start_command(&mut self, command: PendingCommand) {
        match command {
            PendingCommand::RunTurn(pending) => self.start_turn(pending),
            PendingCommand::RetryTask(pending) => {
                let result = pending
                    .executor
                    .retry_task(pending.workspace_id, pending.session_id, pending.task_id)
                    .await;
                let _ = pending.reply.send(result);
            }
            PendingCommand::CancelTask(pending) => {
                let result = pending
                    .executor
                    .cancel_task(pending.workspace_id, pending.session_id, pending.task_id)
                    .await;
                let _ = pending.reply.send(result);
            }
            PendingCommand::RunOperation(pending) => {
                let result = pending.operation.await;
                let _ = pending.reply.send(result);
            }
        }
    }

    async fn drain_pending_until_running(&mut self) {
        while self.running.is_none() && self.state != ExecutionState::Stopped {
            let Some(next) = self.pending.pop_front() else {
                self.state = ExecutionState::Idle;
                return;
            };
            self.execute_or_start_command(next).await;
        }
    }

    fn start_turn(&mut self, pending: PendingTurn) {
        let turn_id = format!("turn_{}", uuid::Uuid::new_v4().simple());
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let PendingTurn {
            request,
            executor,
            reply,
        } = pending;
        let join =
            tokio::spawn(async move { executor.execute_turn(request, task_cancellation).await });

        self.state = ExecutionState::Running {
            turn_id: turn_id.clone(),
        };
        self.running = Some(RunningTurn {
            turn_id,
            cancellation,
            reply,
            join,
        });
    }

    fn handle_cancel(&mut self, reason: String, reply: oneshot::Sender<agent_core::Result<()>>) {
        if let Some(active) = &self.running {
            active.cancellation.cancel();
            self.state = ExecutionState::Cancelling {
                turn_id: active.turn_id.clone(),
            };
            schedule_force_abort(self.sender.clone(), active.turn_id.clone());
        }
        self.reject_pending_cancelled(&reason);
        let _ = reply.send(Ok(()));
    }

    async fn handle_force_abort(&mut self, turn_id: String) {
        let should_abort = matches!(
            &self.state,
            ExecutionState::Cancelling { turn_id: active_turn_id } if active_turn_id == &turn_id
        );
        if !should_abort {
            return;
        }

        if let Some(active) = self.running.take() {
            active.cancellation.cancel();
            active.join.abort();
            let _ = active.reply.send(Err(actor_cancelled(
                "force aborted after cancellation grace period",
            )));
        }

        if self.state != ExecutionState::Stopped {
            self.drain_pending_until_running().await;
        }
    }

    fn reject_pending_cancelled(&mut self, reason: &str) {
        while let Some(command) = self.pending.pop_front() {
            command.reject_cancelled(reason);
        }
    }

    fn handle_shutdown(&mut self, reply: oneshot::Sender<agent_core::Result<()>>) {
        self.state = ExecutionState::Stopped;
        if let Some(active) = self.running.take() {
            active.cancellation.cancel();
            active.join.abort();
            let _ = active.reply.send(Err(actor_stopped()));
        }
        while let Some(command) = self.pending.pop_front() {
            command.reject_stopped();
        }
        let _ = reply.send(Ok(()));
    }

    async fn finish_running_turn(
        &mut self,
        join_result: Result<agent_core::Result<()>, tokio::task::JoinError>,
    ) {
        let Some(active) = self.running.take() else {
            return;
        };
        let result = match join_result {
            Ok(result) => result,
            Err(error) => Err(CoreError::InvalidState(format!(
                "session execution task failed: {error}",
            ))),
        };
        let _ = active.reply.send(result);
        if self.state != ExecutionState::Stopped {
            self.drain_pending_until_running().await;
        }
    }
}

fn actor_stopped() -> CoreError {
    CoreError::InvalidState("session execution actor stopped".into())
}

fn actor_cancelled(reason: &str) -> CoreError {
    CoreError::InvalidState(format!("session execution cancelled: {reason}"))
}

fn actor_busy(session_id: &SessionId, reason: String) -> CoreError {
    CoreError::SessionBusy {
        session_id: session_id.to_string(),
        reason,
    }
}

fn schedule_force_abort(sender: mpsc::Sender<ActorMessage>, turn_id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(cancel_force_abort_grace()).await;
        let _ = sender.send(ActorMessage::ForceAbort { turn_id }).await;
    });
}

#[cfg(test)]
fn cancel_force_abort_grace() -> std::time::Duration {
    std::time::Duration::from_millis(50)
}

#[cfg(not(test))]
fn cancel_force_abort_grace() -> std::time::Duration {
    std::time::Duration::from_secs(5)
}

#[cfg(test)]
#[path = "session_actor_tests.rs"]
mod tests;
