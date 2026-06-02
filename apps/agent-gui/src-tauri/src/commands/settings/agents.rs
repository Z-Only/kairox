use super::config_runtime::open_path_in_system_file_manager;
use super::*;
use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};

#[tauri::command]
#[specta::specta]
pub async fn list_agent_settings(
    state: State<'_, GuiState>,
    project_root: Option<String>,
) -> Result<Vec<AgentSettingsView>, String> {
    state
        .runtime
        .list_agent_settings_for_project(project_root)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_agent_settings(
    state: State<'_, GuiState>,
    input: AgentSettingsInput,
    project_root: Option<String>,
) -> Result<AgentSettingsView, String> {
    state
        .runtime
        .upsert_agent_settings_for_project(input, project_root)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_agent_settings(
    state: State<'_, GuiState>,
    agent_id: String,
    project_root: Option<String>,
) -> Result<(), String> {
    state
        .runtime
        .delete_agent_settings_for_project(agent_id, project_root)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn copy_agent_settings(
    state: State<'_, GuiState>,
    agent_id: String,
    scope: AgentSettingsScope,
    project_root: Option<String>,
) -> Result<AgentSettingsView, String> {
    state
        .runtime
        .copy_agent_settings_for_project(agent_id, scope, project_root)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn open_agents_dir(
    state: State<'_, GuiState>,
    project_root: Option<String>,
) -> Result<Option<String>, String> {
    let Some(agents_dir) = state
        .runtime
        .open_agents_dir_for_project(project_root)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let agents_dir = std::path::PathBuf::from(agents_dir);
    std::fs::create_dir_all(&agents_dir)
        .map_err(|error| format!("failed to create agents dir: {error}"))?;
    open_path_in_system_file_manager(&agents_dir)?;
    Ok(Some(agents_dir.display().to_string()))
}
