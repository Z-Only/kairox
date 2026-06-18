use super::*;
use crate::facade::McpServerSettingsTransport;

/// A bare struct that implements `McpFacade` with no overrides,
/// exercising every default method.
struct BareMcpFacade;

#[async_trait::async_trait]
impl McpFacade for BareMcpFacade {}

#[tokio::test]
async fn list_mcp_server_settings_returns_empty() {
    let facade = BareMcpFacade;
    let result = facade.list_mcp_server_settings(None).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_mcp_server_settings_for_project_delegates() {
    let facade = BareMcpFacade;
    let result = facade
        .list_mcp_server_settings_for_project(None, Some("/tmp".into()))
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn upsert_mcp_server_settings_returns_error() {
    let facade = BareMcpFacade;
    let input = McpServerSettingsInput {
        name: "test".into(),
        transport: McpServerSettingsTransport::Stdio {
            command: "echo".into(),
            args: vec![],
            env: std::collections::BTreeMap::new(),
        },
        enabled: true,
        description: None,
    };
    let err = facade.upsert_mcp_server_settings(input).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not supported"), "got: {msg}");
}

#[tokio::test]
async fn delete_mcp_server_settings_returns_error() {
    let facade = BareMcpFacade;
    let err = facade
        .delete_mcp_server_settings("server-1".into())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn set_mcp_server_enabled_returns_error() {
    let facade = BareMcpFacade;
    let err = facade
        .set_mcp_server_enabled("server-1".into(), true)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn open_mcp_config_file_returns_none() {
    let facade = BareMcpFacade;
    let result = facade.open_mcp_config_file().await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn list_profile_settings_returns_empty() {
    let facade = BareMcpFacade;
    let result = facade.list_profile_settings(None).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_profile_settings_for_project_delegates() {
    let facade = BareMcpFacade;
    let result = facade
        .list_profile_settings_for_project(None, Some("/workspace".into()))
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn upsert_profile_settings_returns_error() {
    let facade = BareMcpFacade;
    let input = ProfileSettingsInput {
        alias: "default".into(),
        provider: "openai".into(),
        model_id: "gpt-4".into(),
        enabled: true,
        context_window: None,
        output_limit: None,
        temperature: None,
        top_p: None,
        top_k: None,
        max_tokens: None,
        base_url: None,
        api_key: None,
        api_key_env: None,
        client_identity: None,
        supports_reasoning: None,
    };
    let err = facade.upsert_profile_settings(input).await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn set_profile_enabled_returns_error() {
    let facade = BareMcpFacade;
    let err = facade
        .set_profile_enabled("default".into(), false)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn delete_profile_settings_returns_error() {
    let facade = BareMcpFacade;
    let err = facade
        .delete_profile_settings("default".into())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn move_profile_in_order_returns_error() {
    let facade = BareMcpFacade;
    let err = facade
        .move_profile_in_order("default".into(), 1)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn open_config_dir_returns_none() {
    let facade = BareMcpFacade;
    assert_eq!(facade.open_config_dir().await.unwrap(), None);
}

#[tokio::test]
async fn open_profiles_config_file_returns_none() {
    let facade = BareMcpFacade;
    assert_eq!(facade.open_profiles_config_file().await.unwrap(), None);
}

#[tokio::test]
async fn list_catalog_returns_empty() {
    let facade = BareMcpFacade;
    let result = facade.list_catalog(CatalogQuery::default()).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn get_catalog_entry_returns_none() {
    let facade = BareMcpFacade;
    let result = facade.get_catalog_entry("test".into(), None).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn refresh_catalog_succeeds() {
    let facade = BareMcpFacade;
    facade.refresh_catalog(None).await.unwrap();
}

#[tokio::test]
async fn install_catalog_entry_returns_runtime_missing() {
    let facade = BareMcpFacade;
    let req = InstallRequest {
        catalog_id: "test".into(),
        source: "builtin".into(),
        server_id_override: None,
        env_overrides: std::collections::BTreeMap::new(),
        trust_grant: false,
        auto_start: false,
    };
    let outcome = facade.install_catalog_entry(req).await.unwrap();
    assert_eq!(outcome.kind, "runtime_missing");
    assert_eq!(outcome.server_id, None);
}

#[tokio::test]
async fn uninstall_catalog_entry_succeeds() {
    let facade = BareMcpFacade;
    facade.uninstall_catalog_entry("test".into()).await.unwrap();
}

#[tokio::test]
async fn list_installed_entries_returns_empty() {
    let facade = BareMcpFacade;
    let result = facade.list_installed_entries().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_catalog_sources_returns_builtin() {
    let facade = BareMcpFacade;
    let sources = facade.list_catalog_sources().await.unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, "builtin");
    assert_eq!(sources[0].kind, "builtin");
    assert_eq!(sources[0].default_trust, "verified");
    assert!(sources[0].enabled);
}

#[tokio::test]
async fn add_catalog_source_succeeds() {
    let facade = BareMcpFacade;
    let req = AddCatalogSourceRequest {
        id: "new".into(),
        display_name: "New".into(),
        kind: "mcp_registry".into(),
        url: "https://example.com".into(),
        api_key_env: None,
        priority: None,
        default_trust: None,
        enabled: None,
        cache_ttl_seconds: None,
    };
    facade.add_catalog_source(req).await.unwrap();
}

#[tokio::test]
async fn remove_catalog_source_succeeds() {
    let facade = BareMcpFacade;
    facade.remove_catalog_source("test".into()).await.unwrap();
}

#[tokio::test]
async fn set_catalog_source_enabled_succeeds() {
    let facade = BareMcpFacade;
    facade
        .set_catalog_source_enabled("test".into(), false)
        .await
        .unwrap();
}
