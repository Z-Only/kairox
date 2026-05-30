use std::fs;

use super::*;

fn write_plugin(root: &Path, name: &str) {
    let manifest_dir = root.join(name).join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        format!(r#"{{"name":"{name}","description":"Plugin {name}"}}"#),
    )
    .expect("manifest");
}

#[tokio::test]
async fn list_plugin_settings_maps_scope_and_inventory() {
    let user = tempfile::tempdir().expect("user");
    write_plugin(user.path(), "github");

    let views = list_plugin_settings(PluginSettingsRoots {
        user_root: Some(user.path().to_path_buf()),
        ..PluginSettingsRoots::default()
    })
    .await
    .expect("views");

    assert_eq!(views.len(), 1);
    assert_eq!(views[0].settings_id, "user:github");
    assert_eq!(views[0].scope, ConfigScope::User);
    assert_eq!(views[0].manifest_kind, "kairox");
}

#[tokio::test]
async fn list_plugin_settings_maps_security_metadata() {
    let user = tempfile::tempdir().expect("user");
    let manifest_dir = user.path().join("signed-tools").join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{
          "name": "signed-tools",
          "description": "Signed tools",
          "publisher": "Kairox Labs",
          "trust": "community",
          "signature": "minisign:RWQabc123",
          "checksum": "sha256:abc123",
          "sha256": "abc123"
        }"#,
    )
    .expect("manifest");

    let views = list_plugin_settings(PluginSettingsRoots {
        user_root: Some(user.path().to_path_buf()),
        ..PluginSettingsRoots::default()
    })
    .await
    .expect("views");

    assert_eq!(views.len(), 1);
    assert_eq!(views[0].security.publisher.as_deref(), Some("Kairox Labs"));
    assert_eq!(views[0].security.trust.as_deref(), Some("community"));
    assert_eq!(
        views[0].security.signature.as_deref(),
        Some("minisign:RWQabc123")
    );
    assert_eq!(views[0].security.checksum.as_deref(), Some("sha256:abc123"));
    assert_eq!(views[0].security.sha256.as_deref(), Some("abc123"));
}
