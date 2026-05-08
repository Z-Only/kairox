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
pub mod kairox_json;
pub mod mcp_registry;
pub mod smithery;

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
mod build_tests {
    use super::*;
    use crate::catalog::TrustLevel;

    #[test]
    fn build_provider_returns_correct_impl_per_kind() {
        let http = SharedHttpClient::new().unwrap();
        let cache = Arc::new(HttpResponseCache::new(
            std::env::temp_dir().join("kairox-test-cache"),
        ));
        let mcp = build_provider(
            RemoteSourceConfig {
                id: "m".into(),
                display_name: "m".into(),
                kind: RemoteSourceKind::McpRegistry,
                url: "https://registry.modelcontextprotocol.io".into(),
                api_key_env: None,
                priority: 50,
                default_trust: TrustLevel::Community,
                enabled: true,
                cache_ttl_seconds: None,
            },
            http,
            cache,
        );
        assert_eq!(mcp.source_id(), "m");
    }
}

/// Which adapter implementation should back a remote catalog source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteSourceKind {
    /// The official MCP Registry (`https://registry.modelcontextprotocol.io`).
    McpRegistry,
}

/// A single remote catalog source as configured by the user.
///
/// Sourced from `[[catalog_sources]]` entries in `~/.kairox/mcp_servers.toml`
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
mod tests {
    use super::*;

    #[test]
    fn remote_source_kind_serde_round_trip() {
        let k1 = RemoteSourceKind::McpRegistry;
        let s = serde_json::to_string(&k1).unwrap();
        assert_eq!(s, "\"mcp_registry\"");
        let back: RemoteSourceKind = serde_json::from_str(&s).unwrap();
        assert_eq!(back, k1);
    }

    #[test]
    fn remote_source_config_round_trips_via_json() {
        let cfg = RemoteSourceConfig {
            id: "mcp-registry".into(),
            display_name: "MCP Servers".into(),
            kind: RemoteSourceKind::McpRegistry,
            url: "https://registry.modelcontextprotocol.io".into(),
            api_key_env: None,
            priority: 50,
            default_trust: TrustLevel::Community,
            enabled: true,
            cache_ttl_seconds: Some(600),
        };
        let s = serde_json::to_string(&cfg).unwrap();
        let back: RemoteSourceConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn remote_error_into_catalog_error_preserves_message() {
        let e = RemoteError::Http("status 503".into());
        let c: CatalogError = e.into();
        match c {
            CatalogError::Provider(msg) => assert!(msg.contains("503")),
            _ => panic!("expected Provider variant"),
        }
    }
}
