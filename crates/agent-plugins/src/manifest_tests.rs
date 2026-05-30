use std::fs;

use super::*;

#[tokio::test]
async fn reads_codex_plugin_manifest_and_inventory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".codex-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r##"{
              "name": "github",
              "version": "0.1.0",
              "description": "GitHub workflows",
              "author": {"name": "OpenAI"},
              "skills": "./skills/",
              "apps": "./.app.json",
              "interface": {
                "displayName": "GitHub",
                "category": "Coding",
                "brandColor": "#24292F"
              }
            }"##,
    )
    .expect("manifest");
    fs::create_dir_all(dir.path().join("skills").join("review")).expect("skill dir");
    fs::write(
        dir.path().join(".mcp.json"),
        r#"{"mcpServers":{"github":{"command":"npx","args":["github-mcp"]}}}"#,
    )
    .expect("mcp");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");

    assert_eq!(plugin.name, "github");
    assert_eq!(plugin.version.as_deref(), Some("0.1.0"));
    assert_eq!(plugin.manifest_kind, PluginManifestKind::Codex);
    assert_eq!(plugin.author_name.as_deref(), Some("OpenAI"));
    assert_eq!(plugin.interface.display_name.as_deref(), Some("GitHub"));
    assert_eq!(plugin.inventory.skill_count, 1);
    assert_eq!(plugin.inventory.mcp_server_count, 1);
    assert!(plugin.valid);
}

#[tokio::test]
async fn reads_plugin_permission_hints() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{
              "name": "repo-tools",
              "description": "Repository automation",
              "permissions": {
                "approvalPolicy": "on_request",
                "sandboxPolicy": "workspace_write",
                "networkAccess": true,
                "writableRoots": ["./.kairox/plugins/repo-tools"],
                "tools": ["shell", "fs.read"]
              }
            }"#,
    )
    .expect("manifest");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");

    assert_eq!(
        plugin.permissions.approval_policy.as_deref(),
        Some("on_request")
    );
    assert_eq!(
        plugin.permissions.sandbox_policy.as_deref(),
        Some("workspace_write")
    );
    assert!(plugin.permissions.network_access);
    assert_eq!(
        plugin.permissions.writable_roots,
        vec!["./.kairox/plugins/repo-tools"]
    );
    assert_eq!(plugin.permissions.tools, vec!["shell", "fs.read"]);
}

#[tokio::test]
async fn reads_plugin_compatibility_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{
              "name": "workflow-kit",
              "description": "Workflow helpers",
              "compatibility": {
                "kairoxVersion": ">=0.33.0 <0.35.0",
                "platforms": ["macos", "linux"],
                "requires": ["node >=20", "git"]
              }
            }"#,
    )
    .expect("manifest");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");

    assert_eq!(
        plugin.compatibility.kairox_version.as_deref(),
        Some(">=0.33.0 <0.35.0")
    );
    assert_eq!(plugin.compatibility.platforms, vec!["macos", "linux"]);
    assert_eq!(plugin.compatibility.requires, vec!["node >=20", "git"]);
}

#[tokio::test]
async fn reads_claude_plugin_manifest_with_folder_skill_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".claude-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{
              "name": "commit-commands",
              "description": "Git commit automation",
              "version": "1.0.0"
            }"#,
    )
    .expect("manifest");
    fs::create_dir_all(dir.path().join("skills").join("commit")).expect("skill dir");
    fs::write(
        dir.path().join("skills").join("commit").join("SKILL.md"),
        "---\ndescription: Make a commit\n---\nCommit changes.\n",
    )
    .expect("skill");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");

    assert_eq!(plugin.name, "commit-commands");
    assert_eq!(plugin.manifest_kind, PluginManifestKind::Claude);
    assert_eq!(plugin.inventory.skill_count, 1);
    assert_eq!(plugin.inventory.skill_names, vec!["commit"]);
}

#[tokio::test]
async fn invalid_manifest_stays_visible() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"description":"missing name"}"#,
    )
    .expect("manifest");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");

    assert_eq!(
        plugin.name,
        dir.path().file_name().unwrap().to_string_lossy()
    );
    assert_eq!(plugin.manifest_kind, PluginManifestKind::Kairox);
    assert!(!plugin.valid);
    assert!(plugin.validation_error.unwrap().contains("name"));
}
