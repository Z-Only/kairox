use std::collections::HashMap;
use std::path::Path;

use agent_config::{Config, McpServerConfig, McpTransportType};
use agent_core::facade::McpServerSettingsView;
use agent_core::CoreError;
use toml_edit::{DocumentMut, Item};

use super::document::parse_document;

#[derive(Debug, Clone)]
pub(super) struct McpSettingsRow {
    pub(super) name: String,
    pub(super) transport: String,
    pub(super) enabled: bool,
    pub(super) description: Option<String>,
    pub(super) source: String,
    pub(super) writable: bool,
}

pub(super) async fn settings_view_from_file(
    config_path: &Path,
    server_id: &str,
) -> agent_core::Result<McpServerSettingsView> {
    let rows = settings_rows_from_file(config_path, "user_config", true).await?;
    let row = rows.get(server_id).ok_or_else(|| {
        CoreError::InvalidState(format!("saved MCP server was not found: {server_id}"))
    })?;
    Ok(McpServerSettingsView {
        id: server_id.to_string(),
        name: server_id.to_string(),
        transport: row.transport.clone(),
        enabled: row.enabled,
        runtime_status: "stopped".to_string(),
        trusted: false,
        tool_count: None,
        last_error: None,
        writable: true,
        config_path: Some(config_path.display().to_string()),
        description: row.description.clone(),
        source: row.source.clone(),
        verified: true,
    })
}

pub(super) fn settings_rows_from_config(
    config: &Config,
    source: &str,
    writable: bool,
) -> HashMap<String, McpSettingsRow> {
    config
        .mcp_servers
        .iter()
        .map(|(server_id, server_config)| {
            (
                server_id.clone(),
                McpSettingsRow {
                    name: server_id.clone(),
                    transport: transport_label(server_config),
                    enabled: true,
                    description: None,
                    source: source.to_string(),
                    writable,
                },
            )
        })
        .collect()
}

pub(super) async fn settings_rows_from_file(
    config_path: &Path,
    source: &str,
    writable: bool,
) -> agent_core::Result<HashMap<String, McpSettingsRow>> {
    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    let raw = tokio::fs::read_to_string(config_path)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to read MCP config: {error}")))?;
    let document = parse_document(&raw)?;
    Ok(settings_rows_from_document(&document, source, writable))
}

fn settings_rows_from_document(
    document: &DocumentMut,
    source: &str,
    writable: bool,
) -> HashMap<String, McpSettingsRow> {
    let Some(servers) = document["mcp_servers"].as_table() else {
        return HashMap::new();
    };

    servers
        .iter()
        .filter_map(|(server_id, item)| {
            let table = item.as_table()?;
            let transport = table
                .get("type")
                .and_then(Item::as_str)
                .unwrap_or_else(|| {
                    if table.get("url").is_some() {
                        "sse"
                    } else {
                        "stdio"
                    }
                })
                .to_string();
            let enabled = table.get("enabled").and_then(Item::as_bool).unwrap_or(true);
            let description = table
                .get("description")
                .and_then(Item::as_str)
                .map(ToString::to_string);
            Some((
                server_id.to_string(),
                McpSettingsRow {
                    name: server_id.to_string(),
                    transport,
                    enabled,
                    description,
                    source: source.to_string(),
                    writable,
                },
            ))
        })
        .collect()
}

fn transport_label(config: &McpServerConfig) -> String {
    match config.r#type {
        McpTransportType::Stdio => "stdio".to_string(),
        McpTransportType::Sse => "sse".to_string(),
        McpTransportType::StreamableHttp => "streamable_http".to_string(),
    }
}
