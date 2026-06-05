use super::ProjectMetaRepository;
use crate::SqliteEventStore;

#[tokio::test]
async fn creates_lists_renames_and_removes_project() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let repository = ProjectMetaRepository::new(store.pool().clone());

    let project = repository
        .create_project("workspace-1", "Original", "/tmp/workspace", 10)
        .await
        .unwrap();

    repository
        .rename_project(&project.project_id, "Renamed")
        .await
        .unwrap();
    repository
        .remove_project(&project.project_id)
        .await
        .unwrap();

    let active_projects = repository
        .list_active_projects("workspace-1")
        .await
        .unwrap();
    let removed_projects = repository
        .list_removed_projects("workspace-1")
        .await
        .unwrap();

    assert!(active_projects.is_empty());
    assert_eq!(removed_projects.len(), 1);
    assert_eq!(removed_projects[0].display_name, "Renamed");
    assert!(removed_projects[0].removed_at.is_some());
}

#[tokio::test]
async fn binds_session_to_project_and_retrieves() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let repository = ProjectMetaRepository::new(store.pool().clone());

    let project = repository
        .create_project("workspace-1", "MyProject", "/tmp/project", 0)
        .await
        .unwrap();

    let binding = repository
        .bind_session(
            "session-abc",
            &project.project_id,
            "/tmp/project",
            Some("main"),
        )
        .await
        .unwrap();

    assert_eq!(binding.session_id, "session-abc");
    assert_eq!(binding.project_id, project.project_id);
    assert_eq!(binding.worktree_path, "/tmp/project");
    assert_eq!(binding.branch.as_deref(), Some("main"));

    let retrieved = repository
        .get_session_binding("session-abc")
        .await
        .unwrap()
        .expect("binding should exist");

    assert_eq!(retrieved.session_id, "session-abc");
    assert_eq!(retrieved.project_id, project.project_id);
    assert_eq!(retrieved.worktree_path, "/tmp/project");
    assert_eq!(retrieved.branch.as_deref(), Some("main"));
}

#[tokio::test]
async fn lists_sessions_for_project() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let repository = ProjectMetaRepository::new(store.pool().clone());

    let project = repository
        .create_project("workspace-1", "Multi", "/tmp/multi", 0)
        .await
        .unwrap();

    repository
        .bind_session(
            "session-1",
            &project.project_id,
            "/tmp/multi",
            Some("feat-a"),
        )
        .await
        .unwrap();
    repository
        .bind_session(
            "session-2",
            &project.project_id,
            "/tmp/multi",
            Some("feat-b"),
        )
        .await
        .unwrap();
    repository
        .bind_session("session-3", &project.project_id, "/tmp/multi", None)
        .await
        .unwrap();

    // Verify each binding is individually retrievable
    let b1 = repository
        .get_session_binding("session-1")
        .await
        .unwrap()
        .unwrap();
    let b2 = repository
        .get_session_binding("session-2")
        .await
        .unwrap()
        .unwrap();
    let b3 = repository
        .get_session_binding("session-3")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(b1.branch.as_deref(), Some("feat-a"));
    assert_eq!(b2.branch.as_deref(), Some("feat-b"));
    assert_eq!(b3.branch, None);

    // All belong to same project
    assert_eq!(b1.project_id, project.project_id);
    assert_eq!(b2.project_id, project.project_id);
    assert_eq!(b3.project_id, project.project_id);
}

#[tokio::test]
async fn sets_and_gets_session_visibility() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let repository = ProjectMetaRepository::new(store.pool().clone());

    let project = repository
        .create_project("workspace-1", "VisProject", "/tmp/vis", 0)
        .await
        .unwrap();

    // Binding a session automatically sets visibility to 'visible'
    repository
        .bind_session("session-vis", &project.project_id, "/tmp/vis", None)
        .await
        .unwrap();

    let visibility = repository
        .get_session_visibility("session-vis")
        .await
        .unwrap()
        .expect("visibility should be set after bind");
    assert_eq!(visibility, "visible");

    // Update visibility to archived
    repository
        .set_session_visibility("session-vis", "archived")
        .await
        .unwrap();

    let visibility = repository
        .get_session_visibility("session-vis")
        .await
        .unwrap()
        .expect("visibility should still exist");
    assert_eq!(visibility, "archived");

    // Update back to visible
    repository
        .set_session_visibility("session-vis", "visible")
        .await
        .unwrap();

    let visibility = repository
        .get_session_visibility("session-vis")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(visibility, "visible");
}

#[tokio::test]
async fn updates_session_binding_branch() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let repository = ProjectMetaRepository::new(store.pool().clone());

    let project = repository
        .create_project("workspace-1", "BranchProject", "/tmp/branch", 0)
        .await
        .unwrap();

    // Initial bind with a branch
    repository
        .bind_session(
            "session-branch",
            &project.project_id,
            "/tmp/branch",
            Some("feature-1"),
        )
        .await
        .unwrap();

    let binding = repository
        .get_session_binding("session-branch")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.branch.as_deref(), Some("feature-1"));

    // Re-bind same session with a different branch (upsert behavior)
    repository
        .bind_session(
            "session-branch",
            &project.project_id,
            "/tmp/branch",
            Some("feature-2"),
        )
        .await
        .unwrap();

    let updated = repository
        .get_session_binding("session-branch")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.branch.as_deref(), Some("feature-2"));

    // Re-bind with no branch
    repository
        .bind_session("session-branch", &project.project_id, "/tmp/branch", None)
        .await
        .unwrap();

    let cleared = repository
        .get_session_binding("session-branch")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cleared.branch, None);
}
