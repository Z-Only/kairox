use crossterm::event::{Event, KeyCode};

use super::*;
use crate::components::{
    Command, CommandPaletteSnapshot, Component, CrossPanelEffect, EventContext, FocusTarget,
    ModelProfileEntry, SkillEntry,
};

fn model_profile(alias: &str) -> ModelProfileEntry {
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
        supports_reasoning: false,
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

fn test_ctx() -> EventContext<'static> {
    use agent_core::projection::SessionProjection;
    static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
    let projection = PROJECTION.get_or_init(SessionProjection::default);
    static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
        std::sync::OnceLock::new();
    let sessions = SESSIONS.get_or_init(Vec::new);
    static WORKSPACE: std::sync::OnceLock<agent_core::WorkspaceId> = std::sync::OnceLock::new();
    let workspace = WORKSPACE.get_or_init(agent_core::WorkspaceId::new);
    static SESSION: std::sync::OnceLock<Option<agent_core::SessionId>> = std::sync::OnceLock::new();
    let session = SESSION.get_or_init(|| Some(agent_core::SessionId::new()));
    EventContext {
        focus: FocusTarget::CommandPalette,
        current_session: projection,
        projects: &[],
        sessions,
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: workspace,
        current_session_id: session,
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(crossterm::event::KeyEvent::new(
        code,
        crossterm::event::KeyModifiers::NONE,
    ))
}

#[test]
fn filters_commands_by_prefix() {
    let entries = builtin_entries();
    let filtered = filter_entries("skill", entries);
    assert!(!filtered.is_empty());
    assert!(filtered.iter().all(|e| e.label.contains("skill")
        || e.description.to_lowercase().contains("skill")
        || e.id.contains("skill")));
    // ":compact" should NOT be in skill results.
    assert!(!filtered.iter().any(|e| e.id == "compact"));
}

#[test]
fn empty_filter_returns_all_entries() {
    let entries = builtin_entries();
    let filtered = filter_entries("", entries);
    assert_eq!(filtered.len(), entries.len());
}

#[test]
fn case_insensitive_match() {
    let entries = builtin_entries();
    let filtered = filter_entries("MODEL", entries);
    assert!(filtered.iter().any(|e| e.id == "model"));
}

#[test]
fn invisible_by_default() {
    let p = CommandPalette::new();
    assert!(!p.is_visible());
}

#[test]
fn show_makes_visible_and_resets_state() {
    let mut p = CommandPalette::new();
    p.filter.push('x');
    p.selected = 5;
    p.show();
    assert!(p.is_visible());
    assert_eq!(p.filter(), "");
    assert_eq!(p.selected_index(), 0);
}

#[test]
fn typing_filters_and_navigation_clamps() {
    let mut p = CommandPalette::new();
    p.show();
    let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char('s')));
    let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
    let visible: Vec<_> = p
        .visible_entries()
        .iter()
        .map(|e| e.id.as_ref().to_string())
        .collect();
    assert!(visible
        .iter()
        .all(|id| id.contains("skill") || id == "skills"));
    // Navigate past end and back.
    for _ in 0..10 {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Down));
    }
    assert!(p.selected_index() < p.visible_entries().len());
    let _ = p.handle_event(&test_ctx(), &key(KeyCode::Up));
    assert!(p.selected_index() < p.visible_entries().len());
}

#[test]
fn enter_dispatches_compact_command() {
    let mut p = CommandPalette::new();
    p.show();
    for c in "compact".chars() {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
    }
    let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(matches!(&commands[..], [Command::CompactSession { .. }]));
    assert!(effects
        .iter()
        .any(|e| matches!(e, CrossPanelEffect::DismissCommandPalette)));
    assert!(!p.is_visible());
}

#[test]
fn enter_dispatches_clear_projection_command() {
    let mut p = CommandPalette::new();
    p.show();
    for c in "clear".chars() {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
    }

    let visible_ids: Vec<_> = p
        .visible_entries()
        .into_iter()
        .map(|e| e.id.into_owned())
        .collect();
    assert_eq!(visible_ids, vec!["clear".to_string()]);
    let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));

    assert!(matches!(&commands[..], [Command::ClearSessionProjection]));
    assert!(effects
        .iter()
        .any(|e| matches!(e, CrossPanelEffect::DismissCommandPalette)));
}

#[test]
fn enter_dispatches_list_skills() {
    let mut p = CommandPalette::new();
    p.show();
    // Filter to :skills exactly (id "skills"). Type "skills".
    for c in "skills".chars() {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
    }
    // The first matching entry should be ":skills" itself.
    let first = p.visible_entries()[0].id.clone();
    assert_eq!(first, "skills");
    let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(matches!(&commands[..], [Command::ListSkills]));
}

#[test]
fn enter_dispatches_open_plugins_overlay() {
    let mut p = CommandPalette::new();
    p.show();
    for c in "plugins".chars() {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
    }
    let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(matches!(&commands[..], [Command::OpenPluginsOverlay]));
}

#[test]
fn enter_dispatches_open_agent_settings_overlay() {
    let mut p = CommandPalette::new();
    p.show();
    for c in "agents".chars() {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
    }
    let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(matches!(&commands[..], [Command::OpenAgentSettingsOverlay]));
}

#[test]
fn enter_dispatches_overlay_and_session_actions() {
    let expected = [
        ("mcp", "mcp-manager"),
        ("skills manager", "skills-manager"),
        ("hooks", "hooks"),
        ("model selector", "model-selector"),
        ("new session", "session-new"),
        ("cancel session", "session-cancel"),
    ];

    for (filter, expected_id) in expected {
        let mut p = CommandPalette::new();
        p.show();
        for c in filter.chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }
        assert_eq!(p.visible_entries()[0].id.as_ref(), expected_id);
        let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        match expected_id {
            "mcp-manager" => assert!(matches!(&commands[..], [Command::OpenMcpOverlay])),
            "skills-manager" => {
                assert!(matches!(&commands[..], [Command::OpenSkillsOverlay]))
            }
            "hooks" => assert!(matches!(&commands[..], [Command::OpenHooksOverlay])),
            "model-selector" => assert!(matches!(&commands[..], [Command::OpenModelOverlay])),
            "session-new" => assert!(matches!(&commands[..], [Command::StartSession { .. }])),
            "session-cancel" => {
                assert!(matches!(&commands[..], [Command::CancelSession { .. }]))
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn palette_exposes_project_create_and_import_prefills() {
    let entries = builtin_entries();

    let create = entries
        .iter()
        .find(|entry| entry.id == "project-create")
        .expect("project create entry should exist");
    assert_eq!(
        prefill_text(&create.action),
        Some(":project create "),
        "project create should prefill a slash command"
    );

    let import = entries
        .iter()
        .find(|entry| entry.id == "project-import")
        .expect("project import entry should exist");
    assert_eq!(
        prefill_text(&import.action),
        Some(":project import "),
        "project import should prefill a slash command"
    );
}

#[test]
fn palette_exposes_attachment_prefills() {
    let entries = builtin_entries();

    let attach = entries
        .iter()
        .find(|entry| entry.id == "attach")
        .expect("attach entry should exist");
    assert_eq!(prefill_text(&attach.action), Some(":attach "));

    let detach_all = entries
        .iter()
        .find(|entry| entry.id == "detach-all")
        .expect("detach all entry should exist");
    assert_eq!(prefill_text(&detach_all.action), Some(":detach"));

    let detach = entries
        .iter()
        .find(|entry| entry.id == "detach")
        .expect("detach entry should exist");
    assert_eq!(prefill_text(&detach.action), Some(":detach "));
}

#[test]
fn enter_dispatches_queue_actions() {
    let expected = [
        (
            "queue send",
            "queue-send-now",
            crate::components::QueueAction::SendSelectedNow,
        ),
        (
            "queue edit",
            "queue-edit",
            crate::components::QueueAction::RestoreSelectedForEdit,
        ),
        (
            "queue delete",
            "queue-delete",
            crate::components::QueueAction::DeleteSelected,
        ),
        (
            "queue up",
            "queue-move-up",
            crate::components::QueueAction::MoveSelectedUp,
        ),
        (
            "queue down",
            "queue-move-down",
            crate::components::QueueAction::MoveSelectedDown,
        ),
    ];

    for (filter, expected_id, expected_action) in expected {
        let mut p = CommandPalette::new();
        p.show();
        for c in filter.chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }
        assert_eq!(p.visible_entries()[0].id.as_ref(), expected_id);
        let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(
            commands.as_slice(),
            [Command::ApplyQueueAction(action)] if action == &expected_action
        ));
    }
}

#[test]
fn enter_dispatches_overlay_utility_actions() {
    let expected = [
        ("config dir", "config-dir"),
        ("mcp config", "mcp-config"),
        ("profiles config", "profiles-config"),
        ("agents dir", "agents-dir"),
        ("skills dir", "skills-dir"),
        ("system prompt", "system-prompt"),
        ("refresh catalog", "skill-catalog-refresh"),
    ];

    for (filter, expected_id) in expected {
        let mut p = CommandPalette::new();
        p.show();
        for c in filter.chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }
        assert_eq!(p.visible_entries()[0].id.as_ref(), expected_id);
        let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        match expected_id {
            "config-dir" => assert!(matches!(&commands[..], [Command::OpenConfigDir])),
            "mcp-config" => assert!(matches!(&commands[..], [Command::OpenMcpConfig])),
            "profiles-config" => {
                assert!(matches!(&commands[..], [Command::OpenProfilesConfig]))
            }
            "agents-dir" => assert!(matches!(&commands[..], [Command::OpenAgentsDir])),
            "skills-dir" => assert!(matches!(&commands[..], [Command::OpenSkillsDir])),
            "system-prompt" => {
                assert!(matches!(&commands[..], [Command::OpenSystemPromptOverlay]))
            }
            "skill-catalog-refresh" => {
                assert!(matches!(
                    &commands[..],
                    [Command::RefreshSkillCatalog {
                        keyword: None,
                        sources: None
                    }]
                ))
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn dynamic_model_profile_entries_switch_model_directly() {
    let mut p = CommandPalette::new();
    p.handle_effect(&CrossPanelEffect::UpdateCommandPalette(
        CommandPaletteSnapshot {
            model_profiles: vec![model_profile("fast")],
            skills: Vec::new(),
        },
    ));
    p.show();
    for c in "fast".chars() {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
    }

    let visible_ids: Vec<_> = p
        .visible_entries()
        .into_iter()
        .map(|e| e.id.into_owned())
        .collect();
    assert_eq!(visible_ids, vec!["model-profile-fast".to_string()]);
    let (_effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));

    assert!(matches!(
        &commands[..],
        [Command::SwitchModel {
            alias,
            reasoning_effort: None,
            ..
        }] if alias == "fast"
    ));
}

#[test]
fn dynamic_skill_entries_activate_discovered_skill() {
    let mut p = CommandPalette::new();
    p.handle_effect(&CrossPanelEffect::UpdateCommandPalette(
        CommandPaletteSnapshot {
            model_profiles: Vec::new(),
            skills: vec![skill_entry("review", true)],
        },
    ));
    p.show();
    for c in "skill-review".chars() {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
    }

    let visible_ids: Vec<_> = p
        .visible_entries()
        .into_iter()
        .map(|e| e.id.into_owned())
        .collect();
    assert_eq!(visible_ids, vec!["skill-review".to_string()]);
    let (_effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));

    assert!(matches!(
        &commands[..],
        [Command::ActivateSkill { skill_id, .. }] if skill_id == "review"
    ));
}

#[test]
fn enter_emits_prefill_for_model() {
    let mut p = CommandPalette::new();
    p.show();
    for c in "model".chars() {
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
    }
    let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
    assert!(commands.is_empty());
    assert!(effects.iter().any(|e| matches!(
        e,
        CrossPanelEffect::PrefillChatInput(text) if text == ":model "
    )));
    assert!(effects
        .iter()
        .any(|e| matches!(e, CrossPanelEffect::DismissCommandPalette)));
}

#[test]
fn esc_dismisses_palette() {
    let mut p = CommandPalette::new();
    p.show();
    let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Esc));
    assert!(commands.is_empty());
    assert!(matches!(
        effects.as_slice(),
        [CrossPanelEffect::DismissCommandPalette]
    ));
    assert!(!p.is_visible());
}

#[test]
fn backspace_removes_last_filter_char() {
    let mut p = CommandPalette::new();
    p.show();
    let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char('s')));
    let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
    assert_eq!(p.filter(), "sk");
    let _ = p.handle_event(&test_ctx(), &key(KeyCode::Backspace));
    assert_eq!(p.filter(), "s");
}

#[test]
fn renders_without_panic() {
    let mut p = CommandPalette::new();
    p.show();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| p.render(f.area(), f)).expect("render");
}
