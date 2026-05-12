//! MCP catalog: trait + data types for browsing and installing MCP servers
//! from one or more sources (built-in JSON today; remote registry later).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single server entry returned by a [`CatalogProvider`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ServerEntry {
    pub id: String,
    pub source: String,
    pub display_name: String,
    pub summary: String,
    pub description: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    pub install: InstallSpec,
    pub requirements: Vec<RuntimeRequirement>,
    pub trust: TrustLevel,
    pub default_env: Vec<EnvVarSpec>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum InstallSpec {
    Stdio {
        command: String,
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
        #[serde(default)]
        cwd: Option<String>,
    },
    Sse {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RuntimeRequirement {
    pub kind: RuntimeKind,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub install_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Node,
    Python,
    Uvx,
    Docker,
    Bun,
    Deno,
    Other,
}

impl RuntimeKind {
    /// Stable lower-case identifier for this runtime kind. Suitable for
    /// surfacing to GUIs and logs without relying on `Debug` formatting
    /// (which is not part of the public API contract).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Python => "python",
            Self::Uvx => "uvx",
            Self::Docker => "docker",
            Self::Bun => "bun",
            Self::Deno => "deno",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EnvVarSpec {
    pub key: String,
    pub label: String,
    pub description: String,
    pub required: bool,
    pub secret: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Unverified,
    Community,
    Verified,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CatalogQuery {
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub trust_min: Option<TrustLevel>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstalledEntry {
    pub server_id: String,
    pub catalog_id: Option<String>,
    pub source: Option<String>,
    pub display_name: String,
    pub installed_at: String,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallRequest {
    pub catalog_id: String,
    pub source: String,
    #[serde(default)]
    pub server_id_override: Option<String>,
    #[serde(default)]
    pub env_overrides: BTreeMap<String, String>,
    pub trust_grant: bool,
    pub auto_start: bool,
}

/// Errors specific to catalog/installer operations.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("entry not found: {0}")]
    NotFound(String),
    #[error("invalid catalog data: {0}")]
    InvalidData(String),
    #[error("provider error: {0}")]
    Provider(String),
}

pub type CatalogResult<T> = std::result::Result<T, CatalogError>;

/// A source of [`ServerEntry`] data.
#[async_trait]
pub trait CatalogProvider: Send + Sync {
    fn source_id(&self) -> &str;
    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>>;
    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>>;
    async fn refresh(&self) -> CatalogResult<()> {
        Ok(())
    }
}

pub mod aggregate;
pub mod builtin;
#[cfg(feature = "remote-catalog")]
pub mod skills;

#[cfg(feature = "remote-catalog")]
pub mod remote;

pub use aggregate::AggregateCatalogProvider;
pub use builtin::BuiltinCatalogProvider;

/// Sink for domain events emitted by catalog infrastructure.
///
/// Implemented by the runtime layer over its broadcast channel. Defined
/// here (rather than in `agent-runtime`) so that `agent-mcp` can emit
/// observability events without taking a reverse dependency on
/// `agent-runtime`.
#[async_trait]
pub trait DomainEventSink: Send + Sync {
    /// A configured catalog source failed to respond. `error` is a short,
    /// human-readable description suitable for UI surfacing.
    async fn emit_source_failed(&self, source_id: &str, error: &str);
    /// A new catalog source has been added at runtime.
    async fn emit_source_added(&self, source_id: &str);
}
