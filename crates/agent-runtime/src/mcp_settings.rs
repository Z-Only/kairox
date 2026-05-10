use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_config::{Config, McpServerConfig, McpTransportType};
use agent_core::facade::{
    McpServerSettingsInput, McpServerSettingsTransport, McpServerSettingsView,
};
use agent_core::CoreError;
use tokio::sync::Mutex;
use toml_edit::{value, Array, DocumentMut, Item, Table};

use crate::McpServerManager;

const MCP_SERVERS_FILE_NAME: &str = "mcp_servers.toml";

#[async_trait::async_trait]
pub trait McpSettingsLifecycle {
    fn is_server_running(&self, server_id: &str) -> bool;

    async fn stop_server(&mut self, server_id: &str) -> agent_core::Result<()>;
}

pub fn writable_mcp_config_path(config_dir: Option<&Path>) -> agent_core::Result<Option<PathBuf>> {
    Ok(config_dir.map(|dir| dir.join(MCP_SERVERS_FILE_NAME)))
}

pub async fn list_mcp_server_settings(
    config: &Config,
    config_path: Option<&Path>,
    manager: Option<Arc<Mutex<McpServerManager>>>,
) -> agent_core::Result<Vec<McpServerSettingsView>> {
    let mut rows = settings_rows_from_config(config);
    if let Some(path) = config_path {
        rows.extend(settings_rows_from_file(path).await?);
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
                writable: config_path.is_some(),
                config_path: config_path.map(|path| path.display().to_string()),
                description: row.description,
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
        server_table["enabled"] = value(enabled);
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

#[async_trait::async_trait]
impl McpSettingsLifecycle for McpServerManager {
    fn is_server_running(&self, server_id: &str) -> bool {
        self.is_running(server_id).unwrap_or(false)
    }

    async fn stop_server(&mut self, server_id: &str) -> agent_core::Result<()> {
        self.shutdown_server(server_id)
            .await
            .map_err(|error| CoreError::InvalidState(format!("failed to stop MCP server: {error}")))
    }
}

struct NoopMcpSettingsLifecycle;

#[async_trait::async_trait]
impl McpSettingsLifecycle for NoopMcpSettingsLifecycle {
    fn is_server_running(&self, _server_id: &str) -> bool {
        false
    }

    async fn stop_server(&mut self, _server_id: &str) -> agent_core::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct McpSettingsRow {
    name: String,
    transport: String,
    enabled: bool,
    description: Option<String>,
}

async fn settings_view_from_file(
    config_path: &Path,
    server_id: &str,
) -> agent_core::Result<McpServerSettingsView> {
    let rows = settings_rows_from_file(config_path).await?;
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
    })
}

fn settings_rows_from_config(config: &Config) -> HashMap<String, McpSettingsRow> {
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
                },
            )
        })
        .collect()
}

async fn settings_rows_from_file(
    config_path: &Path,
) -> agent_core::Result<HashMap<String, McpSettingsRow>> {
    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    let raw = tokio::fs::read_to_string(config_path)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to read MCP config: {error}")))?;
    let document = parse_document(&raw)?;
    Ok(settings_rows_from_document(&document))
}

fn settings_rows_from_document(document: &DocumentMut) -> HashMap<String, McpSettingsRow> {
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
                },
            ))
        })
        .collect()
}

async fn mutate_mcp_config<F>(config_path: &Path, mutate: F) -> agent_core::Result<()>
where
    F: FnOnce(&mut DocumentMut) -> agent_core::Result<()>,
{
    let raw = match tokio::fs::read_to_string(config_path).await {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(CoreError::InvalidState(format!(
                "failed to read MCP config: {error}"
            )))
        }
    };
    let mut document = parse_document(&raw)?;
    mutate(&mut document)?;

    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            CoreError::InvalidState(format!("failed to create MCP config directory: {error}"))
        })?;
    }
    tokio::fs::write(config_path, document.to_string())
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to write MCP config: {error}")))
}

fn parse_document(raw: &str) -> agent_core::Result<DocumentMut> {
    raw.parse::<DocumentMut>()
        .map_err(|error| CoreError::InvalidState(format!("failed to parse MCP config: {error}")))
}

fn upsert_server_table(document: &mut DocumentMut, input: &McpServerSettingsInput) {
    let server_table = ensure_server_table(document, &input.name);
    server_table["enabled"] = value(input.enabled);
    match &input.description {
        Some(description) => server_table["description"] = value(description.clone()),
        None => {
            server_table.remove("description");
        }
    }

    match &input.transport {
        McpServerSettingsTransport::Stdio { command, args, env } => {
            server_table["type"] = value("stdio");
            server_table["command"] = value(command.clone());
            server_table["args"] = value(string_array(args));
            replace_optional_table(server_table, "env", env);
            server_table.remove("url");
            server_table.remove("headers");
        }
        McpServerSettingsTransport::Sse { url, headers } => {
            server_table["type"] = value("sse");
            server_table["url"] = value(url.clone());
            replace_optional_table(server_table, "headers", headers);
            server_table.remove("command");
            server_table.remove("args");
            server_table.remove("env");
        }
    }
}

fn ensure_server_table<'a>(document: &'a mut DocumentMut, server_id: &str) -> &'a mut Table {
    let servers_table = ensure_mcp_servers_table(document);
    if !servers_table.contains_key(server_id) || !servers_table[server_id].is_table() {
        servers_table[server_id] = Item::Table(Table::new());
    }
    servers_table[server_id]
        .as_table_mut()
        .expect("server table should exist")
}

fn ensure_mcp_servers_table(document: &mut DocumentMut) -> &mut Table {
    if !document.as_table().contains_key("mcp_servers") || !document["mcp_servers"].is_table() {
        document["mcp_servers"] = Item::Table(Table::new());
    }
    document["mcp_servers"]
        .as_table_mut()
        .expect("mcp_servers table should exist")
}

fn replace_optional_table(server_table: &mut Table, key: &str, values: &BTreeMap<String, String>) {
    if values.is_empty() {
        server_table.remove(key);
        return;
    }

    server_table[key] = Item::Table(string_map_table(values));
}

fn string_map_table(values: &BTreeMap<String, String>) -> Table {
    let mut table = Table::new();
    for (key, value_text) in values {
        table[key] = value(value_text.clone());
    }
    table
}

fn string_array(values: &[String]) -> Array {
    let mut array = Array::default();
    for value_text in values {
        array.push(value_text.as_str());
    }
    array
}

fn transport_label(config: &McpServerConfig) -> String {
    match config.r#type {
        McpTransportType::Stdio => "stdio".to_string(),
        McpTransportType::Sse => "sse".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeMcpSettingsLifecycle {
        running_servers: HashSet<String>,
        started_servers: Vec<String>,
        stopped_servers: Vec<String>,
        stop_error: Option<String>,
    }

    impl FakeMcpSettingsLifecycle {
        fn running(server_id: &str) -> Self {
            Self {
                running_servers: HashSet::from([server_id.to_string()]),
                started_servers: Vec::new(),
                stopped_servers: Vec::new(),
                stop_error: None,
            }
        }

        fn running_with_stop_error(server_id: &str) -> Self {
            Self {
                running_servers: HashSet::from([server_id.to_string()]),
                started_servers: Vec::new(),
                stopped_servers: Vec::new(),
                stop_error: Some("stop failed".to_string()),
            }
        }

        fn stopped(_server_id: &str) -> Self {
            Self {
                running_servers: HashSet::new(),
                started_servers: Vec::new(),
                stopped_servers: Vec::new(),
                stop_error: None,
            }
        }

        fn started_servers(&self) -> Vec<String> {
            self.started_servers.clone()
        }

        fn stopped_servers(&self) -> Vec<String> {
            self.stopped_servers.clone()
        }
    }

    #[async_trait::async_trait]
    impl McpSettingsLifecycle for FakeMcpSettingsLifecycle {
        fn is_server_running(&self, server_id: &str) -> bool {
            self.running_servers.contains(server_id)
        }

        async fn stop_server(&mut self, server_id: &str) -> agent_core::Result<()> {
            if let Some(error) = &self.stop_error {
                return Err(CoreError::InvalidState(error.clone()));
            }
            self.running_servers.remove(server_id);
            self.stopped_servers.push(server_id.to_string());
            Ok(())
        }
    }

    fn write_mcp_config_fixture(raw: &str) -> PathBuf {
        let file = tempfile::NamedTempFile::new().expect("temp file should be created");
        let (_file, config_path) = file.keep().expect("temp file path should be kept");
        std::fs::write(&config_path, raw).expect("config fixture should be written");
        config_path
    }

    #[tokio::test]
    async fn disabling_running_server_stops_before_marking_disabled() {
        let config_path = write_mcp_config_fixture(
            "[mcp_servers.files]\ncommand = \"npx\"\nargs = [\"server\"]\nenabled = true\n",
        );
        let mut fake_manager = FakeMcpSettingsLifecycle::running("files");

        set_mcp_server_enabled_in_file(&config_path, &mut fake_manager, "files", false)
            .await
            .expect("server should be disabled");

        assert_eq!(fake_manager.stopped_servers(), vec!["files".to_string()]);
        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(raw.contains("enabled = false"));
    }

    #[tokio::test]
    async fn enabling_server_does_not_start_it() {
        let config_path = write_mcp_config_fixture(
            "[mcp_servers.files]\ncommand = \"npx\"\nargs = [\"server\"]\nenabled = false\n",
        );
        let mut fake_manager = FakeMcpSettingsLifecycle::stopped("files");

        set_mcp_server_enabled_in_file(&config_path, &mut fake_manager, "files", true)
            .await
            .expect("server should be enabled");

        assert!(fake_manager.started_servers().is_empty());
        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(raw.contains("enabled = true"));
    }

    #[tokio::test]
    async fn upsert_writes_stdio_and_sse_server_settings() {
        let config_path = write_mcp_config_fixture("");
        let stdio_input = McpServerSettingsInput {
            name: "files".to_string(),
            transport: McpServerSettingsTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["server".to_string()],
                env: BTreeMap::from([("DEBUG".to_string(), "1".to_string())]),
            },
            enabled: true,
            description: Some("File tools".to_string()),
        };
        let sse_input = McpServerSettingsInput {
            name: "remote".to_string(),
            transport: McpServerSettingsTransport::Sse {
                url: "https://example.test/sse".to_string(),
                headers: BTreeMap::from([(
                    "Authorization".to_string(),
                    "Bearer token".to_string(),
                )]),
            },
            enabled: false,
            description: None,
        };

        upsert_mcp_server_settings_in_file(&config_path, &stdio_input)
            .await
            .expect("stdio server should be written");
        upsert_mcp_server_settings_in_file(&config_path, &sse_input)
            .await
            .expect("sse server should be written");

        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(raw.contains("[mcp_servers.files]"));
        assert!(raw.contains("type = \"stdio\""));
        assert!(raw.contains("command = \"npx\""));
        assert!(raw.contains("[mcp_servers.files.env]"));
        assert!(raw.contains("[mcp_servers.remote]"));
        assert!(raw.contains("type = \"sse\""));
        assert!(raw.contains("url = \"https://example.test/sse\""));
        assert!(raw.contains("[mcp_servers.remote.headers]"));
    }

    #[tokio::test]
    async fn deleting_running_server_stops_before_removing_config() {
        let config_path = write_mcp_config_fixture(
            "[mcp_servers.files]\ncommand = \"npx\"\nargs = [\"server\"]\nenabled = true\n",
        );
        let mut fake_manager = FakeMcpSettingsLifecycle::running("files");

        delete_mcp_server_settings_in_file(&config_path, &mut fake_manager, "files")
            .await
            .expect("server should be deleted");

        assert_eq!(fake_manager.stopped_servers(), vec!["files".to_string()]);
        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(!raw.contains("[mcp_servers.files]"));
    }

    #[tokio::test]
    async fn stop_failure_does_not_write_disabled_state() {
        let original = "[mcp_servers.files]\ncommand = \"npx\"\nenabled = true\n";
        let config_path = write_mcp_config_fixture(original);
        let mut fake_manager = FakeMcpSettingsLifecycle::running_with_stop_error("files");

        let result =
            set_mcp_server_enabled_in_file(&config_path, &mut fake_manager, "files", false).await;

        assert!(result.is_err());
        assert_eq!(fake_manager.stopped_servers(), Vec::<String>::new());
        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert_eq!(raw, original);
    }

    #[tokio::test]
    async fn upsert_preserves_unknown_fields_and_other_tables() {
        let config_path = write_mcp_config_fixture(
            "[catalog]\nname = \"local\"\n\n[mcp_servers.files]\nunknown = \"keep\"\nurl = \"https://old.example/sse\"\n[mcp_servers.files.custom]\nvalue = \"keep\"\n",
        );
        let input = McpServerSettingsInput {
            name: "files".to_string(),
            transport: McpServerSettingsTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["server".to_string()],
                env: BTreeMap::new(),
            },
            enabled: true,
            description: None,
        };

        upsert_mcp_server_settings_in_file(&config_path, &input)
            .await
            .expect("server should be updated");

        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(raw.contains("[catalog]"));
        assert!(raw.contains("unknown = \"keep\""));
        assert!(raw.contains("[mcp_servers.files.custom]"));
        assert!(raw.contains("type = \"stdio\""));
        assert!(raw.contains("command = \"npx\""));
        assert!(!raw.contains("url = \"https://old.example/sse\""));
    }

    #[tokio::test]
    async fn malformed_toml_returns_error_without_rewrite() {
        let original = "[mcp_servers.files\ncommand = \"npx\"\n";
        let config_path = write_mcp_config_fixture(original);
        let input = McpServerSettingsInput {
            name: "files".to_string(),
            transport: McpServerSettingsTransport::Sse {
                url: "https://example.test/sse".to_string(),
                headers: BTreeMap::new(),
            },
            enabled: true,
            description: None,
        };

        let result = upsert_mcp_server_settings_in_file(&config_path, &input).await;

        assert!(result.is_err());
        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert_eq!(raw, original);
    }

    #[tokio::test]
    async fn list_merges_file_runtime_and_trust_state() {
        let config_path = write_mcp_config_fixture(
            "[mcp_servers.files]\ntype = \"stdio\"\ncommand = \"npx\"\nenabled = false\ndescription = \"File tools\"\n",
        );
        let mut config = Config::defaults();
        config.mcp_servers.clear();
        let manager = Arc::new(Mutex::new(McpServerManager::from_config(
            vec![agent_mcp::types::McpServerDef {
                name: "files".to_string(),
                transport: agent_mcp::types::McpTransportDef::Stdio {
                    command: "echo".to_string(),
                    cwd: None,
                },
                args: Vec::new(),
                env: HashMap::new(),
                keep_alive: false,
                idle_timeout_secs: 300,
                auto_restart: false,
                max_restart_attempts: 0,
            }],
            Arc::new(Mutex::new(agent_tools::registry::ToolRegistry::new())),
            Arc::new(Mutex::new(agent_tools::permission::PermissionEngine::new(
                agent_tools::PermissionMode::Suggest,
            ))),
            None,
        )));
        {
            let manager = manager.lock().await;
            manager
                .trust_server("files")
                .await
                .expect("server should be trusted");
        }

        let views = list_mcp_server_settings(&config, Some(&config_path), Some(manager))
            .await
            .expect("settings should list");

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.name, "files");
        assert_eq!(view.transport, "stdio");
        assert!(!view.enabled);
        assert_eq!(view.runtime_status, "stopped");
        assert!(view.trusted);
        assert!(view.writable);
        assert_eq!(view.description.as_deref(), Some("File tools"));
    }
}
