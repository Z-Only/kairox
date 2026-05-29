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
