//! Integration tests for the memory protocol in LocalRuntime.
//!
//! Tests cover: scope-based auto-accept/reject, marker stripping from display
//! output, and injection of stored memories into subsequent requests.

use agent_core::{AppFacade, EventPayload, SendMessageRequest, StartSessionRequest};
use agent_memory::{MemoryEntry, MemoryScope, MemoryStore, SqliteMemoryStore};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use std::sync::Arc;

/// Helper: create an in-memory SQLite pool for the memory store.
async fn memory_pool() -> sqlx::sqlite::SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

/// Helper: create a runtime with an in-memory event store, fake model, and
/// an in-memory SqliteMemoryStore wired in.
async fn runtime_with_memory(
    model_responses: Vec<String>,
    permission_mode: PermissionMode,
) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(model_responses);
    let mem_store = SqliteMemoryStore::new(memory_pool().await).await.unwrap();
    let runtime = LocalRuntime::new(store, model)
        .with_permission_mode(permission_mode)
        .with_memory_store(Arc::new(mem_store));
    runtime
}

/// Helper: open a workspace and start a session, returning (workspace_id, session_id).
async fn start_session(
    runtime: &LocalRuntime<SqliteEventStore, FakeModelClient>,
) -> (agent_core::WorkspaceId, agent_core::SessionId) {
    let workspace = runtime
        .open_workspace("/tmp/test-memory-protocol".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();
    (workspace.workspace_id, session_id)
}

#[tokio::test]
async fn session_scope_memory_auto_accepted() {
    let runtime = runtime_with_memory(
        vec!["<memory scope=\"session\">User likes dark mode</memory> I'll remember that.".into()],
        PermissionMode::Suggest,
    )
    .await;
    let (workspace_id, session_id) = start_session(&runtime).await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content: "I prefer dark mode".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();

    // Verify MemoryAccepted event with scope "session" and content "User likes dark mode"
    let accepted: Vec<_> = trace
        .iter()
        .filter(|t| {
            matches!(
                &t.event.payload,
                EventPayload::MemoryAccepted { scope, content, .. }
                if scope == "session" && content == "User likes dark mode"
            )
        })
        .collect();
    assert!(
        !accepted.is_empty(),
        "Expected MemoryAccepted with scope 'session' and content 'User likes dark mode', found: {:?}",
        trace.iter().map(|t| format!("{:?}", t.event.payload)).collect::<Vec<_>>()
    );

    // Verify assistant message content does NOT contain <memory tags
    let assistant_msgs: Vec<_> = trace
        .iter()
        .filter_map(|t| match &t.event.payload {
            EventPayload::AssistantMessageCompleted { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect();
    assert!(
        !assistant_msgs.is_empty(),
        "Expected at least one AssistantMessageCompleted event"
    );
    for content in &assistant_msgs {
        assert!(
            !content.contains("<memory"),
            "Assistant message should NOT contain <memory tags, got: {content}"
        );
    }

    // Verify the assistant message DOES contain the natural text
    assert!(
        assistant_msgs
            .iter()
            .any(|c| c.contains("I'll remember that.")),
        "Assistant message should contain 'I'll remember that.', got: {:?}",
        assistant_msgs
    );
}

#[tokio::test]
async fn user_scope_memory_requires_approval_in_suggest_mode() {
    let runtime = runtime_with_memory(
        vec!["<memory scope=\"user\" key=\"preferred-language\">Rust</memory> Noted!".into()],
        PermissionMode::Suggest,
    )
    .await;
    let (workspace_id, session_id) = start_session(&runtime).await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content: "I like Rust".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();

    // In Suggest mode, durable memories are auto-denied without a MemoryProposed event.
    // Verify NO MemoryAccepted event with scope "user"
    let accepted: Vec<_> = trace
        .iter()
        .filter(|t| {
            matches!(
                &t.event.payload,
                EventPayload::MemoryAccepted { scope, .. } if scope == "user"
            )
        })
        .collect();
    assert!(
        accepted.is_empty(),
        "Expected NO MemoryAccepted with scope 'user' in Suggest mode, but found {}",
        accepted.len()
    );

    // Verify MemoryRejected event exists with the auto-deny reason
    let rejected: Vec<_> = trace
        .iter()
        .filter(|t| {
            matches!(
                &t.event.payload,
                EventPayload::MemoryRejected { reason, .. }
                if reason == "Auto-denied in Suggest mode"
            )
        })
        .collect();
    assert!(
        !rejected.is_empty(),
        "Expected MemoryRejected with 'Auto-denied in Suggest mode', found: {:?}",
        trace
            .iter()
            .map(|t| format!("{:?}", t.event.payload))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn workspace_scope_memory_auto_accepted_in_autonomous_mode() {
    let runtime = runtime_with_memory(
        vec!["<memory scope=\"workspace\" key=\"build-cmd\">cargo nextest</memory> Got it!".into()],
        PermissionMode::Autonomous,
    )
    .await;
    let (workspace_id, session_id) = start_session(&runtime).await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content: "Use cargo nextest for testing".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();

    // Verify MemoryAccepted with scope "workspace", key "build-cmd", content "cargo nextest"
    let accepted: Vec<_> = trace
        .iter()
        .filter(|t| {
            matches!(
                &t.event.payload,
                EventPayload::MemoryAccepted { scope, key, content, .. }
                if scope == "workspace" && key.as_deref() == Some("build-cmd") && content == "cargo nextest"
            )
        })
        .collect();
    assert!(
        !accepted.is_empty(),
        "Expected MemoryAccepted with scope 'workspace', key 'build-cmd', content 'cargo nextest', found: {:?}",
        trace.iter().map(|t| format!("{:?}", t.event.payload)).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn memory_markers_stripped_from_display() {
    let runtime = runtime_with_memory(
        vec![
            "Here's my answer. <memory scope=\"session\">temp note</memory> End of response."
                .into(),
        ],
        PermissionMode::Suggest,
    )
    .await;
    let (workspace_id, session_id) = start_session(&runtime).await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content: "tell me something".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();

    // Find AssistantMessageCompleted content
    let assistant_content: Vec<_> = trace
        .iter()
        .filter_map(|t| match &t.event.payload {
            EventPayload::AssistantMessageCompleted { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect();

    assert!(
        !assistant_content.is_empty(),
        "Expected at least one AssistantMessageCompleted event"
    );

    for content in &assistant_content {
        assert!(
            !content.contains("<memory"),
            "Display content should NOT contain <memory tags, got: {content}"
        );
    }

    // The display content should still contain the non-memory parts
    let combined = assistant_content.join(" ");
    assert!(
        combined.contains("Here's my answer."),
        "Display content should contain 'Here's my answer.', got: {combined}"
    );
    assert!(
        combined.contains("End of response."),
        "Display content should contain 'End of response.', got: {combined}"
    );
}

#[tokio::test]
async fn stored_memories_injected_into_subsequent_request() {
    // Pre-store a memory in SqliteMemoryStore
    let mem_pool = memory_pool().await;
    let mem_store = SqliteMemoryStore::new(mem_pool).await.unwrap();

    // Store an accepted user-scoped memory
    let entry = MemoryEntry {
        id: format!("mem_{}", uuid::Uuid::new_v4().simple()),
        scope: MemoryScope::User,
        key: Some("theme".into()),
        content: "prefers dark mode".into(),
        accepted: true,
        session_id: None,
        workspace_id: None,
    };
    mem_store.store(entry).await.unwrap();

    // Verify the memory was stored
    let stored = mem_store.list_by_scope(MemoryScope::User).await.unwrap();
    assert_eq!(stored.len(), 1, "Pre-stored memory should be queryable");

    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Acknowledged your preference.".into()]);
    let runtime = LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_memory_store(Arc::new(mem_store));

    let (workspace_id, session_id) = start_session(&runtime).await;

    // Send a message — the memory should be injected into the system prompt
    // internally, allowing the response to be generated without error.
    let result = runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content: "What theme do I like?".into(),
        })
        .await;

    assert!(
        result.is_ok(),
        "send_message should complete successfully with memory injection, got error: {:?}",
        result.err()
    );

    // Verify the session produced a response
    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert!(
        !projection.messages.is_empty(),
        "Should have chat messages after sending a message with stored memory context"
    );
}
