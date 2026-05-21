//! Model profile selector overlay — pop-up modal listing profiles with the
//! current profile/effort highlighted. Mirrors the GUI's `ChatModelSelector`
//! (`apps/agent-gui/src/components/ChatModelSelector.vue`):
//! reasoning-capable profiles expose a side panel for picking effort.
//!
//! Read-only over `ProfileDef`: the App builds a snapshot of
//! [`ModelProfileEntry`] values via `Config::profile_info()` and dispatches
//! `ShowModelOverlay`; the overlay produces a single
//! [`Command::SwitchModel`] on commit which the main loop forwards to
//! `LocalRuntime::switch_model`.

use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, ModelOverlaySnapshot, ModelProfileEntry,
};

/// Effort presets exposed for reasoning-capable profiles. Mirrors the GUI's
/// `DEFAULT_REASONING_EFFORTS` constant in `apps/agent-gui/src/stores/session.ts`.
pub const REASONING_EFFORTS: [&str; 4] = ["low", "middle", "high", "xhigh"];

/// Which sub-panel currently consumes navigation keys inside the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayFocus {
    ProfileList,
    EffortList,
}

pub struct ModelOverlay {
    focused: bool,
    visible: bool,
    profiles: Vec<ModelProfileEntry>,
    current_alias: Option<String>,
    current_effort: Option<String>,
    list_state: ListState,
    effort_state: ListState,
    overlay_focus: OverlayFocus,
}

impl Default for ModelOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            profiles: Vec::new(),
            current_alias: None,
            current_effort: None,
            list_state: ListState::default(),
            effort_state: ListState::default(),
            overlay_focus: OverlayFocus::ProfileList,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: ModelOverlaySnapshot) {
        // Default selection: the current alias if it exists in the list, else 0.
        let select = if snapshot.profiles.is_empty() {
            None
        } else {
            snapshot
                .current_alias
                .as_ref()
                .and_then(|a| snapshot.profiles.iter().position(|p| &p.alias == a))
                .or(Some(0))
        };
        self.list_state.select(select);

        // Effort selection mirrors current_effort when present and the selected
        // profile supports reasoning; else default to "low" so the picker has
        // a visible cursor.
        self.current_alias = snapshot.current_alias;
        self.current_effort = snapshot.current_effort;
        self.profiles = snapshot.profiles;
        let initial_effort = self
            .current_effort
            .as_deref()
            .and_then(|e| REASONING_EFFORTS.iter().position(|x| *x == e))
            .unwrap_or(0);
        self.effort_state.select(Some(initial_effort));
        self.overlay_focus = OverlayFocus::ProfileList;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.profiles.clear();
        self.list_state.select(None);
        self.effort_state.select(None);
        self.current_alias = None;
        self.current_effort = None;
        self.overlay_focus = OverlayFocus::ProfileList;
    }

    #[allow(dead_code)]
    pub fn profiles(&self) -> &[ModelProfileEntry] {
        &self.profiles
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn selected_profile(&self) -> Option<&ModelProfileEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.profiles.get(i))
    }

    /// `true` when the selected profile is reasoning-capable, so the effort
    /// picker should be rendered.
    pub fn shows_effort_picker(&self) -> bool {
        self.selected_profile()
            .map(|p| p.supports_reasoning)
            .unwrap_or(false)
    }

    /// Currently highlighted effort string (only meaningful when the selected
    /// profile supports reasoning).
    pub fn selected_effort(&self) -> Option<&'static str> {
        if !self.shows_effort_picker() {
            return None;
        }
        self.effort_state
            .selected()
            .and_then(|i| REASONING_EFFORTS.get(i).copied())
    }

    /// Available effort options for the selected profile. Empty for
    /// non-reasoning models.
    #[allow(dead_code)]
    pub fn effort_options(&self) -> &'static [&'static str] {
        if self.shows_effort_picker() {
            &REASONING_EFFORTS
        } else {
            &[]
        }
    }

    fn move_down(&mut self) {
        match self.overlay_focus {
            OverlayFocus::ProfileList => {
                if self.profiles.is_empty() {
                    return;
                }
                let next = match self.list_state.selected() {
                    Some(i) if i + 1 < self.profiles.len() => i + 1,
                    Some(_) => self.profiles.len() - 1,
                    None => 0,
                };
                self.list_state.select(Some(next));
            }
            OverlayFocus::EffortList => {
                let len = REASONING_EFFORTS.len();
                let next = match self.effort_state.selected() {
                    Some(i) if i + 1 < len => i + 1,
                    Some(_) => len - 1,
                    None => 0,
                };
                self.effort_state.select(Some(next));
            }
        }
    }

    fn move_up(&mut self) {
        match self.overlay_focus {
            OverlayFocus::ProfileList => {
                if self.profiles.is_empty() {
                    return;
                }
                let next = match self.list_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    _ => 0,
                };
                self.list_state.select(Some(next));
            }
            OverlayFocus::EffortList => {
                let next = match self.effort_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    _ => 0,
                };
                self.effort_state.select(Some(next));
            }
        }
    }

    fn cycle_inner_focus(&mut self) {
        if !self.shows_effort_picker() {
            return;
        }
        self.overlay_focus = match self.overlay_focus {
            OverlayFocus::ProfileList => OverlayFocus::EffortList,
            OverlayFocus::EffortList => OverlayFocus::ProfileList,
        };
    }

    fn commit_command(&self, ctx: &EventContext) -> Option<Command> {
        let entry = self.selected_profile()?;
        let session_id = ctx.current_session_id.clone()?;
        let reasoning_effort = if entry.supports_reasoning {
            self.selected_effort().map(|s| s.to_string())
        } else {
            None
        };
        Some(Command::SwitchModel {
            workspace_id: ctx.workspace_id.clone(),
            session_id,
            alias: entry.alias.clone(),
            reasoning_effort,
        })
    }
}

pub fn render_model_overlay(
    area: Rect,
    frame: &mut Frame,
    overlay: &ModelOverlay,
    list_state: &mut ListState,
    effort_state: &mut ListState,
) {
    let modal_width = 72.min(area.width.saturating_sub(4));
    let modal_height = 20.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " 🤖 Model Profile ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let list_height = inner.height.saturating_sub(2);
    let list_area = Rect::new(inner.x, inner.y, inner.width, list_height);
    let hint_area = Rect::new(
        inner.x,
        inner.y + list_height,
        inner.width,
        inner.height.saturating_sub(list_height),
    );

    let show_effort = overlay.shows_effort_picker();
    let columns = if show_effort {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(list_area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(list_area)
    };

    if overlay.profiles.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No model profiles configured",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, columns[0]);
    } else {
        let items: Vec<ListItem> = overlay
            .profiles
            .iter()
            .map(|p| {
                let is_current = overlay.current_alias.as_deref() == Some(p.alias.as_str());
                let marker = if is_current { "● " } else { "  " };
                let reasoning_tag = if p.supports_reasoning { " [R]" } else { "" };
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(Color::Green)),
                    Span::styled(
                        p.alias.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {}/{}", p.provider_display, p.model_display),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(reasoning_tag, Style::default().fg(Color::Magenta)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let highlight = if overlay.overlay_focus == OverlayFocus::ProfileList {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::Reset)
        };
        let list = List::new(items).highlight_style(highlight);
        frame.render_stateful_widget(list, columns[0], list_state);
    }

    if show_effort {
        let items: Vec<ListItem> = REASONING_EFFORTS
            .iter()
            .map(|effort| {
                let is_current = overlay.current_effort.as_deref() == Some(*effort);
                let marker = if is_current { "● " } else { "  " };
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(Color::Green)),
                    Span::raw(*effort),
                ]);
                ListItem::new(line)
            })
            .collect();
        let highlight = if overlay.overlay_focus == OverlayFocus::EffortList {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::Reset)
        };
        let effort_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " effort ",
                Style::default().fg(Color::Magenta),
            ));
        let effort_inner = effort_block.inner(columns[1]);
        frame.render_widget(effort_block, columns[1]);
        let list = List::new(items).highlight_style(highlight);
        frame.render_stateful_widget(list, effort_inner, effort_state);
    }

    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Tab] effort  ", Style::default().fg(Color::Magenta)),
        Span::styled("[Enter] switch  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

impl Component for ModelOverlay {
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

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Char('h') => self.cycle_inner_focus(),
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissModelOverlay);
            }
            KeyCode::Enter => {
                if let Some(cmd) = self.commit_command(ctx) {
                    commands.push(cmd);
                    self.hide();
                    effects.push(CrossPanelEffect::DismissModelOverlay);
                }
            }
            _ => {}
        }

        (effects, commands)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowModelOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissModelOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut list_state = self.list_state;
        let mut effort_state = self.effort_state;
        render_model_overlay(area, frame, self, &mut list_state, &mut effort_state);
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

    fn entry(alias: &str, supports_reasoning: bool) -> ModelProfileEntry {
        ModelProfileEntry {
            alias: alias.to_string(),
            provider_display: "provider".to_string(),
            model_display: format!("{alias}-model"),
            supports_reasoning,
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
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
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
        let (effects, _) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Esc));
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
}
