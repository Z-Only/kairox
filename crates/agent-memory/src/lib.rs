pub mod compactor;
pub mod context;
pub mod extractor;
pub mod marker;
pub mod memory;
pub mod store;
pub mod workspace_rag;

pub use compactor::{
    render_transcript, Compactor, CompactorError, COMPACTOR_PROMPT, LLM_RETRY_ATTEMPTS,
};
pub use context::{
    ContextAssembler, ContextBudget, ContextBundle, ContextRequest, ContextSource, ImageEntry,
    ImagePruningStrategy,
};
pub use extractor::extract_keywords;
pub use marker::{extract_memory_markers, strip_memory_markers, MemoryMarker};
pub use memory::{durable_memory_requires_confirmation, MemoryDecision, MemoryEntry, MemoryScope};
pub use store::{MemoryQuery, MemoryStore, MemoryStoreError, SqliteMemoryStore};
pub use workspace_rag::{
    CompositeWorkspaceRetriever, EmbeddingBackend, EmbeddingError, HashedEmbeddingBackend,
    KnowledgeBaseDocument, SqliteFtsKnowledgeBase, SqliteFtsKnowledgeBaseConfig, WorkspaceDocument,
    WorkspaceDocumentSource, WorkspaceIndexOptions, WorkspaceIndexOutcome, WorkspaceIndexSummary,
    WorkspaceRagConfig, WorkspaceRagError, WorkspaceRagIndex, WorkspaceRetrieval,
    WorkspaceRetrievalQuery, WorkspaceRetriever,
};
