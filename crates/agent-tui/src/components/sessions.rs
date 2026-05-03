use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, SessionInfo, SessionState,
};
use agent_core::SessionId;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

#[allow(dead_code)]
pub struct SessionsPanel {
    focused: bool,
    pub state: ListState,
    pub context_menu_open: bool,
    pub search_query: Option<String>,
}

impl Default for SessionsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionsPanel {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            focused: false,
            state,
            context_menu_open: false,
            search_query: None,
        }
    }

    #[allow(dead_code)]
    pub fn selected_session_id(&self, sessions: &[SessionInfo]) -> Option<SessionId> {
        self.state
            .selected()
            .and_then(|i| sessions.get(i))
            .map(|s| s.id.clone())
    }

    #[allow(dead_code)]
    pub fn filtered_sessions<'a>(&self, sessions: &'a [SessionInfo]) -> Vec<&'a SessionInfo> {
        if let Some(query) = &self.search_query {
            let q = query.to_lowercase();
            sessions
                .iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&q)
                        || s.model_profile.to_lowercase().contains(&q)
                })
                .collect()
        } else {
            sessions.iter().collect()
        }
    }

    pub fn scroll_up(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        let next = if i == 0 { len - 1 } else { i - 1 };
        self.state.select(Some(next));
    }

    pub fn scroll_down(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        let next = if i >= len - 1 { 0 } else { i + 1 };
        self.state.select(Some(next));
    }
}

fn session_state_icon(state: &SessionState) -> (&'static str, Color) {
    match state {
        SessionState::Active => ("●", Color::Green),
        SessionState::Idle => ("○", Color::DarkGray),
        SessionState::Error(_) => ("✕", Color::Red),
        SessionState::AwaitingPermission => ("⚠", Color::Yellow),
    }
}

pub fn render_sessions(
    area: Rect,
    frame: &mut Frame,
    sessions: &[SessionInfo],
    focused: bool,
    state: &mut ListState,
) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = sessions
        .iter()
        .map(|session| {
            let (icon, icon_color) = session_state_icon(&session.state);
            let pin = if session.pinned { "📌 " } else { "" };
            let mut spans = vec![
                Span::styled(format!("{pin}{icon} "), Style::default().fg(icon_color)),
                Span::raw(&session.title),
                Span::styled(
                    format!(" [{}]", session.model_profile),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ];
            if let SessionState::Error(e) = &session.state {
                spans.push(Span::styled(
                    format!(" {e}"),
                    Style::default().fg(Color::Red),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("❯ ")
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .title(" Sessions ")
                .border_style(border_style),
        );
    frame.render_stateful_widget(list, area, state);
}

impl Component for SessionsPanel {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        _event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        (Vec::new(), Vec::new())
    }

    fn handle_effect(&mut self, _effect: &CrossPanelEffect) {}

    fn render(&self, area: Rect, frame: &mut Frame) {
        let _ = (area, frame);
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

    fn make_session(title: &str, state: SessionState, pinned: bool) -> SessionInfo {
        SessionInfo {
            id: SessionId::new(),
            title: title.into(),
            model_profile: "fast".into(),
            state,
            pinned,
        }
    }

    #[test]
    fn filtered_sessions_returns_all_when_no_query() {
        let panel = SessionsPanel::new();
        let sessions = vec![
            make_session("main", SessionState::Active, false),
            make_session("debug", SessionState::Idle, false),
        ];
        let filtered = panel.filtered_sessions(&sessions);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filtered_sessions_matches_title_case_insensitive() {
        let mut panel = SessionsPanel::new();
        panel.search_query = Some("MAIN".into());
        let sessions = vec![
            make_session("main session", SessionState::Active, false),
            make_session("debug", SessionState::Idle, false),
        ];
        let filtered = panel.filtered_sessions(&sessions);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "main session");
    }

    #[test]
    fn selected_session_id_returns_correct_id() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![
            make_session("first", SessionState::Active, false),
            make_session("second", SessionState::Idle, false),
        ];
        panel.state.select(Some(1));
        assert_eq!(
            panel.selected_session_id(&sessions),
            Some(sessions[1].id.clone())
        );
    }

    #[test]
    fn session_state_icon_values() {
        assert_eq!(session_state_icon(&SessionState::Active).0, "●");
        assert_eq!(session_state_icon(&SessionState::Idle).0, "○");
        assert_eq!(
            session_state_icon(&SessionState::Error("err".into())).0,
            "✕"
        );
        assert_eq!(session_state_icon(&SessionState::AwaitingPermission).0, "⚠");
    }
}
