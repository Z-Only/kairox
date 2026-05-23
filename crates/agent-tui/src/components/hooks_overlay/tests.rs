use super::*;
use agent_core::facade::{
    HookSettingsInput, HookSettingsView, HookTemplateView, HooksSettingsView,
};
use agent_core::ConfigScope;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn hook(id: &str, scope: ConfigScope, enabled: bool) -> HookSettingsView {
    HookSettingsView {
        id: id.into(),
        event: "Stop".into(),
        matcher: Some("*".into()),
        command: "cargo test".into(),
        status_message: Some("Testing".into()),
        timeout_secs: Some(120),
        enabled,
        source: scope,
        config_path: Some(format!("/tmp/{id}.toml")),
    }
}

fn template() -> HookTemplateView {
    HookTemplateView {
        id: "stop-validation".into(),
        name: "Stop validation".into(),
        description: "Run validation".into(),
        event: "Stop".into(),
        matcher: Some("*".into()),
        command: "cargo test --workspace --all-targets".into(),
        status_message: Some("Running validation".into()),
        timeout_secs: Some(600),
    }
}

fn snapshot() -> HooksSettingsView {
    HooksSettingsView {
        user: vec![hook("user-verify", ConfigScope::User, true)],
        project: vec![hook("project-policy", ConfigScope::Project, false)],
        templates: vec![template()],
        user_config_path: "/home/me/.kairox/config.toml".into(),
        project_config_path: Some("/repo/.kairox/config.toml".into()),
    }
}

#[test]
fn reads_user_and_project_hooks_from_snapshot() {
    let mut overlay = HooksOverlay::new();
    overlay.show(snapshot());

    assert!(overlay.is_visible());
    assert_eq!(overlay.user_hooks()[0].id, "user-verify");
    overlay.handle_event_for_test(&key(KeyCode::Tab));
    assert_eq!(overlay.project_hooks()[0].id, "project-policy");
}

#[test]
fn template_fills_editor_form() {
    let mut overlay = HooksOverlay::new();
    overlay.show(snapshot());

    overlay.handle_event_for_test(&key(KeyCode::Tab));
    overlay.handle_event_for_test(&key(KeyCode::Tab));
    overlay.handle_event_for_test(&key(KeyCode::Enter));

    let draft = overlay.draft_for_test();
    assert_eq!(draft.id, "stop-validation");
    assert_eq!(draft.event, "Stop");
    assert_eq!(draft.command, "cargo test --workspace --all-targets");
    assert_eq!(draft.scope, ConfigScope::User);
}

#[test]
fn save_and_delete_emit_hook_commands() {
    let mut overlay = HooksOverlay::new();
    overlay.show(snapshot());
    overlay.replace_draft_for_test(HookSettingsInput {
        scope: ConfigScope::Project,
        id: "project-policy".into(),
        event: "PreToolUse".into(),
        matcher: Some("shell".into()),
        command: "python3 .kairox/hooks/pre_tool_policy.py".into(),
        status_message: Some("Checking policy".into()),
        timeout_secs: Some(30),
        enabled: true,
    });

    let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Enter));
    assert!(matches!(
        &commands[..],
        [Command::SaveHookSettings { input }]
            if input.scope == ConfigScope::Project
                && input.id == "project-policy"
                && input.enabled
    ));

    overlay.show(snapshot());
    overlay.handle_event_for_test(&key(KeyCode::Char('x')));
    let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Delete));
    assert!(matches!(
        &commands[..],
        [Command::DeleteHookSettings { scope, event, id }]
            if *scope == ConfigScope::User && event == "Stop" && id == "user-verify"
    ));
}

#[test]
fn renders_enabled_and_disabled_state() {
    let mut overlay = HooksOverlay::new();
    overlay.show(snapshot());
    let backend = ratatui::backend::TestBackend::new(120, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| overlay.render(f.area(), f))
        .expect("render");

    overlay.handle_event_for_test(&key(KeyCode::Tab));
    terminal
        .draw(|f| overlay.render(f.area(), f))
        .expect("render");
    let buf = terminal.backend().buffer().clone();
    let mut rendered = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            rendered.push_str(buf[(x, y)].symbol());
        }
        rendered.push('\n');
    }

    assert!(rendered.contains("project-policy"), "{rendered}");
    assert!(rendered.contains("disabled"), "{rendered}");
}
