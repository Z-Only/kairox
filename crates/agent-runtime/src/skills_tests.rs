use std::path::PathBuf;

use agent_skills::SkillSourceKind;

use super::build_default_skill_roots;

#[test]
fn skill_roots_use_user_and_workspace_locations() {
    let home = PathBuf::from("/home/user");
    let workspace = PathBuf::from("/workspace/project");
    let roots = build_default_skill_roots(&home, &workspace);

    assert_eq!(roots.len(), 4);
    assert_eq!(roots[0].kind, SkillSourceKind::Builtin);
    assert_eq!(roots[0].path, home.join(".kairox/builtin-skills"));
    assert_eq!(roots[1].kind, SkillSourceKind::User);
    assert_eq!(roots[1].path, home.join(".config/kairox/skills"));
    assert_eq!(roots[2].kind, SkillSourceKind::Workspace);
    assert_eq!(roots[2].path, workspace.join(".kairox/skills"));
    assert_eq!(roots[3].kind, SkillSourceKind::Workspace);
    assert_eq!(roots[3].path, workspace.join(".agents/skills"));
}

#[test]
fn skill_settings_roots_include_builtin_location() {
    let home = PathBuf::from("/home/user");
    let workspace = PathBuf::from("/workspace/project");
    let roots = super::build_default_skill_settings_roots(&home, &workspace);

    assert_eq!(
        roots.builtin_root,
        Some(home.join(".kairox/builtin-skills"))
    );
    assert_eq!(roots.user_root, Some(home.join(".config/kairox/skills")));
    assert_eq!(roots.workspace_root, Some(workspace.join(".kairox/skills")));
}

// ── build_plugin_skill_roots tests ──

use super::build_plugin_skill_roots;
use crate::plugin_settings::PluginSettingsRoots;

fn write_plugin_manifest(plugin_dir: &std::path::Path, name: &str) {
    let manifest_dir = plugin_dir.join(".kairox-plugin");
    std::fs::create_dir_all(&manifest_dir).expect("manifest dir");
    std::fs::write(
        manifest_dir.join("plugin.json"),
        format!(r#"{{"name":"{name}","description":"Plugin {name}"}}"#),
    )
    .expect("manifest");
}

fn write_plugin_skill(plugin_dir: &std::path::Path, skill_name: &str) {
    let skill_dir = plugin_dir.join("skills").join(skill_name);
    std::fs::create_dir_all(&skill_dir).expect("skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {skill_name}\ndescription: {skill_name} skill\n---\nSkill body.\n"),
    )
    .expect("skill file");
}

#[tokio::test]
async fn build_plugin_skill_roots_includes_valid_plugin_skills() {
    let root = tempfile::tempdir().expect("root");
    let plugin_dir = root.path().join("my-plugin");
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
    write_plugin_manifest(&plugin_dir, "my-plugin");
    write_plugin_skill(&plugin_dir, "review");

    let plugin_roots = build_plugin_skill_roots(&PluginSettingsRoots {
        user_root: Some(root.path().to_path_buf()),
        ..PluginSettingsRoots::default()
    })
    .await;

    assert_eq!(plugin_roots.len(), 1);
    assert_eq!(plugin_roots[0].kind, SkillSourceKind::Plugin);
    assert_eq!(plugin_roots[0].namespace.as_deref(), Some("my-plugin"));
    assert_eq!(plugin_roots[0].path, plugin_dir.join("skills"));
}

#[tokio::test]
async fn build_plugin_skill_roots_excludes_disabled_plugin() {
    let root = tempfile::tempdir().expect("root");
    let plugin_dir = root.path().join("disabled-plugin");
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
    write_plugin_manifest(&plugin_dir, "disabled-plugin");
    write_plugin_skill(&plugin_dir, "review");

    // Write plugin state with enabled = false
    std::fs::write(
        root.path().join("plugins-state.toml"),
        "[plugins.disabled-plugin]\nenabled = false\n",
    )
    .expect("state file");

    let plugin_roots = build_plugin_skill_roots(&PluginSettingsRoots {
        user_root: Some(root.path().to_path_buf()),
        ..PluginSettingsRoots::default()
    })
    .await;

    assert!(
        plugin_roots.is_empty(),
        "disabled plugin skills should be excluded"
    );
}

#[tokio::test]
async fn build_plugin_skill_roots_excludes_invalid_plugin() {
    let root = tempfile::tempdir().expect("root");
    let plugin_dir = root.path().join("bad-plugin");
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
    // Write a manifest with no name — will be marked invalid
    let manifest_dir = plugin_dir.join(".kairox-plugin");
    std::fs::create_dir_all(&manifest_dir).expect("manifest dir");
    std::fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"description":"missing name field"}"#,
    )
    .expect("manifest");
    write_plugin_skill(&plugin_dir, "review");

    let plugin_roots = build_plugin_skill_roots(&PluginSettingsRoots {
        user_root: Some(root.path().to_path_buf()),
        ..PluginSettingsRoots::default()
    })
    .await;

    assert!(
        plugin_roots.is_empty(),
        "invalid plugin skills should be excluded"
    );
}

#[tokio::test]
async fn build_plugin_skill_roots_keeps_valid_plugins_when_sibling_manifest_is_invalid() {
    let root = tempfile::tempdir().expect("root");
    let valid_plugin_dir = root.path().join("good-plugin");
    std::fs::create_dir_all(&valid_plugin_dir).expect("valid plugin dir");
    write_plugin_manifest(&valid_plugin_dir, "good-plugin");
    write_plugin_skill(&valid_plugin_dir, "review");

    let invalid_plugin_dir = root.path().join("bad-plugin");
    std::fs::create_dir_all(&invalid_plugin_dir).expect("invalid plugin dir");
    let manifest_dir = invalid_plugin_dir.join(".kairox-plugin");
    std::fs::create_dir_all(&manifest_dir).expect("manifest dir");
    std::fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"description":"missing name field"}"#,
    )
    .expect("manifest");
    write_plugin_skill(&invalid_plugin_dir, "review");

    let plugin_roots = build_plugin_skill_roots(&PluginSettingsRoots {
        user_root: Some(root.path().to_path_buf()),
        ..PluginSettingsRoots::default()
    })
    .await;

    assert_eq!(plugin_roots.len(), 1);
    assert_eq!(plugin_roots[0].namespace.as_deref(), Some("good-plugin"));
    assert_eq!(plugin_roots[0].path, valid_plugin_dir.join("skills"));
}

#[tokio::test]
async fn build_plugin_skill_roots_excludes_plugin_without_skills() {
    let root = tempfile::tempdir().expect("root");
    let plugin_dir = root.path().join("no-skill-plugin");
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
    write_plugin_manifest(&plugin_dir, "no-skill-plugin");
    // No skills directory — inventory.skill_count will be 0

    let plugin_roots = build_plugin_skill_roots(&PluginSettingsRoots {
        user_root: Some(root.path().to_path_buf()),
        ..PluginSettingsRoots::default()
    })
    .await;

    assert!(
        plugin_roots.is_empty(),
        "plugins without skills should be excluded"
    );
}
