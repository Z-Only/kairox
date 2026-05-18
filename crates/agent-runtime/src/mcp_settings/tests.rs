use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use agent_config::Config;
use agent_core::facade::{McpServerSettingsInput, McpServerSettingsTransport};
use agent_core::CoreError;
use tokio::sync::Mutex;

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
            headers: BTreeMap::from([("Authorization".to_string(), "Bearer token".to_string())]),
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
    let manager = Arc::new(Mutex::new(crate::McpServerManager::from_config(
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

    let views = list_mcp_server_settings(&config, Some(&config_path), None, None, Some(manager))
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
