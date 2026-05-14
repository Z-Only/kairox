//! SkillHub catalog provider.
//!
//! Adapts the SkillHub API (`https://skills.palebluedot.live/api/skills`)
//! to [`SkillCatalogEntry`].

use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::skills::remote::RemoteSkillSourceConfig;
use crate::catalog::skills::{
    SkillCatalogEntry, SkillCatalogError, SkillCatalogProvider, SkillCatalogQuery,
    SkillCatalogResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::Mutex;

const DEFAULT_TTL_SECONDS: u64 = 900;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SkillHubResponse {
    #[serde(default)]
    skills: Vec<SkillHubItem>,
    #[serde(default)]
    pagination: Option<SkillHubPagination>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SkillHubPagination {
    #[serde(default)]
    total: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SkillHubItem {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    #[serde(rename = "githubOwner")]
    github_owner: Option<String>,
    #[serde(default)]
    #[serde(rename = "githubRepo")]
    github_repo: Option<String>,
    #[serde(default)]
    #[serde(rename = "githubStars")]
    github_stars: Option<u64>,
    #[serde(default)]
    #[serde(rename = "downloadCount")]
    download_count: Option<u64>,
    #[serde(default)]
    #[serde(rename = "securityScore")]
    security_score: Option<u32>,
    #[serde(default)]
    rating: Option<f64>,
    #[serde(default)]
    #[serde(rename = "packageUrl")]
    package_url: Option<String>,
}

struct CacheEntry {
    entries: Vec<SkillCatalogEntry>,
    fetched_at: Instant,
}

pub struct SkillHubProvider {
    cfg: RemoteSkillSourceConfig,
    http: SharedHttpClient,
    cache: Mutex<HashMap<String, CacheEntry>>,
}

impl SkillHubProvider {
    pub fn new(cfg: RemoteSkillSourceConfig, http: SharedHttpClient) -> Self {
        Self {
            cfg,
            http,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn ttl(&self) -> u64 {
        if self.cfg.cache_ttl_seconds > 0 {
            self.cfg.cache_ttl_seconds
        } else {
            DEFAULT_TTL_SECONDS
        }
    }

    fn build_url(&self, keyword: Option<&str>, limit: usize) -> String {
        let base = self.cfg.url.trim_end_matches('/');
        let template = match (keyword, &self.cfg.list_template) {
            (Some(_), _) => &self.cfg.search_template,
            (None, Some(list_tmpl)) => list_tmpl,
            (None, None) => &self.cfg.search_template,
        };

        let mut url = template.replace("{{limit}}", &limit.to_string());

        if let Some(kw) = keyword {
            let encoded = url::form_urlencoded::byte_serialize(kw.as_bytes()).collect::<String>();
            url = url.replace("{{query}}", &encoded);
        } else {
            url = url
                .replace("?q={{query}}&", "?")
                .replace("&q={{query}}", "");
        }

        if url.starts_with('/') {
            format!("{base}{url}")
        } else {
            url
        }
    }

    fn map_entry(&self, item: SkillHubItem) -> SkillCatalogEntry {
        SkillCatalogEntry {
            catalog_id: item.id.clone(),
            name: item.name,
            description: item.description.unwrap_or_default(),
            source: self.cfg.id.clone(),
            source_url: format!("https://skills.palebluedot.live/skills/{}", item.id),
            install_count: item.download_count,
            github_stars: item.github_stars,
            security_score: item.security_score,
            rating: item.rating,
            package: item.id.clone(),
            package_url: item.package_url,
        }
    }

    async fn fetch(
        &self,
        keyword: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SkillCatalogEntry>, SkillCatalogError> {
        let url = self.build_url(keyword, limit);

        let response = self
            .http
            .get_json(
                &url,
                GetOpts {
                    api_key_env: None,
                    if_none_match: None,
                },
            )
            .await
            .map_err(|e| SkillCatalogError::Http(format!("SkillHub request failed: {e}")))?;

        if !(200..300).contains(&response.status) {
            return Err(SkillCatalogError::Http(format!(
                "SkillHub returned status {}",
                response.status
            )));
        }

        let parsed: SkillHubResponse = serde_json::from_slice(&response.body)
            .map_err(|e| SkillCatalogError::Decode(format!("SkillHub parse: {e}")))?;

        let entries: Vec<SkillCatalogEntry> = parsed
            .skills
            .into_iter()
            .map(|item| self.map_entry(item))
            .collect();

        let cache_key = match keyword {
            Some(kw) => format!("search:{kw}"),
            None => format!("list:{limit}"),
        };
        self.cache.lock().await.insert(
            cache_key,
            CacheEntry {
                entries: entries.clone(),
                fetched_at: Instant::now(),
            },
        );

        Ok(entries)
    }

    async fn cached_fetch(
        &self,
        keyword: Option<&str>,
        limit: usize,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let cache_key = match keyword {
            Some(kw) => format!("search:{kw}"),
            None => format!("list:{limit}"),
        };
        {
            let cache = self.cache.lock().await;
            if let Some(entry) = cache.get(&cache_key) {
                if entry.fetched_at.elapsed().as_secs() < self.ttl() {
                    return Ok(entry.entries.clone());
                }
            }
        }

        match self.fetch(keyword, limit).await {
            Ok(entries) => Ok(entries),
            Err(e) => {
                if let Some(entry) = self.cache.lock().await.get(&cache_key) {
                    tracing::warn!(error=%e, "SkillHub refetch failed, serving stale");
                    return Ok(entry.entries.clone());
                }
                Err(e)
            }
        }
    }
}

#[async_trait]
impl SkillCatalogProvider for SkillHubProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn search(
        &self,
        query: &SkillCatalogQuery,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let keyword = query.keyword.as_deref();
        let limit = query.limit.unwrap_or(50);
        self.cached_fetch(keyword, limit).await
    }

    async fn list(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let limit = query.limit.unwrap_or(50);
        self.cached_fetch(None, limit).await
    }

    async fn refresh(&self) -> SkillCatalogResult<()> {
        self.cache.lock().await.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::skills::remote::SkillSourceKind;

    fn test_cfg() -> RemoteSkillSourceConfig {
        RemoteSkillSourceConfig {
            id: "skillhub".into(),
            display_name: "SkillHub".into(),
            kind: SkillSourceKind::SkillHub,
            url: "https://skills.palebluedot.live".into(),
            search_template: "/api/skills?q={{query}}&limit={{limit}}".into(),
            list_template: Some("/api/skills?limit={{limit}}".into()),
            enabled: true,
            priority: 20,
            cache_ttl_seconds: 900,
        }
    }

    #[test]
    fn build_search_url_with_keyword() {
        let http = SharedHttpClient::new().unwrap();
        let provider = SkillHubProvider::new(test_cfg(), http);
        let url = provider.build_url(Some("code review"), 10);
        assert!(url.contains("q=code+review"));
        assert!(url.contains("limit=10"));
        assert!(url.starts_with("https://skills.palebluedot.live"));
    }

    #[test]
    fn build_list_url_without_keyword() {
        let http = SharedHttpClient::new().unwrap();
        let provider = SkillHubProvider::new(test_cfg(), http);
        let url = provider.build_url(None, 20);
        assert!(!url.contains("q="));
        assert!(url.contains("limit=20"));
        assert!(url.starts_with("https://skills.palebluedot.live"));
    }

    #[test]
    fn skillhub_response_parses_correctly() {
        let json = r#"{"skills":[{"id":"test/skill","name":"test-skill","description":"A test skill","githubStars":100,"downloadCount":50,"securityScore":95,"rating":4.5}]}"#;
        let parsed: SkillHubResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.skills.len(), 1);
        assert_eq!(parsed.skills[0].name, "test-skill");
        assert_eq!(
            parsed.skills[0].description.as_deref(),
            Some("A test skill")
        );
        assert_eq!(parsed.skills[0].github_stars, Some(100));
        assert_eq!(parsed.skills[0].download_count, Some(50));
    }
}
