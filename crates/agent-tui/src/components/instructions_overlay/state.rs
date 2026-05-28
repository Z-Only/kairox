//! State types and editing logic for the instructions overlay.

use agent_core::facade::InstructionsView;
use agent_core::ConfigScope;

use super::types::InstructionsTab;
use crate::components::Command;

pub struct InstructionsOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) tab: InstructionsTab,
    system_text: String,
    user_text: String,
    project_text: String,
    user_cursor: usize,
    project_cursor: usize,
}

impl Default for InstructionsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl InstructionsOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            tab: InstructionsTab::User,
            system_text: String::new(),
            user_text: String::new(),
            project_text: String::new(),
            user_cursor: 0,
            project_cursor: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, view: InstructionsView) {
        self.system_text = view.system;
        self.user_text = view.user.unwrap_or_default();
        self.project_text = view.project.unwrap_or_default();
        self.user_cursor = self.user_text.len();
        self.project_cursor = self.project_text.len();
        self.tab = InstructionsTab::User;
        self.visible = true;
    }

    pub fn set_active_scope(&mut self, scope: ConfigScope) {
        self.tab = match scope {
            ConfigScope::Project => InstructionsTab::Project,
            _ => InstructionsTab::User,
        };
    }

    pub fn show_system_prompt(&mut self, view: InstructionsView) {
        self.show(view);
        self.tab = InstructionsTab::System;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.tab = InstructionsTab::User;
    }

    pub fn active_scope(&self) -> ConfigScope {
        match self.tab {
            InstructionsTab::System | InstructionsTab::User | InstructionsTab::Effective => {
                ConfigScope::User
            }
            InstructionsTab::Project => ConfigScope::Project,
        }
    }

    #[allow(dead_code)]
    pub fn active_tab_label(&self) -> &'static str {
        match self.tab {
            InstructionsTab::System => "System",
            InstructionsTab::User => "User",
            InstructionsTab::Project => "Project",
            InstructionsTab::Effective => "Effective",
        }
    }

    pub fn system_text(&self) -> &str {
        &self.system_text
    }

    pub fn user_text(&self) -> &str {
        &self.user_text
    }

    pub fn project_text(&self) -> &str {
        &self.project_text
    }

    pub fn effective_text(&self) -> String {
        let parts = [
            self.system_text.as_str(),
            self.user_text.as_str(),
            self.project_text.as_str(),
        ];
        parts
            .into_iter()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn selected_buffer_mut(&mut self) -> Option<(&mut String, &mut usize)> {
        match self.tab {
            InstructionsTab::User => Some((&mut self.user_text, &mut self.user_cursor)),
            InstructionsTab::Project => Some((&mut self.project_text, &mut self.project_cursor)),
            InstructionsTab::System | InstructionsTab::Effective => None,
        }
    }

    pub(super) fn insert_char(&mut self, ch: char) {
        let Some((text, cursor)) = self.selected_buffer_mut() else {
            return;
        };
        text.insert(*cursor, ch);
        *cursor += ch.len_utf8();
    }

    pub(super) fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub(super) fn backspace(&mut self) {
        let Some((text, cursor)) = self.selected_buffer_mut() else {
            return;
        };
        if *cursor == 0 {
            return;
        }
        let previous = text[..*cursor]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        text.drain(previous..*cursor);
        *cursor = previous;
    }

    pub(super) fn save_command(&self) -> Option<Command> {
        let scope = self.active_scope();
        match self.tab {
            InstructionsTab::System => None,
            InstructionsTab::User => Some(Command::SaveInstructions {
                scope,
                text: self.user_text.trim().to_string(),
            }),
            InstructionsTab::Project => Some(Command::SaveInstructions {
                scope,
                text: self.project_text.trim().to_string(),
            }),
            InstructionsTab::Effective => None,
        }
    }
}
