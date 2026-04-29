use agent_core::{
    AgentId, AppFacade, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification,
    SendMessageRequest, SessionId, StartSessionRequest, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use agent_store::EventStore;
use async_trait::async_trait;
use futures::{stream, StreamExt};
use std::sync::Arc;

pub struct LocalRuntime<S, M> {
    store: Arc<S>,
    model: Arc<M>,
}

impl<S, M> LocalRuntime<S, M> {
    pub fn new(store: S, model: M) -> Self {
        Self {
            store: Arc::new(store),
            model: Arc::new(model),
        }
    }
}

#[async_trait]
impl<S, M> AppFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    async fn open_workspace(&self, path: String) -> agent_core::Result<WorkspaceInfo> {
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        self.store
            .append(&DomainEvent::new(
                workspace_id.clone(),
                session_id,
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::WorkspaceOpened { path: path.clone() },
            ))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(WorkspaceInfo { workspace_id, path })
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        let session_id = SessionId::new();
        self.store
            .append(&DomainEvent::new(
                request.workspace_id,
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::AgentTaskCreated {
                    task_id: agent_core::TaskId::new(),
                    title: format!("Session using {}", request.model_profile),
                },
            ))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(session_id)
    }

    async fn send_message(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        self.store
            .append(&DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::UserMessageAdded {
                    message_id: "msg_user_latest".into(),
                    content: request.content.clone(),
                },
            ))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;

        let mut stream = self
            .model
            .stream(ModelRequest::user_text("fake", request.content))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let mut assistant = String::new();
        while let Some(event) = stream.next().await {
            match event.map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))? {
                ModelEvent::TokenDelta(delta) => {
                    assistant.push_str(&delta);
                    self.store
                        .append(&DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ModelTokenDelta { delta },
                        ))
                        .await
                        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                }
                ModelEvent::Completed { .. } => {
                    self.store
                        .append(&DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::AssistantMessageCompleted {
                                message_id: "msg_assistant_latest".into(),
                                content: assistant.clone(),
                            },
                        ))
                        .await
                        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                }
                ModelEvent::ToolCallRequested {
                    tool_call_id,
                    tool_id,
                    ..
                } => {
                    self.store
                        .append(&DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ModelToolCallRequested {
                                tool_call_id,
                                tool_id,
                            },
                        ))
                        .await
                        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                }
                ModelEvent::Failed { message } => {
                    return Err(agent_core::CoreError::InvalidState(message));
                }
            }
        }
        Ok(())
    }

    async fn decide_permission(&self, decision: PermissionDecision) -> agent_core::Result<()> {
        let _ = decision;
        Ok(())
    }

    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> agent_core::Result<()> {
        self.store
            .append(&DomainEvent::new(
                workspace_id,
                session_id,
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::SessionCancelled {
                    reason: "user requested cancellation".into(),
                },
            ))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<agent_core::projection::SessionProjection> {
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(agent_core::projection::SessionProjection::from_events(
            &events,
        ))
    }

    async fn get_trace(&self, session_id: SessionId) -> agent_core::Result<Vec<TraceEntry>> {
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(events
            .into_iter()
            .map(|event| TraceEntry { event })
            .collect())
    }

    fn subscribe_session(
        &self,
        session_id: SessionId,
    ) -> futures::stream::BoxStream<'static, DomainEvent> {
        let _ = session_id;
        Box::pin(stream::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
    use agent_models::FakeModelClient;
    use agent_store::SqliteEventStore;

    #[tokio::test]
    async fn send_message_records_user_and_assistant_events() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hello".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hi".into(),
            })
            .await
            .unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].content, "hi");
        assert_eq!(projection.messages[1].content, "hello");
    }
}
