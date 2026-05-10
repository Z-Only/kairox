use std::path::{Path, PathBuf};

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, SkillInstallSource, SkillInstallTarget,
    SkillSettingsDetail, SkillSettingsScope, SkillSettingsView, SkillUpdateState,
};
use agent_core::CoreError;
use agent_skills::settings::LocalSkillSettingsView;
use agent_skills::state::{read_skills_state, write_skills_state};
use agent_skills::{SkillActivationMode, SkillRoot, SkillSourceKind};

use crate::skill_package::SkillPackageManager;
use crate::skills::skill_activation_mode_to_string;

const SKILLS_STATE_FILE_NAME: &str = "skills-state.toml";

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct SkillSettingsRoots {
    pub workspace_root: Option<PathBuf>,
    pub user_root: Option<PathBuf>,
    pub builtin_root: Option<PathBuf>,
}

pub async fn list_skill_settings(
    roots: SkillSettingsRoots,
) -> agent_core::Result<Vec<SkillSettingsView>> {
    list_skill_settings_from_roots(roots).await
}

pub async fn list_skill_settings_from_roots(
    roots: SkillSettingsRoots,
) -> agent_core::Result<Vec<SkillSettingsView>> {
    let projection = agent_skills::settings::discover_skill_settings(skill_roots(&roots))
        .await
        .map_err(skill_error)?;

    Ok(projection
        .skills
        .into_iter()
        .map(local_view_to_core_view)
        .collect())
}

pub async fn get_skill_settings_detail(
    roots: SkillSettingsRoots,
    skill_id: &str,
) -> agent_core::Result<Option<SkillSettingsDetail>> {
    let views = list_skill_settings_from_roots(roots).await?;
    let matching_settings_id_views = views
        .iter()
        .filter(|view| view.settings_id == skill_id)
        .cloned()
        .collect::<Vec<_>>();
    let view = match matching_settings_id_views.as_slice() {
        [view] => view.clone(),
        [] => {
            let matching_id_views = views
                .iter()
                .filter(|candidate| candidate.id == skill_id)
                .cloned()
                .collect::<Vec<_>>();
            match matching_id_views.as_slice() {
                [view] => view.clone(),
                [] => return Ok(None),
                _ => {
                    return Err(CoreError::InvalidState(format!(
                        "ambiguous skill id: {skill_id}"
                    )));
                }
            }
        }
        _ => {
            return Err(CoreError::InvalidState(format!(
                "ambiguous skill settings id: {skill_id}"
            )));
        }
    };
    let matching_views = views
        .into_iter()
        .filter(|candidate| candidate.id == view.id)
        .collect::<Vec<_>>();

    let content = tokio::fs::read_to_string(&view.path)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to read skill: {error}")))?;

    Ok(Some(SkillSettingsDetail {
        view,
        content,
        source_chain: matching_views,
    }))
}

pub async fn set_skill_enabled(
    roots: SkillSettingsRoots,
    skill_id: &str,
    enabled: bool,
) -> agent_core::Result<()> {
    let view = find_skill_settings_view(roots.clone(), skill_id).await?;
    reject_builtin_mutation(&view, "enable or disable")?;
    let root = root_for_scope(&roots, view.scope).ok_or_else(|| {
        CoreError::InvalidState(format!(
            "skill root not configured for {}",
            scope_label(view.scope)
        ))
    })?;
    let state_path = root.join(SKILLS_STATE_FILE_NAME);
    let mut state = read_skills_state(&state_path).await.map_err(skill_error)?;
    state.set_enabled(&view.id, enabled);
    write_skills_state(&state_path, &state)
        .await
        .map_err(skill_error)
}

pub async fn set_skill_activation_mode(
    roots: SkillSettingsRoots,
    skill_id: &str,
    activation_mode: &str,
) -> agent_core::Result<()> {
    let parsed_activation_mode = parse_skill_activation_mode(activation_mode)?;
    let view = find_skill_settings_view(roots.clone(), skill_id).await?;
    reject_builtin_mutation(&view, "set activation mode for")?;
    let root = root_for_scope(&roots, view.scope).ok_or_else(|| {
        CoreError::InvalidState(format!(
            "skill root not configured for {}",
            scope_label(view.scope)
        ))
    })?;
    let state_path = root.join(SKILLS_STATE_FILE_NAME);
    let mut state = read_skills_state(&state_path).await.map_err(skill_error)?;
    state.skill_mut(&view.id).activation_mode = Some(parsed_activation_mode);
    write_skills_state(&state_path, &state)
        .await
        .map_err(skill_error)
}

pub async fn delete_skill(roots: SkillSettingsRoots, skill_id: &str) -> agent_core::Result<()> {
    let view = find_skill_settings_view(roots.clone(), skill_id).await?;
    reject_builtin_mutation(&view, "delete")?;
    if !view.deletable {
        return Err(CoreError::InvalidState(format!(
            "skill is not deletable: {skill_id}"
        )));
    }

    let root = root_for_scope(&roots, view.scope).ok_or_else(|| {
        CoreError::InvalidState(format!(
            "skill root not configured for {}",
            scope_label(view.scope)
        ))
    })?;
    let skill_directory = PathBuf::from(&view.path)
        .parent()
        .ok_or_else(|| CoreError::InvalidState(format!("invalid skill path: {}", view.path)))?
        .to_path_buf();
    validate_directory_under_root(&skill_directory, &root)?;
    tokio::fs::remove_dir_all(&skill_directory)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to delete skill: {error}")))
}

pub async fn install_remote_skill(
    roots: SkillSettingsRoots,
    package_manager: &dyn SkillPackageManager,
    request: InstallRemoteSkillRequest,
) -> agent_core::Result<SkillSettingsView> {
    let root = install_root(&roots, request.target)?;
    install_remote_skill_into_root(package_manager, &root, request).await
}

pub async fn install_remote_skill_into_root(
    package_manager: &dyn SkillPackageManager,
    root: &Path,
    request: InstallRemoteSkillRequest,
) -> agent_core::Result<SkillSettingsView> {
    let before_views =
        list_skill_settings_from_roots(roots_for_install_target(root, request.target)).await?;
    package_manager
        .install_from_registry(root, &request)
        .await?;
    let installed = refresh_installed_view(root, request.target, &before_views).await?;
    write_install_state(root, &installed.id, "registry", &request.package).await?;
    refresh_installed_view_by_id(root, request.target, &installed.id).await
}

pub async fn install_github_skill(
    roots: SkillSettingsRoots,
    package_manager: &dyn SkillPackageManager,
    request: InstallGithubSkillRequest,
) -> agent_core::Result<SkillSettingsView> {
    let root = install_root(&roots, request.target)?;
    let before_views =
        list_skill_settings_from_roots(roots_for_install_target(&root, request.target)).await?;
    package_manager.install_from_github(&root, &request).await?;
    let installed = refresh_installed_view(&root, request.target, &before_views).await?;
    write_install_state(&root, &installed.id, "github", &request.source).await?;
    refresh_installed_view_by_id(&root, request.target, &installed.id).await
}

pub async fn update_skill(
    roots: SkillSettingsRoots,
    package_manager: &dyn SkillPackageManager,
    skill_id: &str,
) -> agent_core::Result<SkillSettingsView> {
    let view = find_skill_settings_view(roots.clone(), skill_id).await?;
    reject_builtin_mutation(&view, "update")?;
    package_manager.update(&view.id).await?;
    let update_state = package_manager.check_updates(&view.id).await?;
    let root = root_for_scope(&roots, view.scope).ok_or_else(|| {
        CoreError::InvalidState(format!(
            "skill root not configured for {}",
            scope_label(view.scope)
        ))
    })?;
    let state_path = root.join(SKILLS_STATE_FILE_NAME);
    let mut state = read_skills_state(&state_path).await.map_err(skill_error)?;
    state.skill_mut(&view.id).update_available = match update_state {
        SkillUpdateState::UpdateAvailable => Some(true),
        SkillUpdateState::UpToDate => Some(false),
        SkillUpdateState::Unknown | SkillUpdateState::CheckFailed => None,
    };
    write_skills_state(&state_path, &state)
        .await
        .map_err(skill_error)?;

    find_skill_settings_view(roots, &view.settings_id).await
}

fn skill_roots(roots: &SkillSettingsRoots) -> Vec<SkillRoot> {
    let mut skill_roots = Vec::new();
    if let Some(root) = &roots.builtin_root {
        skill_roots.push(SkillRoot::new(SkillSourceKind::Builtin, root));
    }
    if let Some(root) = &roots.user_root {
        skill_roots.push(SkillRoot::new(SkillSourceKind::User, root));
    }
    if let Some(root) = &roots.workspace_root {
        skill_roots.push(SkillRoot::new(SkillSourceKind::Workspace, root));
    }
    skill_roots
}

fn local_view_to_core_view(view: LocalSkillSettingsView) -> SkillSettingsView {
    let scope = skill_scope_to_settings_scope(view.scope);
    SkillSettingsView {
        settings_id: skill_settings_id(scope, view.id.as_str()),
        id: view.id.as_str().to_string(),
        name: view.name,
        description: view.description,
        version: view.version,
        scope,
        path: view.path.display().to_string(),
        enabled: view.enabled,
        activation_mode: skill_activation_mode_to_string(view.activation_mode),
        install_source: install_source_from_string(&view.install_source, scope),
        update_state: update_state_from_available(view.update_available),
        effective: view.effective,
        shadowed_by: view.shadowed_by,
        valid: view.valid,
        validation_error: view.validation_error,
        editable: scope != SkillSettingsScope::Builtin,
        deletable: scope != SkillSettingsScope::Builtin,
    }
}

fn skill_scope_to_settings_scope(scope: SkillSourceKind) -> SkillSettingsScope {
    match scope {
        SkillSourceKind::Builtin => SkillSettingsScope::Builtin,
        SkillSourceKind::User => SkillSettingsScope::User,
        SkillSourceKind::Workspace => SkillSettingsScope::Project,
    }
}

fn install_source_from_string(raw_source: &str, scope: SkillSettingsScope) -> SkillInstallSource {
    match raw_source {
        "local" => SkillInstallSource::Local,
        "registry" => SkillInstallSource::Registry,
        "github" | "git" => SkillInstallSource::Github,
        "builtin" => SkillInstallSource::Builtin,
        _ if scope == SkillSettingsScope::Builtin => SkillInstallSource::Builtin,
        _ => SkillInstallSource::Unknown,
    }
}

fn update_state_from_available(update_available: Option<bool>) -> SkillUpdateState {
    match update_available {
        Some(true) => SkillUpdateState::UpdateAvailable,
        Some(false) => SkillUpdateState::UpToDate,
        None => SkillUpdateState::Unknown,
    }
}

fn parse_skill_activation_mode(
    raw_activation_mode: &str,
) -> agent_core::Result<SkillActivationMode> {
    match raw_activation_mode {
        "manual" => Ok(SkillActivationMode::Manual),
        "suggest" => Ok(SkillActivationMode::Suggest),
        "auto" => Ok(SkillActivationMode::Auto),
        _ => Err(CoreError::InvalidState(format!(
            "invalid skill activation mode: {raw_activation_mode}"
        ))),
    }
}

async fn find_skill_settings_view(
    roots: SkillSettingsRoots,
    skill_identifier: &str,
) -> agent_core::Result<SkillSettingsView> {
    let views = list_skill_settings_from_roots(roots).await?;
    let matching_settings_id_views = views
        .iter()
        .filter(|view| view.settings_id == skill_identifier)
        .cloned()
        .collect::<Vec<_>>();
    match matching_settings_id_views.as_slice() {
        [view] => return Ok(view.clone()),
        [] => {}
        _ => {
            return Err(CoreError::InvalidState(format!(
                "ambiguous skill settings id: {skill_identifier}"
            )));
        }
    }

    let matching_views = views
        .into_iter()
        .filter(|view| view.id == skill_identifier)
        .collect::<Vec<_>>();
    match matching_views.as_slice() {
        [view] => Ok(view.clone()),
        [] => Err(CoreError::InvalidState(format!(
            "skill not found: {skill_identifier}"
        ))),
        views => Err(CoreError::InvalidState(format!(
            "ambiguous skill id: {skill_identifier}; matching scopes: {}",
            views
                .iter()
                .map(|view| scope_label(view.scope))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn skill_settings_id(scope: SkillSettingsScope, skill_id: &str) -> String {
    format!("{}:{skill_id}", scope_label(scope))
}

fn reject_builtin_mutation(view: &SkillSettingsView, operation: &str) -> agent_core::Result<()> {
    if view.scope == SkillSettingsScope::Builtin {
        return Err(CoreError::InvalidState(format!(
            "cannot {operation} built-in skill: {}",
            view.id
        )));
    }
    Ok(())
}

fn root_for_scope(roots: &SkillSettingsRoots, scope: SkillSettingsScope) -> Option<PathBuf> {
    match scope {
        SkillSettingsScope::Project => roots.workspace_root.clone(),
        SkillSettingsScope::User => roots.user_root.clone(),
        SkillSettingsScope::Builtin => roots.builtin_root.clone(),
    }
}

fn install_root(
    roots: &SkillSettingsRoots,
    target: SkillInstallTarget,
) -> agent_core::Result<PathBuf> {
    match target {
        SkillInstallTarget::Project => roots.workspace_root.clone(),
        SkillInstallTarget::User => roots.user_root.clone(),
    }
    .ok_or_else(|| {
        CoreError::InvalidState(format!("skill install root not configured for {target:?}"))
    })
}

async fn refresh_installed_view(
    root: &Path,
    target: SkillInstallTarget,
    before_views: &[SkillSettingsView],
) -> agent_core::Result<SkillSettingsView> {
    let after_views =
        list_skill_settings_from_roots(roots_for_install_target(root, target)).await?;
    let before_paths = before_views
        .iter()
        .map(|view| view.path.as_str())
        .collect::<Vec<_>>();
    let added_views = after_views
        .iter()
        .filter(|view| !before_paths.contains(&view.path.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    match added_views.as_slice() {
        [view] => Ok(view.clone()),
        [] => match after_views.as_slice() {
            [view] => Ok(view.clone()),
            [] => Err(CoreError::InvalidState(
                "installed skill was not found after refresh".to_string(),
            )),
            _ => Err(CoreError::InvalidState(
                "installed skill refresh did not find a new skill; unable to identify installed skill"
                    .to_string(),
            )),
        },
        _ => Err(CoreError::InvalidState(
            "installed skill refresh found multiple new skills; unable to identify installed skill"
                .to_string(),
        )),
    }
}

async fn refresh_installed_view_by_id(
    root: &Path,
    target: SkillInstallTarget,
    skill_id: &str,
) -> agent_core::Result<SkillSettingsView> {
    find_skill_settings_view(roots_for_install_target(root, target), skill_id)
        .await
        .map_err(|error| {
            CoreError::InvalidState(format!(
                "installed skill was not found after refresh: {skill_id}: {error}"
            ))
        })
}

fn roots_for_install_target(root: &Path, target: SkillInstallTarget) -> SkillSettingsRoots {
    match target {
        SkillInstallTarget::Project => SkillSettingsRoots {
            workspace_root: Some(root.to_path_buf()),
            user_root: None,
            builtin_root: None,
        },
        SkillInstallTarget::User => SkillSettingsRoots {
            workspace_root: None,
            user_root: Some(root.to_path_buf()),
            builtin_root: None,
        },
    }
}

async fn write_install_state(
    root: &Path,
    skill_id: &str,
    install_source: &str,
    remote: &str,
) -> agent_core::Result<()> {
    let state_path = root.join(SKILLS_STATE_FILE_NAME);
    let mut state = read_skills_state(&state_path).await.map_err(skill_error)?;
    let state_entry = state.skill_mut(skill_id);
    state_entry.install_source = Some(install_source.to_string());
    state_entry.remote = Some(remote.to_string());
    write_skills_state(&state_path, &state)
        .await
        .map_err(skill_error)
}

fn validate_directory_under_root(directory: &Path, root: &Path) -> agent_core::Result<()> {
    let canonical_root = root.canonicalize().map_err(|error| {
        CoreError::InvalidState(format!("failed to canonicalize skill root: {error}"))
    })?;
    let canonical_directory = directory.canonicalize().map_err(|error| {
        CoreError::InvalidState(format!("failed to canonicalize skill directory: {error}"))
    })?;

    if canonical_directory == canonical_root {
        return Err(CoreError::InvalidState(format!(
            "refusing to delete skill root itself: {}",
            directory.display()
        )));
    }

    if !canonical_directory.starts_with(&canonical_root) {
        return Err(CoreError::InvalidState(format!(
            "refusing to delete skill outside configured root: {}",
            directory.display()
        )));
    }

    Ok(())
}

fn scope_label(scope: SkillSettingsScope) -> &'static str {
    match scope {
        SkillSettingsScope::Project => "project",
        SkillSettingsScope::User => "user",
        SkillSettingsScope::Builtin => "builtin",
    }
}

fn skill_error(error: agent_skills::SkillError) -> CoreError {
    CoreError::InvalidState(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use agent_core::facade::{
        InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
        SkillInstallSource, SkillInstallTarget, SkillSettingsScope, SkillUpdateState,
    };

    use super::{
        install_remote_skill_into_root, list_skill_settings_from_roots, set_skill_activation_mode,
        set_skill_enabled, validate_directory_under_root, SkillSettingsRoots,
    };
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

        let workspace_state =
            std::fs::read_to_string(workspace_root.path().join("skills-state.toml"))
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

    fn write_skill(
        root: &Path,
        directory_name: &str,
        skill_name: &str,
        description: &str,
        body: &str,
    ) {
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
}
