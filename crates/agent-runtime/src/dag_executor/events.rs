use crate::event_emitter::append_and_broadcast;
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PrivacyClassification, TaskId, WorkspaceId,
};
use agent_store::EventStore;
use std::sync::Arc;

/// Event emission helpers for the DAG executor.
pub(crate) struct EventEmitter<S: EventStore> {
    pub store: Arc<S>,
    pub event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
}

impl<S: EventStore> EventEmitter<S> {
    pub async fn emit_task_created(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
        title: &str,
        role: AgentRole,
        dependencies: &[TaskId],
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskCreated {
                task_id: task_id.clone(),
                title: title.to_string(),
                role,
                dependencies: dependencies.to_vec(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    pub async fn emit_task_started(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskStarted {
                task_id: task_id.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    pub async fn emit_task_completed(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskCompleted {
                task_id: task_id.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    pub async fn emit_task_failed(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
        error: &str,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskFailed {
                task_id: task_id.clone(),
                error: error.to_string(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    pub async fn emit_task_cancelled(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::TaskCancelled {
                task_id: task_id.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    pub async fn emit_task_blocked(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
        blocking_task_id: &TaskId,
        reason: &str,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::TaskBlocked {
                task_id: task_id.clone(),
                blocking_task_id: blocking_task_id.clone(),
                reason: reason.to_string(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    pub async fn emit_agent_spawned(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        agent_id: &AgentId,
        role: AgentRole,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            agent_id.clone(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentSpawned {
                agent_id: agent_id.to_string(),
                role: role.to_string(),
                task_id: task_id.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    pub async fn emit_agent_idle(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        agent_id: &AgentId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            agent_id.clone(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentIdle {
                agent_id: agent_id.to_string(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }
}
