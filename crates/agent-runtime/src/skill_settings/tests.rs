use std::path::Path;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
    SkillInstallSource, SkillInstallTarget, SkillSettingsScope, SkillUpdateState,
};

use super::actions::validate_directory_under_root;
use super::install::install_remote_skill_into_root;
use super::view::list_skill_settings_from_roots;
use super::{set_skill_activation_mode, set_skill_enabled, SkillSettingsRoots};
use crate::skill_package::{FakeSkillPackageManager, SkillPackageManager};

#[tokio::test]
async fn list_skill_settings_maps_project_skill_to_editable_view() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    write_skill(
        workspace_root.path(),
        "review",
        "review",
        "Review code",
        "Body\n",
    );

    let views = list_skill_settings_from_roots(SkillSettingsRoots {
        workspace_root: Some(workspace_root.path().to_path_buf()),
        user_root: None,
        builtin_root: None,
        plugin_roots: Vec::new(),
    })
    .await
    .expect("settings should list");

    let review = views
        .iter()
        .find(|view| view.id == "review")
        .expect("review skill");
    assert_eq!(review.scope, SkillSettingsScope::Project);
    assert!(review.editable);
    assert!(review.deletable);
}

#[tokio::test]
async fn list_skill_settings_maps_permission_declarations() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let skill_directory = workspace_root.path().join("review");
    std::fs::create_dir_all(&skill_directory).expect("skill directory should exist");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        "---\nname: review\ndescription: Review code\nkairox:\n  permissions:\n    tools: [\"fs.read\"]\n    can_request_tools: [\"shell\", \"search.ripgrep\"]\n---\nBody\n",
    )
    .expect("skill should be written");

    let views = list_skill_settings_from_roots(SkillSettingsRoots {
        workspace_root: Some(workspace_root.path().to_path_buf()),
        user_root: None,
        builtin_root: None,
        plugin_roots: Vec::new(),
    })
    .await
    .expect("settings should list");

    let review = views
        .iter()
        .find(|view| view.id == "review")
        .expect("review skill");
    assert_eq!(review.tools, vec!["fs.read"]);
    assert_eq!(review.can_request_tools, vec!["shell", "search.ripgrep"]);
    assert_eq!(
        review.permission_summary,
        "tools: fs.read; can request: shell, search.ripgrep"
    );
}

#[tokio::test]
async fn installing_remote_skill_refreshes_installed_view() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let package_manager = FakeSkillPackageManager::default();
    write_skill(
        workspace_root.path(),
        "brainstorming",
        "brainstorming",
        "Brainstorm ideas",
        "Body\n",
    );

    let request = InstallRemoteSkillRequest {
        package: "obra/superpowers@brainstorming".to_string(),
        source: "registry".to_string(),
        target: SkillInstallTarget::Project,
        package_url: None,
    };

    let installed =
        install_remote_skill_into_root(&package_manager, workspace_root.path(), request)
            .await
            .expect("remote skill should install");

    assert_eq!(installed.install_source, SkillInstallSource::Registry);
}

#[tokio::test]
async fn set_skill_activation_mode_persists_state_override() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    write_skill_with_activation_mode(
        workspace_root.path(),
        "review",
        "review",
        "Review code",
        "manual",
        "Body\n",
    );

    set_skill_activation_mode(
        SkillSettingsRoots {
            workspace_root: Some(workspace_root.path().to_path_buf()),
            user_root: None,
            builtin_root: None,
            plugin_roots: Vec::new(),
        },
        "review",
        "auto",
    )
    .await
    .expect("activation mode should be updated");

    let views = list_skill_settings_from_roots(SkillSettingsRoots {
        workspace_root: Some(workspace_root.path().to_path_buf()),
        user_root: None,
        builtin_root: None,
        plugin_roots: Vec::new(),
    })
    .await
    .expect("settings should list");

    let review = views
        .iter()
        .find(|view| view.id == "review")
        .expect("review skill");
    assert_eq!(review.activation_mode, "auto");

    let state = std::fs::read_to_string(workspace_root.path().join("skills-state.toml"))
        .expect("state should be written");
    assert!(state.contains("activation_mode = \"auto\""));
}

#[tokio::test]
async fn mutating_duplicate_skill_id_returns_ambiguous_error() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let user_root = tempfile::tempdir().expect("user root");
    write_skill(
        workspace_root.path(),
        "review-project",
        "review",
        "Review code",
        "Project body\n",
    );
    write_skill(
        user_root.path(),
        "review-user",
        "review",
        "Review code",
        "User body\n",
    );

    let error = set_skill_enabled(
        SkillSettingsRoots {
            workspace_root: Some(workspace_root.path().to_path_buf()),
            user_root: Some(user_root.path().to_path_buf()),
            builtin_root: None,
            plugin_roots: Vec::new(),
        },
        "review",
        false,
    )
    .await
    .expect_err("duplicate skill ids should require disambiguation");

    assert!(
        error.to_string().contains("ambiguous skill id"),
        "message was: {error}"
    );
    assert!(!workspace_root.path().join("skills-state.toml").exists());
    assert!(!user_root.path().join("skills-state.toml").exists());
}

#[tokio::test]
async fn mutating_duplicate_skill_id_accepts_settings_id() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let user_root = tempfile::tempdir().expect("user root");
    write_skill(
        workspace_root.path(),
        "review-project",
        "review",
        "Review code",
        "Project body\n",
    );
    write_skill(
        user_root.path(),
        "review-user",
        "review",
        "Review code",
        "User body\n",
    );

    let roots = SkillSettingsRoots {
        workspace_root: Some(workspace_root.path().to_path_buf()),
        user_root: Some(user_root.path().to_path_buf()),
        builtin_root: None,
        plugin_roots: Vec::new(),
    };
    let views = list_skill_settings_from_roots(roots.clone())
        .await
        .expect("settings should list");
    assert!(views
        .iter()
        .any(|view| view.settings_id == "project:review"));
    assert!(views.iter().any(|view| view.settings_id == "user:review"));

    set_skill_enabled(roots, "project:review", false)
        .await
        .expect("project settings id should disambiguate mutation");

    let workspace_state = std::fs::read_to_string(workspace_root.path().join("skills-state.toml"))
        .expect("workspace state should be written");
    assert!(workspace_state.contains("[skills.review]"));
    assert!(workspace_state.contains("enabled = false"));
    assert!(!user_root.path().join("skills-state.toml").exists());
}

#[tokio::test]
async fn installing_versioned_package_uses_installed_skill_metadata_id() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let package_manager = FakeSkillPackageManager::default();
    write_skill(
        workspace_root.path(),
        "code-review",
        "code-review",
        "Review code",
        "Body\n",
    );

    let request = InstallRemoteSkillRequest {
        package: "@skills/code-review@1.2.3".to_string(),
        source: "registry".to_string(),
        target: SkillInstallTarget::Project,
        package_url: None,
    };

    let installed =
        install_remote_skill_into_root(&package_manager, workspace_root.path(), request)
            .await
            .expect("versioned package should resolve installed metadata id");

    assert_eq!(installed.id, "code-review");
    assert_eq!(installed.install_source, SkillInstallSource::Registry);
    let state = std::fs::read_to_string(workspace_root.path().join("skills-state.toml"))
        .expect("state should be written");
    assert!(state.contains("[skills.code-review]"));
    assert!(!state.contains("[skills.1.2.3]"));
}

#[tokio::test]
async fn installing_remote_skill_identifies_new_skill_when_root_has_existing_skills() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    write_skill(
        workspace_root.path(),
        "existing",
        "existing",
        "Existing skill",
        "Existing body\n",
    );
    let package_manager = WritingSkillPackageManager {
        directory_name: "code-review".to_string(),
        skill_name: "code-review".to_string(),
        description: "Review code".to_string(),
    };

    let request = InstallRemoteSkillRequest {
        package: "@skills/code-review@1.2.3".to_string(),
        source: "registry".to_string(),
        target: SkillInstallTarget::Project,
        package_url: None,
    };

    let installed =
        install_remote_skill_into_root(&package_manager, workspace_root.path(), request)
            .await
            .expect("newly installed skill should be identified among existing skills");

    assert_eq!(installed.id, "code-review");
    assert_eq!(installed.install_source, SkillInstallSource::Registry);
    let state = std::fs::read_to_string(workspace_root.path().join("skills-state.toml"))
        .expect("state should be written");
    assert!(state.contains("[skills.code-review]"));
    assert!(!state.contains("[skills.existing]"));
    assert!(!state.contains("[skills.1.2.3]"));
}

#[test]
fn delete_guard_rejects_root_directory_itself() {
    let workspace_root = tempfile::tempdir().expect("workspace root");

    let error = validate_directory_under_root(workspace_root.path(), workspace_root.path())
        .expect_err("root directory should not be a deletable skill directory");

    assert!(
        error.to_string().contains("skill root itself"),
        "message was: {error}"
    );
}

struct WritingSkillPackageManager {
    directory_name: String,
    skill_name: String,
    description: String,
}

#[async_trait::async_trait]
impl SkillPackageManager for WritingSkillPackageManager {
    async fn search(&self, _query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        Ok(Vec::new())
    }

    async fn install_from_registry(
        &self,
        install_root: &Path,
        _request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()> {
        write_skill(
            install_root,
            &self.directory_name,
            &self.skill_name,
            &self.description,
            "Installed body\n",
        );
        Ok(())
    }

    async fn install_from_github(
        &self,
        install_root: &Path,
        _request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        write_skill(
            install_root,
            &self.directory_name,
            &self.skill_name,
            &self.description,
            "Installed body\n",
        );
        Ok(())
    }

    async fn check_updates(&self, _skill_id: &str) -> agent_core::Result<SkillUpdateState> {
        Ok(SkillUpdateState::Unknown)
    }

    async fn update(&self, _skill_id: &str) -> agent_core::Result<()> {
        Ok(())
    }
}

fn write_skill(root: &Path, directory_name: &str, skill_name: &str, description: &str, body: &str) {
    let skill_directory = root.join(directory_name);
    std::fs::create_dir_all(&skill_directory).expect("skill directory should exist");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        format!("---\nname: {skill_name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill should be written");
}

fn write_skill_with_activation_mode(
    root: &Path,
    directory_name: &str,
    skill_name: &str,
    description: &str,
    activation_mode: &str,
    body: &str,
) {
    let skill_directory = root.join(directory_name);
    std::fs::create_dir_all(&skill_directory).expect("skill directory should exist");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        format!(
            "---\nname: {skill_name}\ndescription: {description}\nkairox:\n  activation:\n    mode: {activation_mode}\n---\n{body}"
        ),
    )
    .expect("skill should be written");
}
