use super::*;

fn sample_source(id: &str) -> CatalogSourceConfig {
    CatalogSourceConfig {
        id: id.into(),
        display_name: format!("Display {id}"),
        kind: CatalogSourceKind::McpRegistry,
        url: "https://registry.modelcontextprotocol.io".into(),
        api_key_env: None,
        priority: 100,
        default_trust: "community".into(),
        enabled: true,
        cache_ttl_seconds: None,
    }
}

#[test]
fn read_sources_returns_empty_when_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let mt = MarketplaceToml::new(tmp.path());
    assert!(mt.read_sources().unwrap().is_empty());
}

#[test]
fn add_then_read_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let mt = MarketplaceToml::new(tmp.path());
    mt.add_source(sample_source("mcp-registry")).unwrap();
    let got = mt.read_sources().unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, "mcp-registry");
    assert_eq!(got[0].priority, 100);
}

#[test]
fn add_duplicate_id_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let mt = MarketplaceToml::new(tmp.path());
    mt.add_source(sample_source("a")).unwrap();
    let err = mt.add_source(sample_source("a")).unwrap_err();
    assert!(matches!(err, MarketplaceTomlError::AlreadyExists(_)));
}

#[test]
fn remove_existing_then_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let mt = MarketplaceToml::new(tmp.path());
    mt.add_source(sample_source("a")).unwrap();
    mt.remove_source("a").unwrap();
    assert!(mt.read_sources().unwrap().is_empty());
    let err = mt.remove_source("a").unwrap_err();
    assert!(matches!(err, MarketplaceTomlError::NotFound(_)));
}

#[test]
fn set_enabled_toggles_field() {
    let tmp = tempfile::tempdir().unwrap();
    let mt = MarketplaceToml::new(tmp.path());
    mt.add_source(sample_source("a")).unwrap();
    mt.set_enabled("a", false).unwrap();
    let got = mt.read_sources().unwrap();
    assert!(!got[0].enabled);
    mt.set_enabled("a", true).unwrap();
    assert!(mt.read_sources().unwrap()[0].enabled);
}

#[test]
fn mutations_preserve_other_top_level_tables() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    std::fs::write(
        &path,
        r#"
[mcp_servers.fs]
type = "stdio"
command = "fs-server"
args = []
"#,
    )
    .unwrap();
    let mt = MarketplaceToml::new(tmp.path());
    mt.add_source(sample_source("mcp-registry")).unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    // mcp_servers.fs must survive verbatim.
    assert!(raw.contains("[mcp_servers.fs]"));
    assert!(raw.contains("command = \"fs-server\""));
    // catalog_sources entry must be present.
    assert!(raw.contains("[[catalog_sources]]"));
    assert!(raw.contains("id = \"mcp-registry\""));
}

#[test]
fn write_then_remove_strips_array_when_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let mt = MarketplaceToml::new(tmp.path());
    mt.add_source(sample_source("a")).unwrap();
    mt.remove_source("a").unwrap();
    let raw = std::fs::read_to_string(mt.path()).unwrap();
    assert!(!raw.contains("catalog_sources"));
}
