//! Remote skill catalog source configuration and provider construction.

use crate::catalog::remote::http_client::SharedHttpClient;
use crate::catalog::skills::SkillCatalogProvider;
use std::sync::Arc;

/// Which adapter implementation backs a skill catalog source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSourceKind {
    /// skills.sh (`https://skills.sh/api/search`)
    SkillsSh,
    /// SkillHub (`https://skills.palebluedot.live/api/skills`)
    SkillHub,
}

impl SkillSourceKind {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "skills-sh" | "skills_sh" => Some(Self::SkillsSh),
            "skillhub" => Some(Self::SkillHub),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SkillsSh => "skills-sh",
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
    /// URL template for list, e.g. `/api/skills?limit={{limit}}`. None if not supported.
    pub list_template: Option<String>,
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
        SkillSourceKind::SkillsSh => Arc::new(
            crate::catalog::skills::skills_sh::SkillsShProvider::new(cfg, http),
        ),
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
            SkillSourceKind::from_str("skills-sh"),
            Some(SkillSourceKind::SkillsSh)
        );
        assert_eq!(
            SkillSourceKind::from_str("skillhub"),
            Some(SkillSourceKind::SkillHub)
        );
        assert_eq!(SkillSourceKind::from_str("unknown"), None);
    }

    #[test]
    fn skill_source_kind_as_str() {
        assert_eq!(SkillSourceKind::SkillsSh.as_str(), "skills-sh");
        assert_eq!(SkillSourceKind::SkillHub.as_str(), "skillhub");
    }

    #[test]
    fn build_skill_provider_returns_correct_kind() {
        let http = SharedHttpClient::new().unwrap();
        let provider = build_skill_provider(
            RemoteSkillSourceConfig {
                id: "skills-sh".into(),
                display_name: "skills.sh".into(),
                kind: SkillSourceKind::SkillsSh,
                url: "https://skills.sh".into(),
                search_template: "/api/search?q={{query}}&limit={{limit}}".into(),
                list_template: None,
                enabled: true,
                priority: 10,
                cache_ttl_seconds: 900,
            },
            http,
        );
        assert_eq!(provider.source_id(), "skills-sh");
    }
}
