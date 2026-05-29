use super::*;

#[test]
fn parses_catalog_sources_with_defaults() {
    let market = r#"
[[catalog_sources]]
id           = "mcp-registry"
display_name = "Model Context Protocol Servers"
kind         = "mcp_registry"
url          = "https://registry.modelcontextprotocol.io"
"#;
    let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
    assert_eq!(loaded.catalog_sources.len(), 1);
    let s = &loaded.catalog_sources[0];
    assert_eq!(s.id, "mcp-registry");
    assert_eq!(s.priority, 100);
    assert!(s.enabled);
    assert_eq!(s.default_trust, "community");
}

#[test]
fn parses_multiple_sources_with_full_fields() {
    let market = r#"
[[catalog_sources]]
id            = "internal"
display_name  = "Internal"
kind          = "mcp_registry"
url           = "https://mcp.example.com/v0.1/servers"
api_key_env   = "INTERNAL_KEY"
priority      = 10
default_trust = "verified"
enabled       = true
cache_ttl_seconds = 600

[[catalog_sources]]
id           = "mcp-registry"
display_name = "Model Context Protocol Servers"
kind         = "mcp_registry"
url          = "https://registry.modelcontextprotocol.io"
priority     = 50
enabled      = false
"#;
    let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
    assert_eq!(loaded.catalog_sources.len(), 2);
    let internal = loaded
        .catalog_sources
        .iter()
        .find(|s| s.id == "internal")
        .unwrap();
    assert_eq!(internal.priority, 10);
    assert_eq!(internal.api_key_env.as_deref(), Some("INTERNAL_KEY"));
    assert_eq!(internal.cache_ttl_seconds, Some(600));
    let mcp = loaded
        .catalog_sources
        .iter()
        .find(|s| s.id == "mcp-registry")
        .unwrap();
    assert!(!mcp.enabled);
}

#[test]
fn unknown_kind_is_silently_skipped() {
    let market = r#"
[[catalog_sources]]
id           = "x"
display_name = "X"
kind         = "wat"
url          = "https://x"
"#;
    let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
    assert!(
        !loaded.catalog_sources.iter().any(|s| s.id == "x"),
        "unsupported kind should be silently dropped"
    );
}

#[test]
fn rejects_invalid_url_scheme() {
    let market = r#"
[[catalog_sources]]
id           = "x"
display_name = "X"
kind         = "mcp_registry"
url          = "ftp://x"
"#;
    let err = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap_err();
    assert!(format!("{err:?}").to_lowercase().contains("url"));
}

#[test]
fn missing_marketplace_yields_empty_sources() {
    let loaded = load_with_marketplace_loaded("", None, "k.toml", "m.toml").unwrap();
    assert!(loaded.catalog_sources.is_empty());
}

#[test]
fn catalog_source_loader_ignores_mcp_server_sections() {
    let market = r#"
[mcp_servers.foo]
type      = "stdio"
command   = "echo"
args      = []
"#;
    let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
    assert!(loaded.config.mcp_servers.is_empty());
    assert!(loaded.catalog_sources.is_empty());
}

#[test]
fn defaults_ship_disabled_remote_sources() {
    let defaults = default_catalog_sources();
    let ids: Vec<&str> = defaults.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"mcp-registry"));
    assert_eq!(defaults.len(), 1);
    assert!(
        defaults.iter().all(|s| !s.enabled),
        "defaults must ship disabled so GUI does not auto-fetch on cold start",
    );
    assert!(
        defaults
            .iter()
            .all(|s| s.url.starts_with("https://") || s.url.starts_with("http://")),
        "default urls must be well-formed http(s)",
    );
    let mcp = defaults.iter().find(|s| s.id == "mcp-registry").unwrap();
    assert_eq!(mcp.kind, CatalogSourceKind::McpRegistry);
    assert_eq!(mcp.url, "https://registry.modelcontextprotocol.io");
}

#[test]
fn merge_with_defaults_seeds_all_when_user_empty() {
    let merged = merge_with_defaults(Vec::new());
    let ids: Vec<&str> = merged.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"mcp-registry"));
    assert_eq!(merged.len(), 1);
}

#[test]
fn merge_with_defaults_user_overrides_default_by_id() {
    let user = vec![CatalogSourceConfig {
        id: "mcp-registry".into(),
        display_name: "My MCP Mirror".into(),
        kind: CatalogSourceKind::McpRegistry,
        url: "https://my-mirror.example/v0.1/servers".into(),
        api_key_env: Some("MY_KEY".into()),
        priority: 10,
        default_trust: "verified".into(),
        enabled: true,
        cache_ttl_seconds: Some(120),
    }];
    let merged = merge_with_defaults(user);
    let mcp: Vec<_> = merged.iter().filter(|s| s.id == "mcp-registry").collect();
    assert_eq!(mcp.len(), 1);
    assert_eq!(mcp[0].display_name, "My MCP Mirror");
    assert!(mcp[0].enabled);
    assert_eq!(mcp[0].priority, 10);
    assert_eq!(merged.len(), 1);
}

#[test]
fn merge_with_defaults_preserves_user_ordering_then_appends_defaults() {
    let user = vec![CatalogSourceConfig {
        id: "custom-first".into(),
        display_name: "Custom".into(),
        kind: CatalogSourceKind::McpRegistry,
        url: "https://custom.example/v0.1/servers".into(),
        api_key_env: None,
        priority: 5,
        default_trust: "community".into(),
        enabled: true,
        cache_ttl_seconds: None,
    }];
    let merged = merge_with_defaults(user);
    assert_eq!(merged[0].id, "custom-first");
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[1].id, "mcp-registry");
}
