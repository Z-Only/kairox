//! Key-event handlers for [`AgentOverlay`].
//!
//! Separated from [`super::state`] to keep the data model and selection queries
//! in one file and the interactive key-handling logic in another.

use agent_core::facade::AgentSettingsScope;
use crossterm::event::{Event, KeyCode, KeyModifiers};

use super::state::AgentOverlay;
use super::types::{AgentDraft, AgentOverlayMode, EDITOR_FIELDS};
use crate::components::{Command, CrossPanelEffect};

#[cfg(test)]
use agent_core::facade::AgentSettingsInput;

impl AgentOverlay {
    fn start_create(&mut self, scope: AgentSettingsScope) {
        self.mode = AgentOverlayMode::Editor;
        self.draft = AgentDraft::new(scope);
        self.editor_field_index = 1;
        self.visible = true;
    }

    fn start_edit_selected(&mut self) {
        let Some(agent) = self
            .selected_agent()
            .filter(|agent| agent.editable)
            .cloned()
        else {
            return;
        };
        self.mode = AgentOverlayMode::Editor;
        self.draft = AgentDraft::from_view(&agent);
        self.editor_field_index = 1;
    }

    fn move_down(&mut self) {
        match self.mode {
            AgentOverlayMode::List => {
                if self.agents.is_empty() {
                    return;
                }
                let next = match self.list_state.selected() {
                    Some(index) if index + 1 < self.agents.len() => index + 1,
                    Some(_) => self.agents.len() - 1,
                    None => 0,
                };
                self.list_state.select(Some(next));
            }
            AgentOverlayMode::Editor => {
                self.editor_field_index = (self.editor_field_index + 1) % EDITOR_FIELDS.len();
            }
        }
    }

    fn move_up(&mut self) {
        match self.mode {
            AgentOverlayMode::List => {
                if self.agents.is_empty() {
                    return;
                }
                let next = match self.list_state.selected() {
                    Some(index) if index > 0 => index - 1,
                    _ => 0,
                };
                self.list_state.select(Some(next));
            }
            AgentOverlayMode::Editor => {
                self.editor_field_index = if self.editor_field_index == 0 {
                    EDITOR_FIELDS.len() - 1
                } else {
                    self.editor_field_index - 1
                };
            }
        }
    }

    fn handle_list_key(&mut self, key: KeyCode) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('n') => self.start_create(AgentSettingsScope::User),
            KeyCode::Char('N') => self.start_create(AgentSettingsScope::Project),
            KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::Enter => {
                self.start_edit_selected();
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if let Some(agent) = self.selected_agent() {
                    commands.push(Command::CopyAgentSettings {
                        settings_id: agent.settings_id.clone(),
                        scope: AgentSettingsScope::User,
                    });
                }
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                if let Some(agent) = self.selected_agent() {
                    commands.push(Command::CopyAgentSettings {
                        settings_id: agent.settings_id.clone(),
                        scope: AgentSettingsScope::Project,
                    });
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete => {
                if let Some(agent) = self.selected_agent().filter(|agent| agent.deletable) {
                    commands.push(Command::DeleteAgentSettings {
                        settings_id: agent.settings_id.clone(),
                    });
                }
            }
            KeyCode::Char('o') | KeyCode::Char('O') => commands.push(Command::OpenAgentsDir),
            KeyCode::Char('r') | KeyCode::Char('R') => {
                commands.push(Command::OpenAgentSettingsOverlay);
            }
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissAgentSettingsOverlay);
            }
            _ => {}
        }

        (effects, commands)
    }

    fn handle_editor_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut commands = Vec::new();

        match key {
            KeyCode::Down | KeyCode::Tab => self.move_down(),
            KeyCode::Up | KeyCode::BackTab => self.move_up(),
            KeyCode::Esc => self.mode = AgentOverlayMode::List,
            KeyCode::Backspace => self.draft.backspace(self.current_editor_field()),
            KeyCode::Delete => self.draft.clear_field(self.current_editor_field()),
            KeyCode::Enter => {
                if let Some(input) = self.draft.to_input() {
                    commands.push(Command::SaveAgentSettings { input });
                    self.mode = AgentOverlayMode::List;
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
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        match self.mode {
            AgentOverlayMode::List => self.handle_list_key(key.code),
            AgentOverlayMode::Editor => self.handle_editor_key(key.code, key.modifiers),
        }
    }

    #[cfg(test)]
    pub(super) fn handle_event_for_test(
        &mut self,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    #[cfg(test)]
    pub(super) fn start_create_for_test(&mut self, scope: AgentSettingsScope) {
        self.start_create(scope);
    }

    #[cfg(test)]
    pub(super) fn replace_draft_for_test(&mut self, input: AgentSettingsInput) {
        self.draft = AgentDraft::from_input(input);
        self.mode = AgentOverlayMode::Editor;
        self.visible = true;
    }
}
