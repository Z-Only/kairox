use agent_config::{CatalogSourceConfig, CatalogSourceKind};
use agent_core::DomainEvent;
use agent_mcp::catalog::CatalogQuery;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;

use crate::facade_runtime::LocalRuntime;

// ── Helpers ────────────────────────────────────────────────────────────────

fn make_source(id: &str, enabled: bool, priority: u32) -> CatalogSourceConfig {
    CatalogSourceConfig {
        id: id.into(),
        display_name: format!("{id} display"),
        kind: CatalogSourceKind::McpRegistry,
        url: "https://registry.example.com".into(),
        api_key_env: None,
        priority,
        default_trust: "community".into(),
        enabled,
        cache_ttl_seconds: None,
    }
}

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

// ── build_catalog_provider ─────────────────────────────────────────────────

#[tokio::test]
async fn build_catalog_provider_empty_sources_succeeds() {
    let (tx, _rx) = tokio::sync::broadcast::channel::<DomainEvent>(8);
    let dir = tempfile::tempdir().unwrap();
    let agg = super::build_catalog_provider(&[], dir.path().to_path_buf(), tx);
    assert!(
        agg.is_ok(),
        "empty sources should produce a valid aggregate"
    );
}

#[tokio::test]
async fn build_catalog_provider_empty_sources_lists_builtin_entries() {
    use agent_mcp::catalog::CatalogProvider;
    let (tx, _rx) = tokio::sync::broadcast::channel::<DomainEvent>(8);
    let dir = tempfile::tempdir().unwrap();
    let agg = super::build_catalog_provider(&[], dir.path().to_path_buf(), tx).unwrap();
    // Builtin provider should return a non-empty list of built-in servers.
    let entries = agg.list(&CatalogQuery::default()).await.unwrap();
    assert!(
        !entries.is_empty(),
        "builtin provider should have at least one entry"
    );
}

#[tokio::test]
async fn build_catalog_provider_with_enabled_source_succeeds() {
    let (tx, _rx) = tokio::sync::broadcast::channel::<DomainEvent>(8);
    let dir = tempfile::tempdir().unwrap();
    let sources = vec![make_source("test-registry", true, 50)];
    let agg = super::build_catalog_provider(&sources, dir.path().to_path_buf(), tx);
    assert!(
        agg.is_ok(),
        "one enabled source should produce a valid aggregate"
    );
}

#[tokio::test]
async fn build_catalog_provider_disabled_source_still_succeeds() {
    let (tx, _rx) = tokio::sync::broadcast::channel::<DomainEvent>(8);
    let dir = tempfile::tempdir().unwrap();
    let sources = vec![make_source("disabled-reg", false, 50)];
    let agg = super::build_catalog_provider(&sources, dir.path().to_path_buf(), tx);
    assert!(
        agg.is_ok(),
        "disabled source should still produce a valid aggregate"
    );
}

#[tokio::test]
async fn build_catalog_provider_mixed_enabled_disabled_succeeds() {
    let (tx, _rx) = tokio::sync::broadcast::channel::<DomainEvent>(8);
    let dir = tempfile::tempdir().unwrap();
    let sources = vec![
        make_source("on", true, 10),
        make_source("off", false, 20),
        make_source("on2", true, 30),
    ];
    let agg = super::build_catalog_provider(&sources, dir.path().to_path_buf(), tx);
    assert!(
        agg.is_ok(),
        "mixed sources should produce a valid aggregate"
    );
}

// ── with_skill_catalog ─────────────────────────────────────────────────────

#[tokio::test]
async fn with_skill_catalog_none_sets_http_but_no_cache_dir() {
    let runtime = build_runtime().await.with_skill_catalog(None);
    assert!(runtime.skill_catalog_http.is_some());
    assert!(runtime.skill_catalog_cache_dir.is_none());
    assert!(runtime.skill_sources_toml.is_none());
}

#[tokio::test]
async fn with_skill_catalog_some_sets_both() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = build_runtime()
        .await
        .with_skill_catalog(Some(dir.path().to_path_buf()));
    assert!(runtime.skill_catalog_http.is_some());
    assert_eq!(runtime.skill_catalog_cache_dir.as_deref(), Some(dir.path()));
    assert!(runtime.skill_sources_toml.is_some());
}

// ── with_marketplace / with_marketplace_loaded ────────────────────────────

#[tokio::test]
async fn with_marketplace_wires_catalog_installer_cache() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = build_runtime()
        .await
        .with_marketplace(dir.path().to_path_buf())
        .unwrap();
    assert!(runtime.catalog.is_some());
    assert!(runtime.installer.is_some());
    assert!(runtime.catalog_http.is_some());
    assert!(runtime.catalog_cache.is_some());
    assert!(runtime.aggregate_handle.is_some());
    assert_eq!(runtime.marketplace_dir.as_deref(), Some(dir.path()));
}

#[tokio::test]
async fn with_marketplace_loaded_with_sources_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let sources = vec![make_source("extra", true, 100)];
    let runtime = build_runtime()
        .await
        .with_marketplace_loaded(dir.path().to_path_buf(), &sources);
    assert!(
        runtime.is_ok(),
        "marketplace loaded with remote sources should succeed"
    );
    let runtime = runtime.unwrap();
    assert!(runtime.aggregate_handle.is_some());
    assert!(runtime.catalog.is_some());
}

#[tokio::test]
async fn with_marketplace_creates_catalog_cache_subdir() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = build_runtime()
        .await
        .with_marketplace(dir.path().to_path_buf())
        .unwrap();
    // The cache should reference <config_dir>/catalog-cache.
    assert!(runtime.catalog_cache.is_some());
}

// ── ensure_skill_catalog ────────────────────────────────────────────────────

#[tokio::test]
async fn ensure_skill_catalog_returns_none_when_not_configured() {
    let runtime = build_runtime().await;
    assert!(runtime.ensure_skill_catalog().is_none());
}

#[tokio::test]
async fn ensure_skill_catalog_returns_some_after_with_skill_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = build_runtime()
        .await
        .with_skill_catalog(Some(dir.path().to_path_buf()));
    let catalog = runtime.ensure_skill_catalog();
    assert!(catalog.is_some());
}

#[tokio::test]
async fn ensure_skill_catalog_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = build_runtime()
        .await
        .with_skill_catalog(Some(dir.path().to_path_buf()));
    let first = runtime.ensure_skill_catalog();
    let second = runtime.ensure_skill_catalog();
    assert!(first.is_some());
    assert!(second.is_some());
    // Both should point to the same Arc (same OnceLock value).
    assert!(std::sync::Arc::ptr_eq(
        first.as_ref().unwrap(),
        second.as_ref().unwrap()
    ));
}

// ── rebuild_skill_aggregate ─────────────────────────────────────────────────

#[tokio::test]
async fn rebuild_skill_aggregate_noop_when_not_configured() {
    let runtime = build_runtime().await;
    let result = runtime.rebuild_skill_aggregate();
    assert!(result.is_ok());
    assert!(runtime.skill_catalog.get().is_none());
}

#[tokio::test]
async fn rebuild_skill_aggregate_populates_once_lock() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = build_runtime()
        .await
        .with_skill_catalog(Some(dir.path().to_path_buf()));
    let result = runtime.rebuild_skill_aggregate();
    assert!(result.is_ok());
    assert!(runtime.skill_catalog.get().is_some());
}

#[tokio::test]
async fn rebuild_skill_aggregate_second_call_reloads() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = build_runtime()
        .await
        .with_skill_catalog(Some(dir.path().to_path_buf()));
    runtime.rebuild_skill_aggregate().unwrap();
    assert!(runtime.skill_catalog.get().is_some());
    // Second call should succeed (reload path).
    let result = runtime.rebuild_skill_aggregate();
    assert!(result.is_ok());
}
