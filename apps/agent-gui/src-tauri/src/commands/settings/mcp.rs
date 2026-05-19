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
    let disabled: std::collections::HashSet<&str> = config
        .disabled_mcp_servers
        .iter()
        .map(|s| s.as_str())
        .collect();

    Ok(settings
        .into_iter()
        .map(|view| {
            let source = parse_mcp_source_to_scope(&view.source);
            let disabled_by = if disabled.contains(view.id.as_str()) {
                Some(agent_core::config_scope::ConfigScope::Project)
            } else {
                None
            };
            EffectiveMcpServerView {
                value: view.clone(),
                source,
                overrides: None,
                enabled: disabled_by.is_none() && view.enabled,
                disabled_by,
                writable: view.writable,
                deletable: view.writable,
            }
        })
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

fn parse_mcp_source_to_scope(source: &str) -> agent_core::config_scope::ConfigScope {
    match source {
        "user_config" => agent_core::config_scope::ConfigScope::User,
        "project_config" => agent_core::config_scope::ConfigScope::Project,
        "defaults" => agent_core::config_scope::ConfigScope::Builtin,
        _ => agent_core::config_scope::ConfigScope::Builtin,
    }
}
