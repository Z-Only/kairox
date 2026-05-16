use super::*;

#[tauri::command]
#[specta::specta]
pub async fn list_profiles(state: State<'_, GuiState>) -> Result<Vec<String>, String> {
    Ok(state.config.read().unwrap().profile_names())
}

#[tauri::command]
#[specta::specta]
pub async fn get_profile_info(state: State<'_, GuiState>) -> Result<Vec<ProfileInfo>, String> {
    Ok(state.config.read().unwrap().profile_info())
}

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
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create config dir: {e}"))?;
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
pub async fn list_profile_settings(
    state: State<'_, GuiState>,
    source_filter: Option<String>,
) -> Result<Vec<ProfileSettingsView>, String> {
    state
        .runtime
        .list_profile_settings(source_filter)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_effective_model_profiles(
    state: State<'_, GuiState>,
) -> Result<Vec<EffectiveProfileView>, String> {
    let config = state.config.read().map_err(|e| e.to_string())?;
    let source = match &config.source {
        agent_config::ConfigSource::ProjectFile => agent_core::ConfigScope::Project,
        agent_config::ConfigSource::UserFile => agent_core::ConfigScope::User,
        agent_config::ConfigSource::LocalFile => agent_core::ConfigScope::Local,
        agent_config::ConfigSource::Defaults => agent_core::ConfigScope::Builtin,
    };
    let mut result = Vec::with_capacity(config.profiles.len());
    for (alias, profile) in &config.profiles {
        let has_api_key = profile.api_key.is_some()
            || profile
                .api_key_env
                .as_deref()
                .map(|env| std::env::var(env).is_ok())
                .unwrap_or(false)
            || matches!(profile.provider.as_str(), "ollama" | "fake");
        let view = ProfileSettingsView {
            alias: alias.clone(),
            provider: profile.provider.clone(),
            model_id: profile.model_id.clone(),
            enabled: profile.enabled,
            context_window: profile.context_window,
            output_limit: profile.output_limit,
            temperature: profile.temperature,
            top_p: profile.top_p,
            top_k: profile.top_k,
            max_tokens: profile.max_tokens,
            base_url: profile.base_url.clone(),
            api_key_env: profile.api_key_env.clone(),
            has_api_key,
            writable: source >= agent_core::ConfigScope::User,
            config_path: None,
            source: source.to_string(),
        };
        result.push(EffectiveProfileView::from_view(view, source));
    }
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_profile_settings(
    state: State<'_, GuiState>,
    input: ProfileSettingsInput,
) -> Result<ProfileSettingsView, String> {
    let view = state
        .runtime
        .upsert_profile_settings(input)
        .await
        .map_err(|error| error.to_string())?;
    state.refresh_config()?;
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub async fn set_profile_enabled(
    state: State<'_, GuiState>,
    alias: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_profile_enabled(alias, enabled)
        .await
        .map_err(|error| error.to_string())?;
    state.refresh_config()?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_profile_settings(
    state: State<'_, GuiState>,
    alias: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_profile_settings(alias)
        .await
        .map_err(|error| error.to_string())?;
    state.refresh_config()?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ConnectivityTestResult {
    pub ok: bool,
    pub error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn test_model_connectivity(
    state: State<'_, GuiState>,
    alias: String,
) -> Result<ConnectivityTestResult, String> {
    // Get the profile settings to verify it exists and is configured.
    let profiles = state
        .runtime
        .list_profile_settings(None)
        .await
        .map_err(|e| e.to_string())?;

    let profile = profiles
        .into_iter()
        .find(|p| p.alias == alias)
        .ok_or_else(|| format!("model profile '{}' not found", alias))?;

    // If the profile has a base_url configured, try to reach it.
    if let Some(base_url) = &profile.base_url {
        if !base_url.is_empty() {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())?;

            // Try common endpoints: the base URL itself, then /models.
            let endpoints = [
                base_url.clone(),
                format!("{}/models", base_url.trim_end_matches('/')),
            ];

            let mut last_error: Option<String> = None;
            for endpoint in &endpoints {
                match client.get(endpoint).send().await {
                    Ok(response) => {
                        if response.status().is_success() || response.status().is_client_error() {
                            return Ok(ConnectivityTestResult {
                                ok: true,
                                error: None,
                            });
                        }
                        last_error = Some(format!("unexpected status: {}", response.status()));
                    }
                    Err(e) => {
                        last_error = Some(format!("connection failed: {e}"));
                    }
                }
            }

            return Ok(ConnectivityTestResult {
                ok: false,
                error: last_error,
            });
        }
    }

    // No custom base_url — the profile uses default provider endpoints.
    // Assume it's reachable if the config is valid.
    Ok(ConnectivityTestResult {
        ok: true,
        error: None,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn test_url_connectivity(url: String) -> Result<ConnectivityTestResult, String> {
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        return Ok(ConnectivityTestResult {
            ok: false,
            error: Some("no URL provided".into()),
        });
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    // Try the URL directly and also /models for common API endpoints.
    let endpoints = [
        trimmed.clone(),
        format!("{}/models", trimmed.trim_end_matches('/')),
    ];

    let mut last_error: Option<String> = None;
    for endpoint in &endpoints {
        match client.get(endpoint).send().await {
            Ok(response) => {
                if response.status().is_success() || response.status().is_client_error() {
                    return Ok(ConnectivityTestResult {
                        ok: true,
                        error: None,
                    });
                }
                last_error = Some(format!("unexpected status: {}", response.status()));
            }
            Err(e) => {
                last_error = Some(format!("connection failed: {e}"));
            }
        }
    }

    Ok(ConnectivityTestResult {
        ok: false,
        error: last_error,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn move_profile_in_order(
    state: State<'_, GuiState>,
    alias: String,
    direction: i32,
) -> Result<(), String> {
    state
        .runtime
        .move_profile_in_order(alias, direction)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn open_config_dir(state: State<'_, GuiState>) -> Result<Option<String>, String> {
    let Some(config_dir) = state
        .runtime
        .open_config_dir()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let config_dir = std::path::PathBuf::from(config_dir);
    open_path_in_system_file_manager(&config_dir)?;
    Ok(Some(config_dir.display().to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn open_profiles_config_file(
    state: State<'_, GuiState>,
) -> Result<Option<String>, String> {
    let Some(config_file_path) = state
        .runtime
        .open_profiles_config_file()
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
pub async fn open_skills_dir(state: State<'_, GuiState>) -> Result<Option<String>, String> {
    let Some(skills_dir) = state
        .runtime
        .open_skills_dir()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let skills_dir = std::path::PathBuf::from(skills_dir);
    open_path_in_system_file_manager(&skills_dir)?;
    Ok(Some(skills_dir.display().to_string()))
}

fn open_path_in_system_file_manager(path: &std::path::Path) -> Result<(), String> {
    let mut command = system_file_manager_command(path);
    let status = command
        .status()
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;

    if status.success() {
        return Ok(());
    }

    Err(format!(
        "failed to open {}: system opener exited with {status}",
        path.display()
    ))
}

#[cfg(target_os = "macos")]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("explorer");
    command.arg(path);
    command
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(path);
    command
}

// ---------------------------------------------------------------------------
// MCP commands
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileWithLimits {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    #[specta(type = u32)]
    pub context_window: u64,
    #[specta(type = u32)]
    pub output_limit: u64,
    /// Snake-case `LimitSource`: "user_config" | "builtin_registry" | "runtime_probe" | "fallback".
    pub limit_source: String,
    pub has_api_key: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_config_for_project(
    project_root: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let path = std::path::Path::new(&project_root);
    state.refresh_config_for_project(path)?;
    eprintln!(
        "Config refreshed for project: profiles={:?}",
        state.config.read().unwrap().profile_names()
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_profiles_with_limits(
    state: State<'_, GuiState>,
) -> Result<Vec<ProfileWithLimits>, String> {
    let config = state.config.read().unwrap();
    let mut out = Vec::with_capacity(config.profiles.len());
    for (alias, profile) in &config.profiles {
        let limits = agent_config::resolve_limits(profile);
        let limit_source = match limits.source {
            agent_models::LimitSource::UserConfig => "user_config",
            agent_models::LimitSource::BuiltinRegistry => "builtin_registry",
            agent_models::LimitSource::RuntimeProbe => "runtime_probe",
            agent_models::LimitSource::Fallback => "fallback",
        };
        let has_api_key = profile.api_key.is_some()
            || profile
                .api_key_env
                .as_deref()
                .map(|env| std::env::var(env).is_ok())
                .unwrap_or(false)
            || matches!(profile.provider.as_str(), "ollama" | "fake");
        out.push(ProfileWithLimits {
            alias: alias.clone(),
            provider: profile.provider.clone(),
            model_id: profile.model_id.clone(),
            context_window: limits.context_window,
            output_limit: limits.output_limit,
            limit_source: limit_source.into(),
            has_api_key,
        });
    }
    Ok(out)
}

fn parse_mcp_source_to_scope(source: &str) -> agent_core::config_scope::ConfigScope {
    match source {
        "user_config" => agent_core::config_scope::ConfigScope::User,
        "project_config" => agent_core::config_scope::ConfigScope::Project,
        "defaults" => agent_core::config_scope::ConfigScope::Builtin,
        _ => agent_core::config_scope::ConfigScope::Builtin,
    }
}

#[cfg(test)]
mod profile_with_limits_tests {
    use super::*;

    #[test]
    fn profile_with_limits_serializes_expected_shape() {
        let p = ProfileWithLimits {
            alias: "fast".into(),
            provider: "openai".into(),
            model_id: "gpt-4o-mini".into(),
            context_window: 128_000,
            output_limit: 16_384,
            limit_source: "builtin_registry".into(),
            has_api_key: true,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"alias\":\"fast\""));
        assert!(json.contains("\"context_window\":128000"));
        assert!(json.contains("\"limit_source\":\"builtin_registry\""));
        assert!(json.contains("\"has_api_key\":true"));
    }
}
