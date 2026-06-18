//! TUI parity smoke harness.
//!
//! These tests keep the parity entry points grouped under a stable
//! `parity_smoke` filter so feature work can quickly check that the main
//! TUI surfaces remain reachable.

use std::collections::BTreeMap;

use agent_core::facade::{
    CatalogSourceView, InstallRequest, ServerEntry, SkillCatalogEntry, SkillFieldMappingView,
    SkillInstallTarget, SkillSourceView,
};
use agent_core::facade::{TaskGraphSnapshot, TaskSnapshot};
use agent_core::projection::SessionProjection;
use agent_core::{AgentRole, SessionId, TaskId, TaskState, WorkspaceId};
use agent_tui::app::App;
use agent_tui::components::command_palette::CommandPalette;
use agent_tui::components::mcp_overlay::McpOverlay;
use agent_tui::components::model_overlay::ModelOverlay;
use agent_tui::components::skills_overlay::SkillsOverlay;
use agent_tui::components::trace::{MemoryRow, MemoryScopeFilter, RightPanelTab};
use agent_tui::components::{
    Command, CommandPaletteSnapshot, Component, CrossPanelEffect, EventContext, FocusTarget,
    McpOverlaySnapshot, ModelOverlaySnapshot, ModelProfileEntry, QueueAction, SkillEntry,
    SkillOverlaySnapshot,
};
use agent_tui::keybindings::KeyAction;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

mod support;

use support::render::render_component as render_component_to_string;

fn test_ctx(focus: FocusTarget, current_session_id: Option<SessionId>) -> EventContext<'static> {
    let projection: &'static SessionProjection = Box::leak(Box::new(SessionProjection::default()));
    let sessions: &'static Vec<agent_tui::components::SessionInfo> = Box::leak(Box::default());
    let workspace_id: &'static WorkspaceId = Box::leak(Box::new(WorkspaceId::from_string(
        "wrk_parity_smoke".into(),
    )));
    let current_session_id: &'static Option<SessionId> = Box::leak(Box::new(current_session_id));

    EventContext {
        focus,
        current_session: projection,
        projects: &[],
        sessions,
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id,
        current_session_id,
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_text(component: &mut impl Component, ctx: &EventContext<'_>, text: &str) {
    for ch in text.chars() {
        let (effects, commands) = component.handle_event(ctx, &key(KeyCode::Char(ch)));
        assert!(
            effects.is_empty() && commands.is_empty(),
            "typing {ch:?} should not emit effects or commands; got {effects:?} {commands:?}"
        );
    }
}

fn render_component(component: &impl Component) -> String {
    render_component_to_string(component, 140, 36)
}

fn activate_palette_entry(
    expected_id: &str,
    filter: &str,
) -> (Vec<CrossPanelEffect>, Vec<Command>) {
    let ctx = test_ctx(
        FocusTarget::CommandPalette,
        Some(SessionId::from_string("ses_palette".into())),
    );
    let mut palette = CommandPalette::new();
    palette.handle_effect(&CrossPanelEffect::ShowCommandPalette);
    assert!(palette.is_visible());

    type_text(&mut palette, &ctx, filter);
    let visible_ids = palette
        .visible_entries()
        .into_iter()
        .map(|entry| entry.id.into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        visible_ids,
        vec![expected_id],
        "palette filter {filter:?} should isolate one entry"
    );

    let (effects, commands) = palette.handle_event(&ctx, &key(KeyCode::Enter));
    assert!(
        effects.contains(&CrossPanelEffect::DismissCommandPalette),
        "activating {expected_id} should dismiss palette; got {effects:?}"
    );
    assert!(
        !palette.is_visible(),
        "activating {expected_id} should hide palette"
    );
    (effects, commands)
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
        tags: vec!["smoke".into()],
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

fn model_profile(alias: &str, supports_reasoning: bool) -> ModelProfileEntry {
    ModelProfileEntry {
        alias: alias.into(),
        provider_display: "fake".into(),
        model_display: alias.into(),
        context_window: Some(128_000),
        output_limit: Some(4096),
        temperature: None,
        top_p: None,
        top_k: None,
        max_tokens: None,
        base_url: None,
        api_key_env: None,
        client_identity: None,
        supports_reasoning,
        supports_reasoning_override: Some(supports_reasoning),
        enabled: true,
        writable: true,
        source: "test".into(),
        has_api_key: true,
    }
}

fn skill_entry(id: &str, active: bool) -> SkillEntry {
    SkillEntry {
        id: id.into(),
        name: format!("{id} skill"),
        description: format!("{id} description"),
        source: "test".into(),
        activation_mode: "manual".into(),
        active,
    }
}

fn task_snapshot(
    id: TaskId,
    title: &str,
    role: AgentRole,
    state: TaskState,
    retry_count: usize,
    max_retries: usize,
) -> TaskSnapshot {
    TaskSnapshot {
        id,
        title: title.into(),
        role,
        state,
        dependencies: Vec::new(),
        error: None,
        retry_count,
        max_retries,
        assigned_agent_id: None,
        failure_reason: None,
    }
}

#[test]
fn parity_smoke_command_palette_opens_overlay_entry_points_and_queue_actions() {
    struct Case {
        expected_id: &'static str,
        filter: &'static str,
        assert_command: fn(&[Command]),
    }

    let cases = [
        Case {
            expected_id: "mcp-manager",
            filter: "mcp-manager",
            assert_command: |commands| assert!(matches!(commands, [Command::OpenMcpOverlay])),
        },
        Case {
            expected_id: "skills-manager",
            filter: "skills-manager",
            assert_command: |commands| assert!(matches!(commands, [Command::OpenSkillsOverlay])),
        },
        Case {
            expected_id: "model-selector",
            filter: "model-selector",
            assert_command: |commands| assert!(matches!(commands, [Command::OpenModelOverlay])),
        },
        Case {
            expected_id: "config-dir",
            filter: "config-dir",
            assert_command: |commands| assert!(matches!(commands, [Command::OpenConfigDir])),
        },
        Case {
            expected_id: "plugins",
            filter: "plugins",
            assert_command: |commands| assert!(matches!(commands, [Command::OpenPluginsOverlay])),
        },
        Case {
            expected_id: "hooks",
            filter: "hooks",
            assert_command: |commands| assert!(matches!(commands, [Command::OpenHooksOverlay])),
        },
        Case {
            expected_id: "instructions",
            filter: ":instructions",
            assert_command: |commands| {
                assert!(matches!(commands, [Command::OpenInstructionsOverlay]))
            },
        },
        Case {
            expected_id: "system-prompt",
            filter: "system-prompt",
            assert_command: |commands| {
                assert!(matches!(commands, [Command::OpenSystemPromptOverlay]))
            },
        },
        Case {
            expected_id: "agents",
            filter: "planner worker reviewer",
            assert_command: |commands| {
                assert!(matches!(commands, [Command::OpenAgentSettingsOverlay]))
            },
        },
        Case {
            expected_id: "skills-dir",
            filter: "skills-dir",
            assert_command: |commands| assert!(matches!(commands, [Command::OpenSkillsDir])),
        },
        Case {
            expected_id: "queue-send-now",
            filter: "queue-send-now",
            assert_command: |commands| {
                assert!(matches!(
                    commands,
                    [Command::ApplyQueueAction(QueueAction::SendSelectedNow)]
                ))
            },
        },
    ];

    for case in cases {
        let (_effects, commands) = activate_palette_entry(case.expected_id, case.filter);
        (case.assert_command)(&commands);
    }
}

#[test]
fn parity_smoke_command_palette_runs_clear_and_dynamic_entries() {
    let ctx = test_ctx(
        FocusTarget::CommandPalette,
        Some(SessionId::from_string("ses_palette_dynamic".into())),
    );
    let mut palette = CommandPalette::new();
    palette.handle_effect(&CrossPanelEffect::UpdateCommandPalette(
        CommandPaletteSnapshot {
            model_profiles: vec![model_profile("fast", false)],
            skills: vec![skill_entry("review", true)],
        },
    ));
    palette.handle_effect(&CrossPanelEffect::ShowCommandPalette);

    type_text(&mut palette, &ctx, "clear");
    assert_eq!(
        palette
            .visible_entries()
            .into_iter()
            .map(|entry| entry.id.into_owned())
            .collect::<Vec<_>>(),
        vec!["clear".to_string()]
    );
    let (_effects, commands) = palette.handle_event(&ctx, &key(KeyCode::Enter));
    assert!(matches!(&commands[..], [Command::ClearSessionProjection]));

    palette.handle_effect(&CrossPanelEffect::ShowCommandPalette);
    type_text(&mut palette, &ctx, "fast");
    assert_eq!(
        palette
            .visible_entries()
            .into_iter()
            .map(|entry| entry.id.into_owned())
            .collect::<Vec<_>>(),
        vec!["model-profile-fast".to_string()]
    );
    let (_effects, commands) = palette.handle_event(&ctx, &key(KeyCode::Enter));
    assert!(matches!(
        &commands[..],
        [Command::SwitchModel {
            alias,
            reasoning_effort: None,
            ..
        }] if alias == "fast"
    ));

    palette.handle_effect(&CrossPanelEffect::ShowCommandPalette);
    type_text(&mut palette, &ctx, "skill-review");
    assert_eq!(
        palette
            .visible_entries()
            .into_iter()
            .map(|entry| entry.id.into_owned())
            .collect::<Vec<_>>(),
        vec!["skill-review".to_string()]
    );
    let (_effects, commands) = palette.handle_event(&ctx, &key(KeyCode::Enter));
    assert!(matches!(
        &commands[..],
        [Command::ActivateSkill { skill_id, .. }] if skill_id == "review"
    ));
}

#[test]
fn parity_smoke_mcp_catalog_install_config_collects_required_values() {
    let ctx = test_ctx(FocusTarget::McpOverlay, None);
    let mut entry = mcp_catalog_entry("github", "registry");
    entry.install_spec_json =
        r#"{"transport":"sse","url":"https://mcp.example.test/sse","headers":{"Authorization":"Bearer ${Authorization}"}}"#
            .into();
    entry.default_env_json = r#"[
        {"key":"Authorization","label":"Authorization","description":"Bearer token","required":true,"secret":true,"default":null},
        {"key":"GITHUB_ORG","label":"GitHub org","description":"Organization","required":true,"secret":false,"default":null}
    ]"#
    .into();

    let mut overlay = McpOverlay::new();
    overlay.show(McpOverlaySnapshot {
        runtime_servers: Vec::new(),
        settings: Vec::new(),
        installed: Vec::new(),
        catalog: vec![entry],
        sources: vec![mcp_source("registry", true)],
    });

    for _ in 0..3 {
        let (effects, commands) = overlay.handle_event(&ctx, &key(KeyCode::Tab));
        assert!(effects.is_empty() && commands.is_empty());
    }

    let (effects, commands) = overlay.handle_event(&ctx, &key(KeyCode::Char('i')));
    assert!(effects.is_empty());
    assert!(
        commands.is_empty(),
        "required config should open editor before install command; got {commands:?}"
    );
    let rendered = render_component(&overlay);
    assert!(rendered.contains("Install configuration"), "{rendered}");
    assert!(rendered.contains("Authorization"), "{rendered}");
    assert!(rendered.contains("GITHUB_ORG"), "{rendered}");

    type_text(&mut overlay, &ctx, "Bearer smoke-token");
    let _ = overlay.handle_event(&ctx, &key(KeyCode::Tab));
    type_text(&mut overlay, &ctx, "kairox-dev");
    let (_effects, commands) = overlay.handle_event(&ctx, &key(KeyCode::Enter));

    let expected_overrides = BTreeMap::from([
        (
            "Authorization".to_string(),
            "Bearer smoke-token".to_string(),
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
fn parity_smoke_skills_catalog_detail_installs_selected_target() {
    let ctx = test_ctx(FocusTarget::SkillsOverlay, None);
    let mut overlay = SkillsOverlay::new();
    overlay.show(SkillOverlaySnapshot {
        discovered: Vec::new(),
        installed: Vec::new(),
        catalog: vec![skill_catalog_entry("review")],
        sources: vec![skill_source("skillhub", true)],
        install_target: SkillInstallTarget::User,
    });

    for _ in 0..2 {
        let (effects, commands) = overlay.handle_event(&ctx, &key(KeyCode::Tab));
        assert!(effects.is_empty() && commands.is_empty());
    }

    let (effects, commands) = overlay.handle_event(&ctx, &key(KeyCode::Enter));
    assert!(effects.is_empty() && commands.is_empty());
    let rendered = render_component(&overlay);
    assert!(rendered.contains("Source: https://skillhub.example.test/review"));
    assert!(rendered.contains("Package: review"));
    assert!(rendered.contains("Download: https://skillhub.example.test/review.zip"));
    assert!(rendered.contains("Target: user"));

    let (effects, commands) = overlay.handle_event(&ctx, &key(KeyCode::Char('t')));
    assert!(effects.is_empty() && commands.is_empty());
    let rendered = render_component(&overlay);
    assert!(rendered.contains("Target: project"));

    let (_effects, commands) = overlay.handle_event(&ctx, &key(KeyCode::Char('i')));
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

#[test]
fn parity_smoke_model_overlay_switches_reasoning_profile_with_effort() {
    let session_id = SessionId::from_string("ses_model".into());
    let ctx = test_ctx(FocusTarget::ModelOverlay, Some(session_id.clone()));
    let mut overlay = ModelOverlay::new();
    overlay.show(ModelOverlaySnapshot {
        profiles: vec![
            model_profile("fast", false),
            model_profile("reasoning", true),
        ],
        current_alias: Some("reasoning".into()),
        current_effort: Some("low".into()),
    });

    assert!(overlay.is_visible());
    assert_eq!(
        overlay
            .selected_profile()
            .map(|profile| profile.alias.as_str()),
        Some("reasoning")
    );
    assert!(overlay.shows_effort_picker());

    let _ = overlay.handle_event(&ctx, &key(KeyCode::Tab));
    let _ = overlay.handle_event(&ctx, &key(KeyCode::Char('j')));
    let _ = overlay.handle_event(&ctx, &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_effort(), Some("high"));

    let (effects, commands) = overlay.handle_event(&ctx, &key(KeyCode::Enter));
    assert!(effects.contains(&CrossPanelEffect::DismissModelOverlay));
    assert!(matches!(
        &commands[..],
        [Command::SwitchModel {
            session_id: switched_session,
            alias,
            reasoning_effort,
            ..
        }] if switched_session == &session_id
            && alias == "reasoning"
            && reasoning_effort.as_deref() == Some("high")
    ));
}

#[test]
fn parity_smoke_queue_task_and_memory_panel_actions_remain_reachable() {
    let workspace_id = WorkspaceId::from_string("wrk_panels".into());
    let session_id = SessionId::from_string("ses_panels".into());
    let failed_task_id = TaskId::from_string("task_failed".into());
    let blocked_task_id = TaskId::from_string("task_blocked".into());
    let mut app = App::new("test", workspace_id.clone());
    app.current_session_id = Some(session_id.clone());

    app.chat
        .message_queue
        .push(agent_tui::components::QueuedMessage {
            content: "first queued".into(),
            attachments: Vec::new(),
        });
    app.chat
        .message_queue
        .push(agent_tui::components::QueuedMessage {
            content: "second queued".into(),
            attachments: Vec::new(),
        });
    app.chat.selected_queue_index = 1;
    let commands = app.apply_queue_action(QueueAction::SendSelectedNow);
    assert!(matches!(
        &commands[..],
        [Command::SendQueuedMessageNow {
            workspace_id: command_workspace,
            session_id: command_session,
            queue_index,
        }] if command_workspace == &workspace_id
            && command_session == &session_id
            && *queue_index == 1
    ));
    assert_eq!(app.chat.message_queue.len(), 2);

    app.trace.active_tab = RightPanelTab::Tasks;
    app.trace.selected_task_index = 0;
    app.state.current_session.task_graph = TaskGraphSnapshot {
        tasks: vec![
            task_snapshot(
                failed_task_id.clone(),
                "Retry failed task",
                AgentRole::Worker,
                TaskState::Failed,
                1,
                3,
            ),
            task_snapshot(
                blocked_task_id.clone(),
                "Cancel blocked task",
                AgentRole::Reviewer,
                TaskState::Blocked,
                0,
                3,
            ),
        ],
    };

    let commands = app.apply_action(KeyAction::RetrySelectedTask);
    assert_eq!(
        commands,
        vec![Command::RetryTask {
            workspace_id: workspace_id.clone(),
            session_id: session_id.clone(),
            task_id: failed_task_id,
        }]
    );

    app.trace.selected_task_index = 1;
    let commands = app.apply_action(KeyAction::CancelSelectedTask);
    assert_eq!(
        commands,
        vec![Command::CancelTask {
            workspace_id: workspace_id.clone(),
            session_id,
            task_id: blocked_task_id,
        }]
    );

    let commands = app.apply_action(KeyAction::CycleTraceTabNext);
    assert_eq!(app.trace.active_tab, RightPanelTab::Memory);
    assert_eq!(
        commands,
        vec![Command::LoadMemories {
            scope: None,
            keywords: Vec::new(),
            limit: 100,
        }]
    );

    let commands = app.apply_action(KeyAction::CycleMemoryScope);
    assert_eq!(app.trace.memory_scope_filter, MemoryScopeFilter::Session);
    assert_eq!(
        commands,
        vec![Command::LoadMemories {
            scope: Some(agent_memory::MemoryScope::Session),
            keywords: Vec::new(),
            limit: 100,
        }]
    );

    app.trace.set_memory_rows(vec![MemoryRow::new(
        "mem_user".into(),
        "user".into(),
        Some("preferred-command".into()),
        "Use cargo test -p agent-tui parity_smoke".into(),
    )]);
    let commands = app.apply_action(KeyAction::DeleteSelectedMemory);
    assert!(commands.is_empty());
    assert_eq!(
        app.trace.pending_delete_memory_id(),
        Some("mem_user".into())
    );

    let commands = app.apply_action(KeyAction::ConfirmMemoryDelete);
    assert_eq!(
        commands,
        vec![Command::DeleteMemory {
            memory_id: "mem_user".into(),
        }]
    );
}
