use crossterm::event::{Event, KeyCode};
use ratatui::widgets::ListState;

use super::state::REASONING_EFFORTS;
use super::ModelOverlay;
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, ModelOverlaySnapshot,
    ModelProfileEntry, SessionInfo,
};

fn entry(alias: &str, supports_reasoning: bool) -> ModelProfileEntry {
    ModelProfileEntry {
        alias: alias.to_string(),
        provider_display: "provider".to_string(),
        model_display: format!("{alias}-model"),
        context_window: None,
        output_limit: None,
        temperature: None,
        top_p: None,
        top_k: None,
        max_tokens: None,
        base_url: None,
        api_key_env: None,
        supports_reasoning,
        enabled: true,
        writable: true,
        source: "profiles_toml".to_string(),
        has_api_key: true,
    }
}

fn disabled_entry(alias: &str) -> ModelProfileEntry {
    ModelProfileEntry {
        enabled: false,
        ..entry(alias, false)
    }
}

fn snapshot(
    profiles: Vec<ModelProfileEntry>,
    current_alias: Option<&str>,
    current_effort: Option<&str>,
) -> ModelOverlaySnapshot {
    ModelOverlaySnapshot {
        profiles,
        current_alias: current_alias.map(str::to_string),
        current_effort: current_effort.map(str::to_string),
    }
}

fn test_ctx_with_session(
    session_id: Option<agent_core::SessionId>,
) -> (
    agent_core::WorkspaceId,
    Option<agent_core::SessionId>,
    Vec<SessionInfo>,
    agent_core::projection::SessionProjection,
) {
    (
        agent_core::WorkspaceId::new(),
        session_id,
        Vec::new(),
        agent_core::projection::SessionProjection::default(),
    )
}

fn ctx<'a>(
    ws: &'a agent_core::WorkspaceId,
    sid: &'a Option<agent_core::SessionId>,
    sessions: &'a [SessionInfo],
    projection: &'a agent_core::projection::SessionProjection,
) -> EventContext<'a> {
    EventContext {
        focus: FocusTarget::ModelOverlay,
        current_session: projection,
        projects: &[],
        sessions,
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: ws,
        current_session_id: sid,
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(crossterm::event::KeyEvent::new(
        code,
        crossterm::event::KeyModifiers::NONE,
    ))
}

fn modified_key(code: KeyCode, modifiers: crossterm::event::KeyModifiers) -> Event {
    Event::Key(crossterm::event::KeyEvent::new(code, modifiers))
}

fn press(overlay: &mut ModelOverlay, code: KeyCode) -> Vec<Command> {
    let (ws, sid, sessions, proj) = test_ctx_with_session(None);
    let (_, commands) = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(code));
    commands
}

fn type_text(overlay: &mut ModelOverlay, value: &str) {
    for ch in value.chars() {
        let _ = press(overlay, KeyCode::Char(ch));
    }
}

#[test]
fn overlay_invisible_by_default() {
    let overlay = ModelOverlay::new();
    assert!(!overlay.is_visible());
    assert!(overlay.profiles().is_empty());
}

#[test]
fn shows_reasoning_effort_for_reasoning_models() {
    // TDD start: when a reasoning-capable profile is highlighted, the
    // overlay surfaces the effort picker pre-selecting the current
    // effort. Mirrors the GUI's `ChatModelSelector` reasoning panel.
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(
        vec![
            entry("fast", false),
            entry("opus-reasoning", true),
            entry("local", false),
        ],
        Some("opus-reasoning"),
        Some("high"),
    ));

    assert!(overlay.is_visible());
    assert_eq!(overlay.selected_index(), Some(1));
    assert!(
        overlay.shows_effort_picker(),
        "reasoning-capable selection must expose effort picker"
    );
    assert_eq!(overlay.selected_effort(), Some("high"));
    assert_eq!(overlay.effort_options(), REASONING_EFFORTS);
}

#[test]
fn hides_effort_picker_for_non_reasoning_profile() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(
        vec![entry("fast", false), entry("opus-reasoning", true)],
        Some("fast"),
        None,
    ));
    assert!(!overlay.shows_effort_picker());
    assert!(overlay.selected_effort().is_none());
    assert!(overlay.effort_options().is_empty());
}

#[test]
fn enter_emits_switch_model_with_alias_and_no_effort_for_plain_profile() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(
        vec![entry("fast", false), entry("opus-reasoning", true)],
        Some("opus-reasoning"),
        None,
    ));
    // Navigate up to the non-reasoning profile.
    let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('k')));
    assert_eq!(
        overlay.selected_profile().map(|e| e.alias.as_str()),
        Some("fast")
    );
    let (effects, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        Command::SwitchModel { alias, reasoning_effort, .. }
            if alias == "fast" && reasoning_effort.is_none()
    ));
    assert!(effects.contains(&CrossPanelEffect::DismissModelOverlay));
    assert!(!overlay.is_visible());
}

#[test]
fn enter_does_not_switch_disabled_profile() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(vec![disabled_entry("slow")], Some("fast"), None));
    let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));

    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));

    assert!(
        commands.is_empty(),
        "disabled profiles are visible for management but cannot be switched to"
    );
    assert!(overlay.is_visible());
}

#[test]
fn profile_management_keys_emit_settings_commands() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(
        vec![entry("fast", false), disabled_entry("slow")],
        Some("fast"),
        None,
    ));
    let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));

    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('e')));
    assert!(matches!(
        &commands[..],
        [Command::SetProfileEnabled { alias, enabled }] if alias == "slow" && *enabled
    ));

    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteProfileSettings { alias }] if alias == "slow"
    ));

    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('t')));
    assert!(matches!(
        &commands[..],
        [Command::TestModelProfile { alias }] if alias == "slow"
    ));

    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('o')));
    assert!(matches!(&commands[..], [Command::OpenProfilesConfig]));
}

#[test]
fn editor_ctrl_t_tests_draft_base_url() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(vec![entry("fast", false)], Some("fast"), None));
    let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));

    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('n')));
    type_text(&mut overlay, "draft");
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Down));
    type_text(&mut overlay, "openai");
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Down));
    type_text(&mut overlay, "gpt-4.1");
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Down));
    type_text(&mut overlay, "https://api.example.test/v1");

    let (_, commands) = overlay.handle_event(
        &ctx(&ws, &sid, &sessions, &proj),
        &modified_key(KeyCode::Char('t'), crossterm::event::KeyModifiers::CONTROL),
    );

    assert!(matches!(
        &commands[..],
        [Command::TestModelProfileUrl { alias, base_url }]
            if alias == "draft" && base_url == "https://api.example.test/v1"
    ));
}

#[test]
fn shift_j_and_k_emit_profile_reorder_commands() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(
        vec![entry("fast", false), entry("slow", false)],
        Some("fast"),
        None,
    ));
    let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));

    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('J')));
    assert!(matches!(
        &commands[..],
        [Command::MoveProfileInOrder { alias, direction }] if alias == "fast" && *direction == 1
    ));

    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('K')));
    assert!(matches!(
        &commands[..],
        [Command::MoveProfileInOrder { alias, direction }] if alias == "fast" && *direction == -1
    ));
}

#[test]
fn enter_emits_switch_model_with_selected_effort_for_reasoning_profile() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(
        vec![entry("opus-reasoning", true)],
        Some("opus-reasoning"),
        Some("low"),
    ));
    let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));
    // Tab into effort picker, j to "middle", j to "high".
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Tab));
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_effort(), Some("high"));
    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));
    assert!(matches!(
        &commands[0],
        Command::SwitchModel { alias, reasoning_effort, .. }
            if alias == "opus-reasoning" && reasoning_effort.as_deref() == Some("high")
    ));
}

#[test]
fn enter_with_no_session_emits_no_command() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(vec![entry("fast", false)], Some("fast"), None));
    let (ws, sid, sessions, proj) = test_ctx_with_session(None);
    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));
    assert!(commands.is_empty());
}

#[test]
fn esc_hides_and_emits_dismiss_effect() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(vec![entry("fast", false)], Some("fast"), None));
    let (ws, sid, sessions, proj) = test_ctx_with_session(None);
    let (effects, _) = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Esc));
    assert!(effects.contains(&CrossPanelEffect::DismissModelOverlay));
    assert!(!overlay.is_visible());
}

#[test]
fn show_effect_makes_visible() {
    let mut overlay = ModelOverlay::new();
    overlay.handle_effect(&CrossPanelEffect::ShowModelOverlay(snapshot(
        vec![entry("fast", false)],
        Some("fast"),
        None,
    )));
    assert!(overlay.is_visible());
    assert_eq!(overlay.profiles().len(), 1);
}

#[test]
fn j_and_k_navigate_profile_list() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(
        vec![entry("a", false), entry("b", false), entry("c", false)],
        Some("a"),
        None,
    ));
    let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(1));
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(2));
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(2), "clamps at end");
    let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('k')));
    assert_eq!(overlay.selected_index(), Some(1));
}

#[test]
fn renders_into_test_buffer() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(
        vec![entry("fast", false), entry("opus-reasoning", true)],
        Some("opus-reasoning"),
        Some("middle"),
    ));
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| overlay.render(f.area(), f))
        .expect("render");
}

#[test]
fn ignores_keys_when_hidden() {
    let mut overlay = ModelOverlay::new();
    let (ws, sid, sessions, proj) = test_ctx_with_session(None);
    let (effects, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));
    assert!(effects.is_empty());
    assert!(commands.is_empty());
}

#[test]
fn new_profile_editor_saves_full_profile_input() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(Vec::new(), None, None));
    overlay.replace_draft_for_test(agent_core::facade::ProfileSettingsInput {
        alias: "local-qwen".to_string(),
        provider: "openai-compatible".to_string(),
        model_id: "qwen3-coder".to_string(),
        enabled: true,
        context_window: Some(128000),
        output_limit: Some(8192),
        temperature: Some(0.2),
        top_p: Some(0.9),
        top_k: Some(40),
        max_tokens: Some(4096),
        base_url: Some("http://localhost:11434/v1".to_string()),
        api_key_env: Some("LOCAL_LLM_API_KEY".to_string()),
    });

    let (effects, commands) = overlay.handle_event(
        &ctx(
            &agent_core::WorkspaceId::new(),
            &None,
            &[],
            &agent_core::projection::SessionProjection::default(),
        ),
        &key(KeyCode::Enter),
    );

    assert!(effects.is_empty());
    assert!(matches!(
        &commands[..],
        [Command::SaveProfileSettings { input }]
            if input.alias == "local-qwen"
                && input.provider == "openai-compatible"
                && input.model_id == "qwen3-coder"
                && input.context_window == Some(128000)
                && input.output_limit == Some(8192)
                && input.temperature == Some(0.2)
                && input.top_p == Some(0.9)
                && input.top_k == Some(40)
                && input.max_tokens == Some(4096)
                && input.base_url.as_deref() == Some("http://localhost:11434/v1")
                && input.api_key_env.as_deref() == Some("LOCAL_LLM_API_KEY")
    ));
    assert!(overlay.is_visible());
}

#[test]
fn keyboard_driven_new_profile_editor_collects_required_fields() {
    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(Vec::new(), None, None));

    assert!(press(&mut overlay, KeyCode::Char('n')).is_empty());
    type_text(&mut overlay, "local");
    assert!(press(&mut overlay, KeyCode::Tab).is_empty());
    type_text(&mut overlay, "fake");
    assert!(press(&mut overlay, KeyCode::Tab).is_empty());
    type_text(&mut overlay, "fake-model");

    let commands = press(&mut overlay, KeyCode::Enter);

    assert!(matches!(
        &commands[..],
        [Command::SaveProfileSettings { input }]
            if input.alias == "local"
                && input.provider == "fake"
                && input.model_id == "fake-model"
                && input.enabled
    ));
}

#[test]
fn edit_profile_editor_preserves_alias_and_enabled_state() {
    let mut profile = entry("fast", false);
    profile.provider_display = "openai".to_string();
    profile.model_display = "gpt-5.4".to_string();
    profile.enabled = false;

    let mut overlay = ModelOverlay::new();
    overlay.show(snapshot(vec![profile], Some("fast"), None));
    let (ws, sid, sessions, proj) = test_ctx_with_session(None);
    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('u')));
    assert!(commands.is_empty());

    overlay.replace_draft_for_test(agent_core::facade::ProfileSettingsInput {
        alias: "fast".to_string(),
        provider: "anthropic".to_string(),
        model_id: "claude-opus-4.1".to_string(),
        enabled: false,
        context_window: None,
        output_limit: None,
        temperature: None,
        top_p: None,
        top_k: None,
        max_tokens: None,
        base_url: None,
        api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
    });

    let (_, commands) =
        overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));

    assert!(matches!(
        &commands[..],
        [Command::SaveProfileSettings { input }]
            if input.alias == "fast"
                && input.provider == "anthropic"
                && input.model_id == "claude-opus-4.1"
                && !input.enabled
                && input.api_key_env.as_deref() == Some("ANTHROPIC_API_KEY")
    ));
}

#[test]
fn render_function_handles_empty_profiles() {
    // Smoke-test the `render_model_overlay` re-export so the public API stays
    // wired up after the module split.
    let overlay = ModelOverlay::new();
    let mut list_state = ListState::default();
    let mut effort_state = ListState::default();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            super::render_model_overlay(f.area(), f, &overlay, &mut list_state, &mut effort_state);
        })
        .expect("render");
}
