use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;

use super::{
    CompositeWorkspaceRetriever, HashedEmbeddingBackend, KnowledgeBaseDocument,
    SqliteFtsKnowledgeBase, SqliteFtsKnowledgeBaseConfig, WorkspaceDocument,
    WorkspaceDocumentSource, WorkspaceRagIndex, WorkspaceRetrievalQuery, WorkspaceRetriever,
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

#[tokio::test]
async fn sqlite_fts_knowledge_base_retrieves_matching_documents() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let kb = SqliteFtsKnowledgeBase::new(
        "company-docs",
        pool,
        SqliteFtsKnowledgeBaseConfig {
            table: "company_docs".into(),
            id_column: "doc_id".into(),
            title_column: Some("title".into()),
            content_column: "body".into(),
            workspace_id_column: Some("workspace_id".into()),
        },
    )
    .await
    .unwrap();
    kb.upsert_document(KnowledgeBaseDocument {
        id: "payroll-runbook".into(),
        workspace_id: Some("ws-alpha".into()),
        title: Some("Payroll runbook".into()),
        content: "Payroll incidents are escalated through the finance support queue.".into(),
    })
    .await
    .unwrap();
    kb.upsert_document(KnowledgeBaseDocument {
        id: "sales-playbook".into(),
        workspace_id: Some("ws-beta".into()),
        title: Some("Sales playbook".into()),
        content: "Enterprise sales playbooks describe account planning.".into(),
    })
    .await
    .unwrap();

    let hits = kb
        .retrieve(WorkspaceRetrievalQuery {
            workspace_id: Some("ws-alpha".into()),
            query: "Where are payroll incidents escalated?".into(),
            limit: 4,
            min_score: 0.0,
            source: None,
        })
        .await
        .unwrap();

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "kb://company-docs/payroll-runbook");
    assert_eq!(hits[0].source, WorkspaceDocumentSource::KnowledgeBase);
    assert!(hits[0].content.contains("Payroll runbook"));
    assert!(hits[0].content.contains("finance support queue"));
}

#[tokio::test]
async fn sqlite_fts_knowledge_base_failed_upsert_preserves_existing_document() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let kb = SqliteFtsKnowledgeBase::new(
        "company-docs",
        pool.clone(),
        SqliteFtsKnowledgeBaseConfig {
            table: "company_docs".into(),
            id_column: "doc_id".into(),
            title_column: Some("title".into()),
            content_column: "body".into(),
            workspace_id_column: Some("workspace_id".into()),
        },
    )
    .await
    .unwrap();
    kb.upsert_document(KnowledgeBaseDocument {
        id: "benefits".into(),
        workspace_id: Some("ws-alpha".into()),
        title: Some("Benefits handbook".into()),
        content: "Benefits enrollment uses the quarterly vesting calendar.".into(),
    })
    .await
    .unwrap();

    let bad_kb = SqliteFtsKnowledgeBase::new(
        "company-docs",
        pool,
        SqliteFtsKnowledgeBaseConfig {
            table: "company_docs".into(),
            id_column: "doc_id".into(),
            title_column: Some("title".into()),
            content_column: "missing_body".into(),
            workspace_id_column: Some("workspace_id".into()),
        },
    )
    .await
    .unwrap();
    let error = bad_kb
        .upsert_document(KnowledgeBaseDocument {
            id: "benefits".into(),
            workspace_id: Some("ws-alpha".into()),
            title: Some("Replacement".into()),
            content: "Replacement text should never be committed.".into(),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("missing_body"));

    let hits = kb
        .retrieve(WorkspaceRetrievalQuery {
            workspace_id: Some("ws-alpha".into()),
            query: "quarterly vesting calendar".into(),
            limit: 4,
            min_score: 0.0,
            source: Some(WorkspaceDocumentSource::KnowledgeBase),
        })
        .await
        .unwrap();

    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("quarterly vesting calendar"));
    assert!(!hits[0].content.contains("Replacement text"));
}

#[tokio::test]
async fn composite_workspace_retriever_merges_and_sorts_hits() {
    let vector_index = Arc::new(test_index().await);
    vector_index
        .index_document(WorkspaceDocument::file(
            "ws-alpha",
            "docs/plugins.md",
            "Plugin manifests define skills and MCP servers.",
        ))
        .await
        .unwrap();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let kb = Arc::new(
        SqliteFtsKnowledgeBase::new(
            "support",
            pool,
            SqliteFtsKnowledgeBaseConfig {
                table: "support_docs".into(),
                ..SqliteFtsKnowledgeBaseConfig::default()
            },
        )
        .await
        .unwrap(),
    );
    kb.upsert_document(KnowledgeBaseDocument {
        id: "plugin-support".into(),
        workspace_id: Some("ws-alpha".into()),
        title: Some("Plugin support".into()),
        content: "Plugin support runbooks cover marketplace connector failures.".into(),
    })
    .await
    .unwrap();

    let retriever = CompositeWorkspaceRetriever::new(vec![
        vector_index as Arc<dyn WorkspaceRetriever>,
        kb as Arc<dyn WorkspaceRetriever>,
    ]);
    let hits = retriever
        .retrieve(WorkspaceRetrievalQuery {
            workspace_id: Some("ws-alpha".into()),
            query: "plugin connector support".into(),
            limit: 4,
            min_score: 0.0,
            source: None,
        })
        .await
        .unwrap();

    assert!(hits
        .iter()
        .any(|hit| hit.path == "kb://support/plugin-support"));
    assert!(hits.iter().any(|hit| hit.path == "docs/plugins.md"));
    assert!(hits.windows(2).all(|pair| pair[0].score >= pair[1].score));
}
