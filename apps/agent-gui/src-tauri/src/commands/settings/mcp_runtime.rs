use super::*;

fn map_tool_def(t: agent_mcp::types::McpToolDef) -> McpToolDefResponse {
    McpToolDefResponse {
        name: t.name,
        description: t.description,
        input_schema: t.input_schema,
    }
}

fn map_resource_def(r: agent_mcp::types::McpResourceDef) -> McpResourceDefResponse {
    McpResourceDefResponse {
        uri: r.uri,
        name: r.name,
        description: r.description,
        mime_type: r.mime_type,
    }
}

fn map_prompt_def(p: agent_mcp::types::McpPromptDef) -> McpPromptDefResponse {
    McpPromptDefResponse {
        name: p.name,
        description: p.description,
        argument_count: p.arguments.len(),
    }
}

fn map_content_block(b: agent_mcp::types::McpContentBlock) -> McpContentBlockResponse {
    match b {
        agent_mcp::McpContentBlock::Text { text } => McpContentBlockResponse::Text { text },
        agent_mcp::McpContentBlock::Image { data, mime_type } => {
            McpContentBlockResponse::Image { data, mime_type }
        }
        agent_mcp::McpContentBlock::Resource { resource } => McpContentBlockResponse::Resource {
            uri: resource.uri,
            name: String::new(),
            mime_type: resource.mime_type,
        },
    }
}

fn map_health_result(result: agent_mcp::types::CheckHealthResult) -> CheckMcpHealthResponse {
    CheckMcpHealthResponse {
        tools: result.tools.into_iter().map(map_tool_def).collect(),
        healthy: result.healthy,
        error: result.error,
    }
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
                .map(|tools| tools.into_iter().map(map_tool_def).collect())
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
                .map(|r| r.into_iter().map(map_resource_def).collect())
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
                .map(|p| p.into_iter().map(map_prompt_def).collect())
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
                .map(|blocks| blocks.into_iter().map(map_content_block).collect())
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
    Ok(map_health_result(result))
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_mcp::types::{
        CheckHealthResult, McpContentBlock, McpPromptArgument, McpPromptDef, McpResourceContent,
        McpResourceDef, McpToolDef,
    };
    use serde_json::json;

    #[test]
    fn mcp_runtime_maps_tool_dto_fields() {
        let response = map_tool_def(McpToolDef {
            name: "search".into(),
            description: Some("Search files".into()),
            input_schema: Some(r#"{"type":"object"}"#.into()),
        });

        assert_eq!(response.name, "search");
        assert_eq!(response.description.as_deref(), Some("Search files"));
        assert_eq!(
            response.input_schema.as_deref(),
            Some(r#"{"type":"object"}"#)
        );
    }

    #[test]
    fn mcp_runtime_maps_resource_dto_fields() {
        let response = map_resource_def(McpResourceDef {
            uri: "file:///tmp/context.md".into(),
            name: "context".into(),
            description: Some("Context file".into()),
            mime_type: Some("text/markdown".into()),
        });

        assert_eq!(response.uri, "file:///tmp/context.md");
        assert_eq!(response.name, "context");
        assert_eq!(response.description.as_deref(), Some("Context file"));
        assert_eq!(response.mime_type.as_deref(), Some("text/markdown"));
    }

    #[test]
    fn mcp_runtime_maps_prompt_argument_count() {
        let response = map_prompt_def(McpPromptDef {
            name: "summarize".into(),
            description: Some("Summarize input".into()),
            arguments: vec![
                McpPromptArgument {
                    name: "input".into(),
                    description: None,
                    required: Some(true),
                },
                McpPromptArgument {
                    name: "style".into(),
                    description: Some("Tone".into()),
                    required: None,
                },
            ],
        });

        assert_eq!(response.name, "summarize");
        assert_eq!(response.description.as_deref(), Some("Summarize input"));
        assert_eq!(response.argument_count, 2);
    }

    #[test]
    fn mcp_runtime_maps_and_serializes_content_blocks() {
        let text = map_content_block(McpContentBlock::Text {
            text: "hello".into(),
        });
        let image = map_content_block(McpContentBlock::Image {
            data: "aW1hZ2U=".into(),
            mime_type: "image/png".into(),
        });
        let resource = map_content_block(McpContentBlock::Resource {
            resource: McpResourceContent {
                uri: "file:///tmp/context.md".into(),
                mime_type: Some("text/markdown".into()),
                text: Some("# Context".into()),
            },
        });

        assert_eq!(
            serde_json::to_value(text).unwrap(),
            json!({
                "type": "text",
                "text": "hello",
            })
        );
        assert_eq!(
            serde_json::to_value(image).unwrap(),
            json!({
                "type": "image",
                "data": "aW1hZ2U=",
                "mime_type": "image/png",
            })
        );
        assert_eq!(
            serde_json::to_value(resource).unwrap(),
            json!({
                "type": "resource",
                "uri": "file:///tmp/context.md",
                "name": "",
                "mime_type": "text/markdown",
            })
        );
    }

    #[test]
    fn mcp_runtime_maps_connected_health_response() {
        let response = map_health_result(CheckHealthResult {
            tools: vec![McpToolDef {
                name: "search".into(),
                description: Some("Search files".into()),
                input_schema: Some(r#"{"type":"object"}"#.into()),
            }],
            healthy: true,
            error: None,
        });

        assert!(response.healthy);
        assert!(response.error.is_none());
        assert_eq!(response.tools.len(), 1);
        assert_eq!(response.tools[0].name, "search");
        assert_eq!(
            response.tools[0].description.as_deref(),
            Some("Search files")
        );
        assert_eq!(
            response.tools[0].input_schema.as_deref(),
            Some(r#"{"type":"object"}"#)
        );
    }

    #[test]
    fn mcp_runtime_maps_error_health_response() {
        let response = map_health_result(CheckHealthResult {
            tools: Vec::new(),
            healthy: false,
            error: Some("No MCP servers configured".into()),
        });

        assert!(!response.healthy);
        assert_eq!(response.error.as_deref(), Some("No MCP servers configured"));
        assert!(response.tools.is_empty());
    }
}
