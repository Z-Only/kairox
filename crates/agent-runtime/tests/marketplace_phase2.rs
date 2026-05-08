//! Phase 2 runtime wiring tests: facade methods for catalog source
//! mutations round-trip through the on-disk `mcp_servers.toml`.

use agent_core::{AddCatalogSourceRequest, AppFacade};
use agent_runtime::test_support::build_marketplace_runtime;

#[tokio::test]
async fn list_catalog_sources_returns_builtin_plus_defaults_when_toml_missing() {
    // Cold start: no on-disk mcp_servers.toml. We expect the builtin
    // source plus the shipped default remote source (disabled).
    // The dedicated coverage for the default's identity / disabled state
    // lives in `list_seeds_default_remote_source_when_user_config_missing`;
    // here we just pin the builtin's presence and the overall shape.
    let (rt, _tmp) = build_marketplace_runtime().await;
    let sources = rt.list_catalog_sources().await.unwrap();
    let builtin = sources
        .iter()
        .find(|s| s.id == "builtin")
        .expect("builtin source always present");
    assert_eq!(builtin.kind, "builtin");
    // builtin + 1 shipped default
    assert_eq!(sources.len(), 2);
}

#[tokio::test]
async fn add_then_list_then_remove_round_trips() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    let req = AddCatalogSourceRequest {
        id: "mcp-registry".into(),
        display_name: "My MCP Registry".into(),
        kind: "mcp_registry".into(),
        url: "https://registry.modelcontextprotocol.io".into(),
        api_key_env: None,
        priority: Some(50),
        default_trust: Some("community".into()),
        enabled: Some(true),
        cache_ttl_seconds: None,
    };
    rt.add_catalog_source(req).await.unwrap();

    let sources = rt.list_catalog_sources().await.unwrap();
    // builtin + (user-overridden mcp-registry, replacing the default of
    // the same id).
    assert_eq!(sources.len(), 2, "builtin + overridden mcp-registry");
    let mcp_registry = sources
        .iter()
        .find(|s| s.id == "mcp-registry")
        .expect("mcp-registry present");
    assert_eq!(mcp_registry.priority, 50);
    assert_eq!(mcp_registry.display_name, "My MCP Registry");

    rt.remove_catalog_source("mcp-registry".into())
        .await
        .unwrap();
    let after = rt.list_catalog_sources().await.unwrap();
    // Removing the user override does not remove the default of the same
    // id — it re-surfaces from the shipped defaults, but as disabled.
    let mcp_registry_after = after
        .iter()
        .find(|s| s.id == "mcp-registry")
        .expect("mcp-registry default re-surfaces after user entry removed");
    assert!(!mcp_registry_after.enabled);
    assert_eq!(
        mcp_registry_after.display_name,
        "Model Context Protocol Servers"
    );
    // builtin + 1 default
    assert_eq!(after.len(), 2);
}

#[tokio::test]
async fn set_catalog_source_enabled_toggles_flag() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    rt.add_catalog_source(AddCatalogSourceRequest {
        id: "internal".into(),
        display_name: "Internal".into(),
        kind: "mcp_registry".into(),
        url: "https://mcp.example.com/v0.1/servers".into(),
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
            kind: "mcp_registry".into(),
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
    // builtin survives the noop remove; defaults are still listed too.
    assert!(sources.iter().any(|s| s.id == "builtin"));
    assert_eq!(sources.len(), 2, "builtin + 1 shipped default");
}

// ---------------------------------------------------------------------------
// Phase 2.1: shipped default catalog sources are visible out of the box
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_seeds_default_remote_source_when_user_config_missing() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    let sources = rt.list_catalog_sources().await.unwrap();

    // builtin (always) + 1 shipped default
    assert_eq!(sources.len(), 2, "builtin + 1 default remote source");

    let ids: Vec<&str> = sources.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"builtin"));
    assert!(ids.contains(&"mcp-registry"));

    // All shipped defaults must be enabled=false on cold start so the
    // GUI does not auto-fetch from remote URLs without user opt-in.
    for s in sources.iter().filter(|s| s.id != "builtin") {
        assert!(
            !s.enabled,
            "default source {} must ship disabled, got enabled=true",
            s.id
        );
    }
}

#[tokio::test]
async fn user_added_source_overrides_default_by_id() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    rt.add_catalog_source(AddCatalogSourceRequest {
        id: "mcp-registry".into(),
        display_name: "My MCP Registry Mirror".into(),
        kind: "mcp_registry".into(),
        url: "https://my-mirror.example/catalog.json".into(),
        api_key_env: None,
        priority: Some(10),
        default_trust: Some("verified".into()),
        enabled: Some(true),
        cache_ttl_seconds: None,
    })
    .await
    .unwrap();

    let sources = rt.list_catalog_sources().await.unwrap();
    let ids: Vec<&str> = sources.iter().map(|s| s.id.as_str()).collect();

    // No duplication: still exactly one entry per id.
    let mcp_registry_count = ids.iter().filter(|id| **id == "mcp-registry").count();
    assert_eq!(mcp_registry_count, 1, "user override must not duplicate id");

    // The user's values win.
    let mcp_registry = sources.iter().find(|s| s.id == "mcp-registry").unwrap();
    assert_eq!(mcp_registry.display_name, "My MCP Registry Mirror");
    assert!(mcp_registry.enabled);
    assert_eq!(mcp_registry.priority, 10);

    // No other defaults remain (kairox-official was removed).
    // builtin + (1 user-overridden mcp-registry)
    assert_eq!(sources.len(), 2);
}

#[tokio::test]
async fn set_enabled_seeds_default_when_not_yet_in_toml() {
    let (rt, _tmp) = build_marketplace_runtime().await;

    // Sanity: cold start, default present but disabled.
    let before = rt.list_catalog_sources().await.unwrap();
    let mcp_before = before
        .iter()
        .find(|s| s.id == "mcp-registry")
        .expect("mcp-registry seeded as default");
    assert!(!mcp_before.enabled);

    // Toggle a default that has never been written to disk — must not
    // error with NotFound; instead it should be seeded with enabled=true.
    rt.set_catalog_source_enabled("mcp-registry".into(), true)
        .await
        .expect("toggling a shipped default must succeed even when toml has no entry yet");

    let after = rt.list_catalog_sources().await.unwrap();
    let mcp_after = after
        .iter()
        .find(|s| s.id == "mcp-registry")
        .expect("mcp-registry still listed");
    assert!(mcp_after.enabled);
    assert_eq!(mcp_after.url, "https://registry.modelcontextprotocol.io");
}

#[tokio::test]
async fn set_enabled_unknown_id_still_errors() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    let err = rt
        .set_catalog_source_enabled("does-not-exist".into(), true)
        .await
        .unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("not found")
            || format!("{err:?}").to_lowercase().contains("notfound"),
        "unknown id should still report NotFound, got: {err:?}"
    );
}
