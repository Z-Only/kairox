use crate::app_state::{InputMode, InputState};
use crate::components::{Command, CrossPanelEffect, EventContext, QueuedMessage};
use crate::keybindings::KeyAction;

use super::{is_session_busy, ChatPanel};

impl ChatPanel {
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
            KeyAction::SendInput if !self.input_content.is_empty() => {
                let trimmed = self.input_content.trim();
                if trimmed == ":compact" {
                    self.clear_input();
                    if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::CompactSession {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                        });
                    }
                } else if let Some(alias) = trimmed
                    .strip_prefix(":model ")
                    .map(str::trim)
                    .filter(|a| !a.is_empty())
                {
                    let alias = alias.to_string();
                    self.clear_input();
                    if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::SwitchModel {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            alias,
                            reasoning_effort: None,
                        });
                    }
                } else if trimmed == ":skills" {
                    self.clear_input();
                    commands.push(Command::ListSkills);
                } else if let Some(skill_id) = trimmed
                    .strip_prefix(":skill show ")
                    .map(str::trim)
                    .filter(|skill_id| !skill_id.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    commands.push(Command::ShowSkill { skill_id });
                } else if let Some(skill_id) = trimmed
                    .strip_prefix(":skill activate ")
                    .map(str::trim)
                    .filter(|skill_id| !skill_id.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::ActivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id,
                        });
                    }
                } else if let Some(skill_id) = trimmed
                    .strip_prefix(":skill deactivate ")
                    .map(str::trim)
                    .filter(|skill_id| !skill_id.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::DeactivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id,
                        });
                    }
                } else {
                    self.input_history.push(self.input_content.clone());
                    let content = std::mem::take(&mut self.input_content);
                    self.input_cursor = 0;
                    self.input_history_index = None;

                    if is_session_busy(ctx) {
                        // Session is busy — hold the message locally and drain
                        // it when the session returns to idle.
                        self.message_queue.push(QueuedMessage { content });
                    } else if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::SendMessage {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            content,
                        });
                    }
                }
            }
            KeyAction::InputCharacter(c) => {
                self.input_content.insert(self.input_cursor, c);
                self.input_cursor += c.len_utf8();
            }
            KeyAction::InputBackspace if self.input_cursor > 0 => {
                let prev = prev_char_boundary(&self.input_content, self.input_cursor);
                self.input_content.drain(prev..self.input_cursor);
                self.input_cursor = prev;
            }
            KeyAction::InputNewline if self.input_mode == InputMode::MultiLine => {
                self.input_content.insert(self.input_cursor, '\n');
                self.input_cursor += 1;
            }
            KeyAction::ToggleInputMode => {
                self.input_mode = match self.input_mode {
                    InputMode::SingleLine => InputMode::MultiLine,
                    InputMode::MultiLine => InputMode::SingleLine,
                };
            }
            KeyAction::InputHistoryUp if !self.input_history.is_empty() => {
                let idx = match self.input_history_index {
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                    None => self.input_history.len() - 1,
                };
                self.input_history_index = Some(idx);
                self.input_content = self.input_history[idx].clone();
                self.input_cursor = self.input_content.len();
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
            KeyAction::DenyPermission | KeyAction::DenyAllPermission => {
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
            KeyAction::Escape
                if self.input_mode == InputMode::MultiLine && self.input_content.is_empty() =>
            {
                self.input_mode = InputMode::SingleLine;
            }
            KeyAction::InputPaste(text) => {
                for c in text.chars() {
                    self.input_content.insert(self.input_cursor, c);
                    self.input_cursor += c.len_utf8();
                }
            }
            _ => {}
        }

        (effects, commands)
    }

    fn clear_input(&mut self) {
        self.input_content.clear();
        self.input_cursor = 0;
        self.input_history_index = None;
    }
}

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
