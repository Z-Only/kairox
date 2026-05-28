//! TUI App logic integration tests — MCP marketplace commands and overlay.
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
use app_logic_common::{FakeInstallResult, TuiMcpFakeFacade};
use futures::StreamExt;
use std::sync::Arc;

#[tokio::test]
async fn tui_mcp_marketplace_commands_call_facade_and_refresh_overlay() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;
    use std::collections::BTreeMap;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenMcpOverlay]).await;

    assert!(app.mcp_overlay.is_visible());
    assert_eq!(app.mcp_overlay.settings_len(), 1);
    assert_eq!(app.mcp_overlay.catalog_len(), 1);
    assert_eq!(app.mcp_overlay.sources_len(), 1);
    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| call.starts_with("list_mcp_server_settings")),
        "expected settings list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "list_installed_entries"),
        "expected installed list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call.starts_with("list_catalog")),
        "expected catalog list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "list_catalog_sources"),
        "expected catalog sources call, got {calls:?}"
    );

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::SetMcpServerEnabled {
                server_id: "alpha".into(),
                enabled: false,
            },
            Command::DeleteMcpServerSettings {
                server_id: "alpha".into(),
            },
            Command::InstallMcpServer {
                request: agent_core::facade::InstallRequest {
                    catalog_id: "filesystem".into(),
                    source: "builtin".into(),
                    server_id_override: None,
                    env_overrides: BTreeMap::new(),
                    trust_grant: false,
                    auto_start: true,
                },
            },
            Command::UninstallMcpServer {
                server_id: "alpha".into(),
            },
            Command::SetMcpCatalogSourceEnabled {
                source_id: "builtin".into(),
                enabled: false,
            },
        ],
    )
    .await;

    let calls = runtime.calls();
    for expected in [
        "set_mcp_server_enabled:alpha:false",
        "delete_mcp_server_settings:alpha",
        "install_catalog_entry:filesystem:builtin:true",
        "uninstall_catalog_entry:alpha",
        "set_catalog_source_enabled:builtin:false",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }
}

#[tokio::test]
async fn tui_mcp_install_command_forwards_env_overrides() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;
    use std::collections::BTreeMap;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    let mut env_overrides = BTreeMap::new();
    env_overrides.insert("Authorization".to_string(), "Bearer test-token".to_string());
    env_overrides.insert("GITHUB_ORG".to_string(), "kairox-dev".to_string());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::InstallMcpServer {
            request: agent_core::facade::InstallRequest {
                catalog_id: "github".into(),
                source: "registry".into(),
                server_id_override: None,
                env_overrides: env_overrides.clone(),
                trust_grant: false,
                auto_start: true,
            },
        }],
    )
    .await;

    let request = runtime
        .last_install_request()
        .expect("install request should reach facade");
    assert_eq!(request.catalog_id, "github");
    assert_eq!(request.source, "registry");
    assert_eq!(request.env_overrides, env_overrides);
}

#[tokio::test]
async fn mcp_overlay_install_outcome_persists_after_command_refresh() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::{Command, Component};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::collections::BTreeMap;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn rendered_mcp_overlay(app: &App) -> String {
        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| app.mcp_overlay.render(f.area(), f))
            .expect("render");
        terminal.backend().to_string()
    }

    let cases = [
        (
            FakeInstallResult::Outcome(agent_core::facade::InstallOutcomeView {
                kind: "installed".into(),
                server_id: Some("filesystem".into()),
                started: Some(true),
                missing_runtimes: Vec::new(),
                missing_env_keys: Vec::new(),
            }),
            "install status: installed as filesystem (started)",
        ),
        (
            FakeInstallResult::Outcome(agent_core::facade::InstallOutcomeView {
                kind: "already_installed".into(),
                server_id: Some("filesystem".into()),
                started: None,
                missing_runtimes: Vec::new(),
                missing_env_keys: Vec::new(),
            }),
            "install status: already installed as filesystem",
        ),
        (
            FakeInstallResult::Outcome(agent_core::facade::InstallOutcomeView {
                kind: "invalid_env".into(),
                server_id: None,
                started: None,
                missing_runtimes: Vec::new(),
                missing_env_keys: vec!["Authorization".into()],
            }),
            "install status: missing env Authorization",
        ),
        (
            FakeInstallResult::Outcome(agent_core::facade::InstallOutcomeView {
                kind: "runtime_missing".into(),
                server_id: None,
                started: None,
                missing_runtimes: vec!["node >=18".into()],
                missing_env_keys: Vec::new(),
            }),
            "install status: missing runtime node >=18",
        ),
        (
            FakeInstallResult::Error("write failed".into()),
            "install status: failed invalid state: write failed",
        ),
    ];

    for (install_result, expected) in cases {
        let runtime = Arc::new(TuiMcpFakeFacade::with_install_result(install_result));
        let mut app = App::new("fake", WorkspaceId::new());

        agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenMcpOverlay]).await;
        for _ in 0..3 {
            let ctx = app
                .state
                .event_context(&app.workspace_id, &app.current_session_id);
            let _ = app.mcp_overlay.handle_event(&ctx, &key(KeyCode::Tab));
        }

        agent_tui::app::dispatch_commands(
            &runtime,
            &mut app,
            vec![Command::InstallMcpServer {
                request: agent_core::facade::InstallRequest {
                    catalog_id: "filesystem".into(),
                    source: "builtin".into(),
                    server_id_override: None,
                    env_overrides: BTreeMap::new(),
                    trust_grant: false,
                    auto_start: true,
                },
            }],
        )
        .await;

        let rendered = rendered_mcp_overlay(&app);
        assert!(
            rendered.contains(expected),
            "expected {expected:?}, got {rendered}"
        );
    }
}
