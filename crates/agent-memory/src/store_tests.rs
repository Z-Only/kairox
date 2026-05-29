use super::*;

async fn test_store() -> SqliteMemoryStore {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    SqliteMemoryStore::new(pool).await.unwrap()
}

#[tokio::test]
async fn store_and_query_round_trip() {
    let store = test_store().await;
    let entry = MemoryEntry::new(MemoryScope::Workspace, "Use cargo nextest".into(), true);
    store.store(entry.clone()).await.unwrap();

    let results = store
        .query(MemoryQuery {
            scope: None,
            keywords: vec!["nextest".into()],
            limit: 10,
            session_id: None,
            workspace_id: None,
        })
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "Use cargo nextest");
}

#[tokio::test]
async fn unaccepted_memories_excluded_from_query() {
    let store = test_store().await;
    let entry = MemoryEntry::new(MemoryScope::Workspace, "Hidden".into(), false);
    store.store(entry).await.unwrap();

    let results = store
        .query(MemoryQuery {
            scope: None,
            keywords: vec!["Hidden".into()],
            limit: 10,
            session_id: None,
            workspace_id: None,
        })
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn delete_removes_entry() {
    let store = test_store().await;
    let entry = MemoryEntry::new(MemoryScope::Session, "temp".into(), true);
    store.store(entry.clone()).await.unwrap();
    store.delete(&entry.id).await.unwrap();

    let results = store
        .query(MemoryQuery {
            scope: Some(MemoryScope::Session),
            keywords: vec![],
            limit: 10,
            session_id: None,
            workspace_id: None,
        })
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn list_by_scope_filters_correctly() {
    let store = test_store().await;
    store
        .store(MemoryEntry::new(MemoryScope::User, "u1".into(), true))
        .await
        .unwrap();
    store
        .store(MemoryEntry::new(MemoryScope::Workspace, "w1".into(), true))
        .await
        .unwrap();
    store
        .store(MemoryEntry::new(MemoryScope::Session, "s1".into(), true))
        .await
        .unwrap();

    let user = store.list_by_scope(MemoryScope::User).await.unwrap();
    assert_eq!(user.len(), 1);
    assert_eq!(user[0].content, "u1");
}

#[tokio::test]
async fn same_scope_and_key_deduplicates() {
    let store = test_store().await;
    let e1 = MemoryEntry {
        key: Some("runner".into()),
        ..MemoryEntry::new(MemoryScope::Workspace, "cargo test".into(), true)
    };
    let e2 = MemoryEntry {
        key: Some("runner".into()),
        ..MemoryEntry::new(MemoryScope::Workspace, "cargo nextest".into(), true)
    };
    store.store(e1).await.unwrap();
    store.store(e2).await.unwrap();

    let results = store.list_by_scope(MemoryScope::Workspace).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "cargo nextest");
}

#[tokio::test]
async fn count_filters_by_scope() {
    let store = test_store().await;
    store
        .store(MemoryEntry::new(MemoryScope::User, "u1".into(), true))
        .await
        .unwrap();
    store
        .store(MemoryEntry::new(MemoryScope::Workspace, "w1".into(), true))
        .await
        .unwrap();

    assert_eq!(store.count(None).await.unwrap(), 2);
    assert_eq!(store.count(Some(MemoryScope::User)).await.unwrap(), 1);
}
