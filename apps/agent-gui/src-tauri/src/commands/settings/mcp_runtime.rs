use super::*;

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
