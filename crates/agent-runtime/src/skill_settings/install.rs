use std::path::Path;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, SkillInstallTarget, SkillSettingsView,
};
use agent_core::CoreError;
use agent_skills::state::{read_skills_state, write_skills_state};

use super::roots::{install_root, SkillSettingsRoots};
use super::view::{find_skill_settings_view, list_skill_settings_from_roots};
use super::{skill_error, SKILLS_STATE_FILE_NAME};
use crate::skill_package::SkillPackageManager;

pub async fn install_remote_skill(
    roots: SkillSettingsRoots,
    package_manager: &dyn SkillPackageManager,
    request: InstallRemoteSkillRequest,
) -> agent_core::Result<SkillSettingsView> {
    let root = install_root(&roots, request.target)?;
    install_remote_skill_into_root(package_manager, &root, request).await
}

pub(super) async fn install_remote_skill_into_root(
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
            plugin_roots: Vec::new(),
        },
        SkillInstallTarget::User => SkillSettingsRoots {
            workspace_root: None,
            user_root: Some(root.to_path_buf()),
            builtin_root: None,
            plugin_roots: Vec::new(),
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
