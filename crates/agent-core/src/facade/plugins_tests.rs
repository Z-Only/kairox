use super::*;

// ── DTO serde tests ──────────────────────────────────────────────────────

#[test]
fn plugin_install_target_serializes_as_snake_case() {
    assert_eq!(
        serde_json::to_value(PluginInstallTarget::User).unwrap(),
        serde_json::json!("user")
    );
    assert_eq!(
        serde_json::to_value(PluginInstallTarget::Project).unwrap(),
        serde_json::json!("project")
    );
}

#[test]
fn plugin_install_target_roundtrip() {
    let json = serde_json::to_string(&PluginInstallTarget::Project).unwrap();
    let decoded: PluginInstallTarget = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, PluginInstallTarget::Project);
}

#[test]
fn plugin_component_inventory_view_roundtrip() {
    let inv = PluginComponentInventoryView {
        skill_count: 3,
        skill_names: vec!["review".into(), "test".into(), "deploy".into()],
        mcp_server_count: 1,
        app_count: 0,
        agent_count: 2,
        hook_count: 1,
    };
    let json = serde_json::to_string(&inv).unwrap();
    let decoded: PluginComponentInventoryView = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, inv);
}

#[test]
fn plugin_security_metadata_view_roundtrip() {
    let sec = PluginSecurityMetadataView {
        publisher: Some("acme".into()),
        trust: Some("verified".into()),
        signature: Some("abc123".into()),
        checksum: None,
        sha256: Some("deadbeef".into()),
    };
    let json = serde_json::to_string(&sec).unwrap();
    let decoded: PluginSecurityMetadataView = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, sec);
}

#[test]
fn plugin_security_metadata_all_none() {
    let sec = PluginSecurityMetadataView {
        publisher: None,
        trust: None,
        signature: None,
        checksum: None,
        sha256: None,
    };
    let value: serde_json::Value = serde_json::to_value(&sec).unwrap();
    assert!(value["publisher"].is_null());
    assert!(value["trust"].is_null());
}

fn make_plugin_settings_view() -> PluginSettingsView {
    PluginSettingsView {
        settings_id: "user:my-plugin".into(),
        id: "my-plugin".into(),
        name: "My Plugin".into(),
        description: "A test plugin".into(),
        version: Some("0.1.0".into()),
        scope: crate::config_scope::ConfigScope::User,
        path: "/home/user/.kairox/plugins/my-plugin".into(),
        enabled: true,
        install_source: Some("marketplace".into()),
        marketplace: Some("official".into()),
        effective: true,
        shadowed_by: None,
        valid: true,
        validation_error: None,
        inventory: PluginComponentInventoryView {
            skill_count: 1,
            skill_names: vec!["lint".into()],
            mcp_server_count: 0,
            app_count: 0,
            agent_count: 0,
            hook_count: 0,
        },
        manifest_kind: "plugin".into(),
        security: PluginSecurityMetadataView {
            publisher: Some("acme".into()),
            trust: Some("verified".into()),
            signature: None,
            checksum: None,
            sha256: None,
        },
    }
}

#[test]
fn plugin_settings_view_roundtrip() {
    let view = make_plugin_settings_view();
    let json = serde_json::to_string(&view).unwrap();
    let decoded: PluginSettingsView = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, view);
}

#[test]
fn plugin_settings_view_key_fields_present() {
    let view = make_plugin_settings_view();
    let value: serde_json::Value = serde_json::to_value(&view).unwrap();
    assert_eq!(value["settings_id"], "user:my-plugin");
    assert_eq!(value["enabled"], true);
    assert_eq!(value["manifest_kind"], "plugin");
}

#[test]
fn plugin_detail_view_roundtrip() {
    let detail = PluginDetailView {
        view: make_plugin_settings_view(),
        manifest_path: "/home/user/.kairox/plugins/my-plugin/plugin.toml".into(),
        homepage: Some("https://example.com".into()),
        repository: Some("https://github.com/example/plugin".into()),
        license: Some("MIT".into()),
        keywords: vec!["lint".into(), "code-quality".into()],
    };
    let json = serde_json::to_string(&detail).unwrap();
    let decoded: PluginDetailView = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, detail);
}

#[test]
fn plugin_detail_view_optional_fields() {
    let detail = PluginDetailView {
        view: make_plugin_settings_view(),
        manifest_path: "/path/to/manifest".into(),
        homepage: None,
        repository: None,
        license: None,
        keywords: vec![],
    };
    let value: serde_json::Value = serde_json::to_value(&detail).unwrap();
    assert!(value["homepage"].is_null());
    assert!(value["repository"].is_null());
    assert!(value["license"].is_null());
    assert_eq!(value["keywords"], serde_json::json!([]));
}

#[test]
fn plugin_marketplace_source_view_roundtrip() {
    let source = PluginMarketplaceSourceView {
        id: "official".into(),
        display_name: "Official Marketplace".into(),
        source: "https://marketplace.example.com".into(),
        enabled: true,
        builtin: true,
    };
    let json = serde_json::to_string(&source).unwrap();
    let decoded: PluginMarketplaceSourceView = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, source);
}

#[test]
fn plugin_catalog_entry_roundtrip() {
    let entry = PluginCatalogEntry {
        marketplace_id: "official".into(),
        name: "lint-plugin".into(),
        description: "Linting plugin".into(),
        version: Some("1.2.0".into()),
        source: "https://marketplace.example.com".into(),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: PluginCatalogEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, entry);
}

#[test]
fn install_plugin_request_roundtrip() {
    let req = InstallPluginRequest {
        marketplace_id: "official".into(),
        plugin_name: "lint-plugin".into(),
        target: PluginInstallTarget::User,
    };
    let json = serde_json::to_string(&req).unwrap();
    let decoded: InstallPluginRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, req);
}

#[test]
fn install_plugin_request_target_serializes_correctly() {
    let req = InstallPluginRequest {
        marketplace_id: "m".into(),
        plugin_name: "p".into(),
        target: PluginInstallTarget::Project,
    };
    let value: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert_eq!(value["target"], "project");
}

// ── PluginsFacade trait default tests ────────────────────────────────────

struct BarePluginsFacade;

#[async_trait::async_trait]
impl PluginsFacade for BarePluginsFacade {}

#[tokio::test]
async fn list_plugin_settings_returns_empty() {
    let facade = BarePluginsFacade;
    let result = facade.list_plugin_settings().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn get_plugin_detail_returns_none() {
    let facade = BarePluginsFacade;
    let result = facade.get_plugin_detail("test".into()).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn set_plugin_enabled_returns_error() {
    let facade = BarePluginsFacade;
    let err = facade
        .set_plugin_enabled("test".into(), true)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not configured"));
}

#[tokio::test]
async fn delete_plugin_settings_returns_error() {
    let facade = BarePluginsFacade;
    let err = facade
        .delete_plugin_settings("test".into())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not configured"));
}

#[tokio::test]
async fn list_plugin_marketplace_sources_returns_empty() {
    let facade = BarePluginsFacade;
    let result = facade.list_plugin_marketplace_sources().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn set_plugin_marketplace_source_enabled_returns_error() {
    let facade = BarePluginsFacade;
    let err = facade
        .set_plugin_marketplace_source_enabled("src".into(), true)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not configured"));
}

#[tokio::test]
async fn list_plugin_catalog_returns_empty() {
    let facade = BarePluginsFacade;
    let result = facade.list_plugin_catalog(None, None).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn install_plugin_returns_error() {
    let facade = BarePluginsFacade;
    let req = InstallPluginRequest {
        marketplace_id: "m".into(),
        plugin_name: "p".into(),
        target: PluginInstallTarget::User,
    };
    let err = facade.install_plugin(req).await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}
