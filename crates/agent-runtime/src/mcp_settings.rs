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

const CONFIG_FILE_NAME: &str = "config.toml";

#[async_trait::async_trait]
pub trait McpSettingsLifecycle {
    fn is_server_running(&self, server_id: &str) -> bool;

    async fn stop_server(&mut self, server_id: &str) -> agent_core::Result<()>;
}

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

fn read_disabled_tools_from_document(document: &DocumentMut, server_id: &str) -> HashSet<String> {
    let Some(servers) = document["mcp_servers"].as_table() else {
        return HashSet::new();
    };
    let Some(server) = servers.get(server_id).and_then(|s| s.as_table()) else {
        return HashSet::new();
    };
    let Some(arr) = server.get("disabled_tools").and_then(|v| v.as_array()) else {
        return HashSet::new();
    };
    arr.iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect()
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
    source: String,
    writable: bool,
}

async fn settings_view_from_file(
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

fn settings_rows_from_config(
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

async fn settings_rows_from_file(
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
        McpServerSettingsTransport::StreamableHttp { url, headers } => {
            server_table["type"] = value("streamable_http");
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

    server_table[key] = string_map_to_inline(values);
}

fn string_map_to_inline(values: &BTreeMap<String, String>) -> Item {
    let mut inline = toml_edit::InlineTable::new();
    for (key, value_text) in values {
        inline.insert(key, value_text.clone().into());
    }
    Item::Value(toml_edit::Value::InlineTable(inline))
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
        McpTransportType::StreamableHttp => "streamable_http".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_map_to_inline_uses_equals_not_colon() {
        let input: BTreeMap<String, String> = BTreeMap::from([("REPO_PATH".into(), ".".into())]);
        let item = string_map_to_inline(&input);
        let rendered = item.to_string();
        assert!(
            !rendered.contains("\":"),
            "inline table must use '=' not ':':\n{rendered}",
        );
        // Also verify toml 1.1.2 can parse it.
        let table_str = format!("[t]\nenv = {rendered}");
        let parsed: toml::value::Table =
            toml::from_str(&table_str).expect("string_map_to_inline must produce valid TOML");
        let env = parsed["t"]["env"].as_table().unwrap();
        assert_eq!(env["REPO_PATH"].as_str(), Some("."));
    }

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

    #[test]
    fn writable_mcp_config_path_targets_main_config_toml() {
        let dir = PathBuf::from("/tmp/kairox-test");
        let path = writable_mcp_config_path(Some(&dir)).unwrap().expect("path");

        assert_eq!(path, dir.join("config.toml"));
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
        assert!(raw.contains("env = {"));
        assert!(raw.contains("[mcp_servers.remote]"));
        assert!(raw.contains("type = \"sse\""));
        assert!(raw.contains("url = \"https://example.test/sse\""));
        assert!(raw.contains("headers = {"));
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

        let views =
            list_mcp_server_settings(&config, Some(&config_path), None, None, Some(manager))
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
