//! TUI App logic integration tests — overlay shortcuts, archive overlay,
//! and destructive-key second-press protection.
//!
//! Split from the former `app_logic.rs`. Shared helpers live in
//! `app_logic_common`.

#![allow(unused_imports)]

mod app_logic_common;

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_skills::{FileSkillRegistry, SkillRoot, SkillSourceKind};
use agent_store::SqliteEventStore;
use app_logic_common::{
    agent_settings_view, test_project, unique_temp_dir, write_test_skill, FakeInstallResult,
    TuiMcpFakeFacade,
};
use futures::StreamExt;
use std::sync::Arc;

#[test]
fn overlay_shortcut_smoke_matrix_emits_open_command_and_dismisses_effect() {
    use agent_core::facade::{
        AgentSettingsScope, HooksSettingsView, InstructionsView, PluginInstallTarget,
    };
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::{
        AgentOverlaySnapshot, Command, CrossPanelEffect, FocusTarget, McpOverlaySnapshot,
        ModelOverlaySnapshot, ModelProfileEntry, PluginOverlaySnapshot, SkillEntry,
        SkillOverlaySnapshot,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn escape_key() -> Event {
        key(KeyCode::Esc, KeyModifiers::NONE)
    }

    fn is_mcp_open(command: &Command) -> bool {
        matches!(command, Command::OpenMcpOverlay)
    }

    fn is_skills_open(command: &Command) -> bool {
        matches!(command, Command::OpenSkillsOverlay)
    }

    fn is_plugins_open(command: &Command) -> bool {
        matches!(command, Command::OpenPluginsOverlay)
    }

    fn is_model_open(command: &Command) -> bool {
        matches!(command, Command::OpenModelOverlay)
    }

    fn is_hooks_open(command: &Command) -> bool {
        matches!(command, Command::OpenHooksOverlay)
    }

    fn is_instructions_open(command: &Command) -> bool {
        matches!(command, Command::OpenInstructionsOverlay)
    }

    fn mcp_visible(app: &App) -> bool {
        app.mcp_overlay.is_visible()
    }

    fn skills_visible(app: &App) -> bool {
        app.skills_overlay.is_visible()
    }

    fn plugins_visible(app: &App) -> bool {
        app.plugin_overlay.is_visible()
    }

    fn model_visible(app: &App) -> bool {
        app.model_overlay.is_visible()
    }

    fn hooks_visible(app: &App) -> bool {
        app.hooks_overlay.is_visible()
    }

    fn instructions_visible(app: &App) -> bool {
        app.instructions_overlay.is_visible()
    }

    fn show_mcp(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowMcpOverlay(McpOverlaySnapshot {
            runtime_servers: Vec::new(),
            settings: Vec::new(),
            installed: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
        })]);
    }

    fn show_skills(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
            SkillOverlaySnapshot::from(vec![SkillEntry {
                id: "smoke-skill".into(),
                name: "Smoke Skill".into(),
                description: "Smoke test skill".into(),
                source: "test".into(),
                activation_mode: "manual".into(),
                active: false,
            }]),
        )]);
    }

    fn show_plugins(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowPluginsOverlay(
            PluginOverlaySnapshot {
                plugins: Vec::new(),
                catalog: Vec::new(),
                sources: Vec::new(),
                install_target: PluginInstallTarget::User,
            },
        )]);
    }

    fn show_model(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowModelOverlay(
            ModelOverlaySnapshot {
                profiles: vec![ModelProfileEntry {
                    alias: "fast".into(),
                    provider_display: "fake".into(),
                    model_display: "fake-model".into(),
                    context_window: Some(128_000),
                    output_limit: Some(4096),
                    temperature: None,
                    top_p: None,
                    top_k: None,
                    max_tokens: None,
                    base_url: None,
                    api_key_env: None,
                    supports_reasoning: false,
                    enabled: true,
                    writable: true,
                    source: "test".into(),
                    has_api_key: true,
                }],
                current_alias: Some("fast".into()),
                current_effort: None,
            },
        )]);
    }

    fn show_hooks(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowHooksOverlay(
            HooksSettingsView {
                user: Vec::new(),
                project: Vec::new(),
                templates: Vec::new(),
                user_config_path: "/tmp/kairox-user.toml".into(),
                project_config_path: Some("/tmp/kairox-project.toml".into()),
            },
        )]);
    }

    fn show_instructions(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowInstructionsOverlay(
            InstructionsView {
                system: "system prompt".into(),
                user: Some("user instructions".into()),
                project: Some("project instructions".into()),
            },
        )]);
    }

    struct OverlaySmokeCase {
        name: &'static str,
        open_key: Event,
        expected_command: fn(&Command) -> bool,
        show: fn(&mut App),
        is_visible: fn(&App) -> bool,
        focus: FocusTarget,
    }

    let cases = [
        OverlaySmokeCase {
            name: "mcp",
            open_key: key(KeyCode::Char('m'), KeyModifiers::CONTROL),
            expected_command: is_mcp_open,
            show: show_mcp,
            is_visible: mcp_visible,
            focus: FocusTarget::McpOverlay,
        },
        OverlaySmokeCase {
            name: "skills",
            open_key: key(KeyCode::Char('s'), KeyModifiers::CONTROL),
            expected_command: is_skills_open,
            show: show_skills,
            is_visible: skills_visible,
            focus: FocusTarget::SkillsOverlay,
        },
        OverlaySmokeCase {
            name: "plugins",
            open_key: key(KeyCode::Char('g'), KeyModifiers::CONTROL),
            expected_command: is_plugins_open,
            show: show_plugins,
            is_visible: plugins_visible,
            focus: FocusTarget::PluginOverlay,
        },
        OverlaySmokeCase {
            name: "model",
            open_key: key(KeyCode::Char('l'), KeyModifiers::CONTROL),
            expected_command: is_model_open,
            show: show_model,
            is_visible: model_visible,
            focus: FocusTarget::ModelOverlay,
        },
        OverlaySmokeCase {
            name: "hooks",
            open_key: key(KeyCode::Char('h'), KeyModifiers::ALT),
            expected_command: is_hooks_open,
            show: show_hooks,
            is_visible: hooks_visible,
            focus: FocusTarget::HooksOverlay,
        },
        OverlaySmokeCase {
            name: "instructions",
            open_key: key(KeyCode::Char('i'), KeyModifiers::ALT),
            expected_command: is_instructions_open,
            show: show_instructions,
            is_visible: instructions_visible,
            focus: FocusTarget::InstructionsOverlay,
        },
    ];

    for case in cases {
        let mut app = App::new("fake", WorkspaceId::new());

        let commands = app.handle_crossterm_event(&case.open_key);
        assert!(
            commands.iter().any(case.expected_command),
            "expected {} open command, got {commands:?}",
            case.name
        );

        (case.show)(&mut app);
        assert!((case.is_visible)(&app), "{} overlay should open", case.name);
        assert_eq!(
            app.state.focus_manager.current(),
            case.focus,
            "{} overlay should take focus",
            case.name
        );

        let commands = app.handle_crossterm_event(&escape_key());
        assert!(
            commands.is_empty(),
            "expected Esc to dismiss {} without commands, got {commands:?}",
            case.name
        );
        assert!(
            !(case.is_visible)(&app),
            "{} overlay should close on Esc",
            case.name
        );
        assert_eq!(
            app.state.focus_manager.current(),
            FocusTarget::Chat,
            "{} overlay should restore chat focus",
            case.name
        );
    }

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowAgentSettingsOverlay(
        AgentOverlaySnapshot {
            agents: vec![agent_settings_view("worker", AgentSettingsScope::Builtin)],
        },
    )]);
    assert!(app.agent_overlay.is_visible());
    assert_eq!(app.state.focus_manager.current(), FocusTarget::AgentOverlay);
    let commands = app.handle_crossterm_event(&escape_key());
    assert!(
        commands.is_empty(),
        "expected Esc to dismiss agents without commands, got {commands:?}"
    );
    assert!(!app.agent_overlay.is_visible());
    assert_eq!(app.state.focus_manager.current(), FocusTarget::Chat);
}

#[test]
fn archive_overlay_smoke_opens_from_sessions_focus_and_restores_selected_session() {
    use agent_core::{ProjectSessionVisibility, SessionId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::{Command, FocusTarget, SessionInfo, SessionState};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    let archived_id = SessionId::from_string("ses_archived".into());
    let app_session = SessionInfo {
        id: archived_id.clone(),
        title: "archived".into(),
        model_profile: "fake".into(),
        state: SessionState::Idle,
        pinned: false,
        archived: true,
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: Some(ProjectSessionVisibility::Archived),
    };
    let mut app = App::new("fake", WorkspaceId::new());
    app.state.sessions = vec![app_session];
    app.state.focus_manager.set(FocusTarget::Sessions);
    app.sync_component_focus();

    let commands = app.handle_crossterm_event(&key(KeyCode::Char('a')));
    assert!(
        commands.is_empty(),
        "expected archive shortcut to open overlay without runtime commands, got {commands:?}"
    );
    assert!(app.sessions.archive_manager_open);

    let commands = app.handle_crossterm_event(&key(KeyCode::Enter));
    assert_eq!(
        commands,
        vec![Command::RestoreSession {
            session_id: archived_id,
        }]
    );
    assert!(!app.sessions.archive_manager_open);
}

#[test]
fn destructive_tui_actions_require_second_keypress_before_command() {
    use agent_core::facade::{
        HookSettingsView, HooksSettingsView, InstalledEntry, PluginComponentInventoryView,
        PluginInstallTarget, PluginSettingsView, SkillInstallSource, SkillInstallTarget,
        SkillSettingsScope, SkillSettingsView,
    };
    use agent_core::{ConfigScope, ProjectId, ProjectSessionVisibility, SessionId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::{
        Command, CrossPanelEffect, FocusTarget, McpOverlaySnapshot, ModelOverlaySnapshot,
        ModelProfileEntry, PluginOverlaySnapshot, ProjectInfo, SessionInfo, SessionState,
        SkillOverlaySnapshot,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn session(session_id: &str, archived: bool) -> SessionInfo {
        SessionInfo {
            id: SessionId::from_string(session_id.to_string()),
            title: session_id.to_string(),
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

    fn project(project_id: &str) -> ProjectInfo {
        ProjectInfo {
            id: ProjectId::from_string(project_id.to_string()),
            display_name: project_id.to_string(),
            root_path: format!("/tmp/{project_id}"),
            expanded: true,
            git_status: None,
            instruction_summary: None,
        }
    }

    fn installed_skill(skill_id: &str) -> SkillSettingsView {
        SkillSettingsView {
            settings_id: format!("user:{skill_id}"),
            id: skill_id.into(),
            name: skill_id.into(),
            description: format!("{skill_id} settings"),
            version: Some("1.0.0".into()),
            scope: SkillSettingsScope::User,
            path: format!("/tmp/{skill_id}/SKILL.md"),
            enabled: true,
            activation_mode: "manual".into(),
            tools: Vec::new(),
            can_request_tools: Vec::new(),
            permission_summary: "no tool permissions declared".into(),
            install_source: SkillInstallSource::Registry,
            update_state: agent_core::facade::SkillUpdateState::UpToDate,
            effective: true,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            editable: true,
            deletable: true,
        }
    }

    fn installed_plugin(settings_id: &str) -> PluginSettingsView {
        PluginSettingsView {
            settings_id: settings_id.into(),
            id: settings_id.replace(':', "-"),
            name: settings_id.into(),
            description: format!("{settings_id} plugin"),
            version: Some("1.2.3".into()),
            scope: ConfigScope::User,
            path: format!("/tmp/{settings_id}"),
            enabled: true,
            install_source: Some("local".into()),
            marketplace: Some("local-market".into()),
            effective: true,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            inventory: PluginComponentInventoryView {
                skill_count: 1,
                skill_names: vec!["review".into()],
                mcp_server_count: 0,
                app_count: 0,
                agent_count: 0,
                hook_count: 0,
            },
            manifest_kind: "kairox".into(),
        }
    }

    fn hook(id: &str) -> HookSettingsView {
        HookSettingsView {
            id: id.into(),
            event: "Stop".into(),
            matcher: Some("*".into()),
            command: "cargo test".into(),
            status_message: Some("Testing".into()),
            timeout_secs: Some(120),
            enabled: true,
            source: ConfigScope::User,
            config_path: Some(format!("/tmp/{id}.toml")),
        }
    }

    let mut app = App::new("fake", WorkspaceId::new());
    let archive_id = SessionId::from_string("ses_active_confirm".into());
    app.state.sessions = vec![SessionInfo {
        id: archive_id.clone(),
        ..session("ses_active_confirm", false)
    }];
    app.state.focus_manager.set(FocusTarget::Sessions);
    app.sync_component_focus();
    assert!(app.sessions.open_action_menu(&[], &app.state.sessions));
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('a')));
    assert!(
        commands.is_empty(),
        "first archive key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('a')));
    assert!(matches!(
        &commands[..],
        [Command::ArchiveSession { session_id }] if session_id == &archive_id
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    let archived_id = SessionId::from_string("ses_archived_confirm".into());
    app.state.sessions = vec![SessionInfo {
        id: archived_id.clone(),
        ..session("ses_archived_confirm", true)
    }];
    app.state.focus_manager.set(FocusTarget::Sessions);
    app.sync_component_focus();
    app.sessions.open_archive_manager(&app.state.sessions);
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('d')));
    assert!(
        commands.is_empty(),
        "first archive-manager delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('d')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteSession { session_id }] if session_id == &archived_id
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    let project_id = ProjectId::from_string("prj_confirm".into());
    app.state.projects = vec![ProjectInfo {
        id: project_id.clone(),
        ..project("prj_confirm")
    }];
    app.state.focus_manager.set(FocusTarget::Sessions);
    app.sync_component_focus();
    assert!(app
        .sessions
        .open_action_menu(&app.state.projects, &app.state.sessions));
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('d')));
    assert!(
        commands.is_empty(),
        "first project remove key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('d')));
    assert!(matches!(
        &commands[..],
        [Command::RemoveProject { project_id: id }] if id == &project_id
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowMcpOverlay(McpOverlaySnapshot {
        runtime_servers: Vec::new(),
        settings: Vec::new(),
        installed: vec![InstalledEntry {
            server_id: "alpha".into(),
            catalog_id: Some("filesystem".into()),
            source: Some("builtin".into()),
            display_name: "Alpha".into(),
            installed_at: "2026-05-21T00:00:00Z".into(),
            running: true,
        }],
        catalog: Vec::new(),
        sources: Vec::new(),
    })]);
    let _ = app.handle_crossterm_event(&key(KeyCode::Tab));
    let _ = app.handle_crossterm_event(&key(KeyCode::Tab));
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first MCP uninstall key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::UninstallMcpServer { server_id }] if server_id == "alpha"
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
        SkillOverlaySnapshot {
            discovered: Vec::new(),
            installed: vec![installed_skill("review")],
            catalog: Vec::new(),
            sources: Vec::new(),
            install_target: SkillInstallTarget::User,
        },
    )]);
    let _ = app.handle_crossterm_event(&key(KeyCode::Tab));
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first skill delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteSkillSettings { skill_id }] if skill_id == "review"
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowPluginsOverlay(
        PluginOverlaySnapshot {
            plugins: vec![installed_plugin("user:alpha")],
            catalog: Vec::new(),
            sources: Vec::new(),
            install_target: PluginInstallTarget::User,
        },
    )]);
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first plugin delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeletePluginSettings { settings_id }] if settings_id == "user:alpha"
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowModelOverlay(
        ModelOverlaySnapshot {
            profiles: vec![ModelProfileEntry {
                alias: "slow".into(),
                provider_display: "fake".into(),
                model_display: "slow-model".into(),
                context_window: Some(128_000),
                output_limit: Some(4096),
                temperature: None,
                top_p: None,
                top_k: None,
                max_tokens: None,
                base_url: None,
                api_key_env: None,
                supports_reasoning: false,
                enabled: true,
                writable: true,
                source: "test".into(),
                has_api_key: true,
            }],
            current_alias: Some("fast".into()),
            current_effort: None,
        },
    )]);
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first model profile delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteProfileSettings { alias }] if alias == "slow"
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowHooksOverlay(
        HooksSettingsView {
            user: vec![hook("user-verify")],
            project: Vec::new(),
            templates: Vec::new(),
            user_config_path: "/tmp/kairox-user.toml".into(),
            project_config_path: Some("/tmp/kairox-project.toml".into()),
        },
    )]);
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first hook delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteHookSettings { scope, event, id }]
            if *scope == ConfigScope::User && event == "Stop" && id == "user-verify"
    ));
}
