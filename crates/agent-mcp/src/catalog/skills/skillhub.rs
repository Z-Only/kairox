//! SkillHub catalog provider.
//!
//! Adapts the SkillHub API (`https://api.skillhub.cn/api/skills`)
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
struct SkillHubResponse {
    #[serde(default)]
    code: Option<i32>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    skills: Vec<SkillHubItem>,
    #[serde(default)]
    data: Option<SkillHubData>,
}

#[derive(Debug, Deserialize)]
struct SkillHubData {
    #[serde(default)]
    skills: Vec<SkillHubItem>,
    #[serde(default, rename = "total")]
    _total: Option<u64>,
    #[serde(default, rename = "count")]
    _count: Option<u64>,
    #[serde(default, rename = "pagination")]
    _pagination: Option<SkillHubPagination>,
}

#[derive(Debug, Deserialize)]
struct SkillHubPagination {
    #[serde(default, rename = "total")]
    _total: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SkillHubItem {
    #[serde(default, alias = "slug")]
    id: String,
    #[serde(default, alias = "displayName")]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    #[serde(rename = "description_zh")]
    description_zh: Option<String>,
    #[serde(default, rename = "githubOwner")]
    _github_owner: Option<String>,
    #[serde(default, rename = "githubRepo")]
    _github_repo: Option<String>,
    #[serde(default)]
    #[serde(rename = "githubStars")]
    github_stars: Option<u64>,
    #[serde(default)]
    #[serde(rename = "downloadCount")]
    download_count: Option<u64>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    installs: Option<u64>,
    #[serde(default)]
    stars: Option<u64>,
    #[serde(default)]
    #[serde(rename = "securityScore")]
    security_score: Option<u32>,
    #[serde(default)]
    rating: Option<f64>,
    #[serde(default)]
    #[serde(rename = "packageUrl")]
    package_url: Option<String>,
    #[serde(default, rename = "homepage")]
    _homepage: Option<String>,
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
        let template = match (keyword, &self.cfg.list_template) {
            (Some(_), _) => &self.cfg.search_template,
            (None, Some(list_tmpl)) => list_tmpl,
            (None, None) => &self.cfg.search_template,
        };

        let mut url = template
            .replace("{{limit}}", &limit.to_string())
            .replace("{{pageSize}}", &limit.to_string());

        if let Some(kw) = keyword {
            let encoded = url::form_urlencoded::byte_serialize(kw.as_bytes()).collect::<String>();
            url = url.replace("{{query}}", &encoded);
        } else {
            url = url
                .replace("?q={{query}}&", "?")
                .replace("&q={{query}}", "")
                .replace("?keyword={{query}}&", "?")
                .replace("&keyword={{query}}", "");
        }

        self.absolute_url(&url)
    }

    fn absolute_url(&self, template: &str) -> String {
        if template.starts_with('/') {
            format!("{}{}", self.cfg.url.trim_end_matches('/'), template)
        } else {
            template.to_string()
        }
    }

    fn package_url(&self, slug: &str) -> String {
        self.absolute_url(
            &self
                .cfg
                .download_template
                .replace("{{slug}}", slug)
                .replace("{{id}}", slug)
                .replace("{{package}}", slug),
        )
    }

    fn map_entry(&self, item: SkillHubItem) -> SkillCatalogEntry {
        let package_url = item
            .package_url
            .or_else(|| Some(self.package_url(&item.id)));
        let description = item.description_zh.or(item.description).unwrap_or_default();

        SkillCatalogEntry {
            catalog_id: item.id.clone(),
            name: item.name,
            description,
            source: self.cfg.id.clone(),
            source_url: format!("https://skillhub.cn/skills/{}", item.id),
            install_count: item.download_count.or(item.downloads).or(item.installs),
            github_stars: item.github_stars.or(item.stars),
            security_score: item.security_score,
            rating: item.rating,
            package: item.id.clone(),
            package_url,
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

        if let Some(code) = parsed.code {
            if code != 0 {
                let message = parsed.message.unwrap_or_else(|| "unknown error".into());
                return Err(SkillCatalogError::Decode(format!(
                    "SkillHub returned code {code}: {message}"
                )));
            }
        }

        let skills = parsed
            .data
            .map(|data| data.skills)
            .filter(|skills| !skills.is_empty())
            .unwrap_or(parsed.skills);

        let entries: Vec<SkillCatalogEntry> = skills
            .into_iter()
            .filter(|item| !item.id.is_empty())
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
#[path = "skillhub_tests.rs"]
mod tests;
