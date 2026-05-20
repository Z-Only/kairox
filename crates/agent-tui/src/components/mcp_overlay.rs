//! MCP server overlay — pop-up modal listing MCP servers with status,
//! supporting start/stop, trust, and refresh from a single keyboard view.
//!
//! Read-only over the runtime's `McpServerManager`: the App constructs a
//! snapshot of [`McpServerEntry`] values before opening the overlay; the
//! overlay produces [`Command`] values that the main loop dispatches back
//! to the manager.

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, McpServerEntry, McpServerStatusView,
};

pub struct McpOverlay {
    focused: bool,
    visible: bool,
    servers: Vec<McpServerEntry>,
    list_state: ListState,
}

impl Default for McpOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl McpOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            servers: Vec::new(),
            list_state: ListState::default(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, servers: Vec<McpServerEntry>) {
        let select = if servers.is_empty() { None } else { Some(0) };
        self.servers = servers;
        self.list_state.select(select);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.servers.clear();
        self.list_state.select(None);
    }

    #[allow(dead_code)]
    pub fn servers(&self) -> &[McpServerEntry] {
        &self.servers
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    fn selected(&self) -> Option<&McpServerEntry> {
        self.list_state.selected().and_then(|i| self.servers.get(i))
    }

    fn move_down(&mut self) {
        if self.servers.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(i) if i + 1 < self.servers.len() => i + 1,
            Some(_) => self.servers.len() - 1,
            None => 0,
        };
        self.list_state.select(Some(next));
    }

    fn move_up(&mut self) {
        if self.servers.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => 0,
        };
        self.list_state.select(Some(next));
    }
}

pub fn render_mcp_overlay(
    area: Rect,
    frame: &mut Frame,
    servers: &[McpServerEntry],
    list_state: &mut ListState,
) {
    let modal_width = 64.min(area.width.saturating_sub(4));
    let modal_height = 18.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " 🔌 MCP Servers ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Magenta));

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

    if servers.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No MCP servers configured",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, list_area);
    } else {
        let items: Vec<ListItem> = servers
            .iter()
            .map(|s| {
                let (status_label, status_color) = match s.status {
                    McpServerStatusView::Running => ("● running", Color::Green),
                    McpServerStatusView::Starting => ("● starting", Color::Yellow),
                    McpServerStatusView::Stopped => ("○ stopped", Color::Gray),
                    McpServerStatusView::Failed => ("✗ failed", Color::Red),
                };
                let trust_label = if s.trusted { " trusted" } else { "" };
                let line = Line::from(vec![
                    Span::styled(status_label, Style::default().fg(status_color)),
                    Span::raw("  "),
                    Span::styled(
                        s.server_id.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  ({} tools)", s.tool_count),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(trust_label, Style::default().fg(Color::Magenta)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_stateful_widget(list, list_area, list_state);
    }

    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter] start/stop  ", Style::default().fg(Color::Yellow)),
        Span::styled("[t] trust  ", Style::default().fg(Color::Magenta)),
        Span::styled("[r] refresh  ", Style::default().fg(Color::Cyan)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

impl Component for McpOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
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
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissMcpOverlay);
            }
            KeyCode::Enter => {
                if let Some(entry) = self.selected() {
                    let server_id = entry.server_id.clone();
                    match entry.status {
                        McpServerStatusView::Running | McpServerStatusView::Starting => {
                            commands.push(Command::StopMcpServer { server_id });
                        }
                        McpServerStatusView::Stopped | McpServerStatusView::Failed => {
                            commands.push(Command::StartMcpServer { server_id });
                        }
                    }
                }
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                if let Some(entry) = self.selected() {
                    commands.push(Command::TrustMcpServer {
                        server_id: entry.server_id.clone(),
                    });
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Some(entry) = self.selected() {
                    commands.push(Command::RefreshMcpTools {
                        server_id: entry.server_id.clone(),
                    });
                }
            }
            _ => {}
        }

        (effects, commands)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowMcpOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissMcpOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut state = self.list_state;
        render_mcp_overlay(area, frame, &self.servers, &mut state);
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
    use crate::components::{FocusTarget, McpServerStatusView};

    fn entry(id: &str, status: McpServerStatusView, trusted: bool, tools: usize) -> McpServerEntry {
        McpServerEntry {
            server_id: id.to_string(),
            status,
            trusted,
            tool_count: tools,
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
            focus: FocusTarget::McpOverlay,
            current_session: projection,
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
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
    fn overlay_invisible_by_default() {
        let overlay = McpOverlay::new();
        assert!(!overlay.is_visible());
        assert!(overlay.servers().is_empty());
    }

    #[test]
    fn renders_server_list_from_runtime() {
        let mut overlay = McpOverlay::new();
        overlay.show(vec![
            entry("alpha", McpServerStatusView::Running, true, 3),
            entry("beta", McpServerStatusView::Stopped, false, 0),
        ]);
        assert!(overlay.is_visible());
        assert_eq!(overlay.servers().len(), 2);
        assert_eq!(overlay.selected_index(), Some(0));
        // Render into a test buffer to ensure no panic and selection drawn.
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| overlay.render(f.area(), f))
            .expect("render");
    }

    #[test]
    fn j_and_k_navigate_selection() {
        let mut overlay = McpOverlay::new();
        overlay.show(vec![
            entry("alpha", McpServerStatusView::Running, false, 1),
            entry("beta", McpServerStatusView::Stopped, false, 0),
            entry("gamma", McpServerStatusView::Failed, false, 0),
        ]);
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(2));
        // Down again clamps at last index.
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Down));
        assert_eq!(overlay.selected_index(), Some(2));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Up));
        assert_eq!(overlay.selected_index(), Some(0));
        // Up at top stays at 0.
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(0));
    }

    #[test]
    fn enter_starts_stopped_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(vec![entry("beta", McpServerStatusView::Stopped, false, 0)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            Command::StartMcpServer { server_id } if server_id == "beta"
        ));
    }

    #[test]
    fn enter_stops_running_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(vec![entry("alpha", McpServerStatusView::Running, true, 5)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            Command::StopMcpServer { server_id } if server_id == "alpha"
        ));
    }

    #[test]
    fn enter_starts_failed_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(vec![entry("crash", McpServerStatusView::Failed, false, 0)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::StartMcpServer { server_id } if server_id == "crash"
        ));
    }

    #[test]
    fn t_emits_trust_command_for_selected_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(vec![
            entry("alpha", McpServerStatusView::Running, false, 1),
            entry("beta", McpServerStatusView::Running, false, 1),
        ]);
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
        assert!(matches!(
            &commands[0],
            Command::TrustMcpServer { server_id } if server_id == "beta"
        ));
    }

    #[test]
    fn r_emits_refresh_tools_command() {
        let mut overlay = McpOverlay::new();
        overlay.show(vec![entry("alpha", McpServerStatusView::Running, false, 1)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
        assert!(matches!(
            &commands[0],
            Command::RefreshMcpTools { server_id } if server_id == "alpha"
        ));
    }

    #[test]
    fn esc_hides_and_emits_dismiss_effect() {
        let mut overlay = McpOverlay::new();
        overlay.show(vec![entry("alpha", McpServerStatusView::Running, false, 1)]);
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
        assert!(commands.is_empty());
        assert!(effects.contains(&CrossPanelEffect::DismissMcpOverlay));
        assert!(!overlay.is_visible());
    }

    #[test]
    fn ignores_keys_when_hidden() {
        let mut overlay = McpOverlay::new();
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
    }

    #[test]
    fn show_effect_makes_visible() {
        let mut overlay = McpOverlay::new();
        overlay.handle_effect(&CrossPanelEffect::ShowMcpOverlay(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            false,
            1,
        )]));
        assert!(overlay.is_visible());
        assert_eq!(overlay.servers().len(), 1);
    }

    #[test]
    fn enter_with_no_servers_emits_nothing() {
        let mut overlay = McpOverlay::new();
        overlay.show(Vec::new());
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
    }
}
