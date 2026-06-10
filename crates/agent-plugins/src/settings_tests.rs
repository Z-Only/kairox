use std::fs;

use super::*;

fn write_plugin(root: &std::path::Path, name: &str, description: &str) {
    let manifest_dir = root.join(name).join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        format!(r#"{{"name":"{name}","description":"{description}"}}"#),
    )
    .expect("manifest");
}

#[tokio::test]
async fn project_plugin_overrides_user_and_builtin() {
    let builtin = tempfile::tempdir().expect("builtin");
    let user = tempfile::tempdir().expect("user");
    let project = tempfile::tempdir().expect("project");
    write_plugin(builtin.path(), "workflow", "Built in");
    write_plugin(user.path(), "workflow", "User");
    write_plugin(project.path(), "workflow", "Project");

    let settings = discover_plugin_settings(vec![
        PluginRoot::new(PluginScope::Builtin, builtin.path()),
        PluginRoot::new(PluginScope::User, user.path()),
        PluginRoot::new(PluginScope::Project, project.path()),
    ])
    .await
    .expect("settings");

    assert_eq!(settings.plugins.len(), 3);
    let project_view = settings
        .plugins
        .iter()
        .find(|plugin| plugin.settings_id == "project:workflow")
        .expect("project plugin");
    assert!(project_view.effective);
    assert_eq!(project_view.description, "Project");
    assert!(
        settings
            .plugins
            .iter()
            .find(|plugin| plugin.settings_id == "user:workflow")
            .expect("user plugin")
            .shadowed_by
            .as_deref()
            == Some("project")
    );
}

#[tokio::test]
async fn plugin_state_persists_enabled_flag() {
    let user = tempfile::tempdir().expect("user");
    write_plugin(user.path(), "github", "GitHub workflows");
    write_plugin_state(
        user.path(),
        "github",
        false,
        Some("marketplace"),
        Some("official"),
    )
    .await
    .expect("state write");

    let settings = discover_plugin_settings(vec![PluginRoot::new(PluginScope::User, user.path())])
        .await
        .expect("settings");

    let view = settings.plugins.first().expect("plugin");
    assert_eq!(view.settings_id, "user:github");
    assert!(!view.enabled);
    assert_eq!(view.install_source.as_deref(), Some("marketplace"));
    assert_eq!(view.marketplace.as_deref(), Some("official"));
}

#[tokio::test]
async fn empty_roots_returns_empty_projection() {
    let settings = discover_plugin_settings(vec![])
        .await
        .expect("empty roots should succeed");
    assert!(settings.plugins.is_empty());
    assert!(settings.state_errors.is_empty());
}

#[tokio::test]
async fn missing_root_directory_is_silently_skipped() {
    let missing = std::path::PathBuf::from("/tmp/nonexistent-plugin-root-99999");
    let settings = discover_plugin_settings(vec![PluginRoot::new(PluginScope::User, &missing)])
        .await
        .expect("missing root should not fail");
    assert!(settings.plugins.is_empty());
}

#[tokio::test]
async fn plugin_state_round_trips_through_write_and_discover() {
    let root = tempfile::tempdir().expect("root");
    write_plugin(root.path(), "test-plugin", "A test plugin");

    // Initially enabled (default).
    let settings1 = discover_plugin_settings(vec![PluginRoot::new(PluginScope::User, root.path())])
        .await
        .expect("initial discover");
    assert!(settings1.plugins[0].enabled);

    // Disable it.
    write_plugin_state(root.path(), "test-plugin", false, None, None)
        .await
        .expect("disable");
    let settings2 = discover_plugin_settings(vec![PluginRoot::new(PluginScope::User, root.path())])
        .await
        .expect("re-discover");
    assert!(!settings2.plugins[0].enabled);

    // Re-enable it.
    write_plugin_state(root.path(), "test-plugin", true, None, None)
        .await
        .expect("re-enable");
    let settings3 = discover_plugin_settings(vec![PluginRoot::new(PluginScope::User, root.path())])
        .await
        .expect("re-discover again");
    assert!(settings3.plugins[0].enabled);
}

#[tokio::test]
async fn multiple_plugins_sorted_by_id() {
    let root = tempfile::tempdir().expect("root");
    write_plugin(root.path(), "zebra", "Zebra plugin");
    write_plugin(root.path(), "alpha", "Alpha plugin");
    write_plugin(root.path(), "middle", "Middle plugin");

    let settings = discover_plugin_settings(vec![PluginRoot::new(PluginScope::User, root.path())])
        .await
        .expect("discover");

    let ids: Vec<&str> = settings.plugins.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec!["alpha", "middle", "zebra"]);
}

#[tokio::test]
async fn invalid_state_file_records_error_and_uses_defaults() {
    let root = tempfile::tempdir().expect("root");
    write_plugin(root.path(), "test-plugin", "Test");
    std::fs::write(
        root.path().join("plugins-state.toml"),
        "this is not {{{ valid toml",
    )
    .expect("write invalid state");

    let settings = discover_plugin_settings(vec![PluginRoot::new(PluginScope::User, root.path())])
        .await
        .expect("should not fail");

    assert!(!settings.state_errors.is_empty());
    // Plugin should still be discovered with default enabled=true.
    assert!(settings.plugins[0].enabled);
}

#[tokio::test]
async fn settings_id_includes_scope_prefix() {
    let root = tempfile::tempdir().expect("root");
    write_plugin(root.path(), "my-plugin", "My plugin");

    let builtin =
        discover_plugin_settings(vec![PluginRoot::new(PluginScope::Builtin, root.path())])
            .await
            .expect("builtin");
    assert_eq!(builtin.plugins[0].settings_id, "builtin:my-plugin");

    let user = discover_plugin_settings(vec![PluginRoot::new(PluginScope::User, root.path())])
        .await
        .expect("user");
    assert_eq!(user.plugins[0].settings_id, "user:my-plugin");

    let project =
        discover_plugin_settings(vec![PluginRoot::new(PluginScope::Project, root.path())])
            .await
            .expect("project");
    assert_eq!(project.plugins[0].settings_id, "project:my-plugin");
}
