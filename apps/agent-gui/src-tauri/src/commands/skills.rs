use super::*;

#[tauri::command]
#[specta::specta]
pub async fn list_skills(state: State<'_, GuiState>) -> Result<Vec<agent_core::SkillView>, String> {
    state
        .runtime
        .list_skills()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_skill_detail(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<agent_core::SkillDetail, String> {
    state
        .runtime
        .get_skill(skill_id.clone())
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
    state
        .runtime
        .list_skill_settings()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_skill_settings_detail(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<SkillSettingsDetail, String> {
    state
        .runtime
        .get_skill_settings_detail(skill_id.clone())
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
    state
        .runtime
        .set_skill_enabled(skill_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_skill_settings(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_skill_settings(skill_id)
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
    state
        .runtime
        .install_remote_skill(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_github_skill(
    state: State<'_, GuiState>,
    request: InstallGithubSkillRequest,
) -> Result<SkillSettingsView, String> {
    state
        .runtime
        .install_github_skill(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_skill(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<SkillSettingsView, String> {
    state
        .runtime
        .update_skill(skill_id)
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
