use super::*;
use std::collections::BTreeMap;

use agent_core::facade::{CatalogSourceView, InstalledEntry, McpServerSettingsView, ServerEntry};

use crate::components::{
    FocusTarget, McpOverlaySnapshot, McpPromptEntry, McpResourceEntry, McpServerEntry,
    McpServerStatusView, McpToolEntry,
};

fn entry(id: &str, status: McpServerStatusView, trusted: bool, tools: usize) -> McpServerEntry {
    McpServerEntry {
        server_id: id.to_string(),
        status,
        trusted,
        tool_count: tools,
    }
}

fn setting(id: &str, enabled: bool) -> McpServerSettingsView {
    McpServerSettingsView {
        id: id.to_string(),
        name: id.to_string(),
        transport: "stdio".to_string(),
        enabled,
        runtime_status: "stopped".to_string(),
        trusted: false,
        tool_count: Some(2),
        last_error: None,
        writable: true,
        config_path: Some("/tmp/kairox/config.toml".to_string()),
        description: Some(format!("{id} settings")),
        source: "user".to_string(),
        verified: false,
        diagnostic_summary: String::new(),
    }
}

fn installed(id: &str, running: bool) -> InstalledEntry {
    InstalledEntry {
        server_id: id.to_string(),
        catalog_id: Some(format!("{id}-catalog")),
        source: Some("builtin".to_string()),
        display_name: id.to_string(),
        installed_at: "2026-05-21T00:00:00Z".to_string(),
        running,
    }
}

fn catalog_entry(id: &str, source: &str) -> ServerEntry {
    ServerEntry {
        id: id.to_string(),
        source: source.to_string(),
        display_name: format!("{id} MCP"),
        summary: format!("{id} summary"),
        description: format!("{id} description"),
        categories: vec!["dev".to_string()],
        tags: vec!["local".to_string()],
        author: Some("Kairox".to_string()),
        homepage: None,
        version: Some("1.0.0".to_string()),
        trust: "verified".to_string(),
        verified: true,
        icon: None,
        install_spec_json: "{}".to_string(),
        requirements_json: "[]".to_string(),
        default_env_json: "[]".to_string(),
    }
}

fn source(id: &str, enabled: bool) -> CatalogSourceView {
    CatalogSourceView {
        id: id.to_string(),
        display_name: id.to_string(),
        kind: "mcp_registry".to_string(),
        url: format!("https://example.com/{id}"),
        api_key_env: None,
        priority: 10,
        default_trust: "community".to_string(),
        enabled,
        cache_ttl_seconds: Some(300),
        last_error: None,
    }
}

fn tool(name: &str, disabled: bool) -> McpToolEntry {
    McpToolEntry {
        server_id: "alpha".to_string(),
        name: name.to_string(),
        description: Some(format!("{name} tool")),
        input_schema: None,
        disabled,
    }
}

fn resource(uri: &str) -> McpResourceEntry {
    McpResourceEntry {
        server_id: "alpha".to_string(),
        uri: uri.to_string(),
        name: "App log".to_string(),
        description: Some("Application log".to_string()),
        mime_type: Some("text/plain".to_string()),
    }
}

fn prompt(name: &str) -> McpPromptEntry {
    McpPromptEntry {
        server_id: "alpha".to_string(),
        name: name.to_string(),
        description: Some(format!("{name} prompt")),
        argument_count: 2,
    }
}

fn advance_tabs(overlay: &mut McpOverlay, count: usize) {
    for _ in 0..count {
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    }
}

fn snapshot() -> McpOverlaySnapshot {
    McpOverlaySnapshot {
        runtime_servers: vec![
            entry("alpha", McpServerStatusView::Running, true, 3),
            entry("beta", McpServerStatusView::Stopped, false, 0),
        ],
        settings: vec![setting("alpha", true), setting("beta", false)],
        installed: vec![installed("alpha", true)],
        catalog: vec![catalog_entry("filesystem", "builtin")],
        sources: vec![source("registry", true)],
    }
}

fn runtime_snapshot(runtime_servers: Vec<McpServerEntry>) -> McpOverlaySnapshot {
    McpOverlaySnapshot {
        runtime_servers,
        settings: Vec::new(),
        installed: Vec::new(),
        catalog: Vec::new(),
        sources: Vec::new(),
    }
}

fn test_ctx() -> EventContext<'static> {
    use agent_core::projection::SessionProjection;
    static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
    let projection = PROJECTION.get_or_init(SessionProjection::default);
    static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
        std::sync::OnceLock::new();
    let sessions = SESSIONS.get_or_init(Vec::new);
    EventContext {
        focus: FocusTarget::McpOverlay,
        current_session: projection,
        projects: &[],
        sessions,
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: Box::leak(Box::new(agent_core::WorkspaceId::new())),
        current_session_id: Box::leak(Box::new(None)),
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(crossterm::event::KeyEvent::new(
        code,
        crossterm::event::KeyModifiers::NONE,
    ))
}

fn type_text(overlay: &mut McpOverlay, text: &str) {
    for ch in text.chars() {
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char(ch)));
    }
}

fn rendered_overlay(overlay: &McpOverlay) -> String {
    let backend = ratatui::backend::TestBackend::new(120, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| overlay.render(f.area(), f))
        .expect("render");
    terminal.backend().to_string()
}

#[test]
fn overlay_invisible_by_default() {
    let overlay = McpOverlay::new();
    assert!(!overlay.is_visible());
    assert!(overlay.servers().is_empty());
}

#[test]
fn renders_server_list_from_runtime() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![
        entry("alpha", McpServerStatusView::Running, true, 3),
        entry("beta", McpServerStatusView::Stopped, false, 0),
    ]));
    assert!(overlay.is_visible());
    assert_eq!(overlay.servers().len(), 2);
    assert_eq!(overlay.selected_index(), Some(0));
    // Render into a test buffer to ensure no panic and selection drawn.
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| overlay.render(f.area(), f))
        .expect("render");
}

#[test]
fn j_and_k_navigate_selection() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![
        entry("alpha", McpServerStatusView::Running, false, 1),
        entry("beta", McpServerStatusView::Stopped, false, 0),
        entry("gamma", McpServerStatusView::Failed, false, 0),
    ]));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(1));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(2));
    // Down again clamps at last index.
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Down));
    assert_eq!(overlay.selected_index(), Some(2));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
    assert_eq!(overlay.selected_index(), Some(1));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Up));
    assert_eq!(overlay.selected_index(), Some(0));
    // Up at top stays at 0.
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
    assert_eq!(overlay.selected_index(), Some(0));
}

#[test]
fn enter_starts_stopped_server() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "beta",
        McpServerStatusView::Stopped,
        false,
        0,
    )]));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        Command::StartMcpServer { server_id } if server_id == "beta"
    ));
}

#[test]
fn enter_stops_running_server() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        true,
        5,
    )]));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        Command::StopMcpServer { server_id } if server_id == "alpha"
    ));
}

#[test]
fn enter_starts_failed_server() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "crash",
        McpServerStatusView::Failed,
        false,
        0,
    )]));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(matches!(
        &commands[0],
        Command::StartMcpServer { server_id } if server_id == "crash"
    ));
}

#[test]
fn t_emits_trust_command_for_selected_server() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![
        entry("alpha", McpServerStatusView::Running, false, 1),
        entry("beta", McpServerStatusView::Running, false, 1),
    ]));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
    assert!(matches!(
        &commands[0],
        Command::TrustMcpServer { server_id } if server_id == "beta"
    ));
}

#[test]
fn t_emits_revoke_command_for_trusted_server() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        true,
        1,
    )]));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
    assert!(matches!(
        &commands[..],
        [Command::RevokeMcpTrust { server_id }] if server_id == "alpha"
    ));
}

#[test]
fn r_emits_refresh_tools_command() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        false,
        1,
    )]));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
    assert!(matches!(
        &commands[0],
        Command::RefreshMcpTools { server_id } if server_id == "alpha"
    ));
}

#[test]
fn runtime_tab_emits_health_and_connectivity_commands() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        false,
        1,
    )]));

    let (_, health_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('h')));
    assert!(matches!(
        &health_commands[..],
        [Command::CheckMcpHealth { server_id }] if server_id == "alpha"
    ));

    let (_, connectivity_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('c')));
    assert!(matches!(
        &connectivity_commands[..],
        [Command::TestMcpConnectivity { server_id }] if server_id == "alpha"
    ));
}

#[test]
fn tools_tab_toggles_selected_tool_disabled_state() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        false,
        2,
    )]));
    overlay.handle_effect(&CrossPanelEffect::McpToolsLoaded {
        server_id: "alpha".to_string(),
        tools: vec![tool("search", false), tool("write", true)],
        healthy: true,
        error: None,
    });
    advance_tabs(&mut overlay, 5);

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
    assert!(matches!(
        &commands[..],
        [Command::SetMcpToolDisabled {
            server_id,
            tool_name,
            disabled,
        }] if server_id == "alpha" && tool_name == "write" && !disabled
    ));
}

#[test]
fn resources_tab_lists_and_reads_selected_resource() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        false,
        1,
    )]));
    advance_tabs(&mut overlay, 6);

    let (_, list_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
    assert!(matches!(
        &list_commands[..],
        [Command::ListMcpResources { server_id }] if server_id == "alpha"
    ));

    overlay.handle_effect(&CrossPanelEffect::McpResourcesLoaded {
        server_id: "alpha".to_string(),
        resources: vec![resource("file://logs/app.log")],
    });
    let (_, read_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(matches!(
        &read_commands[..],
        [Command::ReadMcpResource { server_id, uri }]
            if server_id == "alpha" && uri == "file://logs/app.log"
    ));
}

#[test]
fn prompts_tab_lists_prompts_for_selected_runtime_server() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        false,
        1,
    )]));
    advance_tabs(&mut overlay, 7);

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
    assert!(matches!(
        &commands[..],
        [Command::ListMcpPrompts { server_id }] if server_id == "alpha"
    ));

    overlay.handle_effect(&CrossPanelEffect::McpPromptsLoaded {
        server_id: "alpha".to_string(),
        prompts: vec![prompt("summarize")],
    });
    assert_eq!(overlay.selected_index(), Some(0));
}

#[test]
fn esc_hides_and_emits_dismiss_effect() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        false,
        1,
    )]));
    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
    assert!(commands.is_empty());
    assert!(effects.contains(&CrossPanelEffect::DismissMcpOverlay));
    assert!(!overlay.is_visible());
}

#[test]
fn ignores_keys_when_hidden() {
    let mut overlay = McpOverlay::new();
    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(effects.is_empty());
    assert!(commands.is_empty());
}

#[test]
fn show_effect_makes_visible() {
    let mut overlay = McpOverlay::new();
    overlay.handle_effect(&CrossPanelEffect::ShowMcpOverlay(runtime_snapshot(vec![
        entry("alpha", McpServerStatusView::Running, false, 1),
    ])));
    assert!(overlay.is_visible());
    assert_eq!(overlay.servers().len(), 1);
}

#[test]
fn enter_with_no_servers_emits_nothing() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(Vec::new()));
    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(effects.is_empty());
    assert!(commands.is_empty());
}

#[test]
fn tabs_preserve_independent_selection() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(1));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    assert_eq!(overlay.selected_index(), Some(0));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(1));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));
    assert_eq!(overlay.selected_index(), Some(1));
}

#[test]
fn settings_tab_emits_enable_and_delete_commands() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, enable_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
    assert!(matches!(
        &enable_commands[..],
        [Command::SetMcpServerEnabled { server_id, enabled }]
            if server_id == "alpha" && !enabled
    ));

    let (_, delete_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
    assert!(matches!(
        &delete_commands[..],
        [Command::DeleteMcpServerSettings { server_id }] if server_id == "alpha"
    ));
}

#[test]
fn settings_tab_opens_config_and_updates_project_scope_disablement() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, open_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('o')));
    assert!(matches!(&open_commands[..], [Command::OpenMcpConfig]));

    let (_, disable_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('d')));
    assert!(matches!(
        &disable_commands[..],
        [Command::DisableMcpServerAtScope { server_id }] if server_id == "alpha"
    ));

    let (_, enable_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));
    assert!(matches!(
        &enable_commands[..],
        [Command::EnableMcpServerAtScope { server_id }] if server_id == "alpha"
    ));
}

#[test]
fn settings_tab_server_editor_saves_new_stdio_server() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('n')));
    assert!(commands.is_empty());
    type_text(&mut overlay, "gamma");
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    type_text(&mut overlay, "npx");
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    type_text(&mut overlay, "-y @modelcontextprotocol/server-filesystem");
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

    assert!(matches!(
        &commands[..],
        [Command::SaveMcpServerSettings { input }]
            if input.name == "gamma"
                && input.enabled
                && input.description.is_none()
                && matches!(
                    &input.transport,
                    agent_core::facade::McpServerSettingsTransport::Stdio { command, args, env }
                        if command == "npx"
                            && args.as_slice() == [
                                "-y".to_string(),
                                "@modelcontextprotocol/server-filesystem".to_string()
                            ]
                            && env.is_empty()
                )
    ));
}

#[test]
fn settings_tab_enter_edits_selected_server() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(commands.is_empty());

    assert_eq!(overlay.server_draft_name_for_test(), Some("alpha"));
}

#[test]
fn catalog_and_installed_tabs_emit_install_uninstall_commands() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let (_, uninstall_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
    assert!(matches!(
        &uninstall_commands[..],
        [Command::UninstallMcpServer { server_id }] if server_id == "alpha"
    ));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let (_, install_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
    assert!(matches!(
        &install_commands[..],
        [Command::InstallMcpServer { request }]
            if request.catalog_id == "filesystem"
                && request.source == "builtin"
                && request.server_id_override.is_none()
                && request.env_overrides == BTreeMap::new()
                && request.auto_start
                && !request.trust_grant
    ));
}

#[test]
fn catalog_install_renders_in_flight_status_after_command_emission() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());
    advance_tabs(&mut overlay, 3);

    let (_, install_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
    assert!(matches!(
        &install_commands[..],
        [Command::InstallMcpServer { request }]
            if request.catalog_id == "filesystem" && request.source == "builtin"
    ));

    let rendered = rendered_overlay(&overlay);
    assert!(
        rendered.contains("install status: installing"),
        "{rendered}"
    );
}

#[test]
fn catalog_install_config_editor_collects_required_overrides() {
    let mut entry = catalog_entry("github", "registry");
    entry.install_spec_json = r#"{"transport":"sse","url":"https://mcp.example.com/sse","headers":{"Authorization":"Bearer ${Authorization}"}}"#.to_string();
    entry.default_env_json = r#"[
            {"key":"Authorization","label":"Authorization","description":"Bearer token","required":true,"secret":true,"default":null},
            {"key":"GITHUB_ORG","label":"GitHub org","description":"Organization","required":true,"secret":false,"default":null}
        ]"#.to_string();

    let mut overlay = McpOverlay::new();
    overlay.show(McpOverlaySnapshot {
        runtime_servers: Vec::new(),
        settings: Vec::new(),
        installed: Vec::new(),
        catalog: vec![entry],
        sources: vec![source("registry", true)],
    });
    advance_tabs(&mut overlay, 3);

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
    assert!(
        commands.is_empty(),
        "expected install config editor before command emission, got {commands:?}"
    );
    let rendered = rendered_overlay(&overlay);
    assert!(rendered.contains("Install configuration"), "{rendered}");
    assert!(rendered.contains("Authorization"), "{rendered}");
    assert!(rendered.contains("GITHUB_ORG"), "{rendered}");

    type_text(&mut overlay, "Bearer test-token");
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    type_text(&mut overlay, "kairox-dev");
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

    let mut expected_overrides = BTreeMap::new();
    expected_overrides.insert("Authorization".to_string(), "Bearer test-token".to_string());
    expected_overrides.insert("GITHUB_ORG".to_string(), "kairox-dev".to_string());
    assert!(matches!(
        &commands[..],
        [Command::InstallMcpServer { request }]
            if request.catalog_id == "github"
                && request.source == "registry"
                && request.server_id_override.is_none()
                && request.env_overrides == expected_overrides
                && request.auto_start
                && !request.trust_grant
    ));
}

#[test]
fn catalog_tab_filters_by_keyword_trust_and_enabled_source() {
    let mut filesystem = catalog_entry("filesystem", "builtin");
    filesystem.summary = "Browse local files".to_string();

    let mut slack = catalog_entry("slack", "registry");
    slack.summary = "Search Slack messages".to_string();
    slack.tags = vec!["chat".to_string()];
    slack.trust = "community".to_string();
    slack.verified = false;

    let mut risky = catalog_entry("risky", "corp");
    risky.summary = "Internal unverified tool".to_string();
    risky.trust = "unverified".to_string();
    risky.verified = false;

    let mut registry = source("registry", true);
    registry.default_trust = "verified".to_string();
    let disabled_corp = source("corp", false);

    let mut overlay = McpOverlay::new();
    overlay.show(McpOverlaySnapshot {
        runtime_servers: Vec::new(),
        settings: Vec::new(),
        installed: Vec::new(),
        catalog: vec![filesystem, slack, risky],
        sources: vec![registry, disabled_corp],
    });
    advance_tabs(&mut overlay, 3);

    assert_eq!(overlay.current_len(), 2);

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
    assert_eq!(overlay.current_len(), 2);

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('/')));
    type_text(&mut overlay, "slack");
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

    assert_eq!(overlay.current_len(), 1);
    assert_eq!(
        overlay
            .selected_catalog_entry()
            .map(|entry| entry.id.as_str()),
        Some("slack")
    );

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
    assert_eq!(overlay.current_len(), 0);
    assert!(overlay.selected_catalog_entry().is_none());
}

#[test]
fn catalog_tab_renders_detail_metadata_for_selected_entry() {
    let mut entry = catalog_entry("github", "registry");
    entry.description = "Browse repositories and issues".to_string();
    entry.categories = vec!["dev".to_string(), "source-control".to_string()];
    entry.tags = vec!["git".to_string(), "issues".to_string()];
    entry.homepage = Some("https://github.com/modelcontextprotocol".to_string());
    entry.install_spec_json = r#"{"transport":"sse","url":"https://mcp.example.com/sse","headers":{"Authorization":"Bearer ${Authorization}"}}"#.to_string();
    entry.requirements_json =
        r#"[{"kind":"node","min_version":"18","install_hint":"Install Node.js"}]"#.to_string();
    entry.default_env_json = r#"[
            {"key":"Authorization","label":"Authorization","description":"Bearer token","required":true,"secret":true,"default":null},
            {"key":"OPTIONAL_MODE","label":"Mode","description":"Optional mode","required":false,"secret":false,"default":"read"}
        ]"#.to_string();

    let mut overlay = McpOverlay::new();
    overlay.show(McpOverlaySnapshot {
        runtime_servers: Vec::new(),
        settings: Vec::new(),
        installed: Vec::new(),
        catalog: vec![entry],
        sources: vec![source("registry", true)],
    });
    advance_tabs(&mut overlay, 3);

    let rendered = rendered_overlay(&overlay);
    assert!(
        rendered.contains("Browse repositories and issues"),
        "{rendered}"
    );
    assert!(rendered.contains("trust: verified"), "{rendered}");
    assert!(rendered.contains("requirements"), "{rendered}");
    assert!(rendered.contains("node >=18"), "{rendered}");
    assert!(rendered.contains("configuration"), "{rendered}");
    assert!(rendered.contains("Authorization"), "{rendered}");
    assert!(rendered.contains("HTTP header"), "{rendered}");
    assert!(rendered.contains("required secret"), "{rendered}");
    assert!(rendered.contains("OPTIONAL_MODE"), "{rendered}");
    assert!(rendered.contains("env optional"), "{rendered}");
}

#[test]
fn sources_tab_emits_source_enable_command() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());
    advance_tabs(&mut overlay, 4);

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
    assert!(matches!(
        &commands[..],
        [Command::SetMcpCatalogSourceEnabled { source_id, enabled }]
            if source_id == "registry" && !enabled
    ));
}

#[test]
fn sources_tab_adds_and_removes_catalog_sources() {
    let mut overlay = McpOverlay::new();
    overlay.show(snapshot());
    advance_tabs(&mut overlay, 4);

    let (_, remove_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
    assert!(matches!(
        &remove_commands[..],
        [Command::RemoveMcpCatalogSource { source_id }] if source_id == "registry"
    ));

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('n')));
    assert!(commands.is_empty());
    type_text(&mut overlay, "corp");
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    type_text(&mut overlay, "Corporate Registry");
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    type_text(&mut overlay, "https://registry.example.com/catalog.json");
    let (_, add_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

    assert!(matches!(
        &add_commands[..],
        [Command::AddMcpCatalogSource { request }]
            if request.id == "corp"
                && request.display_name == "Corporate Registry"
                && request.kind == "mcp_registry"
                && request.url == "https://registry.example.com/catalog.json"
                && request.api_key_env.is_none()
                && request.priority == Some(100)
                && request.default_trust.as_deref() == Some("community")
                && request.enabled == Some(true)
                && request.cache_ttl_seconds.is_none()
    ));
}

#[test]
fn runtime_tab_shows_disabled_tool_count() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        false,
        3,
    )]));
    // Load tools with one disabled
    overlay.handle_effect(&CrossPanelEffect::McpToolsLoaded {
        server_id: "alpha".to_string(),
        tools: vec![
            tool("search", false),
            tool("write", true),
            tool("delete", false),
        ],
        healthy: true,
        error: None,
    });
    // Stay on runtime tab and render
    let text = rendered_overlay(&overlay);
    assert!(
        text.contains("1 off"),
        "runtime tab should show disabled tool count: {text}"
    );
}

#[test]
fn runtime_tab_omits_disabled_count_when_all_enabled() {
    let mut overlay = McpOverlay::new();
    overlay.show(runtime_snapshot(vec![entry(
        "alpha",
        McpServerStatusView::Running,
        false,
        2,
    )]));
    overlay.handle_effect(&CrossPanelEffect::McpToolsLoaded {
        server_id: "alpha".to_string(),
        tools: vec![tool("search", false), tool("write", false)],
        healthy: true,
        error: None,
    });
    let text = rendered_overlay(&overlay);
    assert!(
        !text.contains("off"),
        "runtime tab should not show disabled count when all tools enabled: {text}"
    );
}
