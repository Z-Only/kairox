//! Resilience tests for the marketplace surface of `LocalRuntime`.
//!
//! These tests pin down the contract: when a `LocalRuntime` is constructed
//! WITHOUT calling `with_marketplace(...)` (i.e. the user has no
//! `[mcp_marketplace]` section in `kairox.toml` — the GUI cold-start case),
//! read-only catalog operations must degrade to a builtin-only view rather
//! than failing with `"marketplace not configured"`.

use agent_core::{AppFacade, CatalogQuery};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

/// Construct a `LocalRuntime` WITHOUT calling `with_marketplace(...)`.
/// This simulates the GUI cold-start path where `kairox.toml` has no
/// `[mcp_marketplace]` section.
async fn build_runtime_without_marketplace() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    LocalRuntime::new(store, model)
}

#[tokio::test]
async fn list_catalog_sources_returns_builtin_when_marketplace_not_configured() {
    let facade = build_runtime_without_marketplace().await;
    let sources = facade
        .list_catalog_sources()
        .await
        .expect("list_catalog_sources must not error when marketplace is unconfigured");
    assert!(
        sources.iter().any(|s| s.id == "builtin"),
        "built-in source must always be present, got: {sources:?}"
    );
}

#[tokio::test]
async fn list_catalog_returns_builtin_entries_when_marketplace_not_configured() {
    let facade = build_runtime_without_marketplace().await;
    let entries = facade
        .list_catalog(CatalogQuery::default())
        .await
        .expect("list_catalog must not error when marketplace is unconfigured");
    // Built-in catalog ships curated entries; the resilience contract here is
    // just "non-empty" — the exact entry count is locked in
    // `marketplace_integration.rs::lists_all_24_builtin_entries`.
    assert!(
        !entries.is_empty(),
        "built-in entries must be returned when marketplace is unconfigured"
    );
    assert!(
        entries.iter().any(|e| e.source == "builtin"),
        "at least one entry must come from the builtin source"
    );
}

#[tokio::test]
async fn get_catalog_entry_returns_builtin_when_marketplace_not_configured() {
    let facade = build_runtime_without_marketplace().await;
    let entry = facade
        .get_catalog_entry("filesystem".into(), None)
        .await
        .expect("get_catalog_entry must not error when marketplace is unconfigured");
    assert!(
        entry.is_some(),
        "filesystem entry must be reachable from the builtin catalog"
    );
}

#[tokio::test]
async fn refresh_catalog_is_noop_when_marketplace_not_configured() {
    let facade = build_runtime_without_marketplace().await;
    facade
        .refresh_catalog(None)
        .await
        .expect("refresh_catalog must be a noop (Ok) when marketplace is unconfigured");
}

#[tokio::test]
async fn list_installed_entries_returns_empty_when_marketplace_not_configured() {
    let facade = build_runtime_without_marketplace().await;
    let installed = facade
        .list_installed_entries()
        .await
        .expect("list_installed_entries must not error when marketplace is unconfigured");
    assert!(
        installed.is_empty(),
        "no entries can be installed when marketplace is unconfigured"
    );
}
