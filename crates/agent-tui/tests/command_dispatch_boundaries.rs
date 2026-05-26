use std::collections::BTreeMap;
use std::sync::Arc;

use agent_config::{Config, ConfigSource, ContextPolicy, FeatureFlags, ProfileDef};
use agent_core::facade::{McpServerSettingsInput, McpServerSettingsTransport};
use agent_core::{AppFacade, ProjectId, StartSessionRequest, WorkspaceId};
use agent_mcp::{McpServerDef, McpTransportDef};
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tui::app::App;
use agent_tui::app_state::SettingsConfigSource;
use agent_tui::components::{Command, McpServerStatusView, ProjectInfo};

fn profile(model_id: &str, supports_reasoning: bool) -> ProfileDef {
    ProfileDef {
        provider: "fake".into(),
        model_id: model_id.into(),
        base_url: None,
        api_key: None,
        api_key_env: None,
        context_window: Some(4096),
        output_limit: Some(2048),
        response: Some("ok".into()),
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: Some(supports_reasoning),
        extra_params: None,
        enabled: true,
    }
}

fn dispatch_test_config() -> Arc<Config> {
    Arc::new(Config {
        profiles: vec![
            ("reasoning".into(), profile("fake-reasoning", true)),
            ("plain".into(), profile("fake-plain", false)),
        ],
        mcp_servers: Vec::new(),
        source: ConfigSource::Defaults,
        context: ContextPolicy::default(),
        disabled_mcp_servers: Vec::new(),
        instructions: None,
        features: FeatureFlags::default(),
        hooks: Vec::new(),
    })
}

async fn make_runtime() -> Arc<LocalRuntime<SqliteEventStore, ModelRouter>> {
    let config = dispatch_test_config();
    let router = config.build_router();
    let store = SqliteEventStore::in_memory().await.expect("store");
    Arc::new(LocalRuntime::new(store, router).with_config(config))
}

async fn make_runtime_with_mcp() -> Arc<LocalRuntime<SqliteEventStore, ModelRouter>> {
    let config = dispatch_test_config();
    let router = config.build_router();
    let store = SqliteEventStore::in_memory().await.expect("store");
    let runtime = LocalRuntime::new(store, router)
        .with_config(config)
        .with_mcp_servers(vec![McpServerDef {
            name: "files".into(),
            transport: McpTransportDef::Stdio {
                command: "echo".into(),
                cwd: None,
            },
            args: Vec::new(),
            env: Default::default(),
            keep_alive: false,
            idle_timeout_secs: 300,
            auto_restart: false,
            max_restart_attempts: 0,
        }])
        .await;
    Arc::new(runtime)
}

fn new_app() -> App {
    App::new("reasoning", WorkspaceId::new())
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
}

fn test_project(project_id: &str, root_path: &std::path::Path) -> ProjectInfo {
    ProjectInfo {
        id: ProjectId::from_string(project_id.to_string()),
        display_name: project_id.to_string(),
        root_path: root_path.display().to_string(),
        expanded: true,
        git_status: None,
        instruction_summary: None,
    }
}

fn select_project_settings(app: &mut App, project_root: &std::path::Path) {
    let project = test_project("prj_dispatch", project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);
}

#[tokio::test]
async fn runtime_dispatch_handles_runtime_owned_save_draft() {
    let runtime = make_runtime().await;
    let mut app = new_app();
    let workspace = runtime
        .open_workspace("/tmp/kairox-dispatch-draft".into())
        .await
        .expect("workspace");
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id,
            model_profile: "reasoning".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .expect("session");

    agent_tui::runtime_dispatch::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SaveDraft {
            session_id: session_id.clone(),
            draft_text: "draft from runtime dispatcher".into(),
        }],
    )
    .await;

    let draft = runtime
        .store()
        .get_draft(session_id.as_str())
        .await
        .expect("draft should load");
    assert_eq!(draft, "draft from runtime dispatcher");
}

#[tokio::test]
async fn runtime_dispatch_delegates_generic_app_overlay_commands() {
    let runtime = make_runtime().await;
    let mut app = new_app();

    agent_tui::runtime_dispatch::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::OpenSkillsOverlay],
    )
    .await;

    assert!(app.skills_overlay.is_visible());
}

#[tokio::test]
async fn runtime_dispatch_refreshes_runtime_mcp_snapshot_after_settings_mutation() {
    let runtime = make_runtime_with_mcp().await;
    let project_root = unique_temp_dir("kairox-tui-runtime-mcp-dispatch");
    let mut app = new_app();
    select_project_settings(&mut app, &project_root);

    agent_tui::runtime_dispatch::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::OpenMcpOverlay],
    )
    .await;
    assert_eq!(
        app.mcp_overlay.servers(),
        &[agent_tui::components::McpServerEntry {
            server_id: "files".into(),
            status: McpServerStatusView::Stopped,
            trusted: false,
            tool_count: 0,
        }]
    );

    agent_tui::runtime_dispatch::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SaveMcpServerSettings {
            input: McpServerSettingsInput {
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

    assert!(
        app.mcp_overlay
            .servers()
            .iter()
            .any(|server| server.server_id == "files"),
        "runtime MCP servers should remain present after generic settings refresh"
    );
    assert_eq!(app.mcp_overlay.settings_len(), 1);

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn runtime_dispatch_refreshes_model_overlay_after_profile_mutation_with_reasoning_flags() {
    let runtime = make_runtime().await;
    let project_root = unique_temp_dir("kairox-tui-runtime-model-dispatch");
    let mut app = new_app();
    select_project_settings(&mut app, &project_root);

    agent_tui::runtime_dispatch::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::OpenModelOverlay],
    )
    .await;
    assert!(app
        .model_overlay
        .profiles()
        .iter()
        .any(|profile| profile.alias == "reasoning" && profile.supports_reasoning));

    agent_tui::runtime_dispatch::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SetProfileEnabled {
            alias: "reasoning".into(),
            enabled: false,
        }],
    )
    .await;

    let reasoning = app
        .model_overlay
        .profiles()
        .iter()
        .find(|profile| profile.alias == "reasoning")
        .expect("reasoning profile should stay visible");
    assert!(!reasoning.enabled);
    assert!(reasoning.supports_reasoning);

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}
