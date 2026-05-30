use super::*;

#[test]
fn read_empty_when_file_does_not_exist() {
    let dir = tempfile::tempdir().unwrap();
    let toml = SkillSourcesToml::new(dir.path());
    assert!(toml.read().is_empty());
}

#[test]
fn write_and_read_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let toml = SkillSourcesToml::new(dir.path());
    let sources = default_skill_sources();
    toml.write(&sources).unwrap();
    let read_back = toml.read();
    assert_eq!(read_back.len(), 1);
    assert_eq!(read_back[0].id, "skillhub");
}

#[test]
fn merge_user_wins_over_default() {
    let dir = tempfile::tempdir().unwrap();
    let toml = SkillSourcesToml::new(dir.path());
    let user = vec![SkillSourceView {
        id: "skillhub".into(),
        display_name: "Custom SkillHub".into(),
        kind: "skillhub".into(),
        url: "https://custom.example".into(),
        search_template: "/api/skills?q={{query}}".into(),
        download_template: "/api/download?slug={{slug}}".into(),
        list_template: None,
        detail_template: None,
        field_mapping: SkillFieldMappingView::default(),
        enabled: false,
        priority: 0,
        cache_ttl_seconds: 600,
        last_error: None,
    }];
    let merged = toml.merge_with_defaults(&user);
    let hub = merged.iter().find(|s| s.id == "skillhub").unwrap();
    assert_eq!(hub.display_name, "Custom SkillHub");
    assert!(!hub.enabled);
    assert_eq!(merged.len(), 1);
}

#[test]
fn merge_migrates_legacy_default_skillhub_source() {
    let dir = tempfile::tempdir().unwrap();
    let toml = SkillSourcesToml::new(dir.path());
    let mut legacy = default_skill_sources()[0].clone();
    legacy.url = "https://skills.palebluedot.live".into();
    legacy.search_template = "/api/skills?q={{query}}&limit={{limit}}".into();
    legacy.list_template = Some("/api/skills?limit={{limit}}".into());
    legacy.enabled = false;

    let merged = toml.merge_with_defaults(&[legacy]);
    let hub = merged.iter().find(|s| s.id == "skillhub").unwrap();
    assert_eq!(hub.url, "https://api.skillhub.cn");
    assert!(hub.search_template.contains("keyword={{query}}"));
    assert!(!hub.enabled);
    assert_eq!(merged.len(), 1);
}
