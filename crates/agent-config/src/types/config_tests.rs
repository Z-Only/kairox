use super::*;

fn temp_config_path(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "kairox-agent-config-{}-{nanos}-{name}",
        std::process::id()
    ))
}

#[test]
fn merge_config_preserves_profile_order_and_appends_new_profiles() {
    let mut base = crate::loader::load_from_str(
        r#"
[profiles.base-alpha]
provider = "fake"
model_id = "fake"

[profiles.shared]
provider = "fake"
model_id = "old-shared"

[profiles.base-beta]
provider = "fake"
model_id = "fake"
"#,
        "base.toml",
    )
    .expect("base config parses");
    let alpha = base.get_profile("base-alpha").expect("base-alpha").clone();
    let shared = base.get_profile("shared").expect("shared").clone();
    let beta = base.get_profile("base-beta").expect("base-beta").clone();
    base.profiles = vec![
        ("base-alpha".into(), alpha),
        ("shared".into(), shared),
        ("base-beta".into(), beta),
    ];
    let overlay_path = temp_config_path("overlay.toml");
    std::fs::write(
        &overlay_path,
        r#"
[profiles.shared]
provider = "fake"
model_id = "new-shared"

[profiles.overlay-new]
provider = "fake"
model_id = "fake"
"#,
    )
    .expect("write overlay config");

    let merged =
        Config::merge_config(base, &overlay_path, ConfigSource::UserFile).expect("merge config");
    let _ = std::fs::remove_file(&overlay_path);

    let names = merged.profile_names();
    assert_eq!(
        names,
        vec!["base-alpha", "shared", "base-beta", "overlay-new"]
    );
    assert_eq!(
        merged
            .get_profile("shared")
            .expect("shared profile is present")
            .model_id,
        "new-shared"
    );
}

#[test]
fn merge_config_replaces_and_appends_knowledge_bases() {
    let mut base = Config::defaults();
    base.knowledge_bases = vec![
        (
            "shared".into(),
            crate::KnowledgeBaseConfig {
                kind: crate::KnowledgeBaseKind::SqliteFts,
                path: Some("old.sqlite".into()),
                profile_aliases: vec!["fast".into()],
                ..crate::KnowledgeBaseConfig::default()
            },
        ),
        (
            "base-only".into(),
            crate::KnowledgeBaseConfig {
                kind: crate::KnowledgeBaseKind::Weaviate,
                endpoint: Some("https://weaviate.example.com".into()),
                ..crate::KnowledgeBaseConfig::default()
            },
        ),
    ];
    let overlay_path = temp_config_path("kb-overlay.toml");
    std::fs::write(
        &overlay_path,
        r#"
[knowledge_bases.shared]
kind = "sqlite_fts"
path = "new.sqlite"
profile_aliases = ["wide"]

[knowledge_bases.overlay-only]
kind = "pinecone"
endpoint = "https://pinecone.example.com"
index_name = "support"
"#,
    )
    .expect("write overlay config");

    let merged =
        Config::merge_config(base, &overlay_path, ConfigSource::ProjectFile).expect("merge config");
    let _ = std::fs::remove_file(&overlay_path);

    let ids: Vec<_> = merged
        .knowledge_bases
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(ids, vec!["base-only", "overlay-only", "shared"]);
    let shared = merged
        .knowledge_bases
        .iter()
        .find(|(id, _)| id == "shared")
        .map(|(_, kb)| kb)
        .expect("shared KB is present");
    assert_eq!(shared.path.as_deref(), Some("new.sqlite"));
    assert_eq!(shared.profile_aliases, vec!["wide"]);
}
