//! Official MCP Registry catalog provider.
//!
//! Adapts the Model Context Protocol Registry API
//! (`/v0.1/servers` endpoint) to our internal [`ServerEntry`] schema.
//!
//! The API returns a cursor-paginated list of server entries. Each entry
//! contains a `server` object (with `name`, `description`, `title`,
//! `version`, `remotes`, `packages`, `repository`) and `_meta` with
//! publish / latest-version metadata.
//!
//! Only entries whose `_meta` has `isLatest == true` are kept so the
//! catalog shows one entry per server rather than one per version.
//!
//! API DTO types and the pure mapping functions live in
//! [`super::mcp_mapping`]; this module handles fetching, caching, lock
//! coordination, and the [`CatalogProvider`] trait implementation.

use crate::catalog::remote::http_cache::HttpResponseCache;
use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::remote::mcp_mapping::{is_latest, map_mcp_to_entry, McpListResponse};
use crate::catalog::remote::{RemoteError, RemoteSourceConfig};
use crate::catalog::{CatalogError, CatalogProvider, CatalogQuery, CatalogResult, ServerEntry};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Maximum number of servers to collect across all pages.
const MAX_SERVERS_TO_FETCH: usize = 500;

/// Page size requested per cursor-based page.
const PAGE_SIZE: usize = 100;

// ── Provider ─────────────────────────────────────────────────────────

pub struct McpRegistryProvider {
    cfg: RemoteSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
    /// In-memory cache of fetched entries. Survives for the provider's
    /// lifetime (session-scoped). Never persisted to disk — app restart
    /// always starts cold and fetches fresh data.
    cached_entries: Mutex<Option<Vec<ServerEntry>>>,
}

impl McpRegistryProvider {
    pub fn new(
        cfg: RemoteSourceConfig,
        http: SharedHttpClient,
        cache: Arc<HttpResponseCache>,
    ) -> Self {
        Self {
            cfg,
            http,
            cache,
            cached_entries: Mutex::new(None),
        }
    }

    async fn fetch(&self) -> Result<Vec<ServerEntry>, RemoteError> {
        let base = format!("{}/v0.1/servers", self.cfg.url.trim_end_matches('/'));
        let ceiling = self.cfg.default_trust;
        let mut all_entries: Vec<ServerEntry> = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let url = match &cursor {
                Some(c) => format!("{base}?limit={PAGE_SIZE}&cursor={c}"),
                None => format!("{base}?limit={PAGE_SIZE}"),
            };

            let resp = self
                .http
                .get_json(
                    &url,
                    GetOpts {
                        api_key_env: self.cfg.api_key_env.as_deref(),
                        if_none_match: None,
                    },
                )
                .await?;
            if !(200..300).contains(&resp.status) {
                return Err(RemoteError::Http(format!("status {}", resp.status)));
            }

            let parsed: McpListResponse = serde_json::from_slice(&resp.body)
                .map_err(|e| RemoteError::Decode(format!("mcp registry: {e}")))?;

            for wrapper in &parsed.servers {
                // Only keep the latest version of each server.
                if !is_latest(&wrapper.meta) {
                    continue;
                }
                match map_mcp_to_entry(&self.cfg.id, wrapper, ceiling) {
                    Ok(entry) => all_entries.push(entry),
                    Err(e) => {
                        tracing::warn!(
                            name=%wrapper.server.name,
                            error=%e,
                            "skipping mcp registry entry"
                        );
                    }
                }
            }

            let next = parsed.metadata.as_ref().and_then(|m| m.next_cursor.clone());

            if next.is_none() || all_entries.len() >= MAX_SERVERS_TO_FETCH {
                break;
            }
            cursor = next;
        }

        all_entries.truncate(MAX_SERVERS_TO_FETCH);
        Ok(all_entries)
    }

    /// Serve from in-memory cache when warm. On cache miss, fetch from the
    /// network, store in cache, and return. The cache is session-scoped
    /// (never persisted to disk) so app restart always fetches fresh data.
    async fn entries(&self) -> CatalogResult<Vec<ServerEntry>> {
        // Fast path: in-memory cache hit.
        if let Some(cached) = self.cached_entries.lock().await.as_ref() {
            return Ok(cached.clone());
        }
        // Slow path: fetch from network with single-flight lock.
        let lock = self.cache.lock_for(&self.cfg.id).await;
        let _guard = lock.lock().await;
        // Double-check: another task may have populated the cache while we
        // waited for the lock.
        if let Some(cached) = self.cached_entries.lock().await.as_ref() {
            return Ok(cached.clone());
        }
        let entries = self.fetch().await.map_err(CatalogError::from)?;
        *self.cached_entries.lock().await = Some(entries.clone());
        Ok(entries)
    }
}

#[async_trait]
impl CatalogProvider for McpRegistryProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let mut entries = self.entries().await?;
        if let Some(kw) = &query.keyword {
            let kw_lower = kw.to_lowercase();
            entries.retain(|e| {
                let haystack = format!(
                    "{} {}",
                    e.display_name.to_lowercase(),
                    e.summary.to_lowercase()
                );
                haystack.contains(&kw_lower)
            });
        }
        if let Some(min) = query.trust_min {
            entries.retain(|e| e.trust >= min);
        }
        if let Some(limit) = query.limit {
            entries.truncate(limit);
        }
        Ok(entries)
    }

    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
        Ok(self.entries().await?.into_iter().find(|e| e.id == id))
    }

    async fn refresh(&self) -> CatalogResult<()> {
        let lock = self.cache.lock_for(&self.cfg.id).await;
        let _guard = lock.lock().await;
        // Clear the in-memory cache so the next read will see fresh data.
        self.cached_entries.lock().await.take();
        let entries = self.fetch().await?;
        *self.cached_entries.lock().await = Some(entries);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::remote::RemoteSourceKind;
    use crate::catalog::TrustLevel;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn end_to_end_list_fetches_and_filters() {
        let server = MockServer::start().await;
        let body = json!({
            "servers": [
                {
                    "server": {
                        "name": "com.example/a",
                        "description": "A test server.",
                        "title": "Test Server",
                        "version": "1.0.0",
                        "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                    },
                    "_meta": {
                        "io.modelcontextprotocol.registry/official": {
                            "status": "active",
                            "isLatest": true
                        }
                    }
                },
                {
                    "server": {
                        "name": "com.example/b",
                        "description": "A test server.",
                        "title": "Test Server",
                        "version": "1.0.0",
                        "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                    },
                    "_meta": {
                        "io.modelcontextprotocol.registry/official": {
                            "status": "active",
                            "isLatest": false
                        }
                    }
                },
                {
                    "server": {
                        "name": "com.example/c",
                        "description": "A test server.",
                        "title": "Test Server",
                        "version": "1.0.0",
                        "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                    },
                    "_meta": {
                        "io.modelcontextprotocol.registry/official": {
                            "status": "active",
                            "isLatest": true
                        }
                    }
                }
            ],
            "metadata": {"count": 3}
        });
        Mock::given(method("GET"))
            .and(path("/v0.1/servers"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
        let provider = McpRegistryProvider::new(
            RemoteSourceConfig {
                id: "mcp-registry".into(),
                display_name: "MCP Servers".into(),
                kind: RemoteSourceKind::McpRegistry,
                url: server.uri(),
                api_key_env: None,
                priority: 50,
                default_trust: TrustLevel::Community,
                enabled: true,
                cache_ttl_seconds: None,
            },
            SharedHttpClient::new().unwrap(),
            cache,
        );
        let entries = provider.list(&CatalogQuery::default()).await.unwrap();
        // Only isLatest==true entries are returned (a and c).
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "com.example/a");
        assert_eq!(entries[1].id, "com.example/c");
        assert_eq!(entries[0].source, "mcp-registry");
    }

    #[tokio::test]
    async fn cached_entries_served_on_second_list() {
        let server = MockServer::start().await;
        let body = json!({
            "servers": [
                {
                    "server": {
                        "name": "com.example/warm",
                        "description": "Cache warm test.",
                        "title": "Warm",
                        "version": "1.0.0",
                        "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                    },
                    "_meta": {
                        "io.modelcontextprotocol.registry/official": {
                            "isLatest": true
                        }
                    }
                }
            ],
            "metadata": {"count": 1}
        });
        Mock::given(method("GET"))
            .and(path("/v0.1/servers"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()))
            .expect(1) // only one HTTP call; second list() hits cache
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
        let provider = McpRegistryProvider::new(
            RemoteSourceConfig {
                id: "mcp-registry".into(),
                display_name: "MCP Servers".into(),
                kind: RemoteSourceKind::McpRegistry,
                url: server.uri(),
                api_key_env: None,
                priority: 50,
                default_trust: TrustLevel::Community,
                enabled: true,
                cache_ttl_seconds: None,
            },
            SharedHttpClient::new().unwrap(),
            cache,
        );

        // First call populates cache.
        let first = provider.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, "com.example/warm");

        // Second call serves from in-memory cache — no HTTP call.
        let second = provider.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].id, "com.example/warm");
    }

    #[tokio::test]
    async fn refresh_clears_cache_and_refetches() {
        let server = MockServer::start().await;
        // First response.
        let body1 = json!({
            "servers": [
                {
                    "server": {
                        "name": "com.example/v1",
                        "description": "First version.",
                        "title": "V1",
                        "version": "1.0.0",
                        "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                    },
                    "_meta": {
                        "io.modelcontextprotocol.registry/official": {
                            "isLatest": true
                        }
                    }
                }
            ],
            "metadata": {"count": 1}
        });
        Mock::given(method("GET"))
            .and(path("/v0.1/servers"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body1.to_string()))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        // Second response (after refresh).
        let body2 = json!({
            "servers": [
                {
                    "server": {
                        "name": "com.example/v1",
                        "description": "First version.",
                        "title": "V1",
                        "version": "1.0.0",
                        "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                    },
                    "_meta": {
                        "io.modelcontextprotocol.registry/official": {
                            "isLatest": true
                        }
                    }
                },
                {
                    "server": {
                        "name": "com.example/v2",
                        "description": "Second version.",
                        "title": "V2",
                        "version": "2.0.0",
                        "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
                    },
                    "_meta": {
                        "io.modelcontextprotocol.registry/official": {
                            "isLatest": true
                        }
                    }
                }
            ],
            "metadata": {"count": 2}
        });
        Mock::given(method("GET"))
            .and(path("/v0.1/servers"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body2.to_string()))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
        let provider = McpRegistryProvider::new(
            RemoteSourceConfig {
                id: "mcp-registry".into(),
                display_name: "MCP Servers".into(),
                kind: RemoteSourceKind::McpRegistry,
                url: server.uri(),
                api_key_env: None,
                priority: 50,
                default_trust: TrustLevel::Community,
                enabled: true,
                cache_ttl_seconds: None,
            },
            SharedHttpClient::new().unwrap(),
            cache,
        );

        let first = provider.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(first.len(), 1);

        provider.refresh().await.unwrap();
        let second = provider.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(second.len(), 2);
    }
}
