//! Memory store integration (direct via memory_store()).

use agent_core::{AppFacade, StartSessionRequest};
use agent_memory::{MemoryEntry, MemoryQuery, MemoryScope};

use super::support::make_runtime_with_memory;

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

            permission_mode: None,
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
