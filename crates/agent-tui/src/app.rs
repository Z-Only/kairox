//! App — Component composition and event routing for the interactive TUI.

use agent_core::projection::ProjectedMessage;
use agent_core::{DomainEvent, EventPayload, SessionId, WorkspaceId};
use agent_tools::PermissionMode;
use crossterm::event::Event;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app_state::{AppState, CtrlCAction, InputMode, InputState};
use crate::components::chat::ChatPanel;
use crate::components::permission_modal::PermissionModal;
use crate::components::sessions::SessionsPanel;
use crate::components::status_bar::{PermissionModeExt, StatusBar};
use crate::components::trace::TracePanel;
use crate::components::{
    Command, Component, CrossPanelEffect, FocusTarget, PermissionRequest, RiskLevel, SessionState,
};
use crate::keybindings::{resolve_key, resolve_paste, KeyAction};

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub state: AppState,
    pub chat: ChatPanel,
    pub sessions: SessionsPanel,
    pub trace: TracePanel,
    pub status_bar: StatusBar,
    pub permission_modal: PermissionModal,
    pub workspace_id: WorkspaceId,
    pub current_session_id: Option<SessionId>,
    pub domain_events: Vec<DomainEvent>,
    pub quit_confirmed: bool,
    pub quitting: bool,
}

impl App {
    pub fn new(
        model_profile: &str,
        permission_mode: PermissionMode,
        workspace_id: WorkspaceId,
    ) -> Self {
        Self {
            state: AppState::new(model_profile, permission_mode),
            chat: ChatPanel::new(),
            sessions: SessionsPanel::new(),
            trace: TracePanel::new(),
            status_bar: StatusBar::new(),
            permission_modal: PermissionModal::new(),
            workspace_id,
            current_session_id: None,
            domain_events: Vec::new(),
            quit_confirmed: false,
            quitting: false,
        }
    }

    // -----------------------------------------------------------------------
    // Event handling
    // -----------------------------------------------------------------------

    /// Handle a raw crossterm event, returning any commands to dispatch.
    pub fn handle_crossterm_event(&mut self, event: &Event) -> Vec<Command> {
        match event {
            Event::Key(key_event) => {
                let permission_pending =
                    matches!(self.state.input_state, InputState::PermissionWait { .. })
                        || self.permission_modal.is_visible();
                let action = resolve_key(
                    *key_event,
                    self.state.focus_manager.current(),
                    permission_pending,
                    self.state.input_mode,
                );
                self.apply_action(action)
            }
            Event::Resize(_, _) => {
                self.state.render_scheduler.mark_dirty_immediate();
                Vec::new()
            }
            Event::Paste(text) => {
                if text.contains('\n') && self.state.input_mode == InputMode::SingleLine {
                    self.state.input_mode = InputMode::MultiLine;
                    self.chat.input_mode = InputMode::MultiLine;
                }
                let action = resolve_paste(text.clone());
                self.apply_action(action)
            }
            _ => Vec::new(),
        }
    }

    /// Route a resolved key action, returning any commands to dispatch.
    pub fn apply_action(&mut self, action: KeyAction) -> Vec<Command> {
        let mut commands = Vec::new();

        match action {
            // -- Ctrl-C progressive exit -----------------------------------
            KeyAction::InterruptOrQuit => match self.state.record_ctrl_c() {
                CtrlCAction::Interrupt => {
                    if let Some(session_id) = &self.current_session_id {
                        commands.push(Command::CancelSession {
                            workspace_id: self.workspace_id.clone(),
                            session_id: session_id.clone(),
                        });
                    }
                    self.state.render_scheduler.mark_dirty();
                }
                CtrlCAction::ConfirmQuit => {
                    self.quit_confirmed = true;
                    self.state.render_scheduler.mark_dirty();
                }
                CtrlCAction::ForceQuit => {
                    self.quitting = true;
                }
            },

            // -- Quit (Alt+Q) ---------------------------------------------
            KeyAction::Quit => {
                self.quit_confirmed = true;
                self.state.render_scheduler.mark_dirty();
            }

            // -- Escape ----------------------------------------------------
            KeyAction::Escape => {
                if self.quit_confirmed {
                    self.quit_confirmed = false;
                    self.state.reset_ctrl_c();
                    self.state.render_scheduler.mark_dirty();
                }
                // Also delegate to chat for input-mode escape handling
                let (effects, cmds) = {
                    let focus = self.state.focus_manager.current();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let permission_mode = self.state.permission_mode;
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let ctx = crate::components::EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        permission_mode,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                    };
                    self.chat.apply_key_action(KeyAction::Escape, &ctx)
                };
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }

            // -- Sidebar toggles ------------------------------------------
            KeyAction::ToggleSessionsSidebar => {
                self.state.sidebar_left_visible = !self.state.sidebar_left_visible;
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::ToggleTraceSidebar => {
                self.state.sidebar_right_visible = !self.state.sidebar_right_visible;
                self.state.render_scheduler.mark_dirty_immediate();
            }

            // -- Focus management -----------------------------------------
            KeyAction::FocusCycleNext => {
                self.state.focus_manager.cycle_next();
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::FocusChat => {
                self.state.focus_manager.set(FocusTarget::Chat);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::FocusSessions => {
                self.state.focus_manager.set(FocusTarget::Sessions);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::FocusTrace => {
                self.state.focus_manager.set(FocusTarget::Trace);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty();
            }

            // -- Redraw ----------------------------------------------------
            KeyAction::Redraw => {
                self.state.render_scheduler.mark_dirty_immediate();
            }

            // -- Trace density toggle -------------------------------------
            KeyAction::ToggleTraceDensity => {
                self.trace.density = self.trace.density.next();
                self.state.render_scheduler.mark_dirty();
            }

            // -- New session ----------------------------------------------
            KeyAction::NewSession => {
                commands.push(Command::StartSession {
                    workspace_id: self.workspace_id.clone(),
                    model_profile: self.state.model_profile.clone(),
                });
            }

            // -- All other input/permission keys -> delegate to chat ------
            KeyAction::SendInput
            | KeyAction::InputCharacter(_)
            | KeyAction::InputBackspace
            | KeyAction::InputDelete
            | KeyAction::InputNewline
            | KeyAction::ToggleInputMode
            | KeyAction::InputHistoryUp
            | KeyAction::InputHistoryDown
            | KeyAction::InputPaste(_)
            | KeyAction::AllowPermission
            | KeyAction::DenyPermission
            | KeyAction::DenyAllPermission
            | KeyAction::ContextMenu => {
                let (effects, cmds) = {
                    let focus = self.state.focus_manager.current();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let permission_mode = self.state.permission_mode;
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let ctx = crate::components::EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        permission_mode,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                    };
                    self.chat.apply_key_action(action, &ctx)
                };
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }

            // Scroll actions
            KeyAction::ScrollUp | KeyAction::ScrollDown => {
                self.state.render_scheduler.mark_dirty();
            }

            // Help, profile selector, rename session — future
            KeyAction::Help
            | KeyAction::OpenProfileSelector
            | KeyAction::RenameSession
            | KeyAction::Unhandled => {}
        }

        commands
    }

    // -----------------------------------------------------------------------
    // Domain events
    // -----------------------------------------------------------------------

    /// Process a domain event from the runtime, updating projection and state.
    pub fn handle_domain_event(&mut self, event: &DomainEvent) {
        let mut effects = Vec::new();

        match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => {
                self.state.current_session.messages.push(ProjectedMessage {
                    role: agent_core::projection::ProjectedRole::User,
                    content: content.clone(),
                });
                self.state.render_scheduler.mark_dirty();
            }

            EventPayload::ModelTokenDelta { delta } => {
                self.state.current_session.token_stream.push_str(delta);
                self.state.render_scheduler.mark_dirty();
                self.state.render_scheduler.set_streaming(true);
                self.state.render_scheduler.add_tokens(delta.len());
            }

            EventPayload::AssistantMessageCompleted { content, .. } => {
                self.state.current_session.messages.push(ProjectedMessage {
                    role: agent_core::projection::ProjectedRole::Assistant,
                    content: content.clone(),
                });
                self.state.current_session.token_stream.clear();
                self.state.render_scheduler.set_streaming(false);
                self.state.render_scheduler.mark_dirty();
                effects.push(CrossPanelEffect::StopStreaming);
            }

            EventPayload::SessionCancelled { .. } => {
                self.state.current_session.cancelled = true;
                self.state.render_scheduler.mark_dirty();
            }

            EventPayload::ToolInvocationStarted { .. } => {
                self.state.render_scheduler.mark_dirty();
                if let Some(session) = self.state.sessions.first_mut() {
                    session.state = SessionState::Active;
                }
            }

            EventPayload::ToolInvocationCompleted { .. } => {
                self.state.render_scheduler.mark_dirty();
                if let Some(session) = self.state.sessions.first_mut() {
                    session.state = SessionState::Idle;
                }
            }

            EventPayload::ToolInvocationFailed { .. } => {
                self.state.render_scheduler.mark_dirty();
                if let Some(session) = self.state.sessions.first_mut() {
                    session.state = SessionState::Idle;
                }
            }

            EventPayload::PermissionRequested {
                request_id,
                tool_id,
                preview,
            } => {
                // Classify risk: use Write for everything for now
                let risk_level = RiskLevel::Write;
                let req = PermissionRequest {
                    request_id: request_id.clone(),
                    tool_id: tool_id.clone(),
                    tool_preview: preview.clone(),
                    risk_level: risk_level.clone(),
                };
                effects.push(CrossPanelEffect::ShowPermissionPrompt(req));

                // For Destructive risks, push PermissionModal focus
                if risk_level == RiskLevel::Destructive {
                    self.state.focus_manager.push(FocusTarget::PermissionModal);
                    self.sync_component_focus();
                }

                if let Some(session) = self.state.sessions.first_mut() {
                    session.state = SessionState::AwaitingPermission;
                }
                self.state.render_scheduler.mark_dirty();
            }

            EventPayload::PermissionGranted { .. } | EventPayload::PermissionDenied { .. } => {
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
                if self.state.focus_manager.current() == FocusTarget::PermissionModal {
                    self.state.focus_manager.pop();
                    self.sync_component_focus();
                }
                if let Some(session) = self.state.sessions.first_mut() {
                    session.state = SessionState::Active;
                }
                self.state.render_scheduler.mark_dirty();
            }

            EventPayload::AgentTaskCreated { title, .. } => {
                if let Some(session) = self.state.sessions.first_mut() {
                    if session.title.starts_with("Session using ") {
                        session.title = title.clone();
                    }
                }
                self.state.current_session.task_titles.push(title.clone());
                self.state.render_scheduler.mark_dirty();
            }

            // All other events — just mark dirty
            _ => {
                self.state.render_scheduler.mark_dirty();
            }
        }

        // Store event for trace extraction
        self.domain_events.push(event.clone());

        // Dispatch effects to all components
        self.dispatch_effects(effects);

        // Update status bar
        self.sync_status_bar();
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Split: main area + status bar (1 row at bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        let main_area = chunks[0];
        let status_area = chunks[1];

        // Split main area horizontally: sessions | chat | trace
        let mut constraints = Vec::new();
        let sessions_visible = self.state.sidebar_left_visible;
        let trace_visible = self.state.sidebar_right_visible;

        if sessions_visible {
            constraints.push(Constraint::Length(24));
        }
        constraints.push(Constraint::Min(20));
        if trace_visible {
            constraints.push(Constraint::Length(32));
        }

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(main_area);

        let mut chunk_idx = 0;

        // Sessions sidebar
        let sessions_area = if sessions_visible {
            let a = main_chunks[chunk_idx];
            chunk_idx += 1;
            Some(a)
        } else {
            None
        };

        // Chat panel (messages + input)
        let chat_area = main_chunks[chunk_idx];
        chunk_idx += 1;

        // Trace sidebar
        let trace_area = if trace_visible {
            let a = main_chunks[chunk_idx];
            chunk_idx += 1;
            Some(a)
        } else {
            None
        };

        // Render sessions
        if let Some(sessions_area) = sessions_area {
            crate::components::sessions::render_sessions(
                sessions_area,
                frame,
                &self.state.sessions,
                self.sessions.focused(),
            );
        }

        // Render chat (messages + input)
        let chat_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(chat_area);
        let messages_area = chat_chunks[0];
        let input_area = chat_chunks[1];

        // Messages
        crate::components::chat::render_messages(messages_area, frame, &self.state.current_session);

        // Input area
        self.render_input(input_area, frame);

        // Render trace
        if let Some(trace_area) = trace_area {
            let traces = crate::components::trace::extract_tool_traces(&self.domain_events);
            crate::components::trace::render_trace_l1(
                trace_area,
                frame,
                &traces,
                self.trace.focused(),
            );
        }

        // Render status bar
        self.status_bar.render(status_area, frame);

        // Render permission modal overlay on top of everything
        if self.permission_modal.is_visible() {
            self.permission_modal.render(area, frame);
        }

        // Mark render as done
        self.state.render_scheduler.did_render();
    }

    fn render_input(&self, area: Rect, frame: &mut Frame) {
        let is_focused = self.state.focus_manager.current() == FocusTarget::Chat;

        let border_style = if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mode_label = match self.state.input_mode {
            InputMode::SingleLine => "│ ",
            InputMode::MultiLine => "│M ",
        };

        // Permission-wait state override
        let display_content =
            if let InputState::PermissionWait { pending_prompt, .. } = &self.state.input_state {
                format!("[permission] {}", pending_prompt)
            } else {
                let mut content = format!("{}{}", mode_label, self.chat.input_content);
                // Show streaming cursor when in streaming mode
                if self.state.render_scheduler.is_streaming() {
                    content.push('▌');
                }
                content
            };

        let input_line = Line::from(vec![
            Span::styled(
                if is_focused { ">" } else { " " },
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::raw(display_content),
        ]);

        let paragraph = Paragraph::new(input_line).block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::NONE)
                .style(Style::default()),
        );
        frame.render_widget(paragraph, area);

        // Show a thin border around the input area
        let border_block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::TOP)
            .border_style(border_style);
        frame.render_widget(
            ratatui::widgets::Paragraph::new("").block(border_block),
            area,
        );
    }

    // -----------------------------------------------------------------------
    // Cross-panel effects
    // -----------------------------------------------------------------------

    /// Fan-out cross-panel effects to all components.
    pub fn dispatch_effects(&mut self, effects: Vec<CrossPanelEffect>) {
        for effect in effects {
            self.chat.handle_effect(&effect);
            self.sessions.handle_effect(&effect);
            self.trace.handle_effect(&effect);
            self.status_bar.handle_effect(&effect);
            self.permission_modal.handle_effect(&effect);
        }
    }

    // -----------------------------------------------------------------------
    // Focus sync
    // -----------------------------------------------------------------------

    /// Sync all components' focused states based on the current focus target.
    pub fn sync_component_focus(&mut self) {
        let current = self.state.focus_manager.current();
        self.chat.set_focused(current == FocusTarget::Chat);
        self.sessions.set_focused(current == FocusTarget::Sessions);
        self.trace.set_focused(current == FocusTarget::Trace);
        self.permission_modal
            .set_focused(current == FocusTarget::PermissionModal);
        self.status_bar.set_focused(false);
        self.state.render_scheduler.mark_dirty();
    }

    // -----------------------------------------------------------------------
    // Status bar sync
    // -----------------------------------------------------------------------

    pub fn sync_status_bar(&mut self) {
        let hint = if self.quit_confirmed {
            "Press Ctrl+C again to quit, or Esc to cancel".to_string()
        } else {
            "Alt+Q quit | Tab cycle | Alt+S sessions | Alt+T trace".to_string()
        };

        let info = crate::components::StatusInfo {
            profile: self.state.model_profile.clone(),
            permission_mode: self.state.permission_mode.as_str().to_string(),
            session_count: self.state.sessions.len(),
            hint,
            error: None,
        };
        self.status_bar
            .handle_effect(&CrossPanelEffect::SetStatus(info));
    }
}
