use std::collections::HashMap;
use std::sync::Arc;

use agent_core::{SendMessageRequest, SessionId};
use tokio::sync::Mutex;

use super::session_actor::SessionActorHandle;
use super::types::{ExecutionState, TurnExecutor};

#[derive(Clone)]
pub struct SessionExecutionRuntime {
    executor: Arc<dyn TurnExecutor>,
    actors: Arc<Mutex<HashMap<String, SessionActorHandle>>>,
}

impl SessionExecutionRuntime {
    pub fn new<E>(executor: Arc<E>) -> Self
    where
        E: TurnExecutor,
    {
        let executor: Arc<dyn TurnExecutor> = executor;
        Self {
            executor,
            actors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run_turn(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        let actor = self.actor_for(&request.session_id).await;
        actor.run_turn(request).await
    }

    pub async fn cancel_session(
        &self,
        session_id: &SessionId,
        reason: String,
    ) -> agent_core::Result<()> {
        let actor = self.actor(session_id).await;
        if let Some(actor) = actor {
            actor.cancel(reason).await
        } else {
            Ok(())
        }
    }

    pub async fn shutdown_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        let actor = {
            let mut actors = self.actors.lock().await;
            actors.remove(&session_id.to_string())
        };
        if let Some(actor) = actor {
            actor.shutdown().await
        } else {
            Ok(())
        }
    }

    pub async fn session_state(&self, session_id: &SessionId) -> Option<ExecutionState> {
        let actor = self.actor(session_id).await?;
        actor.state().await
    }

    pub async fn actor_count(&self) -> usize {
        self.actors.lock().await.len()
    }

    async fn actor_for(&self, session_id: &SessionId) -> SessionActorHandle {
        let mut actors = self.actors.lock().await;
        if let Some(actor) = actors.get(&session_id.to_string()) {
            return actor.clone();
        }

        let actor = SessionActorHandle::spawn(session_id.clone(), Arc::clone(&self.executor));
        actors.insert(session_id.to_string(), actor.clone());
        actor
    }

    async fn actor(&self, session_id: &SessionId) -> Option<SessionActorHandle> {
        self.actors
            .lock()
            .await
            .get(&session_id.to_string())
            .cloned()
    }
}
