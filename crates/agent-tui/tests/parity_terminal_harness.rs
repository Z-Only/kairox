//! Terminal-level TUI parity harness.
//!
//! These scenarios drive `App::handle_crossterm_event` instead of calling
//! individual components directly, giving the TUI a compact counterpart to
//! GUI e2e-pilot coverage for key parity paths.

use std::collections::BTreeMap;

use agent_core::facade::{
    CatalogSourceView, InstallRequest, ServerEntry, SkillCatalogEntry, SkillFieldMappingView,
    SkillInstallTarget, SkillSourceView,
};
use agent_core::{ProjectSessionVisibility, SessionId, WorkspaceId};
use agent_tools::PermissionMode;
use agent_tui::app::App;
use agent_tui::app_state::SettingsConfigSource;
use agent_tui::components::{
    Command, CrossPanelEffect, FocusTarget, McpOverlaySnapshot, QueueAction, SessionInfo,
    SessionState, SkillOverlaySnapshot,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

mod support;

use support::render::render_app;

struct TerminalHarness {
    app: App,
}

impl TerminalHarness {
    fn new() -> Self {
        let workspace_id = WorkspaceId::from_string("wrk_terminal_harness".into());
        let session_id = SessionId::from_string("ses_terminal_harness".into());
        let mut app = App::new("fake", PermissionMode::Suggest, workspace_id);
        app.current_session_id = Some(session_id);
        Self { app }
    }

    fn key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Vec<Command> {
        self.app
            .handle_crossterm_event(&Event::Key(KeyEvent::new(code, modifiers)))
    }

    fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            let commands = self.key(KeyCode::Char(ch), KeyModifiers::NONE);
            assert!(
                commands.is_empty(),
                "typing {ch:?} should not emit commands; got {commands:?}"
            );
        }
    }

    fn activate_palette(&mut self, filter: &str) -> Vec<Command> {
        let commands = self.key(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert!(commands.is_empty(), "Ctrl+P should only open the palette");
        assert!(self.app.command_palette.is_visible());
        assert_eq!(
            self.app.state.focus_manager.current(),
            FocusTarget::CommandPalette
        );

        self.type_text(filter);
        let commands = self.key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!self.app.command_palette.is_visible());
        assert_eq!(self.app.state.focus_manager.current(), FocusTarget::Chat);
        commands
    }

    fn render(&mut self) -> String {
        render_app(&mut self.app, 160, 42)
    }
}

fn mcp_source(id: &str, enabled: bool) -> CatalogSourceView {
    CatalogSourceView {
        id: id.into(),
        display_name: id.into(),
        kind: "mcp_registry".into(),
        url: format!("https://{id}.example.test/catalog"),
        api_key_env: None,
        priority: 10,
        default_trust: "verified".into(),
        enabled,
        cache_ttl_seconds: Some(900),
        last_error: None,
    }
}

fn mcp_catalog_entry(id: &str, source: &str) -> ServerEntry {
    ServerEntry {
        id: id.into(),
        source: source.into(),
        display_name: id.into(),
        summary: format!("{id} summary"),
        description: format!("{id} server"),
        categories: vec!["dev".into()],
        tags: vec!["terminal".into()],
        author: Some("Kairox".into()),
        homepage: Some(format!("https://{id}.example.test")),
        version: Some("1.0.0".into()),
        trust: "verified".into(),
        verified: true,
        icon: None,
        install_spec_json: "{}".into(),
        requirements_json: "[]".into(),
        default_env_json: "[]".into(),
    }
}

fn skill_source(id: &str, enabled: bool) -> SkillSourceView {
    SkillSourceView {
        id: id.into(),
        display_name: id.into(),
        kind: "skillhub".into(),
        url: format!("https://{id}.example.test"),
        search_template: "/api/skills?keyword={{query}}".into(),
        download_template: "/api/v1/download?slug={{slug}}".into(),
        list_template: Some("/api/skills?pageSize={{limit}}".into()),
        detail_template: Some("/api/v1/skills/{{slug}}".into()),
        field_mapping: SkillFieldMappingView::default(),
        enabled,
        priority: 10,
        cache_ttl_seconds: 900,
        last_error: None,
    }
}

fn skill_catalog_entry(name: &str) -> SkillCatalogEntry {
    SkillCatalogEntry {
        catalog_id: name.into(),
        name: format!("{name} catalog skill"),
        description: "Review and smoke-test TUI parity".into(),
        source: "skillhub".into(),
        source_url: format!("https://skillhub.example.test/{name}"),
        install_count: Some(42),
        github_stars: Some(7),
        security_score: Some(95),
        rating: Some(4.8),
        package: name.into(),
        package_url: Some(format!("https://skillhub.example.test/{name}.zip")),
    }
}

fn session_info(id: SessionId, title: &str, archived: bool) -> SessionInfo {
    SessionInfo {
        id,
        title: title.into(),
        model_profile: "fake".into(),
        state: SessionState::Idle,
        pinned: false,
        archived,
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: archived.then_some(ProjectSessionVisibility::Archived),
    }
}

#[test]
fn terminal_harness_command_palette_routes_entry_points_and_prefills() {
    let mut harness = TerminalHarness::new();

    let commands = harness.activate_palette("mcp-manager");
    assert!(matches!(&commands[..], [Command::OpenMcpOverlay]));

    let commands = harness.activate_palette("skills-manager");
    assert!(matches!(&commands[..], [Command::OpenSkillsOverlay]));

    let commands = harness.activate_palette("settings-source-project");
    assert!(matches!(
        &commands[..],
        [Command::SetSettingsConfigSource {
            source: SettingsConfigSource::Project
        }]
    ));

    let commands = harness.activate_palette("queue-delete");
    assert!(matches!(
        &commands[..],
        [Command::ApplyQueueAction(QueueAction::DeleteSelected)]
    ));

    let commands = harness.activate_palette("skill-install-github");
    assert!(commands.is_empty());
    assert_eq!(harness.app.chat.input_content, ":skill install github ");
    assert_eq!(harness.app.state.focus_manager.current(), FocusTarget::Chat);
}

#[test]
fn terminal_harness_overlay_shortcuts_keep_navigation_inside_overlays() {
    let mut harness = TerminalHarness::new();

    let commands = harness.key(KeyCode::Char('m'), KeyModifiers::CONTROL);
    assert!(matches!(&commands[..], [Command::OpenMcpOverlay]));
    harness
        .app
        .dispatch_effects(vec![CrossPanelEffect::ShowMcpOverlay(McpOverlaySnapshot {
            runtime_servers: Vec::new(),
            settings: Vec::new(),
            installed: Vec::new(),
            catalog: vec![mcp_catalog_entry("filesystem", "builtin")],
            sources: vec![mcp_source("builtin", true)],
        })]);
    assert!(harness.app.mcp_overlay.is_visible());
    assert_eq!(
        harness.app.state.focus_manager.current(),
        FocusTarget::McpOverlay
    );

    let commands = harness.key(KeyCode::Tab, KeyModifiers::NONE);
    assert!(commands.is_empty());
    assert!(harness.app.mcp_overlay.is_visible());
    assert_eq!(
        harness.app.state.focus_manager.current(),
        FocusTarget::McpOverlay
    );

    let commands = harness.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(commands.is_empty());
    assert!(!harness.app.mcp_overlay.is_visible());
    assert_eq!(harness.app.state.focus_manager.current(), FocusTarget::Chat);

    let commands = harness.key(KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(matches!(&commands[..], [Command::OpenSkillsOverlay]));
    harness
        .app
        .dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
            SkillOverlaySnapshot {
                discovered: Vec::new(),
                installed: Vec::new(),
                catalog: vec![skill_catalog_entry("review")],
                sources: vec![skill_source("skillhub", true)],
                install_target: SkillInstallTarget::User,
            },
        )]);
    assert!(harness.app.skills_overlay.is_visible());
    assert_eq!(
        harness.app.state.focus_manager.current(),
        FocusTarget::SkillsOverlay
    );

    let commands = harness.key(KeyCode::Tab, KeyModifiers::NONE);
    assert!(commands.is_empty());
    assert!(harness.app.skills_overlay.is_visible());
    assert_eq!(
        harness.app.state.focus_manager.current(),
        FocusTarget::SkillsOverlay
    );

    let commands = harness.key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(commands.is_empty());
    assert!(!harness.app.skills_overlay.is_visible());
    assert_eq!(harness.app.state.focus_manager.current(), FocusTarget::Chat);
}

#[test]
fn terminal_harness_queue_shortcuts_and_archive_restore_stay_reachable() {
    let mut harness = TerminalHarness::new();
    let archived_id = SessionId::from_string("ses_archived".into());

    harness
        .app
        .chat
        .message_queue
        .push(agent_tui::components::QueuedMessage {
            content: "first queued".into(),
            attachments: Vec::new(),
        });
    harness
        .app
        .chat
        .message_queue
        .push(agent_tui::components::QueuedMessage {
            content: "second queued".into(),
            attachments: Vec::new(),
        });

    let commands = harness.key(KeyCode::Down, KeyModifiers::ALT);
    assert!(commands.is_empty());
    assert_eq!(harness.app.chat.selected_queue_index(), Some(1));

    let commands = harness.key(KeyCode::Left, KeyModifiers::ALT);
    assert!(commands.is_empty());
    assert_eq!(harness.app.chat.selected_queue_index(), Some(0));
    assert_eq!(harness.app.chat.message_queue[0].content, "second queued");

    let commands = harness.key(KeyCode::Enter, KeyModifiers::ALT);
    assert!(matches!(
        &commands[..],
        [Command::SendQueuedMessageNow { queue_index: 0, .. }]
    ));
    assert_eq!(harness.app.chat.message_queue.len(), 2);

    let commands = harness.key(KeyCode::Backspace, KeyModifiers::ALT);
    assert!(commands.is_empty());
    assert_eq!(harness.app.chat.message_queue.len(), 1);
    assert_eq!(harness.app.chat.message_queue[0].content, "first queued");

    harness.app.state.sessions = vec![session_info(archived_id.clone(), "archived", true)];
    harness.app.state.focus_manager.set(FocusTarget::Sessions);
    harness.app.sync_component_focus();

    let commands = harness.key(KeyCode::Char('a'), KeyModifiers::NONE);
    assert!(commands.is_empty());
    assert!(harness.app.sessions.archive_manager_open);

    let commands = harness.key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        commands,
        vec![Command::RestoreSession {
            session_id: archived_id
        }]
    );
    assert!(!harness.app.sessions.archive_manager_open);
}

#[test]
fn terminal_harness_mcp_install_config_collects_required_values() {
    let mut harness = TerminalHarness::new();
    let mut entry = mcp_catalog_entry("github", "registry");
    entry.install_spec_json =
        r#"{"transport":"sse","url":"https://mcp.example.test/sse","headers":{"Authorization":"Bearer ${Authorization}"}}"#
            .into();
    entry.default_env_json = r#"[
        {"key":"Authorization","label":"Authorization","description":"Bearer token","required":true,"secret":true,"default":null},
        {"key":"GITHUB_ORG","label":"GitHub org","description":"Organization","required":true,"secret":false,"default":null}
    ]"#
    .into();

    harness
        .app
        .dispatch_effects(vec![CrossPanelEffect::ShowMcpOverlay(McpOverlaySnapshot {
            runtime_servers: Vec::new(),
            settings: Vec::new(),
            installed: Vec::new(),
            catalog: vec![entry],
            sources: vec![mcp_source("registry", true)],
        })]);

    for _ in 0..3 {
        let commands = harness.key(KeyCode::Tab, KeyModifiers::NONE);
        assert!(commands.is_empty());
    }

    let commands = harness.key(KeyCode::Char('i'), KeyModifiers::NONE);
    assert!(
        commands.is_empty(),
        "required config should open editor before install command; got {commands:?}"
    );
    let rendered = harness.render();
    assert!(rendered.contains("Install configuration"), "{rendered}");
    assert!(rendered.contains("Authorization"), "{rendered}");
    assert!(rendered.contains("GITHUB_ORG"), "{rendered}");

    harness.type_text("Bearer terminal-token");
    let commands = harness.key(KeyCode::Tab, KeyModifiers::NONE);
    assert!(commands.is_empty());
    harness.type_text("kairox-dev");

    let commands = harness.key(KeyCode::Enter, KeyModifiers::NONE);
    let expected_overrides = BTreeMap::from([
        (
            "Authorization".to_string(),
            "Bearer terminal-token".to_string(),
        ),
        ("GITHUB_ORG".to_string(), "kairox-dev".to_string()),
    ]);
    assert!(matches!(
        &commands[..],
        [Command::InstallMcpServer { request }]
            if request == &InstallRequest {
                catalog_id: "github".into(),
                source: "registry".into(),
                server_id_override: None,
                env_overrides: expected_overrides,
                trust_grant: false,
                auto_start: true,
            }
    ));
}

#[test]
fn terminal_harness_skills_catalog_detail_installs_selected_target() {
    let mut harness = TerminalHarness::new();
    harness
        .app
        .dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
            SkillOverlaySnapshot {
                discovered: Vec::new(),
                installed: Vec::new(),
                catalog: vec![skill_catalog_entry("review")],
                sources: vec![skill_source("skillhub", true)],
                install_target: SkillInstallTarget::User,
            },
        )]);

    for _ in 0..2 {
        let commands = harness.key(KeyCode::Tab, KeyModifiers::NONE);
        assert!(commands.is_empty());
    }

    let commands = harness.key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(commands.is_empty());
    let rendered = harness.render();
    assert!(rendered.contains("Source: https://skillhub.example.test/review"));
    assert!(rendered.contains("Package: review"));
    assert!(rendered.contains("Download: https://skillhub.example.test/review.zip"));
    assert!(rendered.contains("Target: user"));

    let commands = harness.key(KeyCode::Char('t'), KeyModifiers::NONE);
    assert!(commands.is_empty());
    let rendered = harness.render();
    assert!(rendered.contains("Target: project"));

    let commands = harness.key(KeyCode::Char('i'), KeyModifiers::NONE);
    assert!(matches!(
        &commands[..],
        [Command::InstallRemoteSkill { request }]
            if request.package == "review"
                && request.source == "skillhub"
                && request.target == SkillInstallTarget::Project
                && request.package_url.as_deref()
                    == Some("https://skillhub.example.test/review.zip")
    ));
}
