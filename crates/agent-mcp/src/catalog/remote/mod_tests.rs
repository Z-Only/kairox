use super::*;

#[test]
fn remote_source_kind_serde_round_trip() {
    let k1 = RemoteSourceKind::McpRegistry;
    let s = serde_json::to_string(&k1).unwrap();
    assert_eq!(s, "\"mcp_registry\"");
    let back: RemoteSourceKind = serde_json::from_str(&s).unwrap();
    assert_eq!(back, k1);
}

#[test]
fn remote_source_config_round_trips_via_json() {
    let cfg = RemoteSourceConfig {
        id: "mcp-registry".into(),
        display_name: "MCP Servers".into(),
        kind: RemoteSourceKind::McpRegistry,
        url: "https://registry.modelcontextprotocol.io".into(),
        api_key_env: None,
        priority: 50,
        default_trust: TrustLevel::Community,
        enabled: true,
        cache_ttl_seconds: Some(600),
    };
    let s = serde_json::to_string(&cfg).unwrap();
    let back: RemoteSourceConfig = serde_json::from_str(&s).unwrap();
    assert_eq!(back, cfg);
}

#[test]
fn remote_error_into_catalog_error_preserves_message() {
    let e = RemoteError::Http("status 503".into());
    let c: CatalogError = e.into();
    match c {
        CatalogError::Provider(msg) => assert!(msg.contains("503")),
        _ => panic!("expected Provider variant"),
    }
}
