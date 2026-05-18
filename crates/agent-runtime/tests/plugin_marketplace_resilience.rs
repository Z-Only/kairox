use std::path::{Path, PathBuf};

use agent_core::facade::{
    InstallPluginRequest, PluginInstallTarget, PluginMarketplaceSourceView, PluginsFacade,
};
use agent_models::FakeModelClient;
use agent_runtime::plugin_settings::PluginSettingsRoots;
use agent_runtime::plugin_sources_toml::{default_plugin_marketplace_sources, PluginSourcesToml};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use tempfile::TempDir;

struct PluginRuntimeFixture {
    runtime: LocalRuntime<SqliteEventStore, FakeModelClient>,
    _tmp: TempDir,
    config_dir: PathBuf,
    user_plugins: PathBuf,
    project_plugins: PathBuf,
}

async fn build_plugin_runtime() -> PluginRuntimeFixture {
    let tmp = TempDir::new().expect("tempdir");
    let config_dir = tmp.path().join("config");
    let user_plugins = tmp.path().join("user-plugins");
    let project_plugins = tmp.path().join("workspace").join(".kairox").join("plugins");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    let runtime = LocalRuntime::new(store, model)
        .with_plugin_settings_roots(PluginSettingsRoots {
            workspace_root: Some(project_plugins.clone()),
            user_root: Some(user_plugins.clone()),
            builtin_root: None,
        })
        .with_marketplace(config_dir.clone())
        .expect("with_marketplace");

    PluginRuntimeFixture {
        runtime,
        _tmp: tmp,
        config_dir,
        user_plugins,
        project_plugins,
    }
}

fn write_sources(config_dir: &Path, mut sources: Vec<PluginMarketplaceSourceView>) {
    sources.extend(
        default_plugin_marketplace_sources()
            .into_iter()
            .map(|source| PluginMarketplaceSourceView {
                enabled: false,
                builtin: false,
                ..source
            }),
    );
    PluginSourcesToml::new(config_dir)
        .write(&sources)
        .expect("write plugin marketplace sources");
}

fn source(id: &str, path: &Path) -> PluginMarketplaceSourceView {
    PluginMarketplaceSourceView {
        id: id.to_string(),
        display_name: id.to_string(),
        source: path.display().to_string(),
        enabled: true,
        builtin: false,
    }
}

fn write_marketplace_plugin(marketplace_root: &Path, plugin_name: &str, description: &str) {
    let marketplace_dir = marketplace_root.join(".claude-plugin");
    let plugin_dir = marketplace_root.join("plugins").join(plugin_name);
    let manifest_dir = plugin_dir.join(".kairox-plugin");
    std::fs::create_dir_all(&marketplace_dir).expect("marketplace dir");
    std::fs::create_dir_all(&manifest_dir).expect("manifest dir");
    std::fs::write(
        marketplace_dir.join("marketplace.json"),
        format!(
            r#"{{
  "name": "local-marketplace",
  "plugins": [
    {{
      "name": "{plugin_name}",
      "description": "{description}",
      "version": "1.2.3",
      "source": "./plugins/{plugin_name}"
    }}
  ]
}}"#
        ),
    )
    .expect("marketplace json");
    std::fs::write(
        manifest_dir.join("plugin.json"),
        format!(
            r#"{{
  "name": "{plugin_name}",
  "description": "{description}",
  "version": "1.2.3"
}}"#
        ),
    )
    .expect("plugin manifest");
}

fn write_invalid_marketplace(marketplace_root: &Path) {
    let marketplace_dir = marketplace_root.join(".claude-plugin");
    std::fs::create_dir_all(&marketplace_dir).expect("marketplace dir");
    std::fs::write(marketplace_dir.join("marketplace.json"), "{ not json")
        .expect("invalid marketplace json");
}

#[tokio::test]
async fn plugin_catalog_skips_bad_source_and_returns_valid_plugin() {
    let fixture = build_plugin_runtime().await;
    let good_marketplace = fixture.config_dir.join("good-marketplace");
    let bad_marketplace = fixture.config_dir.join("bad-marketplace");
    write_marketplace_plugin(&good_marketplace, "quality-review", "Review code");
    write_invalid_marketplace(&bad_marketplace);
    write_sources(
        &fixture.config_dir,
        vec![
            source("local-good", &good_marketplace),
            source("local-bad", &bad_marketplace),
        ],
    );

    let entries = fixture
        .runtime
        .list_plugin_catalog(None, None)
        .await
        .expect("bad plugin source must not fail the whole catalog");

    assert_eq!(entries.len(), 1);
    let entry = entries.first().expect("catalog entry");
    assert_eq!(entry.marketplace_id, "local-good");
    assert_eq!(entry.name, "quality-review");
    assert_eq!(entry.description, "Review code");
    assert_eq!(
        entry.source,
        good_marketplace
            .join("plugins")
            .join("quality-review")
            .display()
            .to_string()
    );
}

#[tokio::test]
async fn install_plugin_from_local_marketplace_writes_user_plugin_state() {
    let fixture = build_plugin_runtime().await;
    let marketplace = fixture.config_dir.join("local-marketplace");
    write_marketplace_plugin(&marketplace, "quality-review", "Review code");
    write_sources(&fixture.config_dir, vec![source("local", &marketplace)]);

    let installed = fixture
        .runtime
        .install_plugin(InstallPluginRequest {
            marketplace_id: "local".into(),
            plugin_name: "quality-review".into(),
            target: PluginInstallTarget::User,
        })
        .await
        .expect("install plugin");

    assert_eq!(installed.settings_id, "user:quality-review");
    assert_eq!(installed.id, "quality-review");
    assert_eq!(installed.version.as_deref(), Some("1.2.3"));
    assert_eq!(installed.install_source.as_deref(), Some("marketplace"));
    assert_eq!(installed.marketplace.as_deref(), Some("local"));
    assert!(installed.enabled);
    assert!(installed.effective);
    assert!(installed
        .path
        .starts_with(&fixture.user_plugins.display().to_string()));
    assert!(
        fixture
            .user_plugins
            .join("quality-review")
            .join(".kairox-plugin")
            .join("plugin.json")
            .is_file(),
        "plugin manifest should be copied into the user install root"
    );
    assert!(
        !fixture.project_plugins.join("quality-review").exists(),
        "user install must not write into the project plugin root"
    );
}
