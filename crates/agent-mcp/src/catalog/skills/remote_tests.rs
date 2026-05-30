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
