use crossterm::event::Event;

use crate::app_state::{CtrlCAction, InputMode, InputState};
use crate::components::{Command, CrossPanelEffect, EventContext, FocusTarget};
use crate::keybindings::{resolve_key, resolve_paste, KeyAction};

use super::App;

impl App {
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
            KeyAction::Quit => {
                self.quit_confirmed = true;
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::Escape => {
                if self.quit_confirmed {
                    self.quit_confirmed = false;
                    self.state.reset_ctrl_c();
                    self.state.render_scheduler.mark_dirty();
                }
                let (effects, cmds) = self.apply_chat_action(KeyAction::Escape);
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }
            KeyAction::ToggleSessionsSidebar => {
                self.state.sidebar_left_visible = !self.state.sidebar_left_visible;
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::ToggleTraceSidebar => {
                self.state.sidebar_right_visible = !self.state.sidebar_right_visible;
                self.state.render_scheduler.mark_dirty_immediate();
            }
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
            KeyAction::Redraw => {
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::ToggleTraceDensity => {
                self.trace.density = self.trace.density.next();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::NewSession => {
                commands.push(Command::StartSession {
                    workspace_id: self.workspace_id.clone(),
                    model_profile: self.state.model_profile.clone(),
                });
            }
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
                let (effects, cmds) = self.apply_chat_action(action);
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }
            KeyAction::SelectSession => {
                if let Some(session_id) = self.sessions.selected_session_id(&self.state.sessions) {
                    commands.push(Command::SwitchSession { session_id });
                }
            }
            KeyAction::ScrollUp => {
                if self.state.focus_manager.current() == FocusTarget::Sessions {
                    self.sessions.scroll_up(self.state.sessions.len());
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::ScrollDown => {
                if self.state.focus_manager.current() == FocusTarget::Sessions {
                    self.sessions.scroll_down(self.state.sessions.len());
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::Help
            | KeyAction::OpenProfileSelector
            | KeyAction::RenameSession
            | KeyAction::Unhandled => {}
        }

        commands
    }

    fn apply_chat_action(&mut self, action: KeyAction) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let focus = self.state.focus_manager.current();
        let sessions = self.state.sessions.clone();
        let model_profile = self.state.model_profile.clone();
        let permission_mode = self.state.permission_mode;
        let sidebar_left = self.state.sidebar_left_visible;
        let sidebar_right = self.state.sidebar_right_visible;
        let ctx = EventContext {
            focus,
            current_session: &self.state.current_session,
            sessions: &sessions,
            model_profile: &model_profile,
            permission_mode,
            sidebar_left_visible: sidebar_left,
            sidebar_right_visible: sidebar_right,
            workspace_id: &self.workspace_id,
            current_session_id: &self.current_session_id,
        };
        self.chat.apply_key_action(action, &ctx)
    }
}
