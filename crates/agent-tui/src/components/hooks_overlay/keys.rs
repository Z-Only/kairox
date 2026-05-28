//! Key-event handlers for [`HooksOverlay`].
//!
//! Separated from [`super::state`] to keep the data model and selection queries
//! in one file and the interactive key-handling logic in another.

use agent_core::facade::HookSettingsInput;
use agent_core::ConfigScope;
use crossterm::event::{Event, KeyCode, KeyModifiers};

use super::state::HooksOverlay;
use super::types::{HookDraft, HookEditorField, HooksMode, HooksTab, EDITOR_FIELDS};
use crate::components::{Command, CrossPanelEffect};

impl HooksOverlay {
    fn move_down(&mut self) {
        match self.mode {
            HooksMode::List => {
                let len = self.current_len();
                if len == 0 {
                    return;
                }
                let next = match self.current_selected() {
                    Some(index) if index + 1 < len => index + 1,
                    Some(_) => len - 1,
                    None => 0,
                };
                self.current_state_mut().select(Some(next));
            }
            HooksMode::Editor => {
                self.editor_field_index = (self.editor_field_index + 1) % EDITOR_FIELDS.len();
            }
        }
    }

    fn move_up(&mut self) {
        match self.mode {
            HooksMode::List => {
                if self.current_len() == 0 {
                    return;
                }
                let next = match self.current_selected() {
                    Some(index) if index > 0 => index - 1,
                    _ => 0,
                };
                self.current_state_mut().select(Some(next));
            }
            HooksMode::Editor => {
                self.editor_field_index = if self.editor_field_index == 0 {
                    EDITOR_FIELDS.len() - 1
                } else {
                    self.editor_field_index - 1
                };
            }
        }
    }

    fn switch_tab_next(&mut self) {
        self.tab = self.tab.next();
        self.ensure_selection();
    }

    fn switch_tab_previous(&mut self) {
        self.tab = self.tab.previous();
        self.ensure_selection();
    }

    fn start_create(&mut self, scope: ConfigScope) {
        self.mode = HooksMode::Editor;
        self.draft = HookDraft::new(scope);
        self.editor_field_index = 1;
    }

    fn start_edit_selected(&mut self) {
        if self.tab == HooksTab::Templates {
            self.start_template(ConfigScope::User);
            return;
        }
        let Some(scope) = self.tab.scope() else {
            return;
        };
        let Some(hook) = self.selected_hook().cloned() else {
            return;
        };
        self.mode = HooksMode::Editor;
        self.draft = HookDraft::from_hook(&hook, scope);
        self.editor_field_index = 1;
    }

    fn start_template(&mut self, scope: ConfigScope) {
        let Some(template) = self.selected_template().cloned() else {
            return;
        };
        self.mode = HooksMode::Editor;
        self.draft = HookDraft::from_template(&template, scope);
        self.editor_field_index = 1;
    }

    fn selected_delete_command(&self) -> Option<Command> {
        let scope = self.tab.scope()?;
        let hook = self.selected_hook()?;
        Some(Command::DeleteHookSettings {
            scope,
            event: hook.event.clone(),
            id: hook.id.clone(),
        })
    }

    fn selected_toggle_command(&self) -> Option<Command> {
        let scope = self.tab.scope()?;
        let hook = self.selected_hook()?;
        Some(Command::SaveHookSettings {
            input: HookSettingsInput {
                scope,
                id: hook.id.clone(),
                event: hook.event.clone(),
                matcher: hook.matcher.clone(),
                command: hook.command.clone(),
                status_message: hook.status_message.clone(),
                timeout_secs: hook.timeout_secs,
                enabled: !hook.enabled,
            },
        })
    }

    fn handle_list_key(&mut self, key: KeyCode) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Tab | KeyCode::Right => self.switch_tab_next(),
            KeyCode::BackTab | KeyCode::Left => self.switch_tab_previous(),
            KeyCode::Char('n') => {
                let scope = self.tab.scope().unwrap_or(ConfigScope::User);
                self.start_create(scope);
            }
            KeyCode::Char('N') => {
                let scope = match self.tab.scope().unwrap_or(ConfigScope::User) {
                    ConfigScope::User => ConfigScope::Project,
                    _ => ConfigScope::User,
                };
                self.start_create(scope);
            }
            KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::Enter => {
                self.start_edit_selected();
            }
            KeyCode::Char('u') | KeyCode::Char('U') if self.tab == HooksTab::Templates => {
                self.start_template(ConfigScope::User);
            }
            KeyCode::Char('p') | KeyCode::Char('P') if self.tab == HooksTab::Templates => {
                self.start_template(ConfigScope::Project);
            }
            KeyCode::Char(' ') => {
                if let Some(command) = self.selected_toggle_command() {
                    commands.push(command);
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete => {
                if let Some(command) = self.selected_delete_command() {
                    commands.push(command);
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => commands.push(Command::OpenHooksOverlay),
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissHooksOverlay);
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
        let field = self.current_editor_field();

        match key {
            KeyCode::Down | KeyCode::Tab => self.move_down(),
            KeyCode::Up | KeyCode::BackTab => self.move_up(),
            KeyCode::Esc => self.mode = HooksMode::List,
            KeyCode::Backspace => self.draft.backspace(field),
            KeyCode::Delete => self.draft.clear_field(field),
            KeyCode::Left if field == HookEditorField::Event => self.draft.cycle_event(-1),
            KeyCode::Right if field == HookEditorField::Event => self.draft.cycle_event(1),
            KeyCode::Enter => {
                if let Some(input) = self.draft.to_input() {
                    commands.push(Command::SaveHookSettings { input });
                    self.mode = HooksMode::List;
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.draft.push_char(field, ch);
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
            HooksMode::List => self.handle_list_key(key.code),
            HooksMode::Editor => self.handle_editor_key(key.code, key.modifiers),
        }
    }

    #[cfg(test)]
    pub(super) fn handle_event_for_test(
        &mut self,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }
}
