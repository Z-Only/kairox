//! Remote skill catalog source configuration and provider construction.

use crate::catalog::remote::http_client::SharedHttpClient;
use crate::catalog::skills::SkillCatalogProvider;
use std::str::FromStr;
use std::sync::Arc;

/// Which adapter implementation backs a skill catalog source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSourceKind {
    /// SkillHub (`https://api.skillhub.cn/api/skills`)
    SkillHub,
}

impl FromStr for SkillSourceKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "skillhub" => Ok(Self::SkillHub),
            _ => Err(()),
        }
    }
}

impl SkillSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SkillHub => "skillhub",
        }
    }
}

/// A single remote skill catalog source configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSkillSourceConfig {
    pub id: String,
    pub display_name: String,
    pub kind: SkillSourceKind,
    pub url: String,
    /// URL template for search, e.g. `/api/search?q={{query}}&limit={{limit}}`
    pub search_template: String,
    /// URL template for package download, e.g. `/api/v1/download?slug={{slug}}`.
    pub download_template: String,
    /// URL template for list, e.g. `/api/skills?limit={{limit}}`. None if not supported.
    pub list_template: Option<String>,
    /// URL template for detail, e.g. `/api/v1/skills/{{slug}}`. None if not supported.
    pub detail_template: Option<String>,
    pub enabled: bool,
    pub priority: u32,
    pub cache_ttl_seconds: u64,
}

/// Construct the right [`SkillCatalogProvider`] implementation.
pub fn build_skill_provider(
    cfg: RemoteSkillSourceConfig,
    http: SharedHttpClient,
) -> Arc<dyn SkillCatalogProvider> {
    match cfg.kind {
        SkillSourceKind::SkillHub => Arc::new(
            crate::catalog::skills::skillhub::SkillHubProvider::new(cfg, http),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_source_kind_from_str_round_trip() {
        assert_eq!(
            SkillSourceKind::from_str("skillhub"),
            Ok(SkillSourceKind::SkillHub)
        );
        assert_eq!(SkillSourceKind::from_str("unknown"), Err(()));
    }

    #[test]
    fn skill_source_kind_as_str() {
        assert_eq!(SkillSourceKind::SkillHub.as_str(), "skillhub");
    }

    #[test]
    fn build_skill_provider_returns_correct_kind() {
        let http = SharedHttpClient::new().unwrap();
        let provider = build_skill_provider(
            RemoteSkillSourceConfig {
                id: "skillhub".into(),
                display_name: "SkillHub".into(),
                kind: SkillSourceKind::SkillHub,
                url: "https://api.skillhub.cn".into(),
                search_template: "/api/skills?keyword={{query}}&pageSize={{limit}}".into(),
                download_template: "/api/v1/download?slug={{slug}}".into(),
                list_template: Some("/api/skills?pageSize={{limit}}".into()),
                detail_template: Some("/api/v1/skills/{{slug}}".into()),
                enabled: true,
                priority: 20,
                cache_ttl_seconds: 900,
            },
            http,
        );
        assert_eq!(provider.source_id(), "skillhub");
    }
}
