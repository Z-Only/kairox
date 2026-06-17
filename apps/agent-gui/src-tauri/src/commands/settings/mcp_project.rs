use super::config_runtime::open_path_in_system_file_manager;
use super::*;

fn project_mcp_config_path(project_root: &str) -> std::path::PathBuf {
    std::path::Path::new(project_root)
        .join(".kairox")
        .join("config.toml")
}

fn parse_project_config(raw: &str) -> Result<toml_edit::DocumentMut, String> {
    raw.parse()
        .map_err(|e| format!("failed to parse project config: {e}"))
}

fn disabled_mcp_servers(doc: &toml_edit::DocumentMut) -> std::collections::BTreeSet<String> {
    doc.get("disabled_mcp_servers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn set_disabled_mcp_servers(
    doc: &mut toml_edit::DocumentMut,
    disabled: std::collections::BTreeSet<String>,
) {
    if disabled.is_empty() {
        doc.remove("disabled_mcp_servers");
        return;
    }

    let mut arr = toml_edit::Array::new();
    for value in disabled {
        arr.push(value);
    }
    doc["disabled_mcp_servers"] = toml_edit::value(arr);
}

fn disable_mcp_server_in_project_config(raw: &str, server_id: &str) -> Result<String, String> {
    let mut doc = parse_project_config(raw)?;
    let mut disabled = disabled_mcp_servers(&doc);
    disabled.insert(server_id.to_string());
    set_disabled_mcp_servers(&mut doc, disabled);
    Ok(doc.to_string())
}

fn enable_mcp_server_in_project_config(raw: &str, server_id: &str) -> Result<String, String> {
    let mut doc = parse_project_config(raw)?;
    let mut disabled = disabled_mcp_servers(&doc);
    disabled.remove(server_id);
    set_disabled_mcp_servers(&mut doc, disabled);
    Ok(doc.to_string())
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
    let config_path = project_mcp_config_path(&project_root);
    let raw = match std::fs::read_to_string(&config_path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("failed to read project config: {e}")),
    };

    let updated = disable_mcp_server_in_project_config(&raw, &server_id)?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create config dir: {e}"))?;
    }
    std::fs::write(&config_path, updated)
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
    let config_path = project_mcp_config_path(&project_root);
    let raw = match std::fs::read_to_string(&config_path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("failed to read project config: {e}")),
    };

    let updated = enable_mcp_server_in_project_config(&raw, &server_id)?;
    std::fs::write(&config_path, updated)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn disabled_servers(raw: &str) -> Vec<String> {
        let doc: toml_edit::DocumentMut = raw.parse().expect("config should parse");
        doc.get("disabled_mcp_servers")
            .and_then(|value| value.as_array())
            .expect("disabled_mcp_servers should be present")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("server id should be a string")
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn project_mcp_config_path_targets_project_kairox_config() {
        let path = project_mcp_config_path("/tmp/workspace");

        assert_eq!(
            path,
            std::path::Path::new("/tmp/workspace")
                .join(".kairox")
                .join("config.toml")
        );
    }

    #[test]
    fn disabling_mcp_server_adds_sorted_unique_project_scope_entry() {
        let raw = "disabled_mcp_servers = [\"zeta\", \"alpha\", \"alpha\"]\n\n[project]\nname = \"demo\"\n";

        let updated =
            disable_mcp_server_in_project_config(raw, "files").expect("config should update");

        assert_eq!(disabled_servers(&updated), vec!["alpha", "files", "zeta"]);
        assert!(updated.contains("[project]\nname = \"demo\""));
    }

    #[test]
    fn enabling_mcp_server_removes_entry_and_sorts_remaining_servers() {
        let raw = "disabled_mcp_servers = [\"zeta\", \"files\", \"alpha\"]\n";

        let updated =
            enable_mcp_server_in_project_config(raw, "files").expect("config should update");

        assert_eq!(disabled_servers(&updated), vec!["alpha", "zeta"]);
    }

    #[test]
    fn enabling_last_disabled_mcp_server_removes_disabled_key() {
        let raw = "disabled_mcp_servers = [\"files\"]\n[project]\nname = \"demo\"\n";

        let updated =
            enable_mcp_server_in_project_config(raw, "files").expect("config should update");
        let doc: toml_edit::DocumentMut = updated.parse().expect("config should parse");

        assert!(doc.get("disabled_mcp_servers").is_none());
        assert!(updated.contains("[project]\nname = \"demo\""));
    }

    #[test]
    fn project_mcp_config_helpers_report_parse_context() {
        let error = disable_mcp_server_in_project_config("[broken", "files")
            .expect_err("invalid TOML should fail");

        assert!(error.starts_with("failed to parse project config:"));
    }
}
