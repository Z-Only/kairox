use super::*;

#[test]
fn set_enabled_seeds_default_source() {
    let dir = tempfile::tempdir().expect("dir");
    let toml = PluginSourcesToml::new(dir.path());

    assert!(toml.set_enabled("claude-plugins-official", false).unwrap());
    let sources = toml.merged_sources();
    let source = sources
        .iter()
        .find(|source| source.id == "claude-plugins-official")
        .unwrap();
    assert!(!source.enabled);
    assert!(!source.builtin);
}

#[test]
fn read_write_round_trips_user_source() {
    let dir = tempfile::tempdir().expect("dir");
    let toml = PluginSourcesToml::new(dir.path());
    std::fs::write(
        dir.path().join("config.toml"),
        "[profiles.fake]\nmodel = \"fake\"\n",
    )
    .expect("config");
    toml.write(&[PluginMarketplaceSourceView {
        id: "local".into(),
        display_name: "Local".into(),
        source: "/tmp/plugins".into(),
        enabled: true,
        builtin: false,
    }])
    .unwrap();

    assert_eq!(toml.read()[0].id, "local");
    let raw = std::fs::read_to_string(dir.path().join("config.toml")).expect("config");
    assert!(raw.contains("[profiles.fake]"));
}
