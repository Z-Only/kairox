use super::*;
use crate::SqliteEventStore;

async fn setup() -> ProjectMetaRepository {
    let store = SqliteEventStore::in_memory().await.unwrap();
    ProjectMetaRepository::new(store.pool().clone())
}

#[tokio::test]
async fn create_and_get_project() {
    let repo = setup().await;

    let project = repo
        .create_project("ws1", "My Project", "/tmp/project", 0)
        .await
        .unwrap();

    assert_eq!(project.workspace_id, "ws1");
    assert_eq!(project.display_name, "My Project");
    assert_eq!(project.root_path, "/tmp/project");
    assert_eq!(project.sort_order, 0);
    assert!(project.expanded);
    assert!(project.removed_at.is_none());

    let fetched = repo.get_project(&project.project_id).await.unwrap();
    assert_eq!(fetched.project_id, project.project_id);
    assert_eq!(fetched.display_name, "My Project");
}

#[tokio::test]
async fn list_active_projects() {
    let repo = setup().await;

    let p1 = repo
        .create_project("ws1", "Active", "/tmp/a", 0)
        .await
        .unwrap();
    let p2 = repo
        .create_project("ws1", "ToRemove", "/tmp/b", 1)
        .await
        .unwrap();

    repo.remove_project(&p2.project_id).await.unwrap();

    let active = repo.list_active_projects("ws1").await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].project_id, p1.project_id);
}

#[tokio::test]
async fn rename_project() {
    let repo = setup().await;

    let project = repo
        .create_project("ws1", "Old Name", "/tmp/r", 0)
        .await
        .unwrap();

    repo.rename_project(&project.project_id, "New Name")
        .await
        .unwrap();

    let fetched = repo.get_project(&project.project_id).await.unwrap();
    assert_eq!(fetched.display_name, "New Name");
}

#[tokio::test]
async fn remove_and_restore_project() {
    let repo = setup().await;

    let project = repo
        .create_project("ws1", "Removable", "/tmp/rm", 0)
        .await
        .unwrap();

    repo.remove_project(&project.project_id).await.unwrap();

    let removed = repo.list_removed_projects("ws1").await.unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].project_id, project.project_id);
    assert!(removed[0].removed_at.is_some());

    let active = repo.list_active_projects("ws1").await.unwrap();
    assert!(active.is_empty());

    let restored = repo.restore_project(&project.project_id).await.unwrap();
    assert!(restored.removed_at.is_none());

    let active_after = repo.list_active_projects("ws1").await.unwrap();
    assert_eq!(active_after.len(), 1);
    assert_eq!(active_after[0].project_id, project.project_id);
}

#[tokio::test]
async fn update_project_order() {
    let repo = setup().await;

    let p1 = repo
        .create_project("ws1", "First", "/tmp/1", 0)
        .await
        .unwrap();
    let p2 = repo
        .create_project("ws1", "Second", "/tmp/2", 1)
        .await
        .unwrap();
    let p3 = repo
        .create_project("ws1", "Third", "/tmp/3", 2)
        .await
        .unwrap();

    // Reverse the order: p3, p1, p2
    let new_order = vec![
        p3.project_id.clone(),
        p1.project_id.clone(),
        p2.project_id.clone(),
    ];
    repo.update_project_order(&new_order).await.unwrap();

    let active = repo.list_active_projects("ws1").await.unwrap();
    assert_eq!(active[0].project_id, p3.project_id);
    assert_eq!(active[1].project_id, p1.project_id);
    assert_eq!(active[2].project_id, p2.project_id);
}

#[tokio::test]
async fn toggle_expanded() {
    let repo = setup().await;

    let project = repo
        .create_project("ws1", "Expandable", "/tmp/e", 0)
        .await
        .unwrap();
    assert!(project.expanded);

    repo.update_project_expanded(&project.project_id, false)
        .await
        .unwrap();
    let fetched = repo.get_project(&project.project_id).await.unwrap();
    assert!(!fetched.expanded);

    repo.update_project_expanded(&project.project_id, true)
        .await
        .unwrap();
    let fetched = repo.get_project(&project.project_id).await.unwrap();
    assert!(fetched.expanded);
}

#[tokio::test]
async fn bind_and_unbind_session() {
    let repo = setup().await;

    let project = repo
        .create_project("ws1", "Bound", "/tmp/bind", 0)
        .await
        .unwrap();

    let binding = repo
        .bind_session("sess1", &project.project_id, "/tmp/bind", Some("main"))
        .await
        .unwrap();

    assert_eq!(binding.session_id, "sess1");
    assert_eq!(binding.project_id, project.project_id);
    assert_eq!(binding.worktree_path, "/tmp/bind");
    assert_eq!(binding.branch.as_deref(), Some("main"));

    let fetched = repo.get_session_binding("sess1").await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().session_id, "sess1");

    // Re-bind to a different project simulates unbind+rebind (upsert)
    let p2 = repo
        .create_project("ws1", "Other", "/tmp/other", 1)
        .await
        .unwrap();
    repo.bind_session("sess1", &p2.project_id, "/tmp/other", None)
        .await
        .unwrap();

    let updated = repo.get_session_binding("sess1").await.unwrap().unwrap();
    assert_eq!(updated.project_id, p2.project_id);
    assert_eq!(updated.branch, None);

    // Nonexistent session returns None
    let none = repo.get_session_binding("nonexistent").await.unwrap();
    assert!(none.is_none());
}

#[tokio::test]
async fn list_project_sessions() {
    let repo = setup().await;

    let project = repo
        .create_project("ws1", "WithSessions", "/tmp/ls", 0)
        .await
        .unwrap();

    repo.bind_session("sess_a", &project.project_id, "/tmp/ls", Some("feat-a"))
        .await
        .unwrap();
    repo.bind_session("sess_b", &project.project_id, "/tmp/ls", Some("feat-b"))
        .await
        .unwrap();
    repo.bind_session("sess_c", &project.project_id, "/tmp/ls", None)
        .await
        .unwrap();

    // Verify all bindings exist via individual lookups
    let b1 = repo.get_session_binding("sess_a").await.unwrap().unwrap();
    let b2 = repo.get_session_binding("sess_b").await.unwrap().unwrap();
    let b3 = repo.get_session_binding("sess_c").await.unwrap().unwrap();

    assert_eq!(b1.project_id, project.project_id);
    assert_eq!(b2.project_id, project.project_id);
    assert_eq!(b3.project_id, project.project_id);
    assert_eq!(b1.branch.as_deref(), Some("feat-a"));
    assert_eq!(b2.branch.as_deref(), Some("feat-b"));
    assert_eq!(b3.branch, None);

    // Archive one and verify it appears in archived list
    repo.set_session_visibility("sess_b", "archived")
        .await
        .unwrap();
    let archived = repo.list_archived_sessions("ws1").await.unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].session_id, "sess_b");
}

#[tokio::test]
async fn set_and_get_session_visibility() {
    let repo = setup().await;

    let project = repo
        .create_project("ws1", "Vis", "/tmp/vis", 0)
        .await
        .unwrap();

    repo.bind_session("sess_v", &project.project_id, "/tmp/vis", None)
        .await
        .unwrap();

    // bind_session sets initial visibility to visible
    let vis = repo.get_session_visibility("sess_v").await.unwrap();
    assert_eq!(vis.as_deref(), Some("visible"));

    repo.set_session_visibility("sess_v", "hidden")
        .await
        .unwrap();
    let vis = repo.get_session_visibility("sess_v").await.unwrap();
    assert_eq!(vis.as_deref(), Some("hidden"));

    repo.set_session_visibility("sess_v", "archived")
        .await
        .unwrap();
    let vis = repo.get_session_visibility("sess_v").await.unwrap();
    assert_eq!(vis.as_deref(), Some("archived"));
}

#[tokio::test]
async fn remove_project_archives_bound_sessions() {
    let repo = setup().await;

    let project = repo
        .create_project("ws1", "Cascade", "/tmp/cascade", 0)
        .await
        .unwrap();

    repo.bind_session("sess_c1", &project.project_id, "/tmp/cascade", None)
        .await
        .unwrap();
    repo.bind_session("sess_c2", &project.project_id, "/tmp/cascade", None)
        .await
        .unwrap();

    // Both should start as visible
    assert_eq!(
        repo.get_session_visibility("sess_c1")
            .await
            .unwrap()
            .as_deref(),
        Some("visible")
    );
    assert_eq!(
        repo.get_session_visibility("sess_c2")
            .await
            .unwrap()
            .as_deref(),
        Some("visible")
    );

    // Remove project cascades archive to bound visible sessions
    repo.remove_project(&project.project_id).await.unwrap();

    assert_eq!(
        repo.get_session_visibility("sess_c1")
            .await
            .unwrap()
            .as_deref(),
        Some("archived")
    );
    assert_eq!(
        repo.get_session_visibility("sess_c2")
            .await
            .unwrap()
            .as_deref(),
        Some("archived")
    );
}
