use agent_core::{
    AgentId, AppFacade, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification,
    SendMessageRequest, SessionId, StartSessionRequest, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_memory::ContextAssembler;
use agent_models::{ModelClient, ModelEvent, ModelRequest, ToolCall};
use agent_store::EventStore;
use agent_tools::{
    BuiltinProvider, PermissionEngine, PermissionMode, PermissionOutcome, ToolInvocation,
    ToolProvider, ToolRegistry,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const MAX_AGENT_LOOP_ITERATIONS: usize = 20;
const EVENT_CHANNEL_CAPACITY: usize = 1024;

pub struct LocalRuntime<S, M> {
    store: Arc<S>,
    model: Arc<M>,
    permission_engine: PermissionEngine,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    context_assembler: ContextAssembler,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
}

impl<S, M> LocalRuntime<S, M> {
    pub fn new(store: S, model: M) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            store: Arc::new(store),
            model: Arc::new(model),
            permission_engine: PermissionEngine::new(PermissionMode::Suggest),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            context_assembler: ContextAssembler::new(100_000),
            event_tx,
        }
    }

    pub fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_engine = PermissionEngine::new(mode);
        self
    }

    pub fn with_context_limit(mut self, max_tokens: usize) -> Self {
        self.context_assembler = ContextAssembler::new(max_tokens);
        self
    }

    pub fn tool_registry(&self) -> Arc<Mutex<ToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Register builtin tools (shell.exec, search.ripgrep, patch.apply, fs.read)
    pub async fn with_builtin_tools(self, workspace_root: PathBuf) -> Self {
        let provider = BuiltinProvider::with_defaults(workspace_root);
        self.tool_registry
            .lock()
            .await
            .add_provider(Box::new(provider))
            .await;
        self
    }

    /// Register a custom tool provider
    pub async fn with_provider(self, provider: Box<dyn ToolProvider>) -> Self {
        self.tool_registry.lock().await.add_provider(provider).await;
        self
    }
}

async fn append_and_broadcast<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    event: &DomainEvent,
) -> agent_core::Result<()> {
    store
        .append(event)
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
    let _ = event_tx.send(event.clone());
    Ok(())
}

fn build_model_messages(
    user_content: &str,
    session_events: &[DomainEvent],
) -> Vec<agent_models::ModelMessage> {
    let mut messages = Vec::new();
    for event in session_events {
        match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => {
                messages.push(agent_models::ModelMessage {
                    role: "user".into(),
                    content: content.clone(),
                });
            }
            EventPayload::AssistantMessageCompleted { content, .. } => {
                messages.push(agent_models::ModelMessage {
                    role: "assistant".into(),
                    content: content.clone(),
                });
            }
            EventPayload::ToolInvocationCompleted { output_preview, .. } => {
                messages.push(agent_models::ModelMessage {
                    role: "tool".into(),
                    content: output_preview.clone(),
                });
            }
            _ => {}
        }
    }
    if messages.is_empty() || messages.last().map(|m| m.content.as_str()) != Some(user_content) {
        messages.push(agent_models::ModelMessage {
            role: "user".into(),
            content: user_content.into(),
        });
    }
    messages
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
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::WorkspaceOpened { path: path.clone() },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
        Ok(WorkspaceInfo { workspace_id, path })
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        let session_id = SessionId::new();
        let event = DomainEvent::new(
            request.workspace_id,
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskCreated {
                task_id: agent_core::TaskId::new(),
                title: format!("Session using {}", request.model_profile),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
        Ok(session_id)
    }

    async fn send_message(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        // Record user message
        let user_event = DomainEvent::new(
            request.workspace_id.clone(),
            request.session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                content: request.content.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &user_event).await?;

        // Load session history for context
        let session_events = self
            .store
            .load_session(&request.session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        let messages = build_model_messages(&request.content, &session_events);

        // Inject registered tool definitions into model request
        let tool_defs = {
            let registry = self.tool_registry.lock().await;
            let definitions = registry.list_all().await;
            definitions
                .into_iter()
                .map(|td| agent_models::ToolDefinition {
                    name: td.tool_id,
                    description: td.description,
                    parameters: serde_json::json!({"type": "object"}),
                })
                .collect()
        };

        // Use the session's model profile to route to the correct model client.
        // When sessions are created via start_session(), the profile is recorded
        // in the AgentTaskCreated event title as "Session using {profile}".
        // We extract it, or fall back to "fake" for backward compatibility.
        let model_profile = session_events
            .iter()
            .find_map(|e| match &e.payload {
                EventPayload::AgentTaskCreated { title, .. } => {
                    title.strip_prefix("Session using ").map(|s| s.to_string())
                }
                _ => None,
            })
            .unwrap_or_else(|| "fake".to_string());

        let model_request = ModelRequest {
            model_profile,
            messages,
            system_prompt: Some("You are a helpful assistant.".into()),
            tools: tool_defs,
        };

        // Agent loop: model -> tool call -> permission -> execute -> feed back
        let mut current_request = model_request;
        let mut iterations = 0;

        loop {
            if iterations >= MAX_AGENT_LOOP_ITERATIONS {
                let event = DomainEvent::new(
                    request.workspace_id.clone(),
                    request.session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::AssistantMessageCompleted {
                        message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                        content: "[agent loop reached maximum iterations]".into(),
                    },
                );
                append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                break;
            }
            iterations += 1;

            let stream_result = self.model.stream(current_request.clone()).await;

            let mut stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    let error_msg = e.to_string();
                    let fail_event = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::AgentTaskFailed {
                            task_id: agent_core::TaskId::new(),
                            error: error_msg.clone(),
                        },
                    );
                    let _ = append_and_broadcast(&*self.store, &self.event_tx, &fail_event).await;
                    return Err(agent_core::CoreError::InvalidState(error_msg));
                }
            };

            let mut assistant_text = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();

            while let Some(event_result) = stream.next().await {
                match event_result {
                    Ok(ModelEvent::TokenDelta(delta)) => {
                        assistant_text.push_str(&delta);
                        let event = DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ModelTokenDelta { delta },
                        );
                        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                    }
                    Ok(ModelEvent::ToolCallRequested {
                        tool_call_id,
                        tool_id,
                        arguments,
                    }) => {
                        let event = DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ModelToolCallRequested {
                                tool_call_id: tool_call_id.clone(),
                                tool_id: tool_id.clone(),
                            },
                        );
                        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                        tool_calls.push(ToolCall {
                            id: tool_call_id,
                            name: tool_id,
                            arguments,
                        });
                    }
                    Ok(ModelEvent::Completed { .. }) => {
                        if !assistant_text.is_empty() {
                            let event = DomainEvent::new(
                                request.workspace_id.clone(),
                                request.session_id.clone(),
                                AgentId::system(),
                                PrivacyClassification::FullTrace,
                                EventPayload::AssistantMessageCompleted {
                                    message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                                    content: assistant_text.clone(),
                                },
                            );
                            append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                        }
                    }
                    Ok(ModelEvent::Failed { message }) => {
                        let fail_event = DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::AgentTaskFailed {
                                task_id: agent_core::TaskId::new(),
                                error: message.clone(),
                            },
                        );
                        let _ =
                            append_and_broadcast(&*self.store, &self.event_tx, &fail_event).await;
                        return Err(agent_core::CoreError::InvalidState(message));
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let fail_event = DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::AgentTaskFailed {
                                task_id: agent_core::TaskId::new(),
                                error: error_msg.clone(),
                            },
                        );
                        let _ =
                            append_and_broadcast(&*self.store, &self.event_tx, &fail_event).await;
                        return Err(agent_core::CoreError::InvalidState(error_msg));
                    }
                }
            }

            // If no tool calls, the agent loop ends
            if tool_calls.is_empty() {
                break;
            }

            // Process tool calls through permission and execution
            let registry = self.tool_registry.lock().await;
            for tc in &tool_calls {
                // Check permission
                let risk = if let Some(tool) = registry.get(&tc.name).await {
                    let inv = ToolInvocation {
                        tool_id: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        workspace_id: request.workspace_id.to_string(),
                        preview: format!("{}({})", tc.name, tc.arguments),
                        timeout_ms: 30_000,
                        output_limit_bytes: 102_400,
                    };
                    tool.risk(&inv)
                } else {
                    agent_tools::ToolRisk::read(&tc.name)
                };

                let permission_event = match self.permission_engine.decide(&risk) {
                    PermissionOutcome::Allowed => DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::PermissionGranted {
                            request_id: tc.id.clone(),
                        },
                    ),
                    PermissionOutcome::RequiresApproval => DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::PermissionDenied {
                            request_id: tc.id.clone(),
                            reason: "requires user approval".into(),
                        },
                    ),
                    PermissionOutcome::Denied(reason) => DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::PermissionDenied {
                            request_id: tc.id.clone(),
                            reason,
                        },
                    ),
                };
                append_and_broadcast(&*self.store, &self.event_tx, &permission_event).await?;

                // Only execute if permission was granted
                if matches!(
                    &permission_event.payload,
                    EventPayload::PermissionGranted { .. }
                ) {
                    let invocation = ToolInvocation {
                        tool_id: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        workspace_id: request.workspace_id.to_string(),
                        preview: format!("{}({})", tc.name, tc.arguments),
                        timeout_ms: 30_000,
                        output_limit_bytes: 102_400,
                    };

                    let tool_start = std::time::Instant::now();

                    let start_event = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::ToolInvocationStarted {
                            invocation_id: tc.id.clone(),
                            tool_id: tc.name.clone(),
                        },
                    );
                    append_and_broadcast(&*self.store, &self.event_tx, &start_event).await?;

                    let result = registry
                        .invoke_with_permission(&self.permission_engine, invocation)
                        .await;

                    let completion_event = match result {
                        Ok(output) => DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ToolInvocationCompleted {
                                invocation_id: tc.id.clone(),
                                tool_id: tc.name.clone(),
                                output_preview: output.text.chars().take(500).collect(),
                                exit_code: None,
                                duration_ms: tool_start.elapsed().as_millis() as u64,
                                truncated: output.truncated,
                            },
                        ),
                        Err(e) => DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ToolInvocationFailed {
                                invocation_id: tc.id.clone(),
                                tool_id: tc.name.clone(),
                                error: e.to_string(),
                            },
                        ),
                    };
                    append_and_broadcast(&*self.store, &self.event_tx, &completion_event).await?;
                }
            }
            drop(registry);

            // Build next request with tool results appended
            current_request = current_request
                .clone()
                .add_message("assistant", &assistant_text);
            let session_events = self
                .store
                .load_session(&request.session_id)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
            for tc in &tool_calls {
                let tool_results_for_call: Vec<String> = session_events
                    .iter()
                    .filter_map(|e| match &e.payload {
                        EventPayload::ToolInvocationCompleted {
                            invocation_id,
                            output_preview,
                            ..
                        } => {
                            if invocation_id == &tc.id {
                                Some(output_preview.clone())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect();
                if !tool_results_for_call.is_empty() {
                    current_request = current_request.add_message(
                        "tool",
                        format!(
                            "tool_call_id={}\ntool_id={}\nresult={}",
                            tc.id,
                            tc.name,
                            tool_results_for_call.join("\n")
                        ),
                    );
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
        let event = DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SessionCancelled {
                reason: "user requested cancellation".into(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<agent_core::projection::SessionProjection> {
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        Ok(agent_core::projection::SessionProjection::from_events(
            &events,
        ))
    }

    async fn get_trace(&self, session_id: SessionId) -> agent_core::Result<Vec<TraceEntry>> {
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        Ok(events
            .into_iter()
            .map(|event| TraceEntry { event })
            .collect())
    }

    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent> {
        let mut rx = self.event_tx.subscribe();
        Box::pin(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                if event.session_id == session_id {
                    yield event;
                }
            }
        })
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
