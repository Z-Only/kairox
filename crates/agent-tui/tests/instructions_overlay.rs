use agent_core::facade::InstructionsView;
use agent_core::ConfigScope;
use agent_tui::components::instructions_overlay::InstructionsOverlay;
use agent_tui::components::{Command, Component, CrossPanelEffect, EventContext, FocusTarget};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn test_ctx() -> EventContext<'static> {
    let projection = Box::leak(Box::new(
        agent_core::projection::SessionProjection::default(),
    ));
    let workspace_id = Box::leak(Box::new(agent_core::WorkspaceId::from_string(
        "wrk_test".into(),
    )));
    EventContext {
        focus: FocusTarget::InstructionsOverlay,
        current_session: projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id,
        current_session_id: &None,
    }
}

fn view() -> InstructionsView {
    InstructionsView {
        system: "System prompt.".into(),
        user: Some("User guidance.".into()),
        project: Some("Project guidance.".into()),
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

#[test]
fn instructions_overlay_shows_snapshot_and_effective_preview() {
    let mut overlay = InstructionsOverlay::new();

    overlay.handle_effect(&CrossPanelEffect::ShowInstructionsOverlay(view()));

    assert!(overlay.is_visible());
    assert_eq!(overlay.active_tab_label(), "User");
    assert_eq!(overlay.active_scope(), ConfigScope::User);
    assert_eq!(overlay.system_text(), "System prompt.");
    assert_eq!(overlay.user_text(), "User guidance.");
    assert_eq!(overlay.project_text(), "Project guidance.");
    assert_eq!(
        overlay.effective_text(),
        "System prompt.\n\nUser guidance.\n\nProject guidance."
    );
}

#[test]
fn instructions_overlay_switches_to_project_and_saves_project_text() {
    let mut overlay = InstructionsOverlay::new();
    overlay.handle_effect(&CrossPanelEffect::ShowInstructionsOverlay(view()));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    for ch in " Updated.".chars() {
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char(ch)));
    }
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::F(2)));

    assert_eq!(overlay.active_scope(), ConfigScope::Project);
    assert_eq!(
        commands,
        vec![Command::SaveInstructions {
            scope: ConfigScope::Project,
            text: "Project guidance. Updated.".into(),
        }]
    );
}

#[test]
fn instructions_overlay_system_tab_is_read_only() {
    let mut overlay = InstructionsOverlay::new();
    overlay.handle_effect(&CrossPanelEffect::ShowInstructionsOverlay(view()));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));
    assert_eq!(overlay.active_tab_label(), "System");
    let before = overlay.effective_text();

    for ch in " edited".chars() {
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char(ch)));
    }
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Backspace));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::F(2)));

    assert!(commands.is_empty());
    assert_eq!(overlay.system_text(), "System prompt.");
    assert_eq!(overlay.effective_text(), before);
}

#[test]
fn instructions_overlay_effective_tab_is_read_only() {
    let mut overlay = InstructionsOverlay::new();
    overlay.handle_effect(&CrossPanelEffect::ShowInstructionsOverlay(view()));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
    assert_eq!(overlay.active_tab_label(), "Effective");
    let before = overlay.effective_text();

    for ch in " edited".chars() {
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char(ch)));
    }
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Backspace));
    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::F(2)));

    assert!(commands.is_empty());
    assert_eq!(overlay.effective_text(), before);
}

#[test]
fn instructions_overlay_escape_dismisses_without_save_command() {
    let mut overlay = InstructionsOverlay::new();
    overlay.handle_effect(&CrossPanelEffect::ShowInstructionsOverlay(view()));

    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));

    assert_eq!(effects, vec![CrossPanelEffect::DismissInstructionsOverlay]);
    assert!(commands.is_empty());
    assert!(!overlay.is_visible());
}
