use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::sqlite::SqlitePool;

use crate::extractor::extract_keywords;

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceRagError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("embedding backend failed: {0}")]
    Embedding(#[from] EmbeddingError),
    #[error("embedding backend returned {actual} vectors for {expected} inputs")]
    EmbeddingCount { expected: usize, actual: usize },
    #[error("invalid SQLite identifier: {0}")]
    InvalidIdentifier(String),
}

pub type Result<T> = std::result::Result<T, WorkspaceRagError>;

#[async_trait]
pub trait EmbeddingBackend: Send + Sync {
    fn model_id(&self) -> &str;
    async fn embed(&self, inputs: &[String]) -> std::result::Result<Vec<Vec<f32>>, EmbeddingError>;
}

#[derive(Debug, Clone)]
pub struct HashedEmbeddingBackend {
    dimensions: usize,
    model_id: String,
}

impl HashedEmbeddingBackend {
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions: dimensions.max(1),
            model_id: "local-hashed-bow-v1".to_string(),
        }
    }
}

impl Default for HashedEmbeddingBackend {
    fn default() -> Self {
        Self::new(256)
    }
}

#[async_trait]
impl EmbeddingBackend for HashedEmbeddingBackend {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    async fn embed(&self, inputs: &[String]) -> std::result::Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(inputs
            .iter()
            .map(|input| hashed_bag_of_words(input, self.dimensions))
            .collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkspaceDocumentSource {
    File,
    Documentation,
    PastConversation,
    KnowledgeBase,
}

impl WorkspaceDocumentSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Documentation => "documentation",
            Self::PastConversation => "past_conversation",
            Self::KnowledgeBase => "knowledge_base",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "documentation" => Self::Documentation,
            "past_conversation" => Self::PastConversation,
            "knowledge_base" => Self::KnowledgeBase,
            _ => Self::File,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDocument {
    pub workspace_id: String,
    pub path: String,
    pub source: WorkspaceDocumentSource,
    pub content: String,
}

impl WorkspaceDocument {
    pub fn new(
        workspace_id: impl Into<String>,
        path: impl Into<String>,
        source: WorkspaceDocumentSource,
        content: impl Into<String>,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            path: normalize_path(path.into()),
            source,
            content: content.into(),
        }
    }

    pub fn file(
        workspace_id: impl Into<String>,
        path: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::new(workspace_id, path, WorkspaceDocumentSource::File, content)
    }

    pub fn documentation(
        workspace_id: impl Into<String>,
        path: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::new(
            workspace_id,
            path,
            WorkspaceDocumentSource::Documentation,
            content,
        )
    }

    pub fn past_conversation(
        workspace_id: impl Into<String>,
        session_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        Self::new(
            workspace_id,
            format!("conversations/{session_id}"),
            WorkspaceDocumentSource::PastConversation,
            content,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceRetrieval {
    pub workspace_id: String,
    pub path: String,
    pub source: WorkspaceDocumentSource,
    pub chunk_index: usize,
    pub content: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceRetrievalQuery {
    pub workspace_id: Option<String>,
    pub query: String,
    pub limit: usize,
    pub min_score: f32,
    pub source: Option<WorkspaceDocumentSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRagConfig {
    pub max_chunk_chars: usize,
}

impl Default for WorkspaceRagConfig {
    fn default() -> Self {
        Self {
            max_chunk_chars: 2_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceIndexOptions {
    pub source: WorkspaceDocumentSource,
    pub max_file_bytes: u64,
    pub include_extensions: Vec<String>,
    pub ignored_directories: Vec<String>,
}

impl Default for WorkspaceIndexOptions {
    fn default() -> Self {
        Self {
            source: WorkspaceDocumentSource::File,
            max_file_bytes: 512 * 1024,
            include_extensions: [
                "md", "mdx", "txt", "rst", "adoc", "toml", "json", "yaml", "yml", "rs", "ts",
                "tsx", "js", "jsx", "vue", "py",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            ignored_directories: [
                ".git",
                ".worktrees",
                "target",
                "node_modules",
                "dist",
                "build",
                ".next",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceIndexOutcome {
    pub path: String,
    pub chunks_indexed: usize,
    pub skipped_unchanged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceIndexSummary {
    pub files_seen: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub chunks_indexed: usize,
}

#[async_trait]
pub trait WorkspaceRetriever: Send + Sync {
    async fn retrieve(&self, query: WorkspaceRetrievalQuery) -> Result<Vec<WorkspaceRetrieval>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeBaseDocument {
    pub id: String,
    pub workspace_id: Option<String>,
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteFtsKnowledgeBaseConfig {
    pub table: String,
    pub id_column: String,
    pub title_column: Option<String>,
    pub content_column: String,
    pub workspace_id_column: Option<String>,
}

impl Default for SqliteFtsKnowledgeBaseConfig {
    fn default() -> Self {
        Self {
            table: "knowledge_base_docs".into(),
            id_column: "doc_id".into(),
            title_column: Some("title".into()),
            content_column: "content".into(),
            workspace_id_column: Some("workspace_id".into()),
        }
    }
}

pub struct SqliteFtsKnowledgeBase {
    id: String,
    pool: SqlitePool,
    config: SqliteFtsKnowledgeBaseConfig,
}

impl SqliteFtsKnowledgeBase {
    pub async fn new(
        id: impl Into<String>,
        pool: SqlitePool,
        config: SqliteFtsKnowledgeBaseConfig,
    ) -> Result<Self> {
        validate_fts_config(&config)?;
        let table = &config.table;
        let mut columns = Vec::new();
        columns.push(format!("{} UNINDEXED", config.id_column));
        if let Some(column) = &config.workspace_id_column {
            columns.push(format!("{column} UNINDEXED"));
        }
        if let Some(column) = &config.title_column {
            columns.push(column.clone());
        }
        columns.push(config.content_column.clone());
        let sql = format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {table} USING fts5({})",
            columns.join(", ")
        );
        sqlx::query(&sql).execute(&pool).await?;

        Ok(Self {
            id: id.into(),
            pool,
            config,
        })
    }

    pub async fn upsert_document(&self, document: KnowledgeBaseDocument) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let table = &self.config.table;
        let delete_sql = format!(
            "DELETE FROM {table} WHERE {} = ?",
            self.config.id_column.as_str()
        );
        sqlx::query(&delete_sql)
            .bind(&document.id)
            .execute(&mut *tx)
            .await?;

        let mut columns = vec![self.config.id_column.as_str()];
        if let Some(column) = &self.config.workspace_id_column {
            columns.push(column.as_str());
        }
        if let Some(column) = &self.config.title_column {
            columns.push(column.as_str());
        }
        columns.push(self.config.content_column.as_str());

        let placeholders = std::iter::repeat_n("?", columns.len())
            .collect::<Vec<_>>()
            .join(", ");
        let insert_sql = format!(
            "INSERT INTO {table} ({}) VALUES ({placeholders})",
            columns.join(", ")
        );
        let mut query = sqlx::query(&insert_sql).bind(&document.id);
        if self.config.workspace_id_column.is_some() {
            query = query.bind(document.workspace_id.unwrap_or_default());
        }
        if self.config.title_column.is_some() {
            query = query.bind(document.title.unwrap_or_default());
        }
        query.bind(document.content).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn retrieve_internal(
        &self,
        query: WorkspaceRetrievalQuery,
    ) -> Result<Vec<WorkspaceRetrieval>> {
        if query.limit == 0 || query.query.trim().is_empty() {
            return Ok(Vec::new());
        }
        if query
            .source
            .is_some_and(|source| source != WorkspaceDocumentSource::KnowledgeBase)
        {
            return Ok(Vec::new());
        }

        let Some(fts_query) = to_fts_query(&query.query) else {
            return Ok(Vec::new());
        };

        let table = &self.config.table;
        let workspace_select = self.config.workspace_id_column.as_deref().unwrap_or("''");
        let title_select = self.config.title_column.as_deref().unwrap_or("''");
        let mut sql = format!(
            "SELECT {}, {workspace_select}, {title_select}, {}, bm25({table}) AS rank FROM {table} WHERE {table} MATCH ?",
            self.config.id_column, self.config.content_column
        );
        if let (Some(column), Some(_)) = (&self.config.workspace_id_column, &query.workspace_id) {
            sql.push_str(&format!(
                " AND ({column} = ? OR {column} = '' OR {column} IS NULL)"
            ));
        }
        sql.push_str(" ORDER BY rank ASC LIMIT ?");

        let mut select =
            sqlx::query_as::<_, (String, String, String, String, f64)>(&sql).bind(fts_query);
        if self.config.workspace_id_column.is_some() {
            if let Some(workspace_id) = &query.workspace_id {
                select = select.bind(workspace_id);
            }
        }
        select = select.bind(query.limit as i64);

        let rows = select.fetch_all(&self.pool).await?;
        let mut hits = Vec::new();
        for (doc_id, row_workspace_id, title, content, rank) in rows {
            let score = (1.0 / (1.0 + rank.abs() as f32)).clamp(0.0, 1.0);
            if score < query.min_score {
                continue;
            }
            let content = if title.trim().is_empty() {
                content
            } else {
                format!("{title}\n{content}")
            };
            hits.push(WorkspaceRetrieval {
                workspace_id: if row_workspace_id.trim().is_empty() {
                    query.workspace_id.clone().unwrap_or_default()
                } else {
                    row_workspace_id
                },
                path: format!("kb://{}/{}", self.id, normalize_path(doc_id)),
                source: WorkspaceDocumentSource::KnowledgeBase,
                chunk_index: 0,
                content,
                score,
            });
        }

        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.chunk_index.cmp(&b.chunk_index))
        });
        hits.truncate(query.limit);
        Ok(hits)
    }
}

#[async_trait]
impl WorkspaceRetriever for SqliteFtsKnowledgeBase {
    async fn retrieve(&self, query: WorkspaceRetrievalQuery) -> Result<Vec<WorkspaceRetrieval>> {
        self.retrieve_internal(query).await
    }
}

pub struct CompositeWorkspaceRetriever {
    retrievers: Vec<Arc<dyn WorkspaceRetriever>>,
}

impl CompositeWorkspaceRetriever {
    pub fn new(retrievers: Vec<Arc<dyn WorkspaceRetriever>>) -> Self {
        Self { retrievers }
    }

    pub fn is_empty(&self) -> bool {
        self.retrievers.is_empty()
    }
}

#[async_trait]
impl WorkspaceRetriever for CompositeWorkspaceRetriever {
    async fn retrieve(&self, query: WorkspaceRetrievalQuery) -> Result<Vec<WorkspaceRetrieval>> {
        if query.limit == 0 || self.retrievers.is_empty() {
            return Ok(Vec::new());
        }

        let mut hits = Vec::new();
        for retriever in &self.retrievers {
            hits.extend(retriever.retrieve(query.clone()).await?);
        }
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.chunk_index.cmp(&b.chunk_index))
        });
        hits.truncate(query.limit);
        Ok(hits)
    }
}

pub struct WorkspaceRagIndex {
    pool: SqlitePool,
    embedder: Arc<dyn EmbeddingBackend>,
    config: WorkspaceRagConfig,
}

impl WorkspaceRagIndex {
    pub async fn new(pool: SqlitePool, embedder: Arc<dyn EmbeddingBackend>) -> Result<Self> {
        Self::with_config(pool, embedder, WorkspaceRagConfig::default()).await
    }

    pub async fn with_config(
        pool: SqlitePool,
        embedder: Arc<dyn EmbeddingBackend>,
        config: WorkspaceRagConfig,
    ) -> Result<Self> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workspace_rag_chunks (
                workspace_id    TEXT NOT NULL,
                path            TEXT NOT NULL,
                source          TEXT NOT NULL,
                chunk_index     INTEGER NOT NULL,
                content         TEXT NOT NULL,
                content_hash    TEXT NOT NULL,
                embedding       TEXT NOT NULL,
                embedding_model TEXT NOT NULL,
                updated_at      TEXT NOT NULL,
                PRIMARY KEY (workspace_id, path, chunk_index)
            )
            "#,
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_workspace_rag_workspace ON workspace_rag_chunks(workspace_id)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_workspace_rag_path ON workspace_rag_chunks(workspace_id, path)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_workspace_rag_source ON workspace_rag_chunks(source)",
        )
        .execute(&pool)
        .await?;

        Ok(Self {
            pool,
            embedder,
            config,
        })
    }

    pub async fn index_document(
        &self,
        document: WorkspaceDocument,
    ) -> Result<WorkspaceIndexOutcome> {
        let content_hash = stable_hash(&document.content);
        let existing_hash: Option<String> = sqlx::query_scalar(
            "SELECT content_hash FROM workspace_rag_chunks WHERE workspace_id = ? AND path = ? LIMIT 1",
        )
        .bind(&document.workspace_id)
        .bind(&document.path)
        .fetch_optional(&self.pool)
        .await?;

        if existing_hash.as_deref() == Some(content_hash.as_str()) {
            return Ok(WorkspaceIndexOutcome {
                path: document.path,
                chunks_indexed: 0,
                skipped_unchanged: true,
            });
        }

        let chunks = chunk_text(&document.content, self.config.max_chunk_chars);
        let embeddings = self.embedder.embed(&chunks).await?;
        if embeddings.len() != chunks.len() {
            return Err(WorkspaceRagError::EmbeddingCount {
                expected: chunks.len(),
                actual: embeddings.len(),
            });
        }

        sqlx::query("DELETE FROM workspace_rag_chunks WHERE workspace_id = ? AND path = ?")
            .bind(&document.workspace_id)
            .bind(&document.path)
            .execute(&self.pool)
            .await?;

        let now = chrono::Utc::now().to_rfc3339();
        for (idx, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
            let embedding_json = serde_json::to_string(embedding)?;
            sqlx::query(
                r#"
                INSERT INTO workspace_rag_chunks
                    (workspace_id, path, source, chunk_index, content, content_hash, embedding, embedding_model, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&document.workspace_id)
            .bind(&document.path)
            .bind(document.source.as_str())
            .bind(idx as i64)
            .bind(chunk)
            .bind(&content_hash)
            .bind(embedding_json)
            .bind(self.embedder.model_id())
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        Ok(WorkspaceIndexOutcome {
            path: document.path,
            chunks_indexed: chunks.len(),
            skipped_unchanged: false,
        })
    }

    pub async fn index_file(
        &self,
        workspace_id: impl Into<String>,
        root: impl AsRef<Path>,
        path: impl AsRef<Path>,
        source: WorkspaceDocumentSource,
    ) -> Result<WorkspaceIndexOutcome> {
        let root = root.as_ref();
        let path = path.as_ref();
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        let content = std::fs::read_to_string(&full_path)?;
        let relative = full_path
            .strip_prefix(root)
            .unwrap_or(&full_path)
            .to_string_lossy()
            .to_string();
        self.index_document(WorkspaceDocument::new(
            workspace_id,
            relative,
            source,
            content,
        ))
        .await
    }

    pub async fn index_workspace_files(
        &self,
        workspace_id: impl Into<String>,
        root: impl AsRef<Path>,
        options: WorkspaceIndexOptions,
    ) -> Result<WorkspaceIndexSummary> {
        let workspace_id = workspace_id.into();
        let root = root.as_ref();
        let paths = collect_indexable_files(root, &options)?;
        let mut summary = WorkspaceIndexSummary {
            files_seen: paths.len(),
            files_indexed: 0,
            files_skipped: 0,
            chunks_indexed: 0,
        };

        for path in paths {
            let outcome = self
                .index_file(&workspace_id, root, &path, options.source)
                .await?;
            if outcome.skipped_unchanged {
                summary.files_skipped += 1;
            } else {
                summary.files_indexed += 1;
                summary.chunks_indexed += outcome.chunks_indexed;
            }
        }

        Ok(summary)
    }

    pub async fn remove_document(
        &self,
        workspace_id: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Result<()> {
        sqlx::query("DELETE FROM workspace_rag_chunks WHERE workspace_id = ? AND path = ?")
            .bind(workspace_id.as_ref())
            .bind(normalize_path(path.as_ref()))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn retrieve(
        &self,
        query: WorkspaceRetrievalQuery,
    ) -> Result<Vec<WorkspaceRetrieval>> {
        self.retrieve_internal(query).await
    }

    async fn retrieve_internal(
        &self,
        query: WorkspaceRetrievalQuery,
    ) -> Result<Vec<WorkspaceRetrieval>> {
        if query.limit == 0 || query.query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let query_embedding = self
            .embedder
            .embed(std::slice::from_ref(&query.query))
            .await?
            .into_iter()
            .next()
            .unwrap_or_default();
        if query_embedding.is_empty() {
            return Ok(Vec::new());
        }

        let mut sql = String::from(
            "SELECT workspace_id, path, source, chunk_index, content, embedding FROM workspace_rag_chunks WHERE 1 = 1",
        );
        if query.workspace_id.is_some() {
            sql.push_str(" AND workspace_id = ?");
        }
        if query.source.is_some() {
            sql.push_str(" AND source = ?");
        }

        let mut select = sqlx::query_as::<_, (String, String, String, i64, String, String)>(&sql);
        if let Some(workspace_id) = &query.workspace_id {
            select = select.bind(workspace_id);
        }
        if let Some(source) = query.source {
            select = select.bind(source.as_str());
        }

        let rows = select.fetch_all(&self.pool).await?;
        let mut hits = Vec::new();
        for (workspace_id, path, source, chunk_index, content, embedding_json) in rows {
            let embedding: Vec<f32> = serde_json::from_str(&embedding_json)?;
            let score = cosine_similarity(&query_embedding, &embedding);
            if score >= query.min_score {
                hits.push(WorkspaceRetrieval {
                    workspace_id,
                    path,
                    source: WorkspaceDocumentSource::parse(&source),
                    chunk_index: chunk_index.max(0) as usize,
                    content,
                    score,
                });
            }
        }

        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.chunk_index.cmp(&b.chunk_index))
        });
        hits.truncate(query.limit);
        Ok(hits)
    }
}

#[async_trait]
impl WorkspaceRetriever for WorkspaceRagIndex {
    async fn retrieve(&self, query: WorkspaceRetrievalQuery) -> Result<Vec<WorkspaceRetrieval>> {
        self.retrieve_internal(query).await
    }
}

fn hashed_bag_of_words(input: &str, dimensions: usize) -> Vec<f32> {
    let mut vector = vec![0.0; dimensions.max(1)];
    let mut tokens = extract_keywords(input);
    if tokens.is_empty() {
        tokens = input
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|token| !token.is_empty())
            .map(|token| token.to_lowercase())
            .collect();
    }

    for token in tokens {
        let mut hasher = DefaultHasher::new();
        token.hash(&mut hasher);
        let idx = (hasher.finish() as usize) % vector.len();
        vector[idx] += 1.0;
    }

    normalize(&mut vector);
    vector
}

fn validate_fts_config(config: &SqliteFtsKnowledgeBaseConfig) -> Result<()> {
    validate_identifier(&config.table)?;
    validate_identifier(&config.id_column)?;
    validate_identifier(&config.content_column)?;
    if let Some(column) = &config.title_column {
        validate_identifier(column)?;
    }
    if let Some(column) = &config.workspace_id_column {
        validate_identifier(column)?;
    }
    Ok(())
}

fn validate_identifier(identifier: &str) -> Result<()> {
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return Err(WorkspaceRagError::InvalidIdentifier(identifier.into()));
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(WorkspaceRagError::InvalidIdentifier(identifier.into()));
    }
    if chars.any(|ch| !(ch == '_' || ch.is_ascii_alphanumeric())) {
        return Err(WorkspaceRagError::InvalidIdentifier(identifier.into()));
    }
    Ok(())
}

fn to_fts_query(input: &str) -> Option<String> {
    let mut tokens = extract_keywords(input);
    if tokens.is_empty() {
        tokens = input
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .filter(|token| !token.is_empty())
            .map(|token| token.to_ascii_lowercase())
            .collect();
    }
    let terms: Vec<String> = tokens
        .into_iter()
        .map(|token| {
            token
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{token}\""))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" OR "))
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}

fn cosine_similarity(lhs: &[f32], rhs: &[f32]) -> f32 {
    lhs.iter()
        .zip(rhs.iter())
        .map(|(a, b)| a * b)
        .sum::<f32>()
        .clamp(-1.0, 1.0)
}

fn chunk_text(content: &str, max_chunk_chars: usize) -> Vec<String> {
    let max_chunk_chars = max_chunk_chars.max(1);
    let normalized = content.replace("\r\n", "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for paragraph in trimmed.split("\n\n") {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            continue;
        }

        if paragraph.chars().count() > max_chunk_chars {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            chunks.extend(split_long_text(paragraph, max_chunk_chars));
            continue;
        }

        let separator = if current.is_empty() { 0 } else { 2 };
        if current.chars().count() + separator + paragraph.chars().count() <= max_chunk_chars {
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(paragraph);
        } else {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            current.push_str(paragraph);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn split_long_text(text: &str, max_chunk_chars: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(max_chunk_chars)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

fn stable_hash(content: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn normalize_path(path: impl AsRef<str>) -> String {
    path.as_ref().replace('\\', "/")
}

fn collect_indexable_files(root: &Path, options: &WorkspaceIndexOptions) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_indexable_files_inner(root, root, options, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_indexable_files_inner(
    root: &Path,
    dir: &Path,
    options: &WorkspaceIndexOptions,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().to_string();
        if file_type.is_dir() {
            if options
                .ignored_directories
                .iter()
                .any(|ignored| ignored == &name)
            {
                continue;
            }
            collect_indexable_files_inner(root, &path, options, out)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }
        if entry.metadata()?.len() > options.max_file_bytes {
            continue;
        }
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if !options
            .include_extensions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(ext))
        {
            continue;
        }
        out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
    }
    Ok(())
}

#[cfg(test)]
#[path = "workspace_rag_tests.rs"]
mod tests;
