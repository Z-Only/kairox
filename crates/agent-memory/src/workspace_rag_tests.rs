use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;

use super::{
    HashedEmbeddingBackend, WorkspaceDocument, WorkspaceDocumentSource, WorkspaceRagIndex,
    WorkspaceRetrievalQuery,
};

async fn test_index() -> WorkspaceRagIndex {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    WorkspaceRagIndex::new(pool, Arc::new(HashedEmbeddingBackend::default()))
        .await
        .unwrap()
}

#[tokio::test]
async fn indexes_workspace_documents_for_vector_retrieval() {
    let index = test_index().await;
    index
        .index_document(WorkspaceDocument::file(
            "ws-alpha",
            "docs/eval.md",
            "Evaluation reports include JSONL scenario summaries and pass rates.",
        ))
        .await
        .unwrap();
    index
        .index_document(WorkspaceDocument::file(
            "ws-alpha",
            "docs/plugins.md",
            "Plugin manifests declare skills, hooks, and MCP servers.",
        ))
        .await
        .unwrap();

    let hits = index
        .retrieve(WorkspaceRetrievalQuery {
            workspace_id: Some("ws-alpha".into()),
            query: "Where are JSONL evaluation reports described?".into(),
            limit: 2,
            min_score: 0.0,
            source: None,
        })
        .await
        .unwrap();

    assert_eq!(hits[0].path, "docs/eval.md");
    assert!(hits[0].score > hits[1].score);
    assert!(hits[0].content.contains("JSONL scenario summaries"));
}

#[tokio::test]
async fn reindexing_same_document_replaces_stale_chunks() {
    let index = test_index().await;
    index
        .index_document(WorkspaceDocument::documentation(
            "ws-alpha",
            "docs/runtime.md",
            "The legacy runtime stores only keyword memories.",
        ))
        .await
        .unwrap();
    let outcome = index
        .index_document(WorkspaceDocument::documentation(
            "ws-alpha",
            "docs/runtime.md",
            "The runtime now retrieves vector chunks during context assembly.",
        ))
        .await
        .unwrap();

    assert!(!outcome.skipped_unchanged);
    let hits = index
        .retrieve(WorkspaceRetrievalQuery {
            workspace_id: Some("ws-alpha".into()),
            query: "How does vector context assembly work?".into(),
            limit: 4,
            min_score: 0.0,
            source: Some(WorkspaceDocumentSource::Documentation),
        })
        .await
        .unwrap();

    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("vector chunks"));
    assert!(!hits[0].content.contains("keyword memories"));
}

#[tokio::test]
async fn unchanged_documents_are_not_reembedded() {
    let index = test_index().await;
    let first = index
        .index_document(WorkspaceDocument::past_conversation(
            "ws-alpha",
            "session-1",
            "User asked about workspace retrieval and incremental indexing.",
        ))
        .await
        .unwrap();
    let second = index
        .index_document(WorkspaceDocument::past_conversation(
            "ws-alpha",
            "session-1",
            "User asked about workspace retrieval and incremental indexing.",
        ))
        .await
        .unwrap();

    assert_eq!(first.chunks_indexed, 1);
    assert_eq!(second.chunks_indexed, 0);
    assert!(second.skipped_unchanged);
}
