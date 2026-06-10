use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeBaseKind {
    #[serde(alias = "sqlite")]
    SqliteFts,
    Tantivy,
    #[serde(rename = "bedrock", alias = "bedrock_knowledge_base")]
    BedrockKnowledgeBase,
    Pinecone,
    Weaviate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeBaseConfig {
    pub kind: KnowledgeBaseKind,
    #[serde(default = "crate::default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub profile_aliases: Vec<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub knowledge_base_id: Option<String>,
    #[serde(default)]
    pub index_name: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub id_column: Option<String>,
    #[serde(default)]
    pub title_column: Option<String>,
    #[serde(default)]
    pub content_column: Option<String>,
    #[serde(default)]
    pub workspace_id_column: Option<String>,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub min_score: Option<f32>,
}

impl Default for KnowledgeBaseConfig {
    fn default() -> Self {
        Self {
            kind: KnowledgeBaseKind::SqliteFts,
            enabled: true,
            profile_aliases: Vec::new(),
            path: None,
            endpoint: None,
            api_key_env: None,
            region: None,
            knowledge_base_id: None,
            index_name: None,
            namespace: None,
            collection: None,
            table: None,
            id_column: None,
            title_column: None,
            content_column: None,
            workspace_id_column: None,
            max_results: None,
            min_score: None,
        }
    }
}
