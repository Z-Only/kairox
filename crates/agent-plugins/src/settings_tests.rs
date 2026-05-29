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
