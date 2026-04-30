//! ChatPanel component — message display and text input for the interactive TUI.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app_state::{InputMode, InputState};
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, PermissionRequest, RiskLevel,
};
use crate::keybindings::KeyAction;

// ---------------------------------------------------------------------------
// ChatPanel
// ---------------------------------------------------------------------------

/// The main chat panel: displays messages from a [`SessionProjection`] and
/// handles text input, history navigation, and permission decisions.
pub struct ChatPanel {
    focused: bool,
    pub input_content: String,
    pub input_cursor: usize,
    pub input_mode: InputMode,
    pub input_state: InputState,
    pub input_history: Vec<String>,
    /// `None` means we're at the "live" position (not browsing history).
    pub input_history_index: Option<usize>,
    pub scroll_offset: usize,
}

impl ChatPanel {
    pub fn new() -> Self {
        Self {
            focused: false,
            input_content: String::new(),
            input_cursor: 0,
            input_mode: InputMode::SingleLine,
            input_state: InputState::Normal,
            input_history: Vec::new(),
            input_history_index: None,
            scroll_offset: 0,
        }
    }

    /// Central input-handling method. Resolves a [`KeyAction`] produced by
    /// the keybinding layer and returns (cross-panel effects, commands).
    pub fn apply_key_action(
        &mut self,
        action: KeyAction,
        ctx: &EventContext,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match action {
            // -- Send ----------------------------------------------------------
            KeyAction::SendInput => {
                if !self.input_content.is_empty() {
                    self.input_history.push(self.input_content.clone());
                    let content = std::mem::take(&mut self.input_content);
                    self.input_cursor = 0;
                    self.input_history_index = None;

                    if let Some(session) = ctx.sessions.first() {
                        commands.push(Command::SendMessage {
                            workspace_id: agent_core::WorkspaceId::new(),
                            session_id: session.id.clone(),
                            content,
                        });
                    }
                }
            }

            // -- Character input -----------------------------------------------
            KeyAction::InputCharacter(c) => {
                self.input_content.insert(self.input_cursor, c);
                self.input_cursor += c.len_utf8();
            }

            // -- Backspace ------------------------------------------------------
            KeyAction::InputBackspace => {
                if self.input_cursor > 0 {
                    let prev = prev_char_boundary(&self.input_content, self.input_cursor);
                    self.input_content.drain(prev..self.input_cursor);
                    self.input_cursor = prev;
                }
            }

            // -- Newline (multi-line only) ------------------------------------
            KeyAction::InputNewline => {
                if self.input_mode == InputMode::MultiLine {
                    self.input_content.insert(self.input_cursor, '\n');
                    self.input_cursor += 1;
                }
            }

            // -- Toggle input mode ---------------------------------------------
            KeyAction::ToggleInputMode => {
                self.input_mode = match self.input_mode {
                    InputMode::SingleLine => InputMode::MultiLine,
                    InputMode::MultiLine => InputMode::SingleLine,
                };
            }

            // -- History navigation --------------------------------------------
            KeyAction::InputHistoryUp => {
                if !self.input_history.is_empty() {
                    let idx = match self.input_history_index {
                        Some(i) if i > 0 => i - 1,
                        Some(i) => i,
                        None => self.input_history.len() - 1,
                    };
                    self.input_history_index = Some(idx);
                    self.input_content = self.input_history[idx].clone();
                    self.input_cursor = self.input_content.len();
                }
            }

            KeyAction::InputHistoryDown => {
                if let Some(i) = self.input_history_index {
                    if i + 1 < self.input_history.len() {
                        let next = i + 1;
                        self.input_history_index = Some(next);
                        self.input_content = self.input_history[next].clone();
                    } else {
                        self.input_history_index = None;
                        self.input_content.clear();
                    }
                    self.input_cursor = self.input_content.len();
                }
            }

            // -- Permission decisions ------------------------------------------
            KeyAction::AllowPermission => {
                if let InputState::PermissionWait { request_id, .. } = &self.input_state {
                    let rid = request_id.clone();
                    self.input_state = InputState::Normal;
                    commands.push(Command::DecidePermission {
                        request_id: rid,
                        approved: true,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                }
            }

            KeyAction::DenyPermission => {
                if let InputState::PermissionWait { request_id, .. } = &self.input_state {
                    let rid = request_id.clone();
                    self.input_state = InputState::Normal;
                    commands.push(Command::DecidePermission {
                        request_id: rid,
                        approved: false,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                }
            }

            KeyAction::DenyAllPermission => {
                if let InputState::PermissionWait { request_id, .. } = &self.input_state {
                    let rid = request_id.clone();
                    self.input_state = InputState::Normal;
                    commands.push(Command::DecidePermission {
                        request_id: rid,
                        approved: false,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                }
            }

            // -- Escape --------------------------------------------------------
            KeyAction::Escape => {
                if self.input_mode == InputMode::MultiLine && self.input_content.is_empty() {
                    self.input_mode = InputMode::SingleLine;
                }
            }

            // -- Paste ---------------------------------------------------------
            KeyAction::InputPaste(text) => {
                for c in text.chars() {
                    self.input_content.insert(self.input_cursor, c);
                    self.input_cursor += c.len_utf8();
                }
            }

            // All other actions are not handled by ChatPanel.
            _ => {}
        }

        (effects, commands)
    }
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ChatPanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let permission_pending = matches!(self.input_state, InputState::PermissionWait { .. });
        let action = crate::keybindings::resolve_key(
            match event {
                crossterm::event::Event::Key(ke) => *ke,
                _ => return (Vec::new(), Vec::new()),
            },
            FocusTarget::Chat,
            permission_pending,
            self.input_mode,
        );
        self.apply_key_action(action, ctx)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowPermissionPrompt(req) => {
                // Only handle Write-level risks in ChatPanel.
                // Destructive risks are handled by PermissionModal.
                if req.risk_level == RiskLevel::Write {
                    self.input_state = InputState::PermissionWait {
                        request_id: req.request_id.clone(),
                        pending_prompt: req.tool_preview.clone(),
                    };
                }
            }
            CrossPanelEffect::DismissPermissionPrompt => {
                if matches!(self.input_state, InputState::PermissionWait { .. }) {
                    self.input_state = InputState::Normal;
                }
            }
            CrossPanelEffect::StartStreaming | CrossPanelEffect::StopStreaming => {
                // No-op for now; will be wired to RenderScheduler later.
            }
            _ => {}
        }
    }

    /// Placeholder — the App layer handles ChatPanel rendering centrally
    /// via [`render_messages`].
    fn render(&self, _area: Rect, _frame: &mut Frame) {}

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

// ---------------------------------------------------------------------------
// Standalone render function
// ---------------------------------------------------------------------------

/// Render the message list from a [`SessionProjection`] into the given area.
///
/// - User messages are prefixed with a cyan `"You:"`.
/// - Assistant messages are prefixed with a green `"Agent:"`.
/// - If the session was cancelled, a yellow `[cancelled]` line is shown.
/// - If `token_stream` is non-empty, the streaming text is shown with a `▌`
///   block cursor appended.
pub fn render_messages(
    area: Rect,
    frame: &mut Frame,
    projection: &agent_core::projection::SessionProjection,
) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &projection.messages {
        let (label, color) = match msg.role {
            agent_core::projection::ProjectedRole::User => ("You:", Color::Cyan),
            agent_core::projection::ProjectedRole::Assistant => ("Agent:", Color::Green),
        };

        // Render each line of the message content with the prefix on the first line.
        let content_lines: Vec<&str> = msg.content.split('\n').collect();
        for (i, line) in content_lines.iter().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", label), Style::default().fg(color)),
                    Span::raw(*line),
                ]));
            } else {
                lines.push(Line::raw(*line));
            }
        }
    }

    // Streaming indicator
    if !projection.token_stream.is_empty() {
        let stream_text = format!("{}▌", projection.token_stream);
        lines.push(Line::from(vec![
            Span::styled("Agent: ", Style::default().fg(Color::Green)),
            Span::raw(stream_text),
        ]));
    }

    // Cancelled indicator
    if projection.cancelled {
        lines.push(Line::from(Span::styled(
            "[cancelled]",
            Style::default().fg(Color::Yellow),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the byte index of the previous character boundary before `pos`.
fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut i = pos - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{EventContext, SessionInfo, SessionState};
    use std::sync::OnceLock;

    /// Shared static [`EventContext`] for tests. We leak the owned data so
    /// that the references inside `EventContext` can be `'static`.
    static TEST_CTX: OnceLock<EventContext<'static>> = OnceLock::new();

    fn test_ctx() -> &'static EventContext<'static> {
        TEST_CTX.get_or_init(|| {
            let projection = Box::leak(Box::new(
                agent_core::projection::SessionProjection::default(),
            ));
            let sessions: &[SessionInfo] = Box::leak(Vec::<SessionInfo>::new().into_boxed_slice());
            EventContext {
                focus: FocusTarget::Chat,
                current_session: projection,
                sessions,
                model_profile: "test",
                permission_mode: agent_tools::PermissionMode::Suggest,
                sidebar_left_visible: true,
                sidebar_right_visible: false,
            }
        })
    }

    // A variant with sessions so SendMessage can be emitted.
    static TEST_CTX_WITH_SESSION: OnceLock<EventContext<'static>> = OnceLock::new();

    fn test_ctx_with_session() -> &'static EventContext<'static> {
        TEST_CTX_WITH_SESSION.get_or_init(|| {
            let projection = Box::leak(Box::new(
                agent_core::projection::SessionProjection::default(),
            ));
            let sessions: &[SessionInfo] = Box::leak(
                vec![SessionInfo {
                    id: agent_core::SessionId::new(),
                    title: "test session".to_string(),
                    model_profile: "fast".to_string(),
                    state: SessionState::Active,
                    pinned: false,
                }]
                .into_boxed_slice(),
            );
            EventContext {
                focus: FocusTarget::Chat,
                current_session: projection,
                sessions,
                model_profile: "test",
                permission_mode: agent_tools::PermissionMode::Suggest,
                sidebar_left_visible: true,
                sidebar_right_visible: false,
            }
        })
    }

    #[test]
    fn input_character_appends_to_content() {
        let mut panel = ChatPanel::new();
        let (effects, cmds) = panel.apply_key_action(KeyAction::InputCharacter('a'), test_ctx());
        assert!(effects.is_empty());
        assert!(cmds.is_empty());
        assert_eq!(panel.input_content, "a");
        assert_eq!(panel.input_cursor, 1);

        panel.apply_key_action(KeyAction::InputCharacter('b'), test_ctx());
        assert_eq!(panel.input_content, "ab");
        assert_eq!(panel.input_cursor, 2);
    }

    #[test]
    fn backspace_removes_character() {
        let mut panel = ChatPanel::new();
        panel.apply_key_action(KeyAction::InputCharacter('x'), test_ctx());
        panel.apply_key_action(KeyAction::InputCharacter('y'), test_ctx());
        assert_eq!(panel.input_content, "xy");
        assert_eq!(panel.input_cursor, 2);

        let (effects, cmds) = panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
        assert!(effects.is_empty());
        assert!(cmds.is_empty());
        assert_eq!(panel.input_content, "x");
        assert_eq!(panel.input_cursor, 1);

        // Backspace at start does nothing
        panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
        assert_eq!(panel.input_content, "");
        assert_eq!(panel.input_cursor, 0);
        panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
        assert_eq!(panel.input_content, "");
        assert_eq!(panel.input_cursor, 0);
    }

    #[test]
    fn toggle_input_mode_switches() {
        let mut panel = ChatPanel::new();
        assert_eq!(panel.input_mode, InputMode::SingleLine);

        let (effects, cmds) = panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
        assert!(effects.is_empty());
        assert!(cmds.is_empty());
        assert_eq!(panel.input_mode, InputMode::MultiLine);

        panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
        assert_eq!(panel.input_mode, InputMode::SingleLine);
    }

    #[test]
    fn permission_wait_state_allows_deny() {
        let mut panel = ChatPanel::new();
        panel.input_state = InputState::PermissionWait {
            request_id: "req-1".to_string(),
            pending_prompt: "rm file?".to_string(),
        };

        let (effects, cmds) = panel.apply_key_action(KeyAction::DenyPermission, test_ctx());
        assert_eq!(effects, vec![CrossPanelEffect::DismissPermissionPrompt]);
        assert_eq!(
            cmds,
            vec![Command::DecidePermission {
                request_id: "req-1".to_string(),
                approved: false,
            }]
        );
        assert_eq!(panel.input_state, InputState::Normal);

        // Allow also works
        panel.input_state = InputState::PermissionWait {
            request_id: "req-2".to_string(),
            pending_prompt: "write file?".to_string(),
        };
        let (effects2, cmds2) = panel.apply_key_action(KeyAction::AllowPermission, test_ctx());
        assert_eq!(effects2, vec![CrossPanelEffect::DismissPermissionPrompt]);
        assert_eq!(
            cmds2,
            vec![Command::DecidePermission {
                request_id: "req-2".to_string(),
                approved: true,
            }]
        );
        assert_eq!(panel.input_state, InputState::Normal);

        // DenyAllPermission also resolves to deny
        panel.input_state = InputState::PermissionWait {
            request_id: "req-3".to_string(),
            pending_prompt: "run cmd?".to_string(),
        };
        let (effects3, cmds3) = panel.apply_key_action(KeyAction::DenyAllPermission, test_ctx());
        assert_eq!(effects3, vec![CrossPanelEffect::DismissPermissionPrompt]);
        assert_eq!(
            cmds3,
            vec![Command::DecidePermission {
                request_id: "req-3".to_string(),
                approved: false,
            }]
        );
        assert_eq!(panel.input_state, InputState::Normal);
    }

    #[test]
    fn history_navigation_works() {
        let mut panel = ChatPanel::new();

        // Pre-populate history
        panel.input_history = vec![
            "first message".to_string(),
            "second message".to_string(),
            "third message".to_string(),
        ];

        // HistoryUp from live position -> goes to most recent (index 2)
        let (effects, cmds) = panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
        assert!(effects.is_empty());
        assert!(cmds.is_empty());
        assert_eq!(panel.input_history_index, Some(2));
        assert_eq!(panel.input_content, "third message");
        assert_eq!(panel.input_cursor, "third message".len());

        // HistoryUp again -> index 1
        panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
        assert_eq!(panel.input_history_index, Some(1));
        assert_eq!(panel.input_content, "second message");

        // HistoryUp again -> index 0 (oldest)
        panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
        assert_eq!(panel.input_history_index, Some(0));
        assert_eq!(panel.input_content, "first message");

        // HistoryUp at oldest -> stays at index 0
        panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
        assert_eq!(panel.input_history_index, Some(0));
        assert_eq!(panel.input_content, "first message");

        // HistoryDown -> index 1
        panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
        assert_eq!(panel.input_history_index, Some(1));
        assert_eq!(panel.input_content, "second message");

        // HistoryDown to index 2
        panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
        assert_eq!(panel.input_history_index, Some(2));
        assert_eq!(panel.input_content, "third message");

        // HistoryDown from index 2 -> back to live position
        panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
        assert_eq!(panel.input_history_index, None);
        assert_eq!(panel.input_content, "");
    }

    #[test]
    fn send_input_clears_content_and_emits_command() {
        let mut panel = ChatPanel::new();
        // Type some content
        panel.apply_key_action(KeyAction::InputCharacter('h'), test_ctx());
        panel.apply_key_action(KeyAction::InputCharacter('i'), test_ctx());
        assert_eq!(panel.input_content, "hi");

        // Send
        let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
        assert!(effects.is_empty());
        assert_eq!(cmds.len(), 1);

        match &cmds[0] {
            Command::SendMessage { content, .. } => assert_eq!(content, "hi"),
            other => panic!("expected SendMessage, got {:?}", other),
        }

        // Content should be cleared
        assert_eq!(panel.input_content, "");
        assert_eq!(panel.input_cursor, 0);

        // History should contain the sent message
        assert_eq!(panel.input_history, vec!["hi"]);

        // History index should be reset
        assert_eq!(panel.input_history_index, None);
    }

    #[test]
    fn send_input_empty_does_nothing() {
        let mut panel = ChatPanel::new();
        let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
        assert!(effects.is_empty());
        assert!(cmds.is_empty());
    }

    #[test]
    fn send_input_no_sessions_no_command() {
        let mut panel = ChatPanel::new();
        panel.apply_key_action(KeyAction::InputCharacter('x'), test_ctx());
        let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx());
        // No sessions -> no command emitted, but content is still consumed
        assert!(effects.is_empty());
        assert!(cmds.is_empty());
        // Content is still pushed to history and cleared
        assert_eq!(panel.input_content, "");
        assert_eq!(panel.input_history, vec!["x"]);
    }

    #[test]
    fn escape_multiline_empty_switches_to_singleline() {
        let mut panel = ChatPanel::new();
        panel.input_mode = InputMode::MultiLine;
        assert!(panel.input_content.is_empty());

        let (effects, cmds) = panel.apply_key_action(KeyAction::Escape, test_ctx());
        assert!(effects.is_empty());
        assert!(cmds.is_empty());
        assert_eq!(panel.input_mode, InputMode::SingleLine);
    }

    #[test]
    fn escape_multiline_nonempty_does_not_switch() {
        let mut panel = ChatPanel::new();
        panel.input_mode = InputMode::MultiLine;
        panel.apply_key_action(KeyAction::InputCharacter('a'), test_ctx());

        panel.apply_key_action(KeyAction::Escape, test_ctx());
        assert_eq!(panel.input_mode, InputMode::MultiLine);
    }

    #[test]
    fn newline_only_in_multiline() {
        let mut panel = ChatPanel::new();
        // SingleLine: newline is a no-op
        panel.apply_key_action(KeyAction::InputNewline, test_ctx());
        assert_eq!(panel.input_content, "");

        // Switch to MultiLine
        panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
        panel.apply_key_action(KeyAction::InputNewline, test_ctx());
        assert_eq!(panel.input_content, "\n");
        assert_eq!(panel.input_cursor, 1);
    }

    #[test]
    fn handle_effect_show_permission_prompt_write_level() {
        let mut panel = ChatPanel::new();
        let req = PermissionRequest {
            request_id: "r1".to_string(),
            tool_id: "write_file".to_string(),
            tool_preview: "write to foo.txt".to_string(),
            risk_level: RiskLevel::Write,
        };
        panel.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(req));
        assert!(matches!(
            panel.input_state,
            InputState::PermissionWait { ref request_id, .. } if request_id == "r1"
        ));
    }

    #[test]
    fn handle_effect_show_permission_prompt_destructive_ignored() {
        let mut panel = ChatPanel::new();
        let req = PermissionRequest {
            request_id: "r2".to_string(),
            tool_id: "delete_file".to_string(),
            tool_preview: "rm -rf /".to_string(),
            risk_level: RiskLevel::Destructive,
        };
        panel.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(req));
        assert_eq!(panel.input_state, InputState::Normal);
    }

    #[test]
    fn handle_effect_dismiss_permission_prompt() {
        let mut panel = ChatPanel::new();
        panel.input_state = InputState::PermissionWait {
            request_id: "r1".to_string(),
            pending_prompt: "test".to_string(),
        };
        panel.handle_effect(&CrossPanelEffect::DismissPermissionPrompt);
        assert_eq!(panel.input_state, InputState::Normal);
    }

    #[test]
    fn handle_effect_start_stop_streaming_noop() {
        let mut panel = ChatPanel::new();
        panel.handle_effect(&CrossPanelEffect::StartStreaming);
        panel.handle_effect(&CrossPanelEffect::StopStreaming);
        // Just verifying no panic and state unchanged.
        assert_eq!(panel.input_state, InputState::Normal);
    }

    #[test]
    fn render_messages_basic() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let projection = agent_core::projection::SessionProjection {
            messages: vec![
                agent_core::projection::ProjectedMessage {
                    role: agent_core::projection::ProjectedRole::User,
                    content: "hello".to_string(),
                },
                agent_core::projection::ProjectedMessage {
                    role: agent_core::projection::ProjectedRole::Assistant,
                    content: "world".to_string(),
                },
            ],
            task_titles: vec![],
            token_stream: String::new(),
            cancelled: false,
        };

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_messages(frame.area(), frame, &projection);
            })
            .expect("render_messages should not panic");
    }

    #[test]
    fn render_messages_with_streaming_and_cancelled() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let projection = agent_core::projection::SessionProjection {
            messages: vec![agent_core::projection::ProjectedMessage {
                role: agent_core::projection::ProjectedRole::User,
                content: "go".to_string(),
            }],
            task_titles: vec![],
            token_stream: "thinking".to_string(),
            cancelled: true,
        };

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_messages(frame.area(), frame, &projection);
            })
            .expect("render_messages should not panic");
    }

    #[test]
    fn focused_and_set_focused() {
        let mut panel = ChatPanel::new();
        assert!(!panel.focused());
        panel.set_focused(true);
        assert!(panel.focused());
        panel.set_focused(false);
        assert!(!panel.focused());
    }
}
