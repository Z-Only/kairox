//! Key-event handlers for [`ModelOverlay`].
//!
//! Separated from [`super::state`] to keep the data model and selection queries
//! in one file and the interactive key-handling logic in another.

use crossterm::event::{Event, KeyCode, KeyModifiers};

use super::state::{ModelOverlay, REASONING_EFFORTS};
use super::types::{OverlayFocus, OverlayMode, ProfileDraft, PROFILE_EDITOR_FIELDS};
use crate::components::{Command, CrossPanelEffect, EventContext};

#[cfg(test)]
use agent_core::facade::ProfileSettingsInput;

impl ModelOverlay {
    fn start_create(&mut self) {
        self.mode = OverlayMode::Editor;
        self.draft = ProfileDraft::new();
        self.editor_field_index = 0;
        self.visible = true;
    }

    fn start_edit_selected(&mut self) {
        let Some(entry) = self.selected_profile().cloned() else {
            return;
        };
        self.mode = OverlayMode::Editor;
        self.draft = ProfileDraft::from_entry(&entry);
        self.editor_field_index = 1;
    }

    fn move_down(&mut self) {
        if self.mode == OverlayMode::Editor {
            self.editor_field_index = (self.editor_field_index + 1) % PROFILE_EDITOR_FIELDS.len();
            return;
        }

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
        if self.mode == OverlayMode::Editor {
            self.editor_field_index = if self.editor_field_index == 0 {
                PROFILE_EDITOR_FIELDS.len() - 1
            } else {
                self.editor_field_index - 1
            };
            return;
        }

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
        if !entry.enabled {
            return None;
        }
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

    fn settings_command(&self, key: KeyCode) -> Option<Command> {
        match key {
            KeyCode::Char('e') | KeyCode::Char('E') => {
                self.selected_profile()
                    .map(|entry| Command::SetProfileEnabled {
                        alias: entry.alias.clone(),
                        enabled: !entry.enabled,
                    })
            }
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete => self
                .selected_profile()
                .filter(|entry| entry.writable)
                .map(|entry| Command::DeleteProfileSettings {
                    alias: entry.alias.clone(),
                }),
            KeyCode::Char('J') => {
                self.selected_profile()
                    .map(|entry| Command::MoveProfileInOrder {
                        alias: entry.alias.clone(),
                        direction: 1,
                    })
            }
            KeyCode::Char('K') => {
                self.selected_profile()
                    .map(|entry| Command::MoveProfileInOrder {
                        alias: entry.alias.clone(),
                        direction: -1,
                    })
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.selected_profile()
                    .map(|entry| Command::TestModelProfile {
                        alias: entry.alias.clone(),
                    })
            }
            KeyCode::Char('o') | KeyCode::Char('O') => Some(Command::OpenProfilesConfig),
            _ => None,
        }
    }

    fn draft_test_command(&self) -> Option<Command> {
        let base_url = self.draft.base_url.trim();
        if base_url.is_empty() {
            return None;
        }
        let alias = self.draft.alias.trim();
        Some(Command::TestModelProfileUrl {
            alias: if alias.is_empty() {
                base_url.to_string()
            } else {
                alias.to_string()
            },
            base_url: base_url.to_string(),
        })
    }

    pub(super) fn handle_list_key(
        &mut self,
        ctx: &EventContext,
        key: KeyCode,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Char('h') => self.cycle_inner_focus(),
            KeyCode::Char('n') | KeyCode::Char('N') => self.start_create(),
            KeyCode::Char('u') | KeyCode::Char('U') => self.start_edit_selected(),
            KeyCode::Char('e')
            | KeyCode::Char('E')
            | KeyCode::Char('x')
            | KeyCode::Char('X')
            | KeyCode::Char('J')
            | KeyCode::Char('K')
            | KeyCode::Char('t')
            | KeyCode::Char('T')
            | KeyCode::Char('o')
            | KeyCode::Char('O')
            | KeyCode::Delete => {
                if let Some(cmd) = self.settings_command(key) {
                    commands.push(cmd);
                }
            }
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

    pub(super) fn handle_editor_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut commands = Vec::new();

        match key {
            KeyCode::Down | KeyCode::Tab => self.move_down(),
            KeyCode::Up | KeyCode::BackTab => self.move_up(),
            KeyCode::Esc => self.mode = OverlayMode::List,
            KeyCode::Backspace => self.draft.backspace(self.current_editor_field()),
            KeyCode::Delete => self.draft.clear_field(self.current_editor_field()),
            KeyCode::Char('t') | KeyCode::Char('T')
                if modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(command) = self.draft_test_command() {
                    commands.push(command);
                }
            }
            KeyCode::Enter => {
                if let Some(input) = self.draft.to_input() {
                    commands.push(Command::SaveProfileSettings { input });
                    self.mode = OverlayMode::List;
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.draft.push_char(self.current_editor_field(), ch);
            }
            _ => {}
        }

        (Vec::new(), commands)
    }

    pub(super) fn handle_key_event(
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

        match self.mode {
            OverlayMode::List => self.handle_list_key(ctx, key.code),
            OverlayMode::Editor => self.handle_editor_key(key.code, key.modifiers),
        }
    }

    #[cfg(test)]
    pub(super) fn replace_draft_for_test(&mut self, input: ProfileSettingsInput) {
        self.draft = ProfileDraft::from_input(input);
        self.mode = OverlayMode::Editor;
        self.visible = true;
    }
}
