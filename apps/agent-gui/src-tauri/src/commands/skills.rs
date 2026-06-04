use super::*;

#[tauri::command]
#[specta::specta]
pub async fn list_skills(state: State<'_, GuiState>) -> Result<Vec<agent_core::SkillView>, String> {
    let roots = current_skill_settings_roots(&state).await?;
    state
        .runtime
        .list_skills_with_roots(roots)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_skill_detail(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<agent_core::SkillDetail, String> {
    let roots = current_skill_settings_roots(&state).await?;
    state
        .runtime
        .get_skill_with_roots(roots, skill_id.clone())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Skill not found: {skill_id}"))
}

#[tauri::command]
#[specta::specta]
pub async fn activate_skill(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<agent_core::ActiveSkillView, String> {
    let workspace_id = {
        let workspace_id = state.workspace_id.lock().await;
        workspace_id.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current_session_id = state.current_session_id.lock().await;
        current_session_id.clone().ok_or("No active session")?
    };

    state
        .runtime
        .activate_skill(agent_core::ActivateSkillRequest {
            workspace_id,
            session_id,
            skill_id,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn deactivate_skill(state: State<'_, GuiState>, skill_id: String) -> Result<(), String> {
    let workspace_id = {
        let workspace_id = state.workspace_id.lock().await;
        workspace_id.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current_session_id = state.current_session_id.lock().await;
        current_session_id.clone().ok_or("No active session")?
    };

    state
        .runtime
        .deactivate_skill(agent_core::DeactivateSkillRequest {
            workspace_id,
            session_id,
            skill_id,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_active_skills(
    state: State<'_, GuiState>,
) -> Result<Vec<agent_core::ActiveSkillView>, String> {
    let session_id = {
        let current_session_id = state.current_session_id.lock().await;
        current_session_id.clone().ok_or("No active session")?
    };

    state
        .runtime
        .list_active_skills(session_id)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
#[specta::specta]
pub async fn list_skill_settings(
    state: State<'_, GuiState>,
) -> Result<Vec<SkillSettingsView>, String> {
    let roots = current_skill_settings_roots(&state).await?;
    agent_runtime::skill_settings::list_skill_settings(roots)
        .await
        .map_err(|error| error.to_string())
}

async fn current_skill_settings_roots(
    state: &State<'_, GuiState>,
) -> Result<agent_runtime::skill_settings::SkillSettingsRoots, String> {
    let roots = state.runtime.skill_settings_roots();
    let current_session_id = {
        let current = state.current_session_id.lock().await;
        current.clone()
    };
    let workspace_id = {
        let workspace_id = state.workspace_id.lock().await;
        workspace_id.clone()
    };
    let Some(workspace_id) = workspace_id else {
        return Ok(roots);
    };
    let projects = state
        .runtime
        .list_projects(&workspace_id)
        .await
        .map_err(|error| format!("Failed to list projects: {error}"))?;
    let mut project_sessions = Vec::new();
    for project in projects {
        let mut sessions = state
            .runtime
            .list_project_sessions(project.project_id)
            .await
            .map_err(|error| format!("Failed to list project sessions: {error}"))?;
        project_sessions.append(&mut sessions);
    }
    Ok(skill_settings_roots_for_session(
        roots,
        current_session_id.as_ref(),
        &project_sessions,
    ))
}

fn skill_settings_roots_for_session(
    mut roots: agent_runtime::skill_settings::SkillSettingsRoots,
    current_session_id: Option<&agent_core::SessionId>,
    project_sessions: &[SessionMeta],
) -> agent_runtime::skill_settings::SkillSettingsRoots {
    if let Some(current_session_id) = current_session_id {
        if let Some(project_session) = project_sessions
            .iter()
            .find(|session| session.session_id == *current_session_id)
        {
            if let Some(worktree_path) = &project_session.worktree_path {
                roots.workspace_root =
                    Some(std::path::Path::new(worktree_path).join(".kairox/skills"));
            }
        }
    }
    roots
}

#[tauri::command]
#[specta::specta]
pub async fn get_effective_skills(
    state: State<'_, GuiState>,
) -> Result<Vec<EffectiveSkillView>, String> {
    let roots = current_skill_settings_roots(&state).await?;
    let settings = agent_runtime::skill_settings::list_skill_settings(roots)
        .await
        .map_err(|e| e.to_string())?;
    Ok(settings
        .into_iter()
        .map(EffectiveSkillView::from_skill_settings)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn get_skill_settings_detail(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<SkillSettingsDetail, String> {
    let roots = current_skill_settings_roots(&state).await?;
    agent_runtime::skill_settings::get_skill_settings_detail(roots, &skill_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Skill not found: {skill_id}"))
}

#[tauri::command]
#[specta::specta]
pub async fn set_skill_enabled(
    state: State<'_, GuiState>,
    skill_id: String,
    enabled: bool,
) -> Result<(), String> {
    let roots = current_skill_settings_roots(&state).await?;
    agent_runtime::skill_settings::set_skill_enabled(roots, &skill_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_skill_settings(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<(), String> {
    let roots = current_skill_settings_roots(&state).await?;
    agent_runtime::skill_settings::delete_skill(roots, &skill_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn search_remote_skills(
    state: State<'_, GuiState>,
    query: String,
) -> Result<Vec<RemoteSkillSearchResult>, String> {
    state
        .runtime
        .search_remote_skills(query)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_remote_skill(
    state: State<'_, GuiState>,
    request: InstallRemoteSkillRequest,
) -> Result<SkillSettingsView, String> {
    let roots = current_skill_settings_roots(&state).await?;
    state
        .runtime
        .install_remote_skill_with_roots(roots, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_github_skill(
    state: State<'_, GuiState>,
    request: InstallGithubSkillRequest,
) -> Result<SkillSettingsView, String> {
    let roots = current_skill_settings_roots(&state).await?;
    state
        .runtime
        .install_github_skill_with_roots(roots, request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_skill(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<SkillSettingsView, String> {
    let roots = current_skill_settings_roots(&state).await?;
    state
        .runtime
        .update_skill_with_roots(roots, skill_id)
        .await
        .map_err(|error| error.to_string())
}

// ── Skill catalog ────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn list_skill_catalog(
    state: State<'_, GuiState>,
    query: SkillCatalogQuery,
) -> Result<Vec<SkillCatalogEntry>, String> {
    state
        .runtime
        .list_skill_catalog(query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_skill_sources(
    state: State<'_, GuiState>,
) -> Result<Vec<SkillSourceView>, String> {
    state
        .runtime
        .list_skill_sources()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn add_skill_source(
    state: State<'_, GuiState>,
    config: SkillSourceView,
) -> Result<(), String> {
    state
        .runtime
        .add_skill_source(config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_skill_source(state: State<'_, GuiState>, id: String) -> Result<(), String> {
    state
        .runtime
        .remove_skill_source(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_skill_source_enabled(
    state: State<'_, GuiState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_skill_source_enabled(id, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_skill_catalog(state: State<'_, GuiState>) -> Result<(), String> {
    state
        .runtime
        .refresh_skill_catalog()
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_meta(
        session_id: &str,
        project_id: Option<&str>,
        worktree_path: Option<&str>,
    ) -> SessionMeta {
        SessionMeta {
            project_id: project_id.map(|id| ProjectId::from_string(id.to_string())),
            worktree_path: worktree_path.map(str::to_string),
            branch: None,
            visibility: None,
            approval_policy: None,
            sandbox_policy: None,
            session_id: agent_core::SessionId::from_string(session_id.to_string()),
            workspace_id: agent_core::WorkspaceId::from_string("wrk_test".to_string()),
            title: "Test".to_string(),
            model_profile: "fake".to_string(),
            model_id: None,
            provider: None,
            deleted_at: None,
            created_at: "2026-06-04T00:00:00Z".to_string(),
            updated_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn project_session_uses_project_worktree_for_skill_settings_root() {
        let roots = agent_runtime::skill_settings::SkillSettingsRoots {
            workspace_root: Some("/app/.kairox/skills".into()),
            user_root: Some("/home/.config/kairox/skills".into()),
            builtin_root: Some("/home/.kairox/builtin-skills".into()),
            ..Default::default()
        };
        let current_session_id = agent_core::SessionId::from_string("ses_project".to_string());
        let project_sessions = vec![session_meta(
            "ses_project",
            Some("prj_test"),
            Some("/tmp/project-worktree"),
        )];

        let resolved =
            skill_settings_roots_for_session(roots, Some(&current_session_id), &project_sessions);

        assert_eq!(
            resolved.workspace_root.as_deref(),
            Some(std::path::Path::new("/tmp/project-worktree/.kairox/skills"))
        );
        assert_eq!(
            resolved.user_root.as_deref(),
            Some(std::path::Path::new("/home/.config/kairox/skills"))
        );
        assert_eq!(
            resolved.builtin_root.as_deref(),
            Some(std::path::Path::new("/home/.kairox/builtin-skills"))
        );
    }

    #[test]
    fn ordinary_session_keeps_existing_skill_settings_root() {
        let roots = agent_runtime::skill_settings::SkillSettingsRoots {
            workspace_root: Some("/app/.kairox/skills".into()),
            ..Default::default()
        };
        let current_session_id = agent_core::SessionId::from_string("ses_plain".to_string());
        let project_sessions = vec![session_meta(
            "ses_project",
            Some("prj_test"),
            Some("/tmp/project-worktree"),
        )];

        let resolved =
            skill_settings_roots_for_session(roots, Some(&current_session_id), &project_sessions);

        assert_eq!(
            resolved.workspace_root.as_deref(),
            Some(std::path::Path::new("/app/.kairox/skills"))
        );
    }
}
