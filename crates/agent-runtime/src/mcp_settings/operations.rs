use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_config::Config;
use agent_core::facade::{McpServerSettingsInput, McpServerSettingsView};
use agent_core::CoreError;
use tokio::sync::Mutex;
use toml_edit::{Array, Item};

use crate::McpServerManager;

use super::document::{
    ensure_server_table, mutate_mcp_config, parse_document, read_disabled_tools_from_document,
    upsert_server_table,
};
use super::lifecycle::{McpSettingsLifecycle, NoopMcpSettingsLifecycle};
use super::rows::{settings_rows_from_config, settings_rows_from_file, settings_view_from_file};
use super::CONFIG_FILE_NAME;

pub fn writable_mcp_config_path(config_dir: Option<&Path>) -> agent_core::Result<Option<PathBuf>> {
    Ok(config_dir.map(|dir| dir.join(CONFIG_FILE_NAME)))
}

pub async fn list_mcp_server_settings(
    config: &Config,
    user_config_path: Option<&Path>,
    project_config_path: Option<&Path>,
    source_filter: Option<&str>,
    manager: Option<Arc<Mutex<McpServerManager>>>,
) -> agent_core::Result<Vec<McpServerSettingsView>> {
    let mut rows = settings_rows_from_config(config, "defaults", false);

    let include_user = source_filter != Some("project");
    let include_project = source_filter != Some("user");

    if include_user {
        if let Some(path) = user_config_path {
            let user_rows = settings_rows_from_file(path, "user_config", true).await?;
            rows.extend(user_rows);
        }
    }
    if include_project {
        if let Some(path) = project_config_path {
            let project_rows = settings_rows_from_file(path, "project_config", true).await?;
            rows.extend(project_rows);
        }
    }

    let mut runtime_statuses = HashMap::new();
    let mut trusted_servers = HashSet::new();
    if let Some(manager) = manager {
        let permission_engine = {
            let manager = manager.lock().await;
            runtime_statuses = manager.server_statuses();
            manager.permission_engine()
        };
        let permission_engine = permission_engine.lock().await;
        trusted_servers = permission_engine.trusted_servers().clone();
    }

    let writable_path = writable_mcp_config_path(None)?;

    let mut views = rows
        .into_iter()
        .map(|(server_id, row)| {
            let runtime_status = runtime_statuses
                .get(&server_id)
                .map(ToString::to_string)
                .unwrap_or_else(|| "stopped".to_string());
            McpServerSettingsView {
                id: server_id.clone(),
                name: server_id,
                transport: row.transport,
                enabled: row.enabled,
                runtime_status,
                trusted: trusted_servers.contains(&row.name),
                tool_count: None,
                last_error: None,
                writable: row.writable,
                config_path: writable_path.as_ref().map(|p| p.display().to_string()),
                description: row.description,
                verified: row.source != "defaults",
                source: row.source,
            }
        })
        .collect::<Vec<_>>();
    views.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(views)
}

pub async fn upsert_mcp_server_settings_in_file(
    config_path: &Path,
    input: &McpServerSettingsInput,
) -> agent_core::Result<()> {
    mutate_mcp_config(config_path, |document| {
        upsert_server_table(document, input);
        Ok(())
    })
    .await
}

pub async fn set_mcp_server_enabled_in_file<L>(
    config_path: &Path,
    lifecycle: &mut L,
    server_id: &str,
    enabled: bool,
) -> agent_core::Result<()>
where
    L: McpSettingsLifecycle + Send,
{
    if !enabled && lifecycle.is_server_running(server_id) {
        lifecycle.stop_server(server_id).await?;
    }

    mutate_mcp_config(config_path, |document| {
        let server_table = ensure_server_table(document, server_id);
        server_table["enabled"] = toml_edit::value(enabled);
        Ok(())
    })
    .await
}

pub async fn delete_mcp_server_settings_in_file<L>(
    config_path: &Path,
    lifecycle: &mut L,
    server_id: &str,
) -> agent_core::Result<()>
where
    L: McpSettingsLifecycle + Send,
{
    if lifecycle.is_server_running(server_id) {
        lifecycle.stop_server(server_id).await?;
    }

    mutate_mcp_config(config_path, |document| {
        if let Some(servers) = document["mcp_servers"].as_table_mut() {
            servers.remove(server_id);
        }
        Ok(())
    })
    .await
}

pub async fn upsert_mcp_server_settings(
    config_path: &Path,
    input: McpServerSettingsInput,
) -> agent_core::Result<McpServerSettingsView> {
    upsert_mcp_server_settings_in_file(config_path, &input).await?;
    settings_view_from_file(config_path, &input.name).await
}

pub async fn set_mcp_server_enabled(
    config_path: &Path,
    manager: Option<Arc<Mutex<McpServerManager>>>,
    server_id: &str,
    enabled: bool,
) -> agent_core::Result<()> {
    if let Some(manager) = manager {
        let mut manager = manager.lock().await;
        return set_mcp_server_enabled_in_file(config_path, &mut *manager, server_id, enabled)
            .await;
    }

    let mut lifecycle = NoopMcpSettingsLifecycle;
    set_mcp_server_enabled_in_file(config_path, &mut lifecycle, server_id, enabled).await
}

pub async fn delete_mcp_server_settings(
    config_path: &Path,
    manager: Option<Arc<Mutex<McpServerManager>>>,
    server_id: &str,
) -> agent_core::Result<()> {
    if let Some(manager) = manager {
        let mut manager = manager.lock().await;
        return delete_mcp_server_settings_in_file(config_path, &mut *manager, server_id).await;
    }

    let mut lifecycle = NoopMcpSettingsLifecycle;
    delete_mcp_server_settings_in_file(config_path, &mut lifecycle, server_id).await
}

/// Read disabled tool names for a server from `config.toml`.
pub async fn get_mcp_disabled_tools(
    config_path: &Path,
    server_id: &str,
) -> agent_core::Result<HashSet<String>> {
    if !config_path.exists() {
        return Ok(HashSet::new());
    }
    let raw = tokio::fs::read_to_string(config_path)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to read MCP config: {error}")))?;
    let document = parse_document(&raw)?;
    Ok(read_disabled_tools_from_document(&document, server_id))
}

/// Add or remove a tool from the `disabled_tools` array for a server in `config.toml`.
pub async fn set_mcp_tool_disabled_in_file(
    config_path: &Path,
    server_id: &str,
    tool_name: &str,
    disabled: bool,
) -> agent_core::Result<()> {
    mutate_mcp_config(config_path, |document| {
        let server_table = ensure_server_table(document, server_id);
        let tools_array = server_table
            .entry("disabled_tools")
            .or_insert_with(|| Item::Value(toml_edit::Value::Array(Array::default())));

        if let Some(array) = tools_array.as_array_mut() {
            if disabled {
                // Add tool_name if not already present
                let already_present = array.iter().any(|v| v.as_str() == Some(tool_name));
                if !already_present {
                    array.push(tool_name);
                }
            } else {
                // Remove tool_name
                let idx = array.iter().position(|v| v.as_str() == Some(tool_name));
                if let Some(i) = idx {
                    array.remove(i);
                }
            }
            // If array is empty after removal, clean it up
            if array.is_empty() {
                server_table.remove("disabled_tools");
            }
        }
        Ok(())
    })
    .await
}
