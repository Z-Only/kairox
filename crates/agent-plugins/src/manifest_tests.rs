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
async fn reads_plugin_security_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{
              "name": "signed-tools",
              "description": "Signed workflow helpers",
              "publisher": "Kairox Labs",
              "trust": "community",
              "signature": "minisign:RWQabc123",
              "checksum": "sha256:abc123",
              "sha256": "abc123"
            }"#,
    )
    .expect("manifest");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");

    assert_eq!(plugin.security.publisher.as_deref(), Some("Kairox Labs"));
    assert_eq!(plugin.security.trust.as_deref(), Some("community"));
    assert_eq!(
        plugin.security.signature.as_deref(),
        Some("minisign:RWQabc123")
    );
    assert_eq!(plugin.security.checksum.as_deref(), Some("sha256:abc123"));
    assert_eq!(plugin.security.sha256.as_deref(), Some("abc123"));
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

#[tokio::test]
async fn missing_manifest_returns_not_found_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    // No manifest directory at all.
    let result = read_plugin_manifest(dir.path()).await;
    assert!(matches!(result, Err(crate::PluginError::ManifestNotFound)));
}

#[tokio::test]
async fn malformed_json_returns_invalid_manifest_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(manifest_dir.join("plugin.json"), "{ not valid json }").expect("write");

    let result = read_plugin_manifest(dir.path()).await;
    match result {
        Err(crate::PluginError::InvalidManifest(msg)) => {
            assert!(!msg.is_empty(), "error message should describe the issue");
        }
        other => panic!("expected InvalidManifest error, got: {other:?}"),
    }
}

#[tokio::test]
async fn author_string_format_is_parsed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"name":"test","description":"Test","author":"Jane Doe"}"#,
    )
    .expect("write");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");
    assert_eq!(plugin.author_name.as_deref(), Some("Jane Doe"));
}

#[tokio::test]
async fn repository_object_format_extracts_url() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"name":"test","description":"Test","repository":{"type":"git","url":"https://github.com/example/repo"}}"#,
    )
    .expect("write");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");
    assert_eq!(
        plugin.repository.as_deref(),
        Some("https://github.com/example/repo")
    );
}

#[tokio::test]
async fn repository_string_format_is_preserved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"name":"test","description":"Test","repository":"https://github.com/example/repo"}"#,
    )
    .expect("write");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");
    assert_eq!(
        plugin.repository.as_deref(),
        Some("https://github.com/example/repo")
    );
}

#[tokio::test]
async fn empty_name_triggers_validation_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"name":"  ","description":"Whitespace name"}"#,
    )
    .expect("write");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");
    assert!(!plugin.valid);
    assert!(plugin.validation_error.is_some());
}

#[tokio::test]
async fn kairox_manifest_has_priority_over_codex_and_claude() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Create all three manifest types.
    for subdir in [".kairox-plugin", ".codex-plugin", ".claude-plugin"] {
        let manifest_dir = dir.path().join(subdir);
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        fs::write(
            manifest_dir.join("plugin.json"),
            format!(r#"{{"name":"from-{subdir}","description":"Test"}}"#),
        )
        .expect("write");
    }

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");
    // Kairox manifest should be resolved first.
    assert_eq!(plugin.manifest_kind, PluginManifestKind::Kairox);
    assert_eq!(plugin.name, "from-.kairox-plugin");
}

#[tokio::test]
async fn manifest_with_keywords_and_license() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{
            "name": "licensed-plugin",
            "description": "A plugin with license",
            "license": "MIT",
            "keywords": ["testing", "automation", "ci"]
        }"#,
    )
    .expect("write");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");
    assert_eq!(plugin.license.as_deref(), Some("MIT"));
    assert_eq!(plugin.keywords, vec!["testing", "automation", "ci"]);
}

#[tokio::test]
async fn security_metadata_with_object_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_dir = dir.path().join(".kairox-plugin");
    fs::create_dir_all(&manifest_dir).expect("manifest dir");
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{
            "name": "obj-security",
            "description": "Object-format security fields",
            "publisher": {"name": "Acme Corp", "id": "acme"},
            "trust": {"level": "verified"},
            "signature": {"signature": "sig123"},
            "checksum": {"sha256": "abc123"}
        }"#,
    )
    .expect("write");

    let plugin = read_plugin_manifest(dir.path()).await.expect("plugin");
    assert_eq!(plugin.security.publisher.as_deref(), Some("Acme Corp"));
    assert_eq!(plugin.security.trust.as_deref(), Some("verified"));
    assert_eq!(plugin.security.signature.as_deref(), Some("sig123"));
    assert_eq!(plugin.security.checksum.as_deref(), Some("abc123"));
}
