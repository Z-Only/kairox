use std::fs;
use std::path::Path;

use super::*;

fn write_skill(root: &Path, directory_name: &str, name: &str, description: &str, body: &str) {
    let skill_directory = root.join(directory_name);
    fs::create_dir_all(&skill_directory).expect("skill directory should be created");

    let skill_markdown = format!("---\nname: {name}\ndescription: {description}\n---\n{body}");
    fs::write(skill_directory.join("SKILL.md"), skill_markdown)
        .expect("skill markdown should be written");
}

#[tokio::test]
async fn settings_projection_keeps_shadowed_entries_visible() {
    let builtin_root = tempfile::tempdir().expect("builtin root");
    let user_root = tempfile::tempdir().expect("user root");
    let workspace_root = tempfile::tempdir().expect("workspace root");
    write_skill(
        builtin_root.path(),
        "builtin-review",
        "review",
        "Builtin review",
        "Builtin body\n",
    );
    write_skill(
        user_root.path(),
        "user-review",
        "review",
        "User review",
        "User body\n",
    );
    write_skill(
        workspace_root.path(),
        "workspace-review",
        "review",
        "Workspace review",
        "Workspace body\n",
    );

    let projection = discover_skill_settings(vec![
        SkillRoot::new(SkillSourceKind::Builtin, builtin_root.path()),
        SkillRoot::new(SkillSourceKind::User, user_root.path()),
        SkillRoot::new(SkillSourceKind::Workspace, workspace_root.path()),
    ])
    .await
    .expect("settings should discover");

    assert_eq!(projection.skills.len(), 3);
    assert_eq!(
        projection
            .skills
            .iter()
            .filter(|skill| skill.effective)
            .count(),
        1
    );
    assert!(projection
        .skills
        .iter()
        .any(|skill| skill.scope == SkillSourceKind::Workspace && skill.effective));
    assert!(projection
        .skills
        .iter()
        .any(|skill| skill.scope == SkillSourceKind::User
            && skill.shadowed_by.as_deref() == Some("workspace")));
}

#[tokio::test]
async fn discover_skill_settings_returns_stably_sorted_skills() {
    let builtin_root = tempfile::tempdir().expect("builtin root");
    let workspace_root = tempfile::tempdir().expect("workspace root");
    write_skill(
        workspace_root.path(),
        "zeta-workspace",
        "zeta",
        "Workspace zeta",
        "Workspace body\n",
    );
    write_skill(
        builtin_root.path(),
        "alpha-builtin",
        "alpha",
        "Builtin alpha",
        "Builtin body\n",
    );

    let projection = discover_skill_settings(vec![
        SkillRoot::new(SkillSourceKind::Workspace, workspace_root.path()),
        SkillRoot::new(SkillSourceKind::Builtin, builtin_root.path()),
    ])
    .await
    .expect("settings should discover");

    let ordered_ids: Vec<_> = projection
        .skills
        .iter()
        .map(|skill| skill.id.as_str())
        .collect();
    assert_eq!(ordered_ids, vec!["alpha", "zeta"]);
}

#[tokio::test]
async fn discover_skill_settings_keeps_permission_declarations() {
    let root = tempfile::tempdir().expect("root");
    let skill_directory = root.path().join("review");
    fs::create_dir_all(&skill_directory).expect("skill directory should exist");
    fs::write(
        skill_directory.join("SKILL.md"),
        "---\nname: review\ndescription: Review code\nkairox:\n  permissions:\n    tools: [\"fs.read\", \"search.ripgrep\"]\n    can_request_tools: [\"shell\"]\n---\nBody\n",
    )
    .expect("skill markdown should be written");

    let projection =
        discover_skill_settings(vec![SkillRoot::new(SkillSourceKind::User, root.path())])
            .await
            .expect("settings should discover");

    let skill = &projection.skills[0];
    assert_eq!(skill.tools, vec!["fs.read", "search.ripgrep"]);
    assert_eq!(skill.can_request_tools, vec!["shell"]);
}

#[tokio::test]
async fn invalid_skill_markdown_uses_frontmatter_name_after_invalid_lines() {
    let root = tempfile::tempdir().expect("root");
    let skill_directory = root.path().join("directory-name");
    fs::create_dir_all(&skill_directory).expect("skill directory should exist");
    fs::write(
        skill_directory.join("SKILL.md"),
        "---\ninvalid frontmatter line\nname: review\n---\nBody\n",
    )
    .expect("skill markdown should be written");

    let projection =
        discover_skill_settings(vec![SkillRoot::new(SkillSourceKind::User, root.path())])
            .await
            .expect("settings should discover degraded skill");

    assert_eq!(projection.skills.len(), 1);
    let skill = &projection.skills[0];
    assert_eq!(skill.id.as_str(), "review");
    assert!(!skill.valid);
    assert!(skill.validation_error.is_some());
}

#[tokio::test]
async fn invalid_state_file_does_not_block_valid_skill_discovery() {
    let root = tempfile::tempdir().expect("root");
    write_skill(
        root.path(),
        "review",
        "review",
        "Review code",
        "Review body\n",
    );
    fs::write(root.path().join("skills-state.toml"), "not valid toml")
        .expect("invalid state should be written");

    let projection =
        discover_skill_settings(vec![SkillRoot::new(SkillSourceKind::User, root.path())])
            .await
            .expect("settings should discover despite invalid state");

    assert_eq!(projection.skills.len(), 1);
    assert!(projection.skills[0].valid);
    assert_eq!(projection.skills[0].id.as_str(), "review");
    assert_eq!(projection.state_errors.len(), 1);
}

#[test]
fn scope_priority_order() {
    assert!(scope_priority(SkillSourceKind::Builtin) < scope_priority(SkillSourceKind::User));
    assert!(scope_priority(SkillSourceKind::User) < scope_priority(SkillSourceKind::Workspace));
}

#[test]
fn default_install_source_builtin_vs_local() {
    assert_eq!(default_install_source(SkillSourceKind::Builtin), "builtin");
    assert_eq!(default_install_source(SkillSourceKind::User), "local");
    assert_eq!(default_install_source(SkillSourceKind::Workspace), "local");
}

#[test]
fn fallback_skill_id_from_directory_name() {
    let path = std::path::Path::new("/tmp/skills/my-skill/SKILL.md");
    assert_eq!(fallback_skill_id(path), "my-skill");
}

#[test]
fn fallback_skill_id_unknown_for_path_without_parent() {
    let path = std::path::Path::new("SKILL.md");
    assert_eq!(fallback_skill_id(path), "unknown");
}

#[test]
fn extract_frontmatter_name_parses_simple_line() {
    let raw = "---\nname: review\ndescription: desc\n---\nBody\n";
    assert_eq!(extract_frontmatter_name(raw).as_deref(), Some("review"));
}

#[test]
fn extract_frontmatter_name_returns_none_when_no_name() {
    let raw = "---\ndescription: desc\n---\nBody\n";
    assert!(extract_frontmatter_name(raw).is_none());
}
