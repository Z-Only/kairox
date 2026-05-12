//! skills.sh catalog provider.
//!
//! Adapts the skills.sh API (`/api/search`) to [`SkillCatalogEntry`].

use crate::catalog::remote::http_cache::{CachedResponse, HttpResponseCache};
use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::skills::remote::RemoteSkillSourceConfig;
use crate::catalog::skills::{
    SkillCatalogEntry, SkillCatalogError, SkillCatalogProvider, SkillCatalogQuery,
    SkillCatalogResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_SECONDS: u64 = 900;

#[derive(Debug, Deserialize)]
struct SkillsShResponse {
    #[serde(default)]
    skills: Vec<SkillsShItem>,
}

#[derive(Debug, Deserialize)]
struct SkillsShItem {
    id: String,
    name: String,
    #[serde(default)]
    installs: Option<u64>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default, rename = "skillId")]
    skill_id: Option<String>,
}

pub struct SkillsShProvider {
    cfg: RemoteSkillSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
}

impl SkillsShProvider {
    pub fn new(
        cfg: RemoteSkillSourceConfig,
        http: SharedHttpClient,
        cache: Arc<HttpResponseCache>,
    ) -> Self {
        Self { cfg, http, cache }
    }

    fn ttl(&self) -> u64 {
        if self.cfg.cache_ttl_seconds > 0 {
            self.cfg.cache_ttl_seconds
        } else {
            DEFAULT_TTL_SECONDS
        }
    }

    fn build_search_url(&self, query: &str, limit: usize) -> String {
        let base = self.cfg.url.trim_end_matches('/');
        let encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        self.cfg
            .search_template
            .replace("{{query}}", &encoded)
            .replace("{{limit}}", &limit.to_string())
            .replacen("{{query}}", &encoded, 1)
            .replacen("{{limit}}", &limit.to_string(), 1)
    }

    async fn fetch_search(
        &self,
        keyword: &str,
        limit: usize,
    ) -> Result<Vec<SkillCatalogEntry>, SkillCatalogError> {
        let url = if self.cfg.search_template.contains("{{query}}") {
            self.build_search_url(keyword, limit)
        } else {
            format!(
                "{}{}",
                self.cfg.url.trim_end_matches('/'),
                self.cfg.search_template
            )
        };

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
            .map_err(|e| SkillCatalogError::Http(format!("skills.sh request failed: {e}")))?;

        if !(200..300).contains(&response.status) {
            return Err(SkillCatalogError::Http(format!(
                "skills.sh returned status {}",
                response.status
            )));
        }

        let parsed: SkillsShResponse = serde_json::from_slice(&response.body)
            .map_err(|e| SkillCatalogError::Decode(format!("skills.sh parse: {e}")))?;

        let entries: Vec<SkillCatalogEntry> = parsed
            .skills
            .into_iter()
            .map(|item| SkillCatalogEntry {
                catalog_id: item.id.clone(),
                name: item.name,
                description: String::new(),
                source: self.cfg.id.clone(),
                source_url: format!("https://skills.sh/skills/{}", item.id),
                install_count: item.installs,
                github_stars: None,
                security_score: None,
                rating: None,
                package: item.id,
            })
            .collect();

        // Cache the result under a search-scoped key.
        let cache_key = format!("{}:search:{}", self.cfg.id, keyword);
        let cached = CachedResponse {
            fetched_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            etag: None,
            last_modified: None,
            entries: entries.clone(),
        };
        let _ = self.cache.put(&cache_key, cached).await;

        Ok(entries)
    }

    async fn cached_search(
        &self,
        keyword: &str,
        limit: usize,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let cache_key = format!("{}:search:{}", self.cfg.id, keyword);
        let lock = self.cache.lock_for(&cache_key).await;
        let _guard = lock.lock().await;

        if let Some(cached) = self.cache.get(&cache_key).await {
            if HttpResponseCache::is_fresh(&cached, self.ttl()) {
                return Ok(cached.entries);
            }
            match self.fetch_search(keyword, limit).await {
                Ok(entries) => Ok(entries),
                Err(e) => {
                    tracing::warn!(error=%e, "skills.sh refetch failed, serving stale");
                    Ok(cached.entries)
                }
            }
        } else {
            self.fetch_search(keyword, limit).await
        }
    }
}

#[async_trait]
impl SkillCatalogProvider for SkillsShProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn search(
        &self,
        query: &SkillCatalogQuery,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let keyword = query.keyword.as_deref().unwrap_or("");
        let limit = query.limit.unwrap_or(50);
        self.cached_search(keyword, limit).await
    }

    async fn refresh(&self) -> SkillCatalogResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::skills::remote::SkillSourceKind;

    #[test]
    fn build_search_url_substitutes_placeholders() {
        let cfg = RemoteSkillSourceConfig {
            id: "skills-sh".into(),
            display_name: "skills.sh".into(),
            kind: SkillSourceKind::SkillsSh,
            url: "https://skills.sh".into(),
            search_template: "/api/search?q={{query}}&limit={{limit}}".into(),
            list_template: None,
            enabled: true,
            priority: 10,
            cache_ttl_seconds: 900,
        };
        let http = SharedHttpClient::new().unwrap();
        let cache = Arc::new(HttpResponseCache::new(
            std::env::temp_dir().join("kairox-test-sh-cache"),
        ));
        let provider = SkillsShProvider::new(cfg, http, cache);
        let url = provider.build_search_url("code review", 10);
        assert!(url.contains("q=code+review"));
        assert!(url.contains("limit=10"));
    }

    #[test]
    fn skills_sh_response_parses_correctly() {
        let json = r#"{"query":"test","skills":[{"id":"test/skill","name":"test-skill","installs":100,"source":"test/repo","skillId":"test-skill"}]}"#;
        let parsed: SkillsShResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.skills.len(), 1);
        assert_eq!(parsed.skills[0].name, "test-skill");
        assert_eq!(parsed.skills[0].installs, Some(100));
    }

    #[test]
    fn skills_sh_response_missing_optional_fields() {
        let json = r#"{"query":"test","skills":[{"id":"test/skill","name":"test-skill"}]}"#;
        let parsed: SkillsShResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.skills[0].installs, None);
        assert_eq!(parsed.skills[0].source, None);
    }
}
