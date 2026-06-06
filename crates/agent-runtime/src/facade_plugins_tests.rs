use super::*;
use agent_core::facade::{InstallPluginRequest, PluginInstallTarget, PluginsFacade};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use std::path::PathBuf;

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

// ---------------------------------------------------------------------------
// LocalRuntime methods
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_plugin_settings_returns_empty_with_default_roots() {
    let runtime = build_runtime().await;
    let settings = runtime.list_plugin_settings().await.unwrap();
    assert!(settings.is_empty());
}

#[tokio::test]
async fn get_plugin_detail_returns_none_for_nonexistent_id() {
    let runtime = build_runtime().await;
    let detail = runtime
        .get_plugin_detail("nonexistent-plugin".to_string())
        .await
        .unwrap();
    assert!(detail.is_none());
}

#[tokio::test]
async fn set_plugin_enabled_errors_for_nonexistent_plugin() {
    let runtime = build_runtime().await;
    let result = runtime
        .set_plugin_enabled("nonexistent-plugin".to_string(), true)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn delete_plugin_settings_errors_for_nonexistent_plugin() {
    let runtime = build_runtime().await;
    let result = runtime
        .delete_plugin_settings("nonexistent-plugin".to_string())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_plugin_marketplace_sources_returns_defaults_without_user_config() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    let runtime = LocalRuntime::new(store, model)
        .with_marketplace(tmp.path().to_path_buf())
        .unwrap();
    let sources = runtime.list_plugin_marketplace_sources().await.unwrap();
    // merged_sources always includes built-in defaults
    assert!(!sources.is_empty());
    assert!(sources.iter().all(|s| s.builtin));
}

#[tokio::test]
async fn set_plugin_marketplace_source_enabled_errors_for_nonexistent_source() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    let runtime = LocalRuntime::new(store, model)
        .with_marketplace(tmp.path().to_path_buf())
        .unwrap();
    let result = runtime
        .set_plugin_marketplace_source_enabled("nonexistent-source".to_string(), true)
        .await;
    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    assert!(error_message.contains("not found"));
}

#[tokio::test]
async fn plugin_marketplace_config_dir_returns_marketplace_dir_when_set() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    let runtime = LocalRuntime::new(store, model)
        .with_marketplace(tmp.path().to_path_buf())
        .unwrap();
    let config_dir = runtime.plugin_marketplace_config_dir().unwrap();
    assert_eq!(config_dir, tmp.path().to_path_buf());
}

#[tokio::test]
async fn list_plugin_catalog_with_nonexistent_marketplace_filter_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    let runtime = LocalRuntime::new(store, model)
        .with_marketplace(tmp.path().to_path_buf())
        .unwrap();
    // Filtering by a non-existent marketplace_id skips all sources
    let catalog = runtime
        .list_plugin_catalog(Some("nonexistent-marketplace".into()), None)
        .await
        .unwrap();
    assert!(catalog.is_empty());
}

#[tokio::test]
async fn install_plugin_errors_when_catalog_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    let runtime = LocalRuntime::new(store, model)
        .with_marketplace(tmp.path().to_path_buf())
        .unwrap();
    let request = InstallPluginRequest {
        marketplace_id: "test-source".to_string(),
        plugin_name: "some-plugin".to_string(),
        target: PluginInstallTarget::User,
    };
    let result = runtime.install_plugin(request).await;
    assert!(result.is_err());
    let error_message = format!("{:?}", result.unwrap_err());
    assert!(error_message.contains("not found"));
}

// ---------------------------------------------------------------------------
// PluginsFacade trait delegation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trait_list_plugin_settings_delegates_to_inherent() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    let runtime = LocalRuntime::new(store, model)
        .with_marketplace(tmp.path().to_path_buf())
        .unwrap();
    let facade: &dyn PluginsFacade = &runtime;
    let settings = facade.list_plugin_settings().await.unwrap();
    assert!(settings.is_empty());
}

// ---------------------------------------------------------------------------
// parse_github_plugin_source
// ---------------------------------------------------------------------------

#[test]
fn parse_github_plugin_source_valid_input() {
    let result = parse_github_plugin_source("github:owner/repo:path/to/plugin");
    assert_eq!(
        result,
        Some(GithubPluginSource {
            repo: "owner/repo".to_string(),
            path: "path/to/plugin".to_string(),
        })
    );
}

#[test]
fn parse_github_plugin_source_empty_path() {
    let result = parse_github_plugin_source("github:owner/repo:");
    assert_eq!(
        result,
        Some(GithubPluginSource {
            repo: "owner/repo".to_string(),
            path: "".to_string(),
        })
    );
}

#[test]
fn parse_github_plugin_source_empty_repo_returns_none() {
    let result = parse_github_plugin_source("github::path");
    assert_eq!(result, None);
}

#[test]
fn parse_github_plugin_source_no_prefix_returns_none() {
    let result = parse_github_plugin_source("owner/repo:path");
    assert_eq!(result, None);
}

#[test]
fn parse_github_plugin_source_no_colon_separator_returns_none() {
    let result = parse_github_plugin_source("github:owner/repo");
    assert_eq!(result, None);
}

// ---------------------------------------------------------------------------
// parse_github_shorthand
// ---------------------------------------------------------------------------

#[test]
fn parse_github_shorthand_valid_input() {
    let result = parse_github_shorthand("owner/repo");
    assert_eq!(result, Some(("owner", "repo")));
}

#[test]
fn parse_github_shorthand_empty_owner_returns_none() {
    let result = parse_github_shorthand("/repo");
    assert_eq!(result, None);
}

#[test]
fn parse_github_shorthand_empty_repo_returns_none() {
    let result = parse_github_shorthand("owner/");
    assert_eq!(result, None);
}

#[test]
fn parse_github_shorthand_three_segments_returns_none() {
    let result = parse_github_shorthand("owner/repo/extra");
    assert_eq!(result, None);
}

// ---------------------------------------------------------------------------
// resolve_catalog_entry_source
// ---------------------------------------------------------------------------

#[test]
fn resolve_catalog_entry_source_relative_path_with_local_root() {
    let root = PathBuf::from("/home/user/marketplace");
    let result = resolve_catalog_entry_source(Some(&root), "./plugins/my-plugin");
    assert_eq!(result, "/home/user/marketplace/plugins/my-plugin");
}

#[test]
fn resolve_catalog_entry_source_relative_path_with_github_root() {
    let root = PathBuf::from("github:owner/repo:");
    let result = resolve_catalog_entry_source(Some(&root), "./plugins/my-plugin");
    assert_eq!(result, "github:owner/repo:plugins/my-plugin");
}

#[test]
fn resolve_catalog_entry_source_no_catalog_root_returns_original() {
    let result = resolve_catalog_entry_source(None, "./plugins/my-plugin");
    assert_eq!(result, "./plugins/my-plugin");
}

#[test]
fn resolve_catalog_entry_source_json_github_source() {
    let json_source = r#"{"source": "github", "repo": "owner/repo"}"#;
    let result = resolve_catalog_entry_source(None, json_source);
    assert_eq!(result, "github:owner/repo:.");
}

#[test]
fn resolve_catalog_entry_source_non_relative_non_json_returns_original() {
    let result = resolve_catalog_entry_source(Some(&PathBuf::from("/root")), "absolute/path");
    assert_eq!(result, "absolute/path");
}

// ---------------------------------------------------------------------------
// InstallRequestTargetLabel
// ---------------------------------------------------------------------------

#[test]
fn install_request_target_label_user() {
    let request = InstallPluginRequest {
        marketplace_id: "m".to_string(),
        plugin_name: "p".to_string(),
        target: PluginInstallTarget::User,
    };
    assert_eq!(request.target_label(), "user");
}

#[test]
fn install_request_target_label_project() {
    let request = InstallPluginRequest {
        marketplace_id: "m".to_string(),
        plugin_name: "p".to_string(),
        target: PluginInstallTarget::Project,
    };
    assert_eq!(request.target_label(), "project");
}
