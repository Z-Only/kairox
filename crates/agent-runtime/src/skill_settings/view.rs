use agent_core::facade::{
    SkillInstallSource, SkillSettingsDetail, SkillSettingsScope, SkillSettingsView,
    SkillUpdateState,
};
use agent_core::CoreError;
use agent_skills::settings::LocalSkillSettingsView;
use agent_skills::{SkillActivationMode, SkillSourceKind};

use super::roots::{skill_roots, SkillSettingsRoots};
use super::{scope_label, skill_error};
use crate::skills::skill_activation_mode_to_string;

pub async fn list_skill_settings(
    roots: SkillSettingsRoots,
) -> agent_core::Result<Vec<SkillSettingsView>> {
    list_skill_settings_from_roots(roots).await
}

pub(super) async fn list_skill_settings_from_roots(
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

pub(super) async fn find_skill_settings_view(
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

fn local_view_to_core_view(view: LocalSkillSettingsView) -> SkillSettingsView {
    let scope = skill_scope_to_settings_scope(view.scope);
    let permission_summary = permission_summary(&view.tools, &view.can_request_tools);
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
        tools: view.tools,
        can_request_tools: view.can_request_tools,
        permission_summary,
        install_source: install_source_from_string(&view.install_source, scope),
        update_state: update_state_from_available(view.update_available),
        effective: view.effective,
        shadowed_by: view.shadowed_by,
        valid: view.valid,
        validation_error: view.validation_error,
        editable: scope != SkillSettingsScope::Builtin && scope != SkillSettingsScope::Plugin,
        deletable: scope != SkillSettingsScope::Builtin && scope != SkillSettingsScope::Plugin,
    }
}

fn permission_summary(tools: &[String], can_request_tools: &[String]) -> String {
    match (tools.is_empty(), can_request_tools.is_empty()) {
        (true, true) => "no tool permissions declared".to_string(),
        (false, true) => format!("tools: {}", tools.join(", ")),
        (true, false) => format!("can request: {}", can_request_tools.join(", ")),
        (false, false) => format!(
            "tools: {}; can request: {}",
            tools.join(", "),
            can_request_tools.join(", ")
        ),
    }
}

fn skill_scope_to_settings_scope(scope: SkillSourceKind) -> SkillSettingsScope {
    match scope {
        SkillSourceKind::Builtin => SkillSettingsScope::Builtin,
        SkillSourceKind::User => SkillSettingsScope::User,
        SkillSourceKind::Workspace => SkillSettingsScope::Project,
        SkillSourceKind::Plugin => SkillSettingsScope::Plugin,
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

pub(super) fn parse_skill_activation_mode(
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

fn skill_settings_id(scope: SkillSettingsScope, skill_id: &str) -> String {
    format!("{}:{skill_id}", scope_label(scope))
}
