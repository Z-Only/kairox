use super::*;

#[tauri::command]
#[specta::specta]
pub async fn list_mcp_server_settings(
    state: State<'_, GuiState>,
    source_filter: Option<String>,
) -> Result<Vec<McpServerSettingsView>, String> {
    state
        .runtime
        .list_mcp_server_settings(source_filter)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_effective_mcp_servers(
    state: State<'_, GuiState>,
) -> Result<Vec<EffectiveMcpServerView>, String> {
    let settings = state
        .runtime
        .list_mcp_server_settings(None)
        .await
        .map_err(|e| e.to_string())?;

    let config = state.config.read().map_err(|e| e.to_string())?;
    Ok(agent_config::build_effective_mcp_server_settings_views(
        settings,
        &config.disabled_mcp_servers,
    )
    .into_iter()
    .map(EffectiveMcpServerView::from_effective)
    .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_mcp_server_settings(
    state: State<'_, GuiState>,
    input: McpServerSettingsInput,
) -> Result<McpServerSettingsView, String> {
    state
        .runtime
        .upsert_mcp_server_settings(input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_mcp_server_enabled(
    state: State<'_, GuiState>,
    server_id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_mcp_server_enabled(server_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_mcp_server_settings(
    state: State<'_, GuiState>,
    server_id: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_mcp_server_settings(server_id)
        .await
        .map_err(|error| error.to_string())
}
