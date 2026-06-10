use super::*;
use agent_memory::{MemoryEntry, MemoryScope, MemoryStore, SqliteMemoryStore};
use agent_store::SqliteEventStore;
use std::sync::Arc;

// ── Helpers ──────────────────────────────────────────────────────

fn make_entry(scope: MemoryScope, key: Option<&str>, content: &str, accepted: bool) -> MemoryEntry {
    MemoryEntry {
        id: format!("mem_test_{}", uuid::Uuid::new_v4().simple()),
        scope,
        key: key.map(String::from),
        content: content.to_string(),
        accepted,
        session_id: Some("sid_1".into()),
        workspace_id: Some("wid_1".into()),
        branch: None,
    }
}

async fn memory_store_with_entries(entries: Vec<MemoryEntry>) -> Arc<dyn MemoryStore> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let mem_store = Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    for entry in entries {
        mem_store.store(entry).await.unwrap();
    }
    mem_store as Arc<dyn MemoryStore>
}

// ── render_memory_section ────────────────────────────────────────

#[test]
fn render_empty_list_returns_none() {
    let result = render_memory_section(&[]);
    assert!(result.is_none());
}

#[test]
fn render_only_unaccepted_returns_none() {
    let entries = vec![
        make_entry(MemoryScope::User, Some("lang"), "Rust", false),
        make_entry(MemoryScope::Workspace, None, "use cargo", false),
    ];
    let result = render_memory_section(&entries);
    assert!(result.is_none());
}

#[test]
fn render_accepted_entry_returns_formatted_section() {
    let entries = vec![make_entry(MemoryScope::User, Some("lang"), "Rust", true)];
    let result = render_memory_section(&entries).unwrap();
    assert!(result.contains("## Relevant Memories"));
    assert!(result.contains("- [user] lang: Rust"));
}

#[test]
fn render_entry_with_key_contains_key() {
    let entries = vec![make_entry(
        MemoryScope::Workspace,
        Some("build-cmd"),
        "cargo build",
        true,
    )];
    let section = render_memory_section(&entries).unwrap();
    assert!(section.contains("build-cmd: cargo build"));
}

#[test]
fn render_entry_without_key_omits_colon() {
    let entries = vec![make_entry(MemoryScope::Session, None, "dark mode", true)];
    let section = render_memory_section(&entries).unwrap();
    assert!(section.contains("- [session] dark mode"));
    // Should NOT have a colon-separated key
    assert!(!section.contains("- [session] :"));
}

#[test]
fn render_multiple_scopes_correctly_tagged() {
    let entries = vec![
        make_entry(MemoryScope::User, Some("editor"), "vim", true),
        make_entry(MemoryScope::Workspace, None, "monorepo", true),
        make_entry(MemoryScope::Session, None, "debugging", true),
    ];
    let section = render_memory_section(&entries).unwrap();
    assert!(section.contains("[user]"));
    assert!(section.contains("[workspace]"));
    assert!(section.contains("[session]"));
}

// ── retrieve_relevant_memories ───────────────────────────────────

#[tokio::test]
async fn retrieve_memories_none_store_returns_empty() {
    let none_store: Option<Arc<dyn MemoryStore>> = None;
    let result = retrieve_relevant_memories(&none_store, "hello").await;
    assert!(result.is_empty());
}

#[tokio::test]
async fn retrieve_memories_with_store_returns_matches() {
    let entries = vec![make_entry(
        MemoryScope::User,
        Some("lang"),
        "prefers Rust",
        true,
    )];
    let mem_store = memory_store_with_entries(entries).await;
    let some_store = Some(mem_store);

    let result = retrieve_relevant_memories(&some_store, "what language").await;
    assert!(!result.is_empty());
    assert!(result.iter().any(|m| m.content.contains("Rust")));
}

// ── retrieve_memory_section ──────────────────────────────────────

#[tokio::test]
async fn retrieve_section_none_store_returns_none() {
    let none_store: Option<Arc<dyn MemoryStore>> = None;
    let result = retrieve_memory_section(&none_store, "test").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn retrieve_section_with_memory_returns_some() {
    let entries = vec![make_entry(
        MemoryScope::User,
        Some("tool"),
        "uses neovim",
        true,
    )];
    let mem_store = memory_store_with_entries(entries).await;
    let some_store = Some(mem_store);

    let result = retrieve_memory_section(&some_store, "editor").await;
    assert!(result.is_some());
    assert!(result.unwrap().contains("neovim"));
}

// ── store_memory_markers ─────────────────────────────────────────

#[tokio::test]
async fn store_markers_empty_text_no_panic() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, _rx) = tokio::sync::broadcast::channel(16);
    let mem_store: Option<Arc<dyn MemoryStore>> = Some(Arc::new(
        SqliteMemoryStore::new(store.pool().clone()).await.unwrap(),
    ));
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    store_memory_markers(
        &store,
        &event_tx,
        &mem_store,
        &workspace_id,
        &session_id,
        "",
    )
    .await;
    // No panic = pass
}

#[tokio::test]
async fn store_markers_none_memory_store_no_panic() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, _rx) = tokio::sync::broadcast::channel(16);
    let mem_store: Option<Arc<dyn MemoryStore>> = None;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    store_memory_markers(
        &store,
        &event_tx,
        &mem_store,
        &workspace_id,
        &session_id,
        r#"<memory scope="session">test</memory>"#,
    )
    .await;
    // No panic = pass
}

#[tokio::test]
async fn store_session_scope_marker_auto_accepted() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, mut rx) = tokio::sync::broadcast::channel(16);
    let sqlite_mem = Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    let mem_store: Option<Arc<dyn MemoryStore>> = Some(sqlite_mem.clone());
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let text = r#"<memory scope="session">User likes dark mode</memory>"#;
    store_memory_markers(
        &store,
        &event_tx,
        &mem_store,
        &workspace_id,
        &session_id,
        text,
    )
    .await;

    // Session-scoped memories should be auto-accepted (MemoryAccepted event)
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(event.payload, EventPayload::MemoryAccepted { ref scope, .. } if scope == "session"),
        "Expected MemoryAccepted with scope 'session', got: {:?}",
        event.payload
    );

    // Verify the entry is stored as accepted
    let all = sqlite_mem
        .query(agent_memory::MemoryQuery {
            scope: None,
            keywords: Vec::new(),
            limit: 10,
            session_id: None,
            workspace_id: None,
            branch: None,
        })
        .await
        .unwrap();
    assert!(all
        .iter()
        .any(|m| m.accepted && m.content == "User likes dark mode"));
}

#[tokio::test]
async fn store_user_scope_marker_produces_proposed_event() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, mut rx) = tokio::sync::broadcast::channel(16);
    let sqlite_mem = Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    let mem_store: Option<Arc<dyn MemoryStore>> = Some(sqlite_mem.clone());
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let text = r#"<memory scope="user" key="preferred-language">Rust</memory>"#;
    store_memory_markers(
        &store,
        &event_tx,
        &mem_store,
        &workspace_id,
        &session_id,
        text,
    )
    .await;

    // User-scoped memories should produce MemoryProposed (requires confirmation)
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(
            event.payload,
            EventPayload::MemoryProposed {
                ref scope,
                ref key,
                ref content,
                ..
            } if scope == "user" && key.as_deref() == Some("preferred-language") && content == "Rust"
        ),
        "Expected MemoryProposed with scope 'user', key 'preferred-language', content 'Rust', got: {:?}",
        event.payload
    );

    // Verify the entry is stored as NOT accepted (pending confirmation)
    let all = sqlite_mem
        .query_including_pending(agent_memory::MemoryQuery {
            scope: None,
            keywords: Vec::new(),
            limit: 10,
            session_id: None,
            workspace_id: None,
            branch: None,
        })
        .await
        .unwrap();
    assert!(all.iter().any(|m| !m.accepted && m.content == "Rust"));
}

#[tokio::test]
async fn store_marker_records_current_branch_when_provided() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let (event_tx, _rx) = tokio::sync::broadcast::channel(16);
    let sqlite_mem = Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    let mem_store: Option<Arc<dyn MemoryStore>> = Some(sqlite_mem.clone());
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    store_memory_markers_with_branch(
        &store,
        &event_tx,
        &mem_store,
        &workspace_id,
        &session_id,
        r#"<memory scope="session">Use the git-aware context path</memory>"#,
        Some("feat/git-context"),
    )
    .await;

    let all = sqlite_mem
        .query(agent_memory::MemoryQuery {
            scope: Some(MemoryScope::Session),
            keywords: Vec::new(),
            limit: 10,
            session_id: Some(session_id.to_string()),
            workspace_id: Some(workspace_id.to_string()),
            branch: Some("feat/git-context".into()),
        })
        .await
        .unwrap();

    assert_eq!(all.len(), 1);
    assert_eq!(all[0].branch.as_deref(), Some("feat/git-context"));
}
