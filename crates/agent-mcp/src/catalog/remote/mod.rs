//! Remote catalog providers and supporting types.
//!
//! This module is gated behind the `remote-catalog` cargo feature. It defines
//! the configuration types ([`RemoteSourceConfig`], [`RemoteSourceKind`]) and
//! error type ([`RemoteError`]) shared by all remote provider implementations
//! (Kairox JSON, MCP Registry).

use crate::catalog::{CatalogError, TrustLevel};
use serde::{Deserialize, Serialize};

pub mod http_cache;
pub mod http_client;
pub mod mcp_mapping;
pub mod mcp_registry;

pub use http_cache::HttpResponseCache;
pub use http_client::SharedHttpClient;
pub use mcp_registry::McpRegistryProvider;

use crate::catalog::CatalogProvider;
use std::sync::Arc;

/// Constructs the right [`CatalogProvider`] implementation based on
/// `cfg.kind`. Shared HTTP client + cache are passed in by the caller so
/// that concurrent providers can share them.
pub fn build_provider(
    cfg: RemoteSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
) -> Arc<dyn CatalogProvider> {
    match cfg.kind {
        RemoteSourceKind::McpRegistry => Arc::new(McpRegistryProvider::new(cfg, http, cache)),
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod build_tests;

/// Which adapter implementation should back a remote catalog source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteSourceKind {
    /// The official MCP Registry (`https://registry.modelcontextprotocol.io`).
    McpRegistry,
}

/// A single remote catalog source as configured by the user.
///
/// Sourced from `[[catalog_sources]]` entries in `~/.kairox/config.toml`
/// and translated by `agent-config` into this struct before being handed to
/// the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSourceConfig {
    /// Stable identifier for this source (used as `ServerEntry.source`).
    pub id: String,
    /// Human-readable name shown in the GUI.
    pub display_name: String,
    /// Adapter kind.
    pub kind: RemoteSourceKind,
    /// Base URL. For McpRegistry this is the registry root URL.
    pub url: String,
    /// If set, the value of this environment variable is sent as a
    /// `Bearer` token in the `Authorization` header.
    pub api_key_env: Option<String>,
    /// Sort key for the aggregate provider; lower = higher precedence.
    pub priority: u32,
    /// Trust ceiling applied to every entry returned from this source. An
    /// entry's claimed trust is clamped to this level.
    pub default_trust: TrustLevel,
    /// Disabled sources are skipped at construction time.
    pub enabled: bool,
    /// Per-source override for the response cache TTL (seconds). When `None`
    /// the adapter's built-in default (15 minutes) is used.
    pub cache_ttl_seconds: Option<u64>,
}

/// Errors that can occur while talking to a remote catalog source.
#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("http error: {0}")]
    Http(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config error: {0}")]
    Config(String),
}

impl From<RemoteError> for CatalogError {
    fn from(e: RemoteError) -> Self {
        CatalogError::Provider(e.to_string())
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
