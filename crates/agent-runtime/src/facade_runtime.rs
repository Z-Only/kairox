use agent_core::{
    AgentId, AppFacade, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification,
    SendMessageRequest, SessionId, StartSessionRequest, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_memory::{
    durable_memory_requires_confirmation, extract_memory_markers, strip_memory_markers,
    ContextAssembler, MemoryEntry, MemoryStore,
};
use agent_models::{ModelClient, ModelEvent, ModelRequest, ToolCall};
use agent_store::{EventStore, SessionRow};
use agent_tools::{
    BuiltinProvider, PermissionEngine, PermissionMode, PermissionOutcome, ToolInvocation,
    ToolProvider, ToolRegistry,
};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const SYSTEM_PROMPT: &str = "\
You are Kairox, a helpful AI assistant with memory capabilities.\n\n\
## Memory Protocol\n\
When you learn something worth remembering about the user or workspace, \
use <memory> tags to save it. Examples:\n\
- <memory scope=\"session\">Temporary note for this session</memory>\n\
- <memory scope=\"user\" key=\"preferred-language\">User prefers Rust</memory>\n\
- <memory scope=\"workspace\" key=\"build-cmd\">Use cargo nextest</memory>\n\n\
Guidelines:\n\
- Use scope=\"session\" for temporary notes (auto-accepted)\n\
- Use scope=\"user\" for user preferences (requires approval)\n\
- Use scope=\"workspace\" for project settings (requires approval)\n\
- Always include a key when using user or workspace scope\n\
- You may include multiple <memory> tags in one response\n\
- The <memory> tags will be stripped from displayed output, so also state \
the information naturally in your response.\n\
";

const MAX_AGENT_LOOP_ITERATIONS: usize = 20;
const EVENT_CHANNEL_CAPACITY: usize = 1024;

pub struct LocalRuntime<S, M> {
    store: Arc<S>,
    model: Arc<M>,
    permission_engine: PermissionEngine,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    context_assembler: ContextAssembler,
    memory_store: Option<Arc<dyn MemoryStore>>,
    pending_permissions:
        Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
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
            context_assembler: ContextAssembler::new_standalone(100_000),
            memory_store: None,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    pub fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_engine = PermissionEngine::new(mode);
        self
    }

    pub fn with_context_limit(mut self, max_tokens: usize) -> Self {
        self.context_assembler = ContextAssembler::new_standalone(max_tokens);
        self
    }

    pub fn tool_registry(&self) -> Arc<Mutex<ToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Set the memory store for persistent memory.
    pub fn with_memory_store(mut self, store: Arc<dyn MemoryStore>) -> Self {
        self.memory_store = Some(store.clone());
        self.context_assembler = ContextAssembler::new(100_000, store);
        self
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

/// Resolve a pending permission request (used by GUI Interactive mode).
impl<S, M> LocalRuntime<S, M> {
    pub async fn resolve_permission(
        &self,
        request_id: &str,
        decision: PermissionDecision,
    ) -> agent_core::Result<()> {
        if let Some(tx) = self.pending_permissions.lock().await.remove(request_id) {
            let _ = tx.send(decision);
        }
        Ok(())
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
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::WorkspaceOpened { path: path.clone() },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        // Persist workspace metadata for session recovery
        if let Err(e) = self
            .store
            .upsert_workspace(&workspace_id.to_string(), &path)
            .await
        {
            eprintln!("[runtime] Failed to persist workspace metadata: {e}");
        }

        Ok(WorkspaceInfo { workspace_id, path })
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        let session_id = SessionId::new();
        let event = DomainEvent::new(
            request.workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskCreated {
                task_id: agent_core::TaskId::new(),
                title: format!("Session using {}", request.model_profile),
                role: agent_core::AgentRole::Planner,
                dependencies: vec![],
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        // Persist session metadata for session recovery
        let now = chrono::Utc::now().to_rfc3339();
        let session_row = SessionRow {
            session_id: session_id.to_string(),
            workspace_id: request.workspace_id.to_string(),
            title: format!("Session using {}", request.model_profile),
            model_profile: request.model_profile.clone(),
            model_id: None,
            provider: None,
            deleted_at: None,
            created_at: now.clone(),
            updated_at: now,
        };
        if let Err(e) = self.store.upsert_session(&session_row).await {
            eprintln!("[runtime] Failed to persist session metadata: {e}");
        }

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

        // Retrieve relevant memories from the MemoryStore and inject them
        // into the system prompt so the model can use prior context.
        let mut system_prompt = SYSTEM_PROMPT.to_string();
        if let Some(ref mem_store) = self.memory_store {
            let keywords = agent_memory::extract_keywords(&request.content);

            // First try keyword-based retrieval; if no matches found,
            // fall back to returning all accepted user/workspace memories.
            // This ensures cross-session context is always available even
            // when the query keywords don't directly match memory content
            // (common with Chinese text where extract_keywords is limited).
            let mut memories = mem_store
                .query(agent_memory::MemoryQuery {
                    scope: None,
                    keywords: keywords.clone(),
                    limit: 20,
                    session_id: None,
                    workspace_id: None,
                })
                .await
                .unwrap_or_default();

            if memories.is_empty() {
                memories = mem_store
                    .query(agent_memory::MemoryQuery {
                        scope: None,
                        keywords: Vec::new(),
                        limit: 20,
                        session_id: None,
                        workspace_id: None,
                    })
                    .await
                    .unwrap_or_default();
            }
            if !memories.is_empty() {
                let memory_section = memories
                    .iter()
                    .filter(|m| m.accepted)
                    .map(|m| {
                        let scope_label = match m.scope {
                            agent_memory::MemoryScope::User => "user",
                            agent_memory::MemoryScope::Workspace => "workspace",
                            agent_memory::MemoryScope::Session => "session",
                        };
                        match &m.key {
                            Some(k) => format!("- [{scope_label}] {k}: {}", m.content),
                            None => format!("- [{scope_label}] {}", m.content),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if !memory_section.is_empty() {
                    system_prompt.push_str("\n\n## Relevant Memories\nThe following memories were previously saved and may be relevant to the user's request. Use this context naturally in your response.\n\n");
                    system_prompt.push_str(&memory_section);
                }
            }
        }

        let model_request = ModelRequest {
            model_profile,
            messages,
            system_prompt: Some(system_prompt),
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
                            let display_content = strip_memory_markers(&assistant_text);
                            let event = DomainEvent::new(
                                request.workspace_id.clone(),
                                request.session_id.clone(),
                                AgentId::system(),
                                PrivacyClassification::FullTrace,
                                EventPayload::AssistantMessageCompleted {
                                    message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                                    content: display_content,
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

            // Process memory markers from assistant response
            if !assistant_text.is_empty() {
                if let Some(ref mem_store) = self.memory_store {
                    let markers = extract_memory_markers(&assistant_text);
                    for marker in markers {
                        let entry = MemoryEntry::from_marker(marker, None, None, false);
                        let mem_id = entry.id.clone();
                        let mem_scope = entry.scope.clone();
                        let mem_key = entry.key.clone();
                        let mem_content = entry.content.clone();
                        if durable_memory_requires_confirmation(&entry.scope) {
                            match self.permission_engine.mode() {
                                PermissionMode::Interactive => {
                                    let (tx, rx) = tokio::sync::oneshot::channel();
                                    self.pending_permissions
                                        .lock()
                                        .await
                                        .insert(mem_id.clone(), tx);
                                    let perm_event = DomainEvent::new(
                                        request.workspace_id.clone(),
                                        request.session_id.clone(),
                                        AgentId::system(),
                                        PrivacyClassification::FullTrace,
                                        EventPayload::MemoryProposed {
                                            memory_id: mem_id.clone(),
                                            scope: format!("{:?}", entry.scope).to_lowercase(),
                                            key: mem_key.clone(),
                                            content: mem_content.clone(),
                                        },
                                    );
                                    let _ = append_and_broadcast(
                                        &*self.store,
                                        &self.event_tx,
                                        &perm_event,
                                    )
                                    .await;
                                    match rx.await {
                                        Ok(PermissionDecision { approve: true, .. }) => {
                                            let mut accepted = entry.clone();
                                            accepted.accepted = true;
                                            let _ = mem_store.store(accepted).await;
                                            let accept_event = DomainEvent::new(
                                                request.workspace_id.clone(),
                                                request.session_id.clone(),
                                                AgentId::system(),
                                                PrivacyClassification::FullTrace,
                                                EventPayload::MemoryAccepted {
                                                    memory_id: mem_id,
                                                    scope: format!("{:?}", mem_scope)
                                                        .to_lowercase(),
                                                    key: mem_key,
                                                    content: mem_content,
                                                },
                                            );
                                            let _ = append_and_broadcast(
                                                &*self.store,
                                                &self.event_tx,
                                                &accept_event,
                                            )
                                            .await;
                                        }
                                        Ok(PermissionDecision {
                                            approve: false,
                                            reason,
                                            ..
                                        }) => {
                                            let reject_event = DomainEvent::new(
                                                request.workspace_id.clone(),
                                                request.session_id.clone(),
                                                AgentId::system(),
                                                PrivacyClassification::FullTrace,
                                                EventPayload::MemoryRejected {
                                                    memory_id: mem_id,
                                                    reason: reason
                                                        .unwrap_or_else(|| "denied".into()),
                                                },
                                            );
                                            let _ = append_and_broadcast(
                                                &*self.store,
                                                &self.event_tx,
                                                &reject_event,
                                            )
                                            .await;
                                        }
                                        Err(_) => {
                                            let reject_event = DomainEvent::new(
                                                request.workspace_id.clone(),
                                                request.session_id.clone(),
                                                AgentId::system(),
                                                PrivacyClassification::FullTrace,
                                                EventPayload::MemoryRejected {
                                                    memory_id: mem_id,
                                                    reason: "cancelled".into(),
                                                },
                                            );
                                            let _ = append_and_broadcast(
                                                &*self.store,
                                                &self.event_tx,
                                                &reject_event,
                                            )
                                            .await;
                                        }
                                    }
                                }
                                PermissionMode::Suggest | PermissionMode::ReadOnly => {
                                    let reject_event = DomainEvent::new(
                                        request.workspace_id.clone(),
                                        request.session_id.clone(),
                                        AgentId::system(),
                                        PrivacyClassification::FullTrace,
                                        EventPayload::MemoryRejected {
                                            memory_id: mem_id,
                                            reason: "Auto-denied in Suggest mode".into(),
                                        },
                                    );
                                    let _ = append_and_broadcast(
                                        &*self.store,
                                        &self.event_tx,
                                        &reject_event,
                                    )
                                    .await;
                                }
                                PermissionMode::Agent | PermissionMode::Autonomous => {
                                    let mut accepted = entry.clone();
                                    accepted.accepted = true;
                                    let _ = mem_store.store(accepted).await;
                                    let accept_event = DomainEvent::new(
                                        request.workspace_id.clone(),
                                        request.session_id.clone(),
                                        AgentId::system(),
                                        PrivacyClassification::FullTrace,
                                        EventPayload::MemoryAccepted {
                                            memory_id: mem_id,
                                            scope: format!("{:?}", mem_scope).to_lowercase(),
                                            key: mem_key,
                                            content: mem_content,
                                        },
                                    );
                                    let _ = append_and_broadcast(
                                        &*self.store,
                                        &self.event_tx,
                                        &accept_event,
                                    )
                                    .await;
                                }
                            }
                        } else {
                            // Session scope: auto-accept
                            let mut accepted = entry.clone();
                            accepted.accepted = true;
                            let _ = mem_store.store(accepted).await;
                            let accept_event = DomainEvent::new(
                                request.workspace_id.clone(),
                                request.session_id.clone(),
                                AgentId::system(),
                                PrivacyClassification::FullTrace,
                                EventPayload::MemoryAccepted {
                                    memory_id: mem_id,
                                    scope: format!("{:?}", mem_scope).to_lowercase(),
                                    key: mem_key,
                                    content: mem_content,
                                },
                            );
                            let _ =
                                append_and_broadcast(&*self.store, &self.event_tx, &accept_event)
                                    .await;
                        }
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
                    PermissionOutcome::Pending => DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::PermissionDenied {
                            request_id: tc.id.clone(),
                            reason: "awaiting user confirmation".into(),
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

    fn subscribe_all(&self) -> BoxStream<'static, DomainEvent> {
        let mut rx = self.event_tx.subscribe();
        Box::pin(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                yield event;
            }
        })
    }

    async fn list_workspaces(&self) -> agent_core::Result<Vec<WorkspaceInfo>> {
        let rows = self
            .store
            .list_workspaces()
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| WorkspaceInfo {
                workspace_id: WorkspaceId::from_string(r.workspace_id),
                path: r.path,
            })
            .collect())
    }

    async fn list_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<agent_core::SessionMeta>> {
        let rows = self
            .store
            .list_active_sessions(&workspace_id.to_string())
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| agent_core::SessionMeta {
                session_id: SessionId::from_string(r.session_id),
                workspace_id: WorkspaceId::from_string(r.workspace_id),
                title: r.title,
                model_profile: r.model_profile,
                model_id: r.model_id,
                provider: r.provider,
                deleted_at: r.deleted_at,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    async fn rename_session(
        &self,
        session_id: &SessionId,
        title: String,
    ) -> agent_core::Result<()> {
        self.store
            .rename_session(&session_id.to_string(), &title)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
    }

    async fn soft_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        self.store
            .soft_delete_session(&session_id.to_string())
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
    }

    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> agent_core::Result<usize> {
        self.store
            .cleanup_expired_sessions(older_than)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
    }

    async fn get_task_graph(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<agent_core::TaskGraphSnapshot> {
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let projection = agent_core::projection::SessionProjection::from_events(&events);
        Ok(projection.task_graph)
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

    #[tokio::test]
    async fn open_workspace_persists_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();

        let workspaces = runtime.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].workspace_id, workspace.workspace_id);
        assert_eq!(workspaces[0].path, "/tmp/project");
    }

    #[tokio::test]
    async fn start_session_persists_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_id);
        assert_eq!(sessions[0].title, "Session using fake");
    }

    #[tokio::test]
    async fn rename_session_updates_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .rename_session(&session_id, "My Custom Title".into())
            .await
            .unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(sessions[0].title, "My Custom Title");
    }

    #[tokio::test]
    async fn soft_delete_hides_session() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime.soft_delete_session(&session_id).await.unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert!(sessions.is_empty());
    }
}
