use super::MonitorOverlay;
use crossterm::event::{Event, KeyCode};

use crate::components::monitor_overlay::types::MonitorEntry;
use crate::components::{Command, Component, CrossPanelEffect, EventContext, FocusTarget};

fn monitor_entry(id: &str, description: &str, persistent: bool) -> MonitorEntry {
    MonitorEntry {
        monitor_id: id.to_string(),
        description: description.to_string(),
        command: format!("tail -f /var/log/{id}.log"),
        persistent,
        timeout_ms: if persistent { 0 } else { 300_000 },
    }
}

fn snapshot() -> crate::components::MonitorOverlaySnapshot {
    crate::components::MonitorOverlaySnapshot {
        monitors: vec![
            monitor_entry("mon_1", "Watch build logs", false),
            monitor_entry("mon_2", "Tail deploy", true),
        ],
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
        focus: FocusTarget::MonitorOverlay,
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

#[test]
fn lists_monitors_from_snapshot() {
    let mut overlay = MonitorOverlay::new();
    overlay.show(snapshot());

    assert!(overlay.is_visible());
    assert_eq!(overlay.selected_index(), Some(0));
    assert_eq!(overlay.monitors().len(), 2);

    let backend = ratatui::backend::TestBackend::new(120, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
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
    assert!(rendered.contains("mon_1"), "monitor id missing: {rendered}");
    assert!(
        rendered.contains("Watch build logs"),
        "description missing: {rendered}"
    );
}

#[test]
fn x_stops_selected_monitor() {
    let mut overlay = MonitorOverlay::new();
    overlay.show(snapshot());

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));

    assert!(matches!(
        &commands[..],
        [Command::MonitorStop { monitor_id }] if monitor_id == "mon_1"
    ));
}

#[test]
fn j_k_navigates_up_and_down() {
    let mut overlay = MonitorOverlay::new();
    overlay.show(snapshot());

    assert_eq!(overlay.selected_index(), Some(0));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
    assert_eq!(overlay.selected_index(), Some(1));

    let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
    assert_eq!(overlay.selected_index(), Some(0));
}

#[test]
fn r_emits_refresh_command() {
    let mut overlay = MonitorOverlay::new();
    overlay.show(snapshot());

    let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));

    assert!(matches!(&commands[..], [Command::OpenMonitorOverlay]));
}

#[test]
fn esc_hides_and_emits_dismiss_effect() {
    let mut overlay = MonitorOverlay::new();
    overlay.show(snapshot());

    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));

    assert!(commands.is_empty());
    assert!(effects.contains(&CrossPanelEffect::DismissMonitorOverlay));
    assert!(!overlay.is_visible());
}

#[test]
fn empty_monitors_show_placeholder() {
    let mut overlay = MonitorOverlay::new();
    overlay.show(crate::components::MonitorOverlaySnapshot {
        monitors: Vec::new(),
    });

    assert!(overlay.is_visible());
    assert_eq!(overlay.selected_index(), None);

    let backend = ratatui::backend::TestBackend::new(120, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
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
    assert!(
        rendered.contains("No active monitors"),
        "placeholder missing: {rendered}"
    );
}

#[test]
fn x_on_empty_list_does_nothing() {
    let mut overlay = MonitorOverlay::new();
    overlay.show(crate::components::MonitorOverlaySnapshot {
        monitors: Vec::new(),
    });

    let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));

    assert!(commands.is_empty());
    assert!(effects.is_empty());
}
