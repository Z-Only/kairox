//! Kairox JSON catalog provider.
//!
//! Fetches a single JSON document at `cfg.url` matching the internal
//! `ServerEntry` schema (schema_version="1"). The body is cached on disk
//! via [`HttpResponseCache`] with TTL + ETag conditional GETs and
//! single-flight refetch protection.

use crate::catalog::remote::http_cache::{CachedResponse, HttpResponseCache};
use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::remote::{RemoteError, RemoteSourceConfig};
use crate::catalog::{
    CatalogError, CatalogProvider, CatalogQuery, CatalogResult, ServerEntry, TrustLevel,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_SECONDS: u64 = 900; // 15 minutes

#[derive(Debug, Deserialize)]
struct Doc {
    schema_version: String,
    #[serde(default)]
    #[allow(dead_code)]
    generated_at: Option<String>,
    entries: Vec<ServerEntry>,
}

pub struct KairoxJsonProvider {
    cfg: RemoteSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
}

impl KairoxJsonProvider {
    pub fn new(
        cfg: RemoteSourceConfig,
        http: SharedHttpClient,
        cache: Arc<HttpResponseCache>,
    ) -> Self {
        Self { cfg, http, cache }
    }

    fn ttl(&self) -> u64 {
        self.cfg.cache_ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS)
    }

    fn clip_trust(entry: &mut ServerEntry, ceiling: TrustLevel) {
        if entry.trust > ceiling {
            entry.trust = ceiling;
        }
    }

    async fn fetch_and_store(&self, etag: Option<&str>) -> Result<CachedResponse, RemoteError> {
        let resp = self
            .http
            .get_json(
                &self.cfg.url,
                GetOpts {
                    api_key_env: self.cfg.api_key_env.as_deref(),
                    if_none_match: etag,
                },
            )
            .await?;
        if resp.status == 304 {
            return Err(RemoteError::Http("304_not_modified".into()));
        }
        if !(200..300).contains(&resp.status) {
            return Err(RemoteError::Http(format!("status {}", resp.status)));
        }
        let doc: Doc = serde_json::from_slice(&resp.body)
            .map_err(|e| RemoteError::Decode(format!("body: {e}")))?;
        if doc.schema_version != "1" {
            return Err(RemoteError::Decode(format!(
                "unsupported schema_version: {}",
                doc.schema_version
            )));
        }
        let ceiling = self.cfg.default_trust;
        let mut entries = doc.entries;
        for entry in &mut entries {
            entry.source = self.cfg.id.clone();
            Self::clip_trust(entry, ceiling);
        }
        let cached = CachedResponse {
            fetched_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            etag: resp.etag,
            last_modified: None,
            entries,
        };
        self.cache.put(&self.cfg.id, cached.clone()).await?;
        Ok(cached)
    }

    async fn entries(&self) -> CatalogResult<Vec<ServerEntry>> {
        let lock = self.cache.lock_for(&self.cfg.id).await;
        let _guard = lock.lock().await;

        let cached = self.cache.get(&self.cfg.id).await;
        if let Some(c) = &cached {
            if HttpResponseCache::is_fresh(c, self.ttl()) {
                return Ok(c.entries.clone());
            }
        }
        let etag = cached.as_ref().and_then(|c| c.etag.clone());
        match self.fetch_and_store(etag.as_deref()).await {
            Ok(c) => Ok(c.entries),
            Err(RemoteError::Http(ref s)) if s == "304_not_modified" => {
                if let Some(mut c) = cached {
                    c.fetched_at_unix = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    self.cache.put(&self.cfg.id, c.clone()).await?;
                    Ok(c.entries)
                } else {
                    Err(CatalogError::Provider(
                        "304 with no cached body".to_string(),
                    ))
                }
            }
            Err(e) => {
                if let Some(c) = cached {
                    tracing::warn!(source=%self.cfg.id, error=%e, "kairox_json refetch failed, serving stale");
                    Ok(c.entries)
                } else {
                    Err(e.into())
                }
            }
        }
    }
}

#[async_trait]
impl CatalogProvider for KairoxJsonProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let mut out: Vec<ServerEntry> = self
            .entries()
            .await?
            .into_iter()
            .filter(|e| {
                if let Some(kw) = &query.keyword {
                    let kw_lc = kw.to_lowercase();
                    let hay = format!(
                        "{} {} {}",
                        e.display_name.to_lowercase(),
                        e.summary.to_lowercase(),
                        e.tags.join(" ").to_lowercase()
                    );
                    if !hay.contains(&kw_lc) {
                        return false;
                    }
                }
                if let Some(cat) = &query.category {
                    if !e.categories.iter().any(|c| c == cat) {
                        return false;
                    }
                }
                if let Some(min) = query.trust_min {
                    if e.trust < min {
                        return false;
                    }
                }
                true
            })
            .collect();
        out.sort_by(|a, b| {
            b.trust
                .cmp(&a.trust)
                .then_with(|| a.display_name.cmp(&b.display_name))
        });
        if let Some(limit) = query.limit {
            out.truncate(limit);
        }
        Ok(out)
    }

    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
        Ok(self.entries().await?.into_iter().find(|e| e.id == id))
    }

    async fn refresh(&self) -> CatalogResult<()> {
        if let Some(mut c) = self.cache.get(&self.cfg.id).await {
            c.fetched_at_unix = 0;
            self.cache.put(&self.cfg.id, c).await?;
        }
        let _ = self.entries().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::remote::RemoteSourceKind;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cfg(url: &str, ceiling: TrustLevel) -> RemoteSourceConfig {
        RemoteSourceConfig {
            id: "kx".into(),
            display_name: "kx".into(),
            kind: RemoteSourceKind::McpRegistry,
            url: url.to_string(),
            api_key_env: None,
            priority: 100,
            default_trust: ceiling,
            enabled: true,
            cache_ttl_seconds: None,
        }
    }

    fn body(trust: &str) -> String {
        format!(
            r#"{{
              "schema_version": "1",
              "entries": [{{
                "id": "x",
                "source": "ignored",
                "display_name": "X",
                "summary": "s",
                "description": "d",
                "categories": ["dev-tools"],
                "tags": ["t"],
                "install": {{"transport":"stdio","command":"echo","args":[],"env":{{}}}},
                "requirements": [],
                "trust": "{trust}",
                "default_env": []
              }}]
            }}"#
        )
    }

    fn provider_for(server: &MockServer, ceiling: TrustLevel) -> KairoxJsonProvider {
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
        let http = SharedHttpClient::new().unwrap();
        // tempdir leaks intentionally — short-lived test process.
        std::mem::forget(dir);
        KairoxJsonProvider::new(
            cfg(&format!("{}/c.json", server.uri()), ceiling),
            http,
            cache,
        )
    }

    #[tokio::test]
    async fn list_returns_entries_and_overwrites_source_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body("verified")))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified);
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "kx");
        assert_eq!(entries[0].trust, TrustLevel::Verified);
    }

    #[tokio::test]
    async fn list_clips_trust_to_ceiling() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body("verified")))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Community);
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries[0].trust, TrustLevel::Community);
    }

    #[tokio::test]
    async fn list_serves_stale_on_5xx_after_first_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body("community")))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified);
        // First call: succeeds and caches.
        let _ = p.list(&CatalogQuery::default()).await.unwrap();
        // Force a refresh path; refresh propagates the 503 once cache is dirtied.
        let _ = p.refresh().await;
        // List must still serve stale entries from cache.
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn list_returns_decode_error_on_bad_schema_version() {
        let server = MockServer::start().await;
        let bad = r#"{"schema_version":"99","entries":[]}"#;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(bad))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified);
        let err = p.list(&CatalogQuery::default()).await.unwrap_err();
        assert!(matches!(err, CatalogError::Provider(_)));
    }

    #[tokio::test]
    async fn second_call_within_ttl_uses_cache_and_does_not_hit_network() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body("community")))
            .expect(1)
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified);
        let _ = p.list(&CatalogQuery::default()).await.unwrap();
        let _ = p.list(&CatalogQuery::default()).await.unwrap();
        // .expect(1) above asserts via wiremock on drop.
    }

    #[tokio::test]
    async fn conditional_get_with_if_none_match_when_etag_known() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "W/\"v1\"")
                    .set_body_string(body("community")),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .and(header("if-none-match", "W/\"v1\""))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified);
        let _ = p.list(&CatalogQuery::default()).await.unwrap();
        // refresh forces a refetch which should send If-None-Match and 304.
        p.refresh().await.unwrap();
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
    }
}
