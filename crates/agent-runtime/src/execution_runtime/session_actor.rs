use std::sync::Arc;

use agent_core::{CoreError, SendMessageRequest, SessionId};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::types::{ExecutionState, TurnExecutor};

const ACTOR_CHANNEL_CAPACITY: usize = 32;

pub(crate) enum ActorMessage {
    RunTurn {
        request: SendMessageRequest,
        executor: Arc<dyn TurnExecutor>,
        reply: oneshot::Sender<agent_core::Result<()>>,
    },
    Cancel {
        reason: String,
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

#[derive(Clone)]
pub(crate) struct SessionActorHandle {
    sender: mpsc::Sender<ActorMessage>,
}

impl SessionActorHandle {
    pub(crate) fn spawn(session_id: SessionId) -> Self {
        let (sender, receiver) = mpsc::channel(ACTOR_CHANNEL_CAPACITY);
        tokio::spawn(SessionExecutionActor::new(session_id, receiver).run());
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
    session_id: SessionId,
    receiver: mpsc::Receiver<ActorMessage>,
    state: ExecutionState,
    running: Option<RunningTurn>,
}

impl SessionExecutionActor {
    fn new(session_id: SessionId, receiver: mpsc::Receiver<ActorMessage>) -> Self {
        Self {
            session_id,
            receiver,
            state: ExecutionState::Idle,
            running: None,
        }
    }

    async fn run(mut self) {
        loop {
            if self.running.is_some() {
                let active = self.running.as_mut().expect("running turn exists");
                tokio::select! {
                    join_result = &mut active.join => {
                        self.finish_running_turn(join_result);
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
                self.handle_run_turn(request, executor, reply);
                true
            }
            ActorMessage::Cancel { reason, reply } => {
                self.handle_cancel(reason, reply);
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

    fn handle_run_turn(
        &mut self,
        request: SendMessageRequest,
        executor: Arc<dyn TurnExecutor>,
        reply: oneshot::Sender<agent_core::Result<()>>,
    ) {
        if self.running.is_some() {
            let _ = reply.send(Err(CoreError::SessionBusy {
                session_id: self.session_id.to_string(),
                reason: "session execution already running".into(),
            }));
            return;
        }

        if self.state == ExecutionState::Stopped {
            let _ = reply.send(Err(actor_stopped()));
            return;
        }

        let turn_id = format!("turn_{}", uuid::Uuid::new_v4().simple());
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
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

    fn handle_cancel(&mut self, _reason: String, reply: oneshot::Sender<agent_core::Result<()>>) {
        if let Some(active) = &self.running {
            active.cancellation.cancel();
            self.state = ExecutionState::Cancelling {
                turn_id: active.turn_id.clone(),
            };
        }
        let _ = reply.send(Ok(()));
    }

    fn handle_shutdown(&mut self, reply: oneshot::Sender<agent_core::Result<()>>) {
        self.state = ExecutionState::Stopped;
        if let Some(active) = self.running.take() {
            active.cancellation.cancel();
            active.join.abort();
            let _ = active.reply.send(Err(actor_stopped()));
        }
        let _ = reply.send(Ok(()));
    }

    fn finish_running_turn(
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
            self.state = ExecutionState::Idle;
        }
    }
}

fn actor_stopped() -> CoreError {
    CoreError::InvalidState("session execution actor stopped".into())
}
