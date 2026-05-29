use super::*;
use crate::catalog::skills::remote::SkillSourceKind;

fn test_cfg() -> RemoteSkillSourceConfig {
    RemoteSkillSourceConfig {
        id: "skillhub".into(),
        display_name: "SkillHub".into(),
        kind: SkillSourceKind::SkillHub,
        url: "https://api.skillhub.cn".into(),
        download_template: "/api/v1/download?slug={{slug}}".into(),
        search_template:
            "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc"
                .into(),
        list_template: Some(
            "/api/skills?page=1&pageSize={{limit}}&sortBy=downloads&order=desc".into(),
        ),
        detail_template: Some("/api/v1/skills/{{slug}}".into()),
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
    assert!(url.contains("keyword=code+review"));
    assert!(url.contains("pageSize=10"));
    assert!(url.starts_with("https://api.skillhub.cn"));
}

#[test]
fn build_list_url_without_keyword() {
    let http = SharedHttpClient::new().unwrap();
    let provider = SkillHubProvider::new(test_cfg(), http);
    let url = provider.build_url(None, 20);
    assert!(!url.contains("q="));
    assert!(url.contains("pageSize=20"));
    assert!(url.starts_with("https://api.skillhub.cn"));
}

#[test]
fn skillhub_response_parses_correctly() {
    let json = r#"{"code":0,"data":{"skills":[{"slug":"test-skill","name":"test-skill","description_zh":"A test skill","stars":100,"downloads":50,"securityScore":95,"rating":4.5}]}}"#;
    let parsed: SkillHubResponse = serde_json::from_str(json).unwrap();
    let skills = parsed.data.unwrap().skills;
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "test-skill");
    assert_eq!(skills[0].description_zh.as_deref(), Some("A test skill"));
    assert_eq!(skills[0].stars, Some(100));
    assert_eq!(skills[0].downloads, Some(50));
}
