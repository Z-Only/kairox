//! Phase 2 runtime wiring tests: facade methods for catalog source
//! mutations round-trip through the on-disk `mcp_servers.toml`.

use agent_core::{AddCatalogSourceRequest, AppFacade};
use agent_runtime::test_support::build_marketplace_runtime;

#[tokio::test]
async fn list_catalog_sources_returns_only_builtin_when_toml_missing() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    let sources = rt.list_catalog_sources().await.unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, "builtin");
    assert_eq!(sources[0].kind, "builtin");
}

#[tokio::test]
async fn add_then_list_then_remove_round_trips() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    let req = AddCatalogSourceRequest {
        id: "smithery".into(),
        display_name: "Smithery".into(),
        kind: "smithery".into(),
        url: "https://registry.smithery.ai".into(),
        api_key_env: None,
        priority: Some(50),
        default_trust: Some("community".into()),
        enabled: Some(true),
        cache_ttl_seconds: None,
    };
    rt.add_catalog_source(req).await.unwrap();

    let sources = rt.list_catalog_sources().await.unwrap();
    assert_eq!(sources.len(), 2, "builtin + smithery");
    assert!(sources
        .iter()
        .any(|s| s.id == "smithery" && s.priority == 50));

    rt.remove_catalog_source("smithery".into()).await.unwrap();
    let after = rt.list_catalog_sources().await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, "builtin");
}

#[tokio::test]
async fn set_catalog_source_enabled_toggles_flag() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    rt.add_catalog_source(AddCatalogSourceRequest {
        id: "internal".into(),
        display_name: "Internal".into(),
        kind: "kairox_json".into(),
        url: "https://mcp.example.com/c.json".into(),
        api_key_env: None,
        priority: Some(10),
        default_trust: Some("verified".into()),
        enabled: Some(true),
        cache_ttl_seconds: None,
    })
    .await
    .unwrap();

    rt.set_catalog_source_enabled("internal".into(), false)
        .await
        .unwrap();
    let sources = rt.list_catalog_sources().await.unwrap();
    let internal = sources.iter().find(|s| s.id == "internal").unwrap();
    assert!(!internal.enabled);

    rt.set_catalog_source_enabled("internal".into(), true)
        .await
        .unwrap();
    let sources = rt.list_catalog_sources().await.unwrap();
    let internal = sources.iter().find(|s| s.id == "internal").unwrap();
    assert!(internal.enabled);
}

#[tokio::test]
async fn add_catalog_source_rejects_invalid_url() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    let err = rt
        .add_catalog_source(AddCatalogSourceRequest {
            id: "bad".into(),
            display_name: "Bad".into(),
            kind: "smithery".into(),
            url: "ftp://nope".into(),
            api_key_env: None,
            priority: None,
            default_trust: None,
            enabled: None,
            cache_ttl_seconds: None,
        })
        .await
        .unwrap_err();
    assert!(format!("{err:?}").to_lowercase().contains("url"));
}

#[tokio::test]
async fn remove_builtin_source_is_noop() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    rt.remove_catalog_source("builtin".into()).await.unwrap();
    let sources = rt.list_catalog_sources().await.unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, "builtin");
}
