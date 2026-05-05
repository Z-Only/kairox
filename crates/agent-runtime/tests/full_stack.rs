//! Full-stack integration tests for the LocalRuntime facade.
//!
//! These tests exercise the FULL pipeline:
//!   LocalRuntime → FakeModelClient/ToolCallingModel → ToolRegistry → MemoryStore → EventStore
//!
//! They cover: workspace management, session lifecycle, messaging (text + tool calls),
//! permission decisions, memory protocol, task graph, cancellation, and persistence.

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_memory::{MemoryEntry, MemoryQuery, MemoryScope, SqliteMemoryStore};
use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{PermissionMode, Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// A model that returns a tool call on the first request, then text on the second.
#[derive(Debug, Clone)]
struct ToolThenTextModel {
    call_count: Arc<AtomicUsize>,
    text_response: String,
}

impl ToolThenTextModel {
    fn new(text_response: &str) -> Self {
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
struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "echo".into(),
            description: "Echoes input as output".into(),
            required_capability: "echo".into(),
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
async fn make_simple_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Simple response".into()]);
    LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest)
}

/// Create an in-memory runtime with tool-calling model and a registered echo tool.
async fn make_tool_runtime() -> LocalRuntime<SqliteEventStore, ToolThenTextModel> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = ToolThenTextModel::new("Tool was executed successfully");
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Agent);
    let registry = runtime.tool_registry();
    registry.lock().await.register(Box::new(EchoTool));
    runtime
}

/// Create an in-memory runtime with memory store.
async fn make_runtime_with_memory() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let mem_store: Arc<dyn agent_memory::MemoryStore> =
        Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    let model = FakeModelClient::new(vec!["response".into()]);
    LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_memory_store(mem_store)
}

// ===========================================================================
// FULL-STACK TESTS
// ===========================================================================

// ---------------------------------------------------------------------------
// 1. Workspace management
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_open_workspace_returns_info() {
    let runtime = make_simple_runtime().await;
    let ws = runtime.open_workspace("/tmp/test-ws".into()).await.unwrap();

    assert!(
        ws.workspace_id.as_str().starts_with("wrk_"),
        "Workspace ID should have wrk_ prefix, got: {}",
        ws.workspace_id.as_str()
    );
    assert_eq!(ws.path, "/tmp/test-ws");

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, ws.workspace_id);
}

#[tokio::test]
async fn full_stack_multiple_workspaces_are_independent() {
    let runtime = make_simple_runtime().await;

    let ws1 = runtime.open_workspace("/tmp/ws1".into()).await.unwrap();
    let ws2 = runtime.open_workspace("/tmp/ws2".into()).await.unwrap();

    assert_ne!(ws1.workspace_id, ws2.workspace_id);

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 2);
}

// ---------------------------------------------------------------------------
// 2. Session lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_start_session_under_workspace() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-session".into())
        .await
        .unwrap();

    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    assert!(
        sid.as_str().starts_with("ses_"),
        "Session ID should have ses_ prefix, got: {}",
        sid.as_str()
    );

    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, sid);
}

#[tokio::test]
async fn full_stack_rename_and_soft_delete_session() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-rename-delete".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Rename
    runtime
        .rename_session(&sid, "New Name".into())
        .await
        .unwrap();
    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert_eq!(sessions[0].title, "New Name");

    // Soft-delete
    runtime.soft_delete_session(&sid).await.unwrap();
    let after_delete = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert!(
        after_delete.is_empty(),
        "Soft-deleted session should not appear in active list"
    );
}

// ---------------------------------------------------------------------------
// 3. Messaging — text only (no tool calls)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_send_message_text_only() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-msg".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "hello world".into(),
        })
        .await
        .unwrap();

    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert_eq!(proj.messages.len(), 2);
    assert_eq!(proj.messages[0].role, ProjectedRole::User);
    assert_eq!(proj.messages[0].content, "hello world");
    assert_eq!(proj.messages[1].role, ProjectedRole::Assistant);
    assert_eq!(proj.messages[1].content, "Simple response");
}

// ---------------------------------------------------------------------------
// 4. Messaging — tool calling with Agent permission mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_tool_call_with_permission_grant() {
    let runtime = make_tool_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-tool".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "test".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "echo something".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(sid.clone()).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    // With PermissionMode::Agent, permission should be auto-granted
    assert!(
        event_types.contains(&"UserMessageAdded"),
        "Missing UserMessageAdded: {event_types:?}"
    );
    assert!(
        event_types.contains(&"ModelToolCallRequested"),
        "Missing ModelToolCallRequested: {event_types:?}"
    );
    assert!(
        event_types.contains(&"PermissionGranted"),
        "Missing PermissionGranted: {event_types:?}"
    );
    assert!(
        event_types.contains(&"ToolInvocationStarted"),
        "Missing ToolInvocationStarted: {event_types:?}"
    );
    assert!(
        event_types.contains(&"ToolInvocationCompleted"),
        "Missing ToolInvocationCompleted: {event_types:?}"
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted"),
        "Missing AssistantMessageCompleted: {event_types:?}"
    );

    // Final projection should include response after tool execution
    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert!(
        !proj.messages.is_empty(),
        "Should have messages after tool execution"
    );
}

// ---------------------------------------------------------------------------
// 5. Task graph
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_task_graph_populated() {
    let runtime = make_tool_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-task-graph".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "test".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "do something".into(),
        })
        .await
        .unwrap();

    let snapshot = runtime.get_task_graph(sid).await.unwrap();
    assert!(
        !snapshot.tasks.is_empty(),
        "Task graph should have at least one task after a message"
    );

    // The root task should be completed or running
    let root = &snapshot.tasks[0];
    assert!(
        matches!(
            root.state,
            agent_core::TaskState::Completed | agent_core::TaskState::Running
        ),
        "Root task should be completed or running, got: {:?}",
        root.state
    );
}

// ---------------------------------------------------------------------------
// 6. Memory store integration (direct via memory_store())
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_memory_store_queries() {
    let runtime = make_runtime_with_memory().await;
    let ws = runtime
        .open_workspace("/tmp/test-memory".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let mem_store = runtime
        .memory_store()
        .expect("memory store should be configured");

    let query = MemoryQuery {
        scope: None,
        keywords: vec![],
        limit: 50,
        session_id: Some(sid.to_string()),
        workspace_id: None,
    };

    let results = mem_store.query(query).await.unwrap();
    assert!(results.is_empty(), "No memories at start");

    // Store a user preference memory
    let entry = MemoryEntry::new(MemoryScope::User, "concise".into(), true);
    mem_store.store(entry).await.unwrap();

    let query2 = MemoryQuery {
        scope: Some(MemoryScope::User),
        keywords: vec![],
        limit: 50,
        session_id: None,
        workspace_id: None,
    };
    let results2 = mem_store.query(query2).await.unwrap();
    assert_eq!(results2.len(), 1);
    assert_eq!(results2[0].content, "concise");
}

// ---------------------------------------------------------------------------
// 7. Session cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_cancel_session() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-cancel".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime
        .cancel_session(ws.workspace_id, sid.clone())
        .await
        .unwrap();

    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert!(proj.cancelled, "Session should be cancelled");
}

// ---------------------------------------------------------------------------
// 8. Event streaming consistency
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_event_stream_matches_trace() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-stream-trace".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let mut stream = runtime.subscribe_session(sid.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "hello".into(),
        })
        .await
        .unwrap();

    // Collect events from stream
    let mut stream_events = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(e) => {
                        stream_events.push(e);
                        if stream_events.len() > 30 { break; }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    let trace = runtime.get_trace(sid).await.unwrap();

    // Both should have events
    assert!(!stream_events.is_empty(), "Stream should have events");
    assert!(!trace.is_empty(), "Trace should have events");

    let stream_types: Vec<&str> = stream_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();
    let trace_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        stream_types.contains(&"UserMessageAdded"),
        "Stream should contain UserMessageAdded"
    );
    assert!(
        trace_types.contains(&"UserMessageAdded"),
        "Trace should contain UserMessageAdded"
    );
}

// ---------------------------------------------------------------------------
// 9. Persistence across reconnection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_data_persists_across_reconnection() {
    let db_path = std::env::temp_dir().join(format!(
        "kairox-fullstack-persist-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let database_url = format!(
        "sqlite:///{}",
        db_path.display().to_string().trim_start_matches('/')
    );

    let original_ws_id = {
        let store = SqliteEventStore::connect(&database_url).await.unwrap();
        let model = FakeModelClient::new(vec!["persisted response".into()]);
        let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

        let ws = runtime
            .open_workspace("/tmp/persist-test".into())
            .await
            .unwrap();
        let sid = runtime
            .start_session(StartSessionRequest {
                workspace_id: ws.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(SendMessageRequest {
                workspace_id: ws.workspace_id.clone(),
                session_id: sid,
                content: "persist this".into(),
            })
            .await
            .unwrap();

        ws.workspace_id.to_string()
    };

    // Reconnect
    {
        let store2 = SqliteEventStore::connect(&database_url).await.unwrap();
        let model2 = FakeModelClient::new(vec!["new response".into()]);
        let runtime2 =
            LocalRuntime::new(store2, model2).with_permission_mode(PermissionMode::Suggest);

        let workspaces = runtime2.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 1, "Should recover workspace");
        assert_eq!(workspaces[0].workspace_id.as_str(), original_ws_id);

        let wid = agent_core::WorkspaceId::from_string(original_ws_id);
        let sessions = runtime2.list_sessions(&wid).await.unwrap();
        assert_eq!(sessions.len(), 1, "Should recover session");
    }

    let _ = std::fs::remove_file(&db_path);
}

// ---------------------------------------------------------------------------
// 10. Agent permission mode auto-approves reads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_stack_agent_mode_completes_without_prompt() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["response".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Agent);

    let ws = runtime
        .open_workspace("/tmp/test-agent-mode".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // With Agent mode, no permission prompt should block
    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: sid.clone(),
            content: "hello".into(),
        })
        .await
        .unwrap();

    let proj = runtime.get_session_projection(sid).await.unwrap();
    assert_eq!(
        proj.messages.len(),
        2,
        "Message should complete without permission prompt"
    );
}
