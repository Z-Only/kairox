use super::config_runtime::open_path_in_system_file_manager;
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

/// Disable an MCP server at the project scope by adding its ID to
/// `disabled_mcp_servers` in `.kairox/config.toml`.
#[tauri::command]
#[specta::specta]
pub async fn disable_mcp_server_at_scope(
    state: State<'_, GuiState>,
    server_id: String,
    project_root: String,
) -> Result<(), String> {
    use std::collections::HashSet;

    let config_path = std::path::Path::new(&project_root)
        .join(".kairox")
        .join("config.toml");

    let raw = match std::fs::read_to_string(&config_path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("failed to read project config: {e}")),
    };

    let mut doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|e| format!("failed to parse project config: {e}"))?;

    let mut disabled: HashSet<String> = doc
        .get("disabled_mcp_servers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    disabled.insert(server_id);

    let mut values: Vec<_> = disabled.into_iter().collect();
    values.sort();
    let mut arr = toml_edit::Array::new();
    for value in values {
        arr.push(value);
    }
    doc["disabled_mcp_servers"] = toml_edit::value(arr);

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create config dir: {e}"))?;
    }
    std::fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("failed to write project config: {e}"))?;

    state.refresh_config_for_project(std::path::Path::new(&project_root))?;
    Ok(())
}

/// Enable an MCP server at the project scope by removing its ID from
/// `disabled_mcp_servers` in `.kairox/config.toml`.
#[tauri::command]
#[specta::specta]
pub async fn enable_mcp_server_at_scope(
    state: State<'_, GuiState>,
    server_id: String,
    project_root: String,
) -> Result<(), String> {
    let config_path = std::path::Path::new(&project_root)
        .join(".kairox")
        .join("config.toml");

    let raw = match std::fs::read_to_string(&config_path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("failed to read project config: {e}")),
    };

    let mut doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|e| format!("failed to parse project config: {e}"))?;

    let mut disabled: Vec<String> = doc
        .get("disabled_mcp_servers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .filter(|id| id != &server_id)
                .collect()
        })
        .unwrap_or_default();
    disabled.sort();

    if disabled.is_empty() {
        doc.remove("disabled_mcp_servers");
    } else {
        let mut arr = toml_edit::Array::new();
        for value in disabled {
            arr.push(value);
        }
        doc["disabled_mcp_servers"] = toml_edit::value(arr);
    }

    std::fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("failed to write project config: {e}"))?;

    state.refresh_config_for_project(std::path::Path::new(&project_root))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn open_mcp_config_file(state: State<'_, GuiState>) -> Result<Option<String>, String> {
    let Some(config_file_path) = state
        .runtime
        .open_mcp_config_file()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };

    let config_file_path = std::path::PathBuf::from(config_file_path);
    open_path_in_system_file_manager(&config_file_path)?;
    Ok(Some(config_file_path.display().to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn list_mcp_servers(
    state: State<'_, GuiState>,
) -> Result<Vec<McpServerStatusResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            Ok(manager
                .server_statuses()
                .into_iter()
                .map(|(id, status)| McpServerStatusResponse {
                    id,
                    status,
                    tool_count: None,
                })
                .collect())
        }
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn start_mcp_server(server_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let mut manager = manager.lock().await;
            manager
                .ensure_server(&server_id)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn stop_mcp_server(server_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let mut manager = manager.lock().await;
            manager
                .shutdown_server(&server_id)
                .await
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_mcp_tools(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<McpToolDefResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let mut manager = manager.lock().await;
            manager
                .refresh_tools(&server_id)
                .await
                .map(|tools| {
                    tools
                        .into_iter()
                        .map(|t| McpToolDefResponse {
                            name: t.name,
                            description: t.description,
                            input_schema: t.input_schema,
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn trust_mcp_server(server_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .trust_server(&server_id)
                .await
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn revoke_mcp_trust(server_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .revoke_trust(&server_id)
                .await
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn list_mcp_resources(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<McpResourceDefResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .list_resources(&server_id)
                .await
                .map(|r| {
                    r.into_iter()
                        .map(|r| McpResourceDefResponse {
                            uri: r.uri,
                            name: r.name,
                            description: r.description,
                            mime_type: r.mime_type,
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn list_mcp_prompts(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<McpPromptDefResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .list_prompts(&server_id)
                .await
                .map(|p| {
                    p.into_iter()
                        .map(|p| McpPromptDefResponse {
                            name: p.name,
                            description: p.description,
                            argument_count: p.arguments.len(),
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn read_mcp_resource(
    server_id: String,
    uri: String,
    state: State<'_, GuiState>,
) -> Result<Vec<McpContentBlockResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .read_resource(&server_id, &uri)
                .await
                .map(|blocks| {
                    blocks
                        .into_iter()
                        .map(|b| match b {
                            agent_mcp::McpContentBlock::Text { text } => {
                                McpContentBlockResponse::Text { text }
                            }
                            agent_mcp::McpContentBlock::Image { data, mime_type } => {
                                McpContentBlockResponse::Image { data, mime_type }
                            }
                            agent_mcp::McpContentBlock::Resource { resource } => {
                                McpContentBlockResponse::Resource {
                                    uri: resource.uri,
                                    name: String::new(),
                                    mime_type: resource.mime_type,
                                }
                            }
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn test_mcp_connectivity(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<agent_mcp::ConnectivityResult, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let mut manager = manager.lock().await;
            manager
                .test_connectivity(&server_id, Some(std::time::Duration::from_secs(15)))
                .await
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn check_mcp_health(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<CheckMcpHealthResponse, String> {
    let runtime = state.runtime.clone();
    let result = runtime
        .check_mcp_health(&server_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CheckMcpHealthResponse {
        tools: result
            .tools
            .into_iter()
            .map(|t| McpToolDefResponse {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
            })
            .collect(),
        healthy: result.healthy,
        error: result.error,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn set_mcp_tool_disabled(
    server_id: String,
    tool_name: String,
    disabled: bool,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let runtime = state.runtime.clone();
    runtime
        .set_mcp_tool_disabled(&server_id, &tool_name, disabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_mcp_tool_states(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<McpToolStatesResponse, String> {
    let runtime = state.runtime.clone();
    let disabled = runtime
        .get_mcp_disabled_tools(&server_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(McpToolStatesResponse {
        disabled_tools: disabled.into_iter().collect(),
    })
}

fn parse_mcp_source_to_scope(source: &str) -> agent_core::config_scope::ConfigScope {
    match source {
        "user_config" => agent_core::config_scope::ConfigScope::User,
        "project_config" => agent_core::config_scope::ConfigScope::Project,
        "defaults" => agent_core::config_scope::ConfigScope::Builtin,
        _ => agent_core::config_scope::ConfigScope::Builtin,
    }
}
