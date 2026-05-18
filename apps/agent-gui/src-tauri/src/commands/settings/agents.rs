use super::config_runtime::open_path_in_system_file_manager;
use super::*;
use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};

#[tauri::command]
#[specta::specta]
pub async fn list_agent_settings(
    state: State<'_, GuiState>,
) -> Result<Vec<AgentSettingsView>, String> {
    state
        .runtime
        .list_agent_settings()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_agent_settings(
    state: State<'_, GuiState>,
    input: AgentSettingsInput,
) -> Result<AgentSettingsView, String> {
    state
        .runtime
        .upsert_agent_settings(input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_agent_settings(
    state: State<'_, GuiState>,
    agent_id: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_agent_settings(agent_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn copy_agent_settings(
    state: State<'_, GuiState>,
    agent_id: String,
    scope: AgentSettingsScope,
) -> Result<AgentSettingsView, String> {
    state
        .runtime
        .copy_agent_settings(agent_id, scope)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn open_agents_dir(state: State<'_, GuiState>) -> Result<Option<String>, String> {
    let Some(agents_dir) = state
        .runtime
        .open_agents_dir()
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
