use super::*;

#[test]
fn catalog_query_default_is_empty() {
    let q = CatalogQuery::default();
    assert_eq!(q.keyword, None);
    assert_eq!(q.category, None);
    assert_eq!(q.trust_min, None);
    assert_eq!(q.source, None);
    assert_eq!(q.limit, None);
}

#[test]
fn catalog_query_roundtrip() {
    let q = CatalogQuery {
        keyword: Some("filesystem".into()),
        category: Some("tools".into()),
        trust_min: Some("community".into()),
        source: Some("builtin".into()),
        limit: Some(10),
    };
    let json = serde_json::to_string(&q).unwrap();
    let decoded: CatalogQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, q);
}

#[test]
fn catalog_query_optional_fields_omitted_with_skip() {
    let q = CatalogQuery::default();
    let json = serde_json::to_string(&q).unwrap();
    // All fields are Option but don't use skip_serializing_if, so they serialize as null.
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("keyword").is_some());
    assert!(value["keyword"].is_null());
}

#[test]
fn server_entry_roundtrip() {
    let entry = ServerEntry {
        id: "fs-server".into(),
        source: "builtin".into(),
        display_name: "Filesystem".into(),
        summary: "Access local files".into(),
        description: "Provides file system access".into(),
        categories: vec!["tools".into()],
        tags: vec!["fs".into(), "local".into()],
        author: Some("kairox".into()),
        homepage: Some("https://example.com".into()),
        version: Some("1.0.0".into()),
        trust: "verified".into(),
        verified: true,
        icon: None,
        install_spec_json: "{}".into(),
        requirements_json: "[]".into(),
        default_env_json: "[]".into(),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: ServerEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, entry);
}

#[test]
fn server_entry_optional_fields_serialize_as_null() {
    let entry = ServerEntry {
        id: "test".into(),
        source: "builtin".into(),
        display_name: "Test".into(),
        summary: "".into(),
        description: "".into(),
        categories: vec![],
        tags: vec![],
        author: None,
        homepage: None,
        version: None,
        trust: "unverified".into(),
        verified: false,
        icon: None,
        install_spec_json: "{}".into(),
        requirements_json: "[]".into(),
        default_env_json: "[]".into(),
    };
    let value: serde_json::Value = serde_json::to_value(&entry).unwrap();
    assert!(value["author"].is_null());
    assert!(value["homepage"].is_null());
    assert!(value["version"].is_null());
    assert!(value["icon"].is_null());
}

#[test]
fn install_request_roundtrip() {
    let req = InstallRequest {
        catalog_id: "fs-server".into(),
        source: "builtin".into(),
        server_id_override: Some("custom-fs".into()),
        env_overrides: BTreeMap::from([("ROOT".into(), "/tmp".into())]),
        trust_grant: true,
        auto_start: false,
    };
    let json = serde_json::to_string(&req).unwrap();
    let decoded: InstallRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, req);
}

#[test]
fn install_request_empty_env_overrides() {
    let req = InstallRequest {
        catalog_id: "test".into(),
        source: "builtin".into(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: true,
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"env_overrides\":{}"));
    let decoded: InstallRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.env_overrides.len(), 0);
}

#[test]
fn install_outcome_view_roundtrip() {
    let outcome = InstallOutcomeView {
        kind: "installed".into(),
        server_id: Some("fs-server".into()),
        started: Some(true),
        missing_runtimes: vec![],
        missing_env_keys: vec![],
    };
    let json = serde_json::to_string(&outcome).unwrap();
    let decoded: InstallOutcomeView = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, outcome);
}

#[test]
fn install_outcome_view_runtime_missing_variant() {
    let outcome = InstallOutcomeView {
        kind: "runtime_missing".into(),
        server_id: None,
        started: None,
        missing_runtimes: vec!["node".into(), "python".into()],
        missing_env_keys: vec![],
    };
    let json = serde_json::to_string(&outcome).unwrap();
    let decoded: InstallOutcomeView = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.missing_runtimes.len(), 2);
    assert_eq!(decoded.kind, "runtime_missing");
}

#[test]
fn installed_entry_roundtrip() {
    let entry = InstalledEntry {
        server_id: "fs-server".into(),
        catalog_id: Some("fs-catalog".into()),
        source: Some("builtin".into()),
        display_name: "Filesystem".into(),
        installed_at: "2025-01-01T00:00:00Z".into(),
        running: true,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: InstalledEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, entry);
}

#[test]
fn catalog_source_view_roundtrip() {
    let source = CatalogSourceView {
        id: "remote-1".into(),
        display_name: "My Registry".into(),
        kind: "mcp_registry".into(),
        url: "https://registry.example.com".into(),
        api_key_env: Some("REGISTRY_KEY".into()),
        priority: 10,
        default_trust: "community".into(),
        enabled: true,
        cache_ttl_seconds: Some(3600),
        last_error: None,
    };
    let json = serde_json::to_string(&source).unwrap();
    let decoded: CatalogSourceView = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, source);
}

#[test]
fn catalog_source_view_builtin_defaults() {
    let source = CatalogSourceView {
        id: "builtin".into(),
        display_name: "Built-in".into(),
        kind: "builtin".into(),
        url: String::new(),
        api_key_env: None,
        priority: 0,
        default_trust: "verified".into(),
        enabled: true,
        cache_ttl_seconds: None,
        last_error: None,
    };
    let value: serde_json::Value = serde_json::to_value(&source).unwrap();
    assert_eq!(value["url"], "");
    assert!(value["api_key_env"].is_null());
}

#[test]
fn add_catalog_source_request_roundtrip() {
    let req = AddCatalogSourceRequest {
        id: "custom".into(),
        display_name: "Custom Source".into(),
        kind: "mcp_registry".into(),
        url: "https://example.com/api".into(),
        api_key_env: Some("MY_KEY".into()),
        priority: Some(5),
        default_trust: Some("unverified".into()),
        enabled: Some(true),
        cache_ttl_seconds: Some(1800),
    };
    let json = serde_json::to_string(&req).unwrap();
    let decoded: AddCatalogSourceRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, req);
}

#[test]
fn add_catalog_source_request_optional_fields() {
    let req = AddCatalogSourceRequest {
        id: "minimal".into(),
        display_name: "Minimal".into(),
        kind: "mcp_registry".into(),
        url: "https://example.com".into(),
        api_key_env: None,
        priority: None,
        default_trust: None,
        enabled: None,
        cache_ttl_seconds: None,
    };
    let value: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert!(value["api_key_env"].is_null());
    assert!(value["priority"].is_null());
    assert!(value["default_trust"].is_null());
    assert!(value["enabled"].is_null());
    assert!(value["cache_ttl_seconds"].is_null());
}
