use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_store::{EventStore, ProjectMetaRepository, SessionRow, SqliteEventStore};
use std::sync::Arc;

async fn new_store() -> SqliteEventStore {
    SqliteEventStore::in_memory().await.unwrap()
}

#[tokio::test]
async fn session_schema_uses_only_approval_and_sandbox_policy() {
    let store = new_store().await;
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('kairox_sessions')")
            .fetch_all(store.pool())
            .await
            .unwrap();

    assert!(!columns.iter().any(|column| column == "permission_mode"));
    assert!(columns.iter().any(|column| column == "approval_policy"));
    assert!(columns.iter().any(|column| column == "sandbox_policy"));
}

// --- Event round-trip test ---

#[tokio::test]
async fn append_and_load_session_events() {
    let store = new_store().await;
    let ws = WorkspaceId::new();
    let sid = SessionId::new();
    store
        .upsert_workspace(ws.as_str(), "/tmp/test")
        .await
        .unwrap();

    let e1 = DomainEvent::new(
        ws.clone(),
        sid.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::UserMessageAdded {
            message_id: "m1".into(),
            content: "hello".into(),
            display_content: None,
        },
    );
    store.append(&e1).await.unwrap();

    let e2 = DomainEvent::new(
        ws.clone(),
        sid.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::AssistantMessageCompleted {
            message_id: "m2".into(),
            content: "hi!".into(),
        },
    );
    store.append(&e2).await.unwrap();

    let loaded = store.load_session(&sid).await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert!(matches!(
        &loaded[0].payload,
        EventPayload::UserMessageAdded { content, .. } if content == "hello"
    ));
    assert!(matches!(
        &loaded[1].payload,
        EventPayload::AssistantMessageCompleted { content, .. } if content == "hi!"
    ));
}

// --- Workspace and session metadata CRUD ---

#[tokio::test]
async fn workspace_and_session_crud() {
    let store = new_store().await;
    let ws_id = "ws-crud";
    let path = "/home/user/project";

    // Create workspace
    store.upsert_workspace(ws_id, path).await.unwrap();
    let workspaces = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, ws_id);

    // Create session
    let sid = SessionId::new();
    let now = chrono::Utc::now().to_rfc3339();
    store
        .upsert_session(&SessionRow {
            session_id: sid.to_string(),
            workspace_id: ws_id.to_string(),
            title: "test session".into(),
            model_profile: "fake".into(),
            model_id: None,
            provider: None,
            approval_policy: None,
            sandbox_policy: None,
            deleted_at: None,
            created_at: now.clone(),
            updated_at: now,
        })
        .await
        .unwrap();

    let sessions = store.list_active_sessions(ws_id).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "test session");

    // Rename
    store
        .rename_session(&sid.to_string(), "renamed")
        .await
        .unwrap();
    let sessions = store.list_active_sessions(ws_id).await.unwrap();
    assert_eq!(sessions[0].title, "renamed");

    // Soft-delete
    store.soft_delete_session(&sid.to_string()).await.unwrap();
    let sessions = store.list_active_sessions(ws_id).await.unwrap();
    assert!(sessions.is_empty());

    // Cleanup: sleep briefly so the soft-deleted timestamp is
    // unambiguously in the past, avoiding a timing race on fast CI.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let cleaned = store
        .cleanup_expired_sessions(std::time::Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(cleaned, 1);
    let sessions = store.list_active_sessions(ws_id).await.unwrap();
    assert!(sessions.is_empty());
}

// --- Concurrent event writes ---

#[tokio::test]
async fn concurrent_event_writes() {
    let store = Arc::new(new_store().await);
    let ws = WorkspaceId::new();
    let sid = SessionId::new();
    store
        .upsert_workspace(ws.as_str(), "/tmp/test")
        .await
        .unwrap();

    let mut handles = vec![];
    for i in 0..10 {
        let store = store.clone();
        let ws = ws.clone();
        let sid = sid.clone();
        handles.push(tokio::spawn(async move {
            let e = DomainEvent::new(
                ws,
                sid,
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::UserMessageAdded {
                    message_id: format!("m{}", i),
                    content: format!("msg {}", i),
                    display_content: None,
                },
            );
            store.append(&e).await.unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let loaded = store.load_session(&sid).await.unwrap();
    assert_eq!(loaded.len(), 10);
}

// --- ProjectMetaRepository tests ---

#[tokio::test]
async fn project_meta_crud() {
    let store = new_store().await;
    let ws_id = "ws-project";
    store.upsert_workspace(ws_id, "/tmp/proj").await.unwrap();

    let repo = ProjectMetaRepository::new(store.pool().clone());

    // Create project
    let project1 = repo
        .create_project(ws_id, "project A", "/tmp/proj", 0)
        .await
        .unwrap();
    assert_eq!(project1.display_name, "project A");

    let project2 = repo
        .create_project(ws_id, "project B", "/tmp/proj", 1)
        .await
        .unwrap();
    assert_eq!(project2.display_name, "project B");

    // List active projects (should be in sort_order: A then B)
    let list = repo.list_active_projects(ws_id).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].display_name, "project A");
    assert_eq!(list[1].display_name, "project B");

    // Reorder: swap so B comes first
    repo.update_project_order(&[project2.project_id.clone(), project1.project_id.clone()])
        .await
        .unwrap();
    let list = repo.list_active_projects(ws_id).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].display_name, "project B");
    assert_eq!(list[1].display_name, "project A");

    // Remove (soft-delete) one project
    repo.remove_project(&project1.project_id).await.unwrap();

    // Active list should have only the remaining project
    let active = repo.list_active_projects(ws_id).await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].display_name, "project B");

    // Removed list should contain the deleted one
    let removed = repo.list_removed_projects(ws_id).await.unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].display_name, "project A");

    // Bind a session to the remaining project, then remove it to verify
    // that remove_project also archives session visibility.
    let sid = SessionId::new();
    let now = chrono::Utc::now().to_rfc3339();
    store
        .upsert_session(&SessionRow {
            session_id: sid.to_string(),
            workspace_id: ws_id.to_string(),
            title: "project session".into(),
            model_profile: "fake".into(),
            model_id: None,
            provider: None,
            approval_policy: None,
            sandbox_policy: None,
            deleted_at: None,
            created_at: now.clone(),
            updated_at: now,
        })
        .await
        .unwrap();

    let binding = repo
        .bind_session(&sid.to_string(), &project2.project_id, "/tmp/proj", None)
        .await
        .unwrap();
    assert_eq!(binding.session_id, sid.to_string());

    let vis = repo.get_session_visibility(&sid.to_string()).await.unwrap();
    assert_eq!(vis, Some("visible".to_string()));

    repo.remove_project(&project2.project_id).await.unwrap();
    let vis = repo.get_session_visibility(&sid.to_string()).await.unwrap();
    assert_eq!(vis, Some("archived".to_string()));
}
