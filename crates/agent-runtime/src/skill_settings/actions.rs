use std::path::{Path, PathBuf};

use agent_core::facade::{SkillSettingsScope, SkillSettingsView, SkillUpdateState};
use agent_core::CoreError;
use agent_skills::state::{read_skills_state, write_skills_state};

use super::roots::{root_for_scope, SkillSettingsRoots};
use super::view::{find_skill_settings_view, parse_skill_activation_mode};
use super::{scope_label, skill_error, SKILLS_STATE_FILE_NAME};
use crate::skill_package::SkillPackageManager;

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

fn reject_builtin_mutation(view: &SkillSettingsView, operation: &str) -> agent_core::Result<()> {
    if view.scope == SkillSettingsScope::Builtin || view.scope == SkillSettingsScope::Plugin {
        return Err(CoreError::InvalidState(format!(
            "cannot {operation} {}-scope skill: {}",
            scope_label(view.scope),
            view.id
        )));
    }
    Ok(())
}

pub(super) fn validate_directory_under_root(
    directory: &Path,
    root: &Path,
) -> agent_core::Result<()> {
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
