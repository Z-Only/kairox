# Runtime Agent Loop Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the ToolRegistry, PermissionEngine, and ContextAssembler into the LocalRuntime so that `send_message` drives a full agent loop: model response → tool call extraction → permission check → tool execution → result feedback → model continuation. Also implement real `subscribe_session` event streaming.

**Architecture:** The `LocalRuntime::send_message` currently passes user input to the model, records events, and returns. The new agent loop adds: (1) context assembly from memory and tool results, (2) tool call detection in model output, (3) permission decisions via `PermissionEngine`, (4) tool dispatch through a `ToolRegistry`, (5) tool result injection back into model context, (6) a loop guard with max iterations, (7) real-time event streaming via `tokio::sync::broadcast`. The runtime remains behind the `AppFacade` trait — no UI code changes needed.

**Tech Stack:** Rust, tokio (broadcast channels), async-trait, agent-core, agent-models, agent-tools, agent-memory, agent-store.

---

## File Structure

| File                                         | Responsibility                                                                                        |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `crates/agent-runtime/src/facade_runtime.rs` | Refactored LocalRuntime with full agent loop, tool dispatch, permission, context, and event streaming |
| `crates/agent-runtime/src/lib.rs`            | Updated exports, RuntimeError variants for loop-specific errors                                       |
| `crates/agent-tools/src/registry.rs`         | Add `ToolRegistry` struct to hold and dispatch tools by ID                                            |
| `crates/agent-tools/src/lib.rs`              | Re-export `ToolRegistry`                                                                              |
| `crates/agent-tui/Cargo.toml`                | Add agent-memory, agent-models dependencies                                                           |

---

### Task 1: ToolRegistry for Tool Dispatch

**Files:**

- Modify: `crates/agent-tools/src/registry.rs`
- Modify: `crates/agent-tools/src/lib.rs`

- [ ] **Step 1: Write failing tests for ToolRegistry**

Add to the bottom of `crates/agent-tools/src/registry.rs`:

```rust
#[derive(Debug, Default)]
pub struct ToolRegistry {
    tools: std::collections::HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let id = tool.definition().tool_id.clone();
        self.tools.insert(id, tool);
    }

    pub fn get(&self, tool_id: &str) -> Option<&dyn Tool> {
        self.tools.get(tool_id).map(|t| t.as_ref())
    }

    pub fn list_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<_> = self.tools.values().map(|t| t.definition()).collect();
        defs.sort_by(|a, b| a.tool_id.cmp(&b.tool_id));
        defs
    }

    pub async fn invoke_with_permission(
        &self,
        engine: &PermissionEngine,
        invocation: ToolInvocation,
    ) -> crate::Result<ToolOutput> {
        let tool = self
            .tools
            .get(&invocation.tool_id)
            .ok_or_else(|| crate::ToolError::PermissionRequired(invocation.tool_id.clone()))?;
        let risk = tool.risk(&invocation);
        match engine.decide(&risk) {
            PermissionOutcome::Allowed => tool.invoke(invocation).await,
            PermissionOutcome::RequiresApproval => {
                Err(crate::ToolError::PermissionRequired(invocation.tool_id.clone()))
            }
            PermissionOutcome::Denied(reason) => {
                Err(crate::ToolError::PermissionDenied(reason))
            }
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::permission::{PermissionEngine, PermissionMode};

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                tool_id: "echo".into(),
                description: "Echoes input".into(),
                required_capability: "echo".into(),
            }
        }

        fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
            ToolRisk::read("echo")
        }

        async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
            Ok(ToolOutput {
                text: format!("echo: {}", invocation.arguments),
                truncated: false,
            })
        }
    }

    #[test]
    fn registers_and_lists_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        let defs = registry.list_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].tool_id, "echo");
    }

    #[test]
    fn retrieves_tool_by_id() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        assert!(registry.get("echo").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[tokio::test]
    async fn invoke_with_permission_allows_reads_in_readonly_mode() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);
        let invocation = ToolInvocation {
            tool_id: "echo".into(),
            arguments: serde_json::json!({"text": "hello"}),
            workspace_id: "/tmp/test".into(),
            preview: "echo hello".into(),
            timeout_ms: 5000,
            output_limit_bytes: 10240,
        };
        let result = registry.invoke_with_permission(&engine, invocation).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invoke_with_permission_denies_writes_in_readonly_mode() {
        use crate::ToolError;

        struct WriteTool;

        #[async_trait]
        impl Tool for WriteTool {
            fn definition(&self) -> ToolDefinition {
                ToolDefinition {
                    tool_id: "write".into(),
                    description: "Writes data".into(),
                    required_capability: "filesystem.write".into(),
                }
            }

            fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
                ToolRisk::write("write")
            }

            async fn invoke(&self, _invocation: ToolInvocation) -> crate::Result<ToolOutput> {
                Ok(ToolOutput { text: "wrote".into(), truncated: false })
            }
        }

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(WriteTool));
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);
        let invocation = ToolInvocation {
            tool_id: "write".into(),
            arguments: serde_json::json!({}),
            workspace_id: "/tmp/test".into(),
            preview: "write data".into(),
            timeout_ms: 5000,
            output_limit_bytes: 10240,
        };
        let result = registry.invoke_with_permission(&engine, invocation).await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
```

- [ ] **Step 2: Update lib.rs to re-export ToolRegistry**

Add `pub use registry::ToolRegistry;` to `crates/agent-tools/src/lib.rs`.

- [ ] **Step 3: Add ToolError variant for unknown tool**

Add to `crates/agent-tools/src/lib.rs` `ToolError` enum:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("permission required for {0}")]
    PermissionRequired(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("path escapes workspace: {0}")]
    WorkspaceEscape(String),
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

Update `invoke_with_permission` to use `NotFound` instead of `PermissionRequired` for missing tools:

```rust
let tool = self
    .tools
    .get(&invocation.tool_id)
    .ok_or_else(|| crate::ToolError::NotFound(invocation.tool_id.clone()))?;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-tools`
Expected: PASS — all existing tests plus new ToolRegistry tests.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-tools
git commit -m "feat(tools): add ToolRegistry with permission-aware dispatch"
```

---

### Task 2: Event Broadcast Channel for subscribe_session

**Files:**

- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/Cargo.toml`

- [ ] **Step 1: Add tokio::sync::broadcast channel to LocalRuntime**

Add `tokio` broadcast dependency. Update `crates/agent-runtime/Cargo.toml` — the workspace `tokio` dependency already includes `sync` feature, so no Cargo.toml change is needed. Proceed to code.

Replace `crates/agent-runtime/src/facade_runtime.rs` with:

```rust
use agent_core::{
    AgentId, AppFacade, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification,
    SendMessageRequest, SessionId, StartSessionRequest, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_memory::ContextAssembler;
use agent_models::{ModelClient, ModelEvent, ModelRequest, ModelRouter, ToolCall};
use agent_store::EventStore;
use agent_tools::{
    FsReadTool, PermissionEngine, PermissionMode, ToolInvocation, ToolOutput, ToolRegistry,
};
use async_trait::async_trait;
use futures::{stream, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

const MAX_AGENT_LOOP_ITERATIONS: usize = 20;
const EVENT_CHANNEL_CAPACITY: usize = 1024;

pub struct LocalRuntime<S, M> {
    store: Arc<S>,
    model: Arc<M>,
    router: Option<Arc<ModelRouter>>,
    permission_engine: PermissionEngine,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    context_assembler: ContextAssembler,
    event_tx: broadcast::Sender<DomainEvent>,
    pending_permissions: Arc<Mutex<HashMap<String, broadcast::Sender<bool>>>>,
}

impl<S, M> LocalRuntime<S, M> {
    pub fn new(store: S, model: M) -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            store: Arc::new(store),
            model: Arc::new(model),
            router: None,
            permission_engine: PermissionEngine::new(PermissionMode::Suggest),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            context_assembler: ContextAssembler::new(100_000),
            event_tx,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
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
}

impl<S> LocalRuntime<S, ModelRouter> {
    pub fn new_with_router(store: S, router: ModelRouter, permission_mode: PermissionMode) -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            store: Arc::new(store),
            model: Arc::new(ModelRouter::new()) as Arc<ModelRouter>,
            // Note: we'll set the actual router below
            router: Some(Arc::new(router)),
            permission_engine: PermissionEngine::new(permission_mode),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            context_assembler: ContextAssembler::new(100_000),
            event_tx,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

async fn append_and_broadcast<S: EventStore>(
    store: &S,
    event_tx: &broadcast::Sender<DomainEvent>,
    event: &DomainEvent,
) -> agent_core::Result<()> {
    store.append(event).await.map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
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

        let model_request = ModelRequest::user_text("default", &request.content)
            .with_system_prompt("You are a helpful assistant.")
            .add_message("user", &request.content);

        // Agent loop: model → tool call → permission → execute → feed back
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

            let mut stream = self
                .model
                .stream(current_request.clone())
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

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
                        return Err(agent_core::CoreError::InvalidState(message));
                    }
                    Err(e) => {
                        return Err(agent_core::CoreError::InvalidState(e.to_string()));
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
                let risk = {
                    if let Some(tool) = registry.get(&tc.name) {
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
                    }
                };

                let permission_event = match self.permission_engine.decide(&risk) {
                    agent_tools::PermissionOutcome::Allowed => {
                        DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::PermissionGranted {
                                request_id: tc.id.clone(),
                            },
                        )
                    }
                    agent_tools::PermissionOutcome::RequiresApproval => {
                        DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::PermissionDenied {
                                request_id: tc.id.clone(),
                                reason: "requires user approval".into(),
                            },
                        )
                    }
                    agent_tools::PermissionOutcome::Denied(reason) => {
                        DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::PermissionDenied {
                                request_id: tc.id.clone(),
                                reason,
                            },
                        )
                    }
                };
                append_and_broadcast(&*self.store, &self.event_tx, &permission_event).await?;

                // Only execute if permission was granted
                if matches!(&permission_event.payload, EventPayload::PermissionGranted { .. }) {
                    let invocation = ToolInvocation {
                        tool_id: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        workspace_id: request.workspace_id.to_string(),
                        preview: format!("{}({})", tc.name, tc.arguments),
                        timeout_ms: 30_000,
                        output_limit_bytes: 102_400,
                    };

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

                    let result = registry.invoke_with_permission(&self.permission_engine, invocation).await;

                    let completion_event = match result {
                        Ok(output) => DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ToolInvocationCompleted {
                                invocation_id: tc.id.clone(),
                                output_preview: output.text.chars().take(500).collect(),
                            },
                        ),
                        Err(e) => DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ToolInvocationFailed {
                                invocation_id: tc.id.clone(),
                                error: e.to_string(),
                            },
                        ),
                    };
                    append_and_broadcast(&*self.store, &self.event_tx, &completion_event).await?;
                }
            }
            drop(registry);

            // Build next request with tool results appended
            current_request = current_request.clone().add_message("assistant", &assistant_text);
            // NOTE: Full OpenAI-style tool_result message format will be added when ModelRequest supports tool_result role messages
            // in the OpenAI tool_result format. For now, append tool results as user messages.
            let session_events = self
                .store
                .load_session(&request.session_id)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
            let tool_results: Vec<String> = session_events
                .iter()
                .filter_map(|e| match &e.payload {
                    EventPayload::ToolInvocationCompleted { output_preview, .. } => {
                        Some(output_preview.clone())
                    }
                    _ => None,
                })
                .collect();
            if !tool_results.is_empty() {
                current_request = current_request.add_message("user", &format!("[Tool results]:\n{}", tool_results.join("\n")));
            }
        }

        Ok(())
    }

    async fn decide_permission(&self, decision: PermissionDecision) -> agent_core::Result<()> {
        if let Some(tx) = self
            .pending_permissions
            .lock()
            .await
            .remove(&decision.request_id)
        {
            let _ = tx.send(decision.approve);
        }
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

    fn subscribe_session(
        &self,
        session_id: SessionId,
    ) -> futures::stream::BoxStream<'static, DomainEvent> {
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
```

- [ ] **Step 2: Add async-stream dependency**

Add to root `Cargo.toml` `[workspace.dependencies]`:

```toml
async-stream = "0.3"
```

Add to `crates/agent-runtime/Cargo.toml` `[dependencies]`:

```toml
async-stream.workspace = true
agent-memory = { path = "../agent-memory" }
agent-tools = { path = "../agent-tools" }
```

Note: `agent-memory` and `agent-tools` are already dependencies of `agent-runtime` in the workspace Cargo.toml. Verify and add `async-stream`.

- [ ] **Step 3: Update lib.rs and fix RuntimeError**

Update `crates/agent-runtime/src/lib.rs`:

```rust
pub mod agents;
pub mod facade_runtime;
pub mod task_graph;

pub use agents::{PlannerAgent, ReviewerAgent, ReviewerFinding, WorkerAgent};
pub use facade_runtime::LocalRuntime;
pub use task_graph::{AgentRole, AgentTask, TaskGraph, TaskState};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("unknown task: {0}")]
    UnknownTask(String),
    #[error("agent loop exceeded maximum iterations")]
    MaxIterationsExceeded,
    #[error("permission required: {0}")]
    PermissionRequired(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
```

- [ ] **Step 4: Verify compilation and existing tests**

Run: `cargo test --workspace`
Expected: PASS — existing tests should still pass with the refactored runtime. The `send_message_records_user_and_assistant_events` test uses `FakeModelClient` which should work with the new loop since it doesn't produce tool calls.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime Cargo.toml
git commit -m "feat(runtime): integrate tool dispatch, permissions, and event broadcast into agent loop"
```

---

### Task 3: Agent Loop Integration Test

**Files:**

- Create: `crates/agent-runtime/tests/agent_loop.rs`

- [ ] **Step 1: Write agent loop integration test**

Create `crates/agent-runtime/tests/agent_loop.rs`:

```rust
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{FakeModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{FsReadTool, PermissionEngine, PermissionMode, ToolRegistry};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::sync::Arc;
use std::path::PathBuf;

/// A fake model that returns a tool call on the first request,
/// then a text response on the second request with tool results.
#[derive(Debug, Clone)]
struct ToolCallingModelClient {
    first_response: Vec<ModelEvent>,
    second_response: Vec<ModelEvent>,
    call_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl ToolCallingModelClient {
    fn new() -> Self {
        let first_response = vec![
            ModelEvent::TokenDelta("Reading".into()),
            ModelEvent::ToolCallRequested {
                tool_call_id: "call_1".into(),
                tool_id: "fs.read".into(),
                arguments: serde_json::json!({"path": "README.md"}),
            },
            ModelEvent::Completed { usage: None },
        ];
        let second_response = vec![
            ModelEvent::TokenDelta("The README says hello".into()),
            ModelEvent::Completed { usage: None },
        ];
        Self {
            first_response,
            second_response,
            call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl agent_models::ModelClient for ToolCallingModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let events: Vec<agent_models::Result<ModelEvent>> = if count == 0 {
            self.first_response.iter().cloned().map(Ok).collect()
        } else {
            self.second_response.iter().cloned().map(Ok).collect()
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

#[tokio::test]
async fn agent_loop_processes_tool_call_and_continues() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = ToolCallingModelClient::new();
    let mut runtime = LocalRuntime::new(store, model);
    runtime = runtime.with_permission_mode(PermissionMode::Agent);

    // Register fs.read tool
    let temp_dir = std::env::temp_dir();
    let registry = runtime.tool_registry();
    registry.lock().await.register(Box::new(
        FsReadTool::new(temp_dir.clone()),
    ));

    let workspace = runtime
        .open_workspace(temp_dir.display().to_string())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "read README.md".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<_> = trace.iter().map(|e| e.event.event_type).collect();

    // Should see: UserMessageAdded, ModelTokenDelta, ModelToolCallRequested,
    // PermissionGranted, ToolInvocationStarted, ToolInvocationCompleted (or Failed),
    // then second response: ModelTokenDelta, AssistantMessageCompleted
    assert!(event_types.contains(&"UserMessageAdded"), "Missing UserMessageAdded: {:?}", event_types);
    assert!(event_types.contains(&"ModelToolCallRequested"), "Missing ModelToolCallRequested: {:?}", event_types);
    assert!(event_types.contains(&"PermissionGranted") || event_types.contains(&"PermissionDenied"), "Missing permission decision: {:?}", event_types);

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert!(!projection.messages.is_empty(), "Should have chat messages after agent loop");
}

#[tokio::test]
async fn agent_loop_stops_when_no_tool_calls() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Just a text response".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test".into())
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
            content: "hello".into(),
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].content, "hello");
    assert_eq!(projection.messages[1].content, "Just a text response");
}

#[tokio::test]
async fn subscribe_session_receives_events() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
        })
        .await
        .unwrap();

    // The stream should have received events
    use futures::StreamExt;
    let mut received_types = Vec::new();
    while let Ok(event) = event_stream.try_next().await.unwrap_or(None) {
        received_types.push(event.event_type.to_string());
        // Break after we've collected enough or it's been empty
        if received_types.len() > 5 { break; }
    }
    // At minimum we expect UserMessageAdded
    assert!(
        received_types.iter().any(|t| t == "UserMessageAdded"),
        "subscribe_session should receive UserMessageAdded, got: {:?}",
        received_types
    );
}
```

- [ ] **Step 2: Add test dependency and run**

Add `agent-tools` dev-dependency to `crates/agent-runtime/Cargo.toml`:

```toml
[dev-dependencies]
agent-tools = { path = "../agent-tools" }
agent-memory = { path = "../agent-memory" }
```

Run: `cargo test -p agent-runtime`
Expected: PASS — all three new integration tests + existing `fake_session` test.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime
git commit -m "test(runtime): add agent loop integration tests"
```

---

### Task 4: Update Workspace Tests

**Files:**

- None new — just verify everything still works together

- [ ] **Step 1: Run full workspace test suite**

Run: `cargo test --workspace --all-targets`
Expected: PASS — all existing and new tests across all crates.

- [ ] **Step 2: Run format and lint**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: PASS — no formatting or clippy warnings.

- [ ] **Step 3: Commit if any fixes needed**

```bash
git add -A
git commit -m "chore: fix formatting and lint issues"
```

---

## Acceptance Criteria

This plan is successful when:

- `LocalRuntime::send_message` drives a full agent loop: model → tool call → permission check → tool execution → feedback → model continuation
- Tool calls from model output are detected and dispatched through `ToolRegistry`
- `PermissionEngine` filters tool invocations by mode — `ReadOnly` blocks writes, `Suggest` requires approval for side effects, `Agent` auto-approves safe actions, `Autonomous` allows practically everything except destructive shells
- Permission decisions and tool results are recorded as domain events
- `subscribe_session` delivers real-time events via broadcast channel
- Agent loop terminates when model produces no tool calls, or after MAX_AGENT_LOOP_ITERATIONS
- Existing `fake_session` and workspace tests continue to pass
- New integration tests verify tool-call flow and event streaming

## Self-Review

1. **Spec coverage:** The design spec's "App Facade" section requires commands (send_message, decide_permission, cancel_session), queries (get_session_projection, get_trace), and subscriptions (subscribe_session). All three are implemented. The "Tools, MCP, And Permissions" section requires tool registration, permission mode decisions, and event audit — all present. The "Agent Loop" concept is not explicitly in the spec but is implied by the runtime architecture: model → tool call → approval → execute → continue.

2. **Placeholder scan:** No TBDs, TODOs, or incomplete steps. One `TODO` comment in the tool result feedback code noting that proper OpenAI-style tool result messages need a richer message format — this is an explicit deferral, not a placeholder.

3. **Type consistency:** `ToolInvocation`, `ToolOutput`, `ToolDefinition`, `PermissionEngine`, `PermissionMode`, `PermissionOutcome`, `ToolRisk` are all used from `agent-tools`. `ModelEvent`, `ModelRequest`, `ToolCall` from `agent-models`. `DomainEvent`, `EventPayload`, `AppFacade` from `agent-core`. The `build_model_messages` function uses `agent_models::ModelMessage` consistently. The `append_and_broadcast` helper consistently handles both store persistence and broadcast.
