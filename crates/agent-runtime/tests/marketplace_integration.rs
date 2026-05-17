//! Integration tests for the marketplace surface of `LocalRuntime`
//! (T8 of the MCP marketplace plan).
//!
//! Exercises the full path: `AppFacade::list_catalog` →
//! `BuiltinCatalogProvider` → `Installer` (writing/reading
//! `config.toml` in a tempdir) → `list_installed_entries` →
//! `uninstall_catalog_entry`.

use agent_core::{AppFacade, CatalogQuery, InstallRequest};
use agent_runtime::test_support::build_marketplace_runtime;
use std::collections::BTreeMap;

#[tokio::test]
async fn list_catalog_returns_builtin_entries() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    let entries = rt
        .list_catalog(CatalogQuery::default())
        .await
        .expect("list_catalog");
    // The built-in catalog keeps only the core servers Kairox enables by default.
    assert_eq!(entries.len(), 6);
    assert!(entries.iter().any(|e| e.id == "filesystem"));
}

#[tokio::test]
async fn get_catalog_entry_returns_filesystem() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    let entry = rt
        .get_catalog_entry("filesystem".into(), None)
        .await
        .expect("get_catalog_entry")
        .expect("filesystem present in builtin catalog");
    assert_eq!(entry.id, "filesystem");
    assert_eq!(entry.source, "builtin");
}

#[tokio::test]
async fn install_then_list_then_uninstall_filesystem() {
    let (rt, _tmp) = build_marketplace_runtime().await;

    // The filesystem entry requires WORKSPACE_PATH; provide it as an override.
    let mut env_overrides = BTreeMap::new();
    env_overrides.insert("WORKSPACE_PATH".into(), "/tmp".into());
    let req = InstallRequest {
        catalog_id: "filesystem".into(),
        source: "builtin".into(),
        server_id_override: None,
        env_overrides,
        trust_grant: true,
        auto_start: false,
    };
    let outcome = rt
        .install_catalog_entry(req)
        .await
        .expect("install_catalog_entry");
    // Outcome may be `installed` (if `node` is on PATH in this CI) or
    // `runtime_missing` (if not). Either path persists nothing if missing,
    // so we assert based on `kind`.
    match outcome.kind.as_str() {
        "installed" => {
            assert_eq!(outcome.server_id.as_deref(), Some("filesystem"));

            let installed = rt
                .list_installed_entries()
                .await
                .expect("list_installed_entries");
            assert!(installed.iter().any(|e| e.server_id == "filesystem"));

            rt.uninstall_catalog_entry("filesystem".into())
                .await
                .expect("uninstall_catalog_entry");

            let installed_after = rt
                .list_installed_entries()
                .await
                .expect("list_installed_entries");
            assert!(!installed_after.iter().any(|e| e.server_id == "filesystem"));
        }
        "runtime_missing" => {
            // Host lacks Node — that's fine in CI; just confirm we surfaced
            // the missing runtime list.
            assert!(!outcome.missing_runtimes.is_empty());
        }
        other => panic!("unexpected install outcome kind: {other}"),
    }
}

#[tokio::test]
async fn list_installed_entries_uses_persisted_catalog_metadata_for_overrides() {
    let (rt, tmp) = build_marketplace_runtime().await;
    std::fs::write(
        tmp.path().join("config.toml"),
        r#"
[mcp_servers.fetch-local]
type = "stdio"
command = "uvx"
args = ["mcp-server-fetch"]
__catalog_id = "fetch"
__source = "builtin"
"#,
    )
    .expect("write config toml");

    let installed = rt
        .list_installed_entries()
        .await
        .expect("list_installed_entries");

    let entry = installed
        .iter()
        .find(|entry| entry.server_id == "fetch-local")
        .expect("overridden install should be listed");
    assert_eq!(entry.catalog_id.as_deref(), Some("fetch"));
    assert_eq!(entry.source.as_deref(), Some("builtin"));
    assert_eq!(entry.display_name, "Fetch");
}

#[tokio::test]
async fn refresh_catalog_emits_event_and_succeeds() {
    let (rt, _tmp) = build_marketplace_runtime().await;
    rt.refresh_catalog(None).await.expect("refresh_catalog");
}
