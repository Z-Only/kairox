use super::super::*;

#[tokio::test]
async fn upsert_and_list_workspaces() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store
        .upsert_workspace("wrk_1", "/tmp/project-a")
        .await
        .unwrap();
    store
        .upsert_workspace("wrk_2", "/tmp/project-b")
        .await
        .unwrap();

    let workspaces = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 2);
    assert_eq!(workspaces[0].workspace_id, "wrk_1");
    assert_eq!(workspaces[0].path, "/tmp/project-a");
}

#[tokio::test]
async fn upsert_workspace_is_idempotent() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/old").await.unwrap();
    store.upsert_workspace("wrk_1", "/tmp/new").await.unwrap();

    let workspaces = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].path, "/tmp/new");
}
