use super::config_runtime::open_path_in_system_file_manager;
use super::*;

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

    state
        .refresh_config_for_project(std::path::Path::new(&project_root))
        .await?;
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

    state
        .refresh_config_for_project(std::path::Path::new(&project_root))
        .await?;
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
