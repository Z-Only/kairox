//! Shared fixtures and fakes for the full-stack integration tests.

use agent_memory::SqliteMemoryStore;
use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{PermissionMode, Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A model that returns a tool call on the first request, then text on the second.
#[derive(Debug, Clone)]
pub(crate) struct ToolThenTextModel {
    call_count: Arc<AtomicUsize>,
    text_response: String,
}

impl ToolThenTextModel {
    pub(crate) fn new(text_response: &str) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            text_response: text_response.to_string(),
        }
    }
}

#[async_trait]
impl ModelClient for ToolThenTextModel {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let events: Vec<agent_models::Result<ModelEvent>> = if count == 0 {
            vec![
                Ok(ModelEvent::TokenDelta("Reading file...".into())),
                Ok(ModelEvent::ToolCallRequested {
                    tool_call_id: "call_tool_1".into(),
                    tool_id: "echo".into(),
                    arguments: serde_json::json!({"text": "test"}),
                }),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        } else {
            vec![
                Ok(ModelEvent::TokenDelta(self.text_response.clone())),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

/// A simple echo tool for testing tool execution.
pub(crate) struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "echo".into(),
            description: "Echoes input as output".into(),
            required_capability: "echo".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> agent_tools::ToolRisk {
        agent_tools::ToolRisk::read("echo")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        Ok(ToolOutput {
            text: format!("ECHO: {}", invocation.arguments),
            truncated: false,
        })
    }
}

/// Create an in-memory runtime with FakeModelClient.
pub(crate) async fn make_simple_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Simple response".into()]);
    LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest)
}

/// Create an in-memory runtime with tool-calling model and a registered echo tool.
pub(crate) async fn make_tool_runtime() -> LocalRuntime<SqliteEventStore, ToolThenTextModel> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = ToolThenTextModel::new("Tool was executed successfully");
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Agent);
    let registry = runtime.tool_registry();
    registry.lock().await.register(Box::new(EchoTool));
    runtime
}

/// Create an in-memory runtime with memory store.
pub(crate) async fn make_runtime_with_memory() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let mem_store: Arc<dyn agent_memory::MemoryStore> =
        Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    let model = FakeModelClient::new(vec!["response".into()]);
    LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_memory_store(mem_store)
}
