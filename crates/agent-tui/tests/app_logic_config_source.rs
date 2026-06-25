//! TUI App logic integration tests — selected-project config source filter.
//!
//! Split from the former `app_logic.rs`. Shared helpers live in
//! `app_logic_common`.

#![allow(unused_imports)]

mod app_logic_common;

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use app_logic_common::{test_project, unique_temp_dir, TuiMcpFakeFacade};
use futures::StreamExt;
use std::sync::Arc;

#[tokio::test]
async fn config_source_model_overlay_uses_selected_project_filter() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenModelOverlay]).await;

    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| call == "list_profile_settings:Some(\"project\")"),
        "expected selected project source filter for model overlay, got {calls:?}"
    );
}

#[tokio::test]
async fn config_source_mcp_overlay_uses_selected_project_filter() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenMcpOverlay]).await;

    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| call == "list_mcp_server_settings:Some(\"project\")"),
        "expected selected project source filter for MCP overlay, got {calls:?}"
    );
}

#[tokio::test]
async fn config_source_model_save_uses_selected_project_config_path() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-model-save-source");
    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_model_save", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SaveProfileSettings {
            input: agent_core::facade::ProfileSettingsInput {
                alias: "project-fast".into(),
                provider: "fake".into(),
                model_id: "project-model".into(),
                enabled: true,
                context_window: Some(64000),
                output_limit: None,
                temperature: Some(0.1),
                top_p: None,
                top_k: None,
                max_tokens: None,
                base_url: None,
                api_key: None,
                api_key_env: None,
                client_identity: None,
                supports_reasoning: None,
            },
        }],
    )
    .await;

    let config_path = project_root.join(".kairox").join("config.toml");
    let raw = std::fs::read_to_string(&config_path)
        .expect("selected project config should receive model profile");
    assert!(raw.contains("[profiles.project-fast]"));
    assert!(raw.contains("model_id = \"project-model\""));
    assert!(
        !runtime
            .calls()
            .iter()
            .any(|call| call.starts_with("upsert_profile_settings")),
        "project save should not write through the user profile facade"
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn config_source_mcp_save_uses_selected_project_config_path() {
    use agent_core::facade::McpServerSettingsTransport;
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;
    use std::collections::BTreeMap;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-mcp-save-source");
    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_mcp_save", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SaveMcpServerSettings {
            input: agent_core::facade::McpServerSettingsInput {
                name: "project-fs".into(),
                transport: McpServerSettingsTransport::Stdio {
                    command: "kairox-mcp".into(),
                    args: vec!["serve".into()],
                    env: BTreeMap::new(),
                },
                enabled: true,
                description: Some("Project MCP".into()),
            },
        }],
    )
    .await;

    let config_path = project_root.join(".kairox").join("config.toml");
    let raw = std::fs::read_to_string(&config_path)
        .expect("selected project config should receive MCP server");
    assert!(raw.contains("[mcp_servers.project-fs]"));
    assert!(raw.contains("command = \"kairox-mcp\""));
    assert!(
        !runtime
            .calls()
            .iter()
            .any(|call| call.starts_with("upsert_mcp_server_settings")),
        "project save should not write through the user MCP facade"
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn config_source_instructions_overlay_uses_selected_project_config_path() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-instructions-source");
    let config_dir = project_root.join(".kairox");
    std::fs::create_dir_all(&config_dir).expect("project config dir should be created");
    std::fs::write(
        config_dir.join("config.toml"),
        "instructions = \"Use selected project instructions.\"\n",
    )
    .expect("project config should be written");

    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_selected_instructions", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenInstructionsOverlay])
        .await;

    assert_eq!(
        app.instructions_overlay.project_text(),
        "Use selected project instructions."
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn config_source_instructions_save_uses_selected_project_config_path() {
    use agent_core::ConfigScope;
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-instructions-save-source");
    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_save_instructions", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SaveInstructions {
            scope: ConfigScope::Project,
            text: "Prefer project-local guidance.".into(),
        }],
    )
    .await;

    let config_path = project_root.join(".kairox").join("config.toml");
    let raw = std::fs::read_to_string(&config_path)
        .expect("selected project config should receive instructions");
    assert!(raw.contains("instructions = \"Prefer project-local guidance.\""));
    assert_eq!(
        app.instructions_overlay.project_text(),
        "Prefer project-local guidance."
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn config_source_hooks_overlay_uses_selected_project_config_path() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-hooks-source");
    let config_dir = project_root.join(".kairox");
    let config_path = config_dir.join("config.toml");
    std::fs::create_dir_all(&config_dir).expect("project config dir should be created");
    std::fs::write(
        &config_path,
        "[features]\nhooks = true\n\n[hooks.Stop.verify]\ncommand = \"cargo test\"\nenabled = true\n",
    )
    .expect("project config should be written");

    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_selected_hooks", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenHooksOverlay]).await;

    let hooks = app.hooks_overlay.project_hooks();
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].id, "verify");
    assert_eq!(
        hooks[0].config_path.as_deref(),
        Some(config_path.display().to_string().as_str())
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn config_source_hooks_save_and_delete_use_selected_project_config_path() {
    use agent_core::ConfigScope;
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-hooks-save-source");
    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_save_hooks", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SaveHookSettings {
            input: agent_core::facade::HookSettingsInput {
                scope: ConfigScope::Project,
                id: "verify".into(),
                event: "Stop".into(),
                matcher: Some("*".into()),
                command: "cargo test -p agent-tui".into(),
                status_message: Some("Checking TUI".into()),
                timeout_secs: Some(120),
                enabled: true,
            },
        }],
    )
    .await;

    let config_path = project_root.join(".kairox").join("config.toml");
    let raw = std::fs::read_to_string(&config_path)
        .expect("selected project config should receive hook settings");
    assert!(raw.contains("[hooks.Stop.verify]"));
    assert!(raw.contains("command = \"cargo test -p agent-tui\""));
    assert_eq!(
        app.state
            .latest_status_message()
            .map(|entry| entry.message.as_str()),
        Some("saved hook Stop.verify")
    );

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::DeleteHookSettings {
            scope: ConfigScope::Project,
            event: "Stop".into(),
            id: "verify".into(),
        }],
    )
    .await;

    let raw = std::fs::read_to_string(&config_path)
        .expect("selected project config should remain readable after delete");
    assert!(!raw.contains("[hooks.Stop.verify]"));
    assert_eq!(
        app.state
            .latest_status_message()
            .map(|entry| entry.message.as_str()),
        Some("deleted hook Stop.verify")
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}
