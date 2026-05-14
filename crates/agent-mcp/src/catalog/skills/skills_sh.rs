//! skills.sh catalog provider.
//!
//! Adapts the skills.sh API (`/api/search`) to [`SkillCatalogEntry`].

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
struct SkillsShResponse {
    #[serde(default)]
    skills: Vec<SkillsShItem>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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

struct CacheEntry {
    entries: Vec<SkillCatalogEntry>,
    fetched_at: Instant,
}

pub struct SkillsShProvider {
    cfg: RemoteSkillSourceConfig,
    http: SharedHttpClient,
    cache: Mutex<HashMap<String, CacheEntry>>,
}

impl SkillsShProvider {
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

    fn build_search_url(&self, query: &str, limit: usize) -> String {
        let base = self.cfg.url.trim_end_matches('/');
        let encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        format!("{base}{}", self.cfg.search_template)
            .replace("{{query}}", &encoded)
            .replace("{{limit}}", &limit.to_string())
    }

    fn map_entry(&self, item: SkillsShItem) -> SkillCatalogEntry {
        SkillCatalogEntry {
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
            package_url: None,
        }
    }

    async fn fetch_search(
        &self,
        keyword: &str,
        limit: usize,
    ) -> Result<Vec<SkillCatalogEntry>, SkillCatalogError> {
        let url = self.build_search_url(keyword, limit);

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
            .map(|item| self.map_entry(item))
            .collect();

        self.cache.lock().await.insert(
            format!("search:{keyword}"),
            CacheEntry {
                entries: entries.clone(),
                fetched_at: Instant::now(),
            },
        );

        Ok(entries)
    }

    async fn cached_search(
        &self,
        keyword: &str,
        limit: usize,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let cache_key = format!("search:{keyword}");
        {
            let cache = self.cache.lock().await;
            if let Some(entry) = cache.get(&cache_key) {
                if entry.fetched_at.elapsed().as_secs() < self.ttl() {
                    return Ok(entry.entries.clone());
                }
            }
        }

        match self.fetch_search(keyword, limit).await {
            Ok(entries) => Ok(entries),
            Err(e) => {
                if let Some(entry) = self.cache.lock().await.get(&cache_key) {
                    tracing::warn!(error=%e, "skills.sh refetch failed, serving stale");
                    return Ok(entry.entries.clone());
                }
                Err(e)
            }
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
        self.cache.lock().await.clear();
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
        let provider = SkillsShProvider::new(cfg, http);
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
