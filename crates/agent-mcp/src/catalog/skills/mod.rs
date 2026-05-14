//! Skills catalog: trait + data types for browsing skill registries.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single skill entry returned by a [`SkillCatalogProvider`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillCatalogEntry {
    pub catalog_id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub source_url: String,
    pub install_count: Option<u64>,
    pub github_stars: Option<u64>,
    pub security_score: Option<u32>,
    pub rating: Option<f64>,
    pub package: String,
    pub package_url: Option<String>,
}

/// Query parameters for skill catalog searches.
#[derive(Debug, Clone, Default)]
pub struct SkillCatalogQuery {
    pub keyword: Option<String>,
    pub sources: Option<Vec<String>>,
    pub limit: Option<usize>,
}

/// Errors specific to skill catalog operations.
#[derive(Debug, thiserror::Error)]
pub enum SkillCatalogError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type SkillCatalogResult<T> = std::result::Result<T, SkillCatalogError>;

/// A source of [`SkillCatalogEntry`] data.
#[async_trait]
pub trait SkillCatalogProvider: Send + Sync {
    fn source_id(&self) -> &str;

    /// Search this source for skills matching the query keyword.
    async fn search(&self, query: &SkillCatalogQuery)
        -> SkillCatalogResult<Vec<SkillCatalogEntry>>;

    /// List entries from this source (no keyword filtering). Returns empty
    /// vec if the source does not support listing.
    async fn list(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let _ = query;
        Ok(Vec::new())
    }

    /// Force-refresh the source's cache.
    async fn refresh(&self) -> SkillCatalogResult<()> {
        Ok(())
    }
}

pub mod aggregate;
pub mod remote;
pub mod skillhub;
pub mod skills_sh;
