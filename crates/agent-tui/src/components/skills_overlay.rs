//! Skills overlay — pop-up modal listing native skills with an active marker,
//! supporting per-session activation/deactivation and inline body preview.
//!
//! The TUI surface for the same data the GUI's `SkillSettingsPane` shows,
//! minus remote-marketplace search. The App constructs a snapshot of
//! [`SkillEntry`] values before opening the overlay; the overlay produces
//! [`Command`] values that the main loop dispatches back to `AppFacade`.

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext, SkillEntry};

/// Inline detail view shown when the user presses Enter on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyView {
    pub skill_id: String,
    pub body: String,
}

pub struct SkillsOverlay {
    focused: bool,
    visible: bool,
    skills: Vec<SkillEntry>,
    list_state: ListState,
    body: Option<BodyView>,
}

impl Default for SkillsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillsOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            skills: Vec::new(),
            list_state: ListState::default(),
            body: None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, skills: Vec<SkillEntry>) {
        let prior_selected_id = self
            .list_state
            .selected()
            .and_then(|i| self.skills.get(i))
            .map(|s| s.id.clone());

        let select = if skills.is_empty() {
            None
        } else if let Some(id) = prior_selected_id {
            skills.iter().position(|s| s.id == id).or(Some(0))
        } else {
            Some(0)
        };

        self.skills = skills;
        self.list_state.select(select);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.skills.clear();
        self.list_state.select(None);
        self.body = None;
    }

    #[allow(dead_code)]
    pub fn skills(&self) -> &[SkillEntry] {
        &self.skills
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    #[allow(dead_code)]
    pub fn body_skill_id(&self) -> Option<&str> {
        self.body.as_ref().map(|b| b.skill_id.as_str())
    }

    fn selected(&self) -> Option<&SkillEntry> {
        self.list_state.selected().and_then(|i| self.skills.get(i))
    }

    fn move_down(&mut self) {
        if self.skills.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(i) if i + 1 < self.skills.len() => i + 1,
            Some(_) => self.skills.len() - 1,
            None => 0,
        };
        self.list_state.select(Some(next));
    }

    fn move_up(&mut self) {
        if self.skills.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => 0,
        };
        self.list_state.select(Some(next));
    }
}

pub fn render_skills_overlay(
    area: Rect,
    frame: &mut Frame,
    skills: &[SkillEntry],
    list_state: &mut ListState,
    body: Option<&BodyView>,
) {
    let modal_width = 72.min(area.width.saturating_sub(4));
    let modal_height = 22.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let title = if body.is_some() {
        " 🧠 Skill detail "
    } else {
        " 🧠 Skills "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let body_height = inner.height.saturating_sub(2);
    let body_area = Rect::new(inner.x, inner.y, inner.width, body_height);
    let hint_area = Rect::new(
        inner.x,
        inner.y + body_height,
        inner.width,
        inner.height.saturating_sub(body_height),
    );

    if let Some(detail) = body {
        let header = Line::from(vec![Span::styled(
            detail.skill_id.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]);
        let mut lines = vec![header, Line::from("")];
        for raw in detail.body.lines() {
            lines.push(Line::from(raw.to_string()));
        }
        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(para, body_area);

        let hints = Line::from(vec![
            Span::styled("[Esc] back  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+S] close", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(hints), hint_area);
        return;
    }

    if skills.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No skills discovered",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, body_area);
    } else {
        let items: Vec<ListItem> = skills
            .iter()
            .map(|s| {
                let (marker, marker_color) = if s.active {
                    ("● active ", Color::Green)
                } else {
                    ("○        ", Color::DarkGray)
                };
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(marker_color)),
                    Span::styled(s.id.clone(), Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("  "),
                    Span::styled(s.description.clone(), Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("  [{} / {}]", s.source, s.activation_mode),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_stateful_widget(list, body_area, list_state);
    }

    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter] body  ", Style::default().fg(Color::Yellow)),
        Span::styled("[a] activate  ", Style::default().fg(Color::Green)),
        Span::styled("[d] deactivate  ", Style::default().fg(Color::Red)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

impl Component for SkillsOverlay {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        let mut effects = Vec::new();
        let mut commands = Vec::new();

        // In body view Esc returns to the list; any other key is ignored
        // so the parent activate/deactivate keys don't fire by accident.
        if self.body.is_some() {
            if matches!(key.code, KeyCode::Esc) {
                self.body = None;
            }
            return (effects, commands);
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissSkillsOverlay);
            }
            KeyCode::Enter => {
                if let Some(entry) = self.selected() {
                    commands.push(Command::ShowSkill {
                        skill_id: entry.id.clone(),
                    });
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                if let (Some(entry), Some(session_id)) =
                    (self.selected(), ctx.current_session_id.as_ref())
                {
                    if !entry.active {
                        commands.push(Command::ActivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id: entry.id.clone(),
                        });
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let (Some(entry), Some(session_id)) =
                    (self.selected(), ctx.current_session_id.as_ref())
                {
                    if entry.active {
                        commands.push(Command::DeactivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id: entry.id.clone(),
                        });
                    }
                }
            }
            _ => {}
        }

        (effects, commands)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowSkillsOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissSkillsOverlay => self.hide(),
            CrossPanelEffect::ShowSkillBody { skill_id, body } if self.visible => {
                self.body = Some(BodyView {
                    skill_id: skill_id.clone(),
                    body: body.clone(),
                });
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut state = self.list_state;
        render_skills_overlay(area, frame, &self.skills, &mut state, self.body.as_ref());
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{FocusTarget, SessionInfo};

    fn entry(id: &str, active: bool) -> SkillEntry {
        SkillEntry {
            id: id.to_string(),
            name: id.to_string(),
            description: format!("{id} description"),
            source: "user".to_string(),
            activation_mode: "manual".to_string(),
            active,
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn test_ctx_session(
        session_id: &Option<agent_core::SessionId>,
        workspace_id: &agent_core::WorkspaceId,
    ) -> EventContext<'static> {
        use agent_core::projection::SessionProjection;
        static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
        let projection = PROJECTION.get_or_init(SessionProjection::default);
        static SESSIONS: std::sync::OnceLock<Vec<SessionInfo>> = std::sync::OnceLock::new();
        let sessions = SESSIONS.get_or_init(Vec::new);
        // The component only reads `workspace_id` and `current_session_id` —
        // leak owned copies so the static-lifetime EventContext compiles for
        // tests without us having to thread a runtime through.
        let ws: &'static agent_core::WorkspaceId = Box::leak(Box::new(workspace_id.clone()));
        let sid: &'static Option<agent_core::SessionId> = Box::leak(Box::new(session_id.clone()));
        EventContext {
            focus: FocusTarget::SkillsOverlay,
            current_session: projection,
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: true,
            workspace_id: ws,
            current_session_id: sid,
        }
    }

    fn test_ctx() -> EventContext<'static> {
        let ws = agent_core::WorkspaceId::new();
        let sid: Option<agent_core::SessionId> = Some(agent_core::SessionId::new());
        test_ctx_session(&sid, &ws)
    }

    #[test]
    fn lists_skills_with_active_marker() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", true), entry("beta", false)]);
        assert!(overlay.is_visible());
        assert_eq!(overlay.skills().len(), 2);
        assert_eq!(overlay.selected_index(), Some(0));

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
        assert!(rendered.contains("alpha"), "alpha row missing: {rendered}");
        assert!(rendered.contains("beta"), "beta row missing: {rendered}");
        assert!(
            rendered.contains("active"),
            "active marker missing for active skill: {rendered}"
        );
    }

    #[test]
    fn overlay_invisible_by_default() {
        let overlay = SkillsOverlay::new();
        assert!(!overlay.is_visible());
        assert!(overlay.skills().is_empty());
    }

    #[test]
    fn j_and_k_navigate_selection() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![
            entry("alpha", false),
            entry("beta", true),
            entry("gamma", false),
        ]);
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(2));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Down));
        assert_eq!(overlay.selected_index(), Some(2));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Up));
        assert_eq!(overlay.selected_index(), Some(0));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(0));
    }

    #[test]
    fn enter_emits_show_skill_for_selected() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false), entry("beta", false)]);
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::ShowSkill { skill_id } if skill_id == "beta"
        ));
    }

    #[test]
    fn body_effect_switches_to_detail_view() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false)]);
        overlay.handle_effect(&CrossPanelEffect::ShowSkillBody {
            skill_id: "alpha".to_string(),
            body: "## Body\n\nDoc text".to_string(),
        });
        assert_eq!(overlay.body_skill_id(), Some("alpha"));

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
        assert!(rendered.contains("Doc text"), "body text missing");

        // Esc in body view returns to the list, not dismiss.
        let (effects, _) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
        assert!(effects.is_empty());
        assert!(overlay.is_visible());
        assert_eq!(overlay.body_skill_id(), None);
    }

    #[test]
    fn a_emits_activate_for_inactive_skill() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));
        assert!(matches!(
            &commands[0],
            Command::ActivateSkill { skill_id, .. } if skill_id == "alpha"
        ));
    }

    #[test]
    fn a_is_no_op_for_already_active_skill() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", true)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));
        assert!(commands.is_empty());
    }

    #[test]
    fn d_emits_deactivate_for_active_skill() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", true)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('d')));
        assert!(matches!(
            &commands[0],
            Command::DeactivateSkill { skill_id, .. } if skill_id == "alpha"
        ));
    }

    #[test]
    fn a_without_session_emits_nothing() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false)]);
        let ws = agent_core::WorkspaceId::new();
        let ctx = test_ctx_session(&None, &ws);
        let (_, commands) = overlay.handle_event(&ctx, &key(KeyCode::Char('a')));
        assert!(commands.is_empty());
    }

    #[test]
    fn esc_hides_and_emits_dismiss_effect() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false)]);
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
        assert!(commands.is_empty());
        assert!(effects.contains(&CrossPanelEffect::DismissSkillsOverlay));
        assert!(!overlay.is_visible());
    }

    #[test]
    fn ignores_keys_when_hidden() {
        let mut overlay = SkillsOverlay::new();
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
    }

    #[test]
    fn show_effect_makes_visible() {
        let mut overlay = SkillsOverlay::new();
        overlay.handle_effect(&CrossPanelEffect::ShowSkillsOverlay(vec![entry(
            "alpha", false,
        )]));
        assert!(overlay.is_visible());
        assert_eq!(overlay.skills().len(), 1);
    }

    #[test]
    fn show_preserves_selection_across_refresh() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false), entry("beta", false)]);
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));
        // Same list, beta now active — selection should stay on beta.
        overlay.show(vec![entry("alpha", false), entry("beta", true)]);
        assert_eq!(overlay.selected_index(), Some(1));
        assert!(overlay.skills()[1].active);
    }
}
