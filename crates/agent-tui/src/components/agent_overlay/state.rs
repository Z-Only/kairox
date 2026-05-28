//! State types and behaviour for [`AgentOverlay`].
//!
//! The overlay tracks list/editor mode plus an in-progress agent draft, and
//! exposes high-level helpers used by the [`Component`](crate::components::Component)
//! implementation in [`super`] and the rendering helpers in
//! [`super::render`].

use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};
use ratatui::widgets::ListState;

use crate::components::AgentOverlaySnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentOverlayMode {
    List,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentEditorField {
    Scope,
    Name,
    Description,
    Tools,
    ModelProfile,
    Skills,
    Nicknames,
    Enabled,
    Instructions,
}

pub(super) const EDITOR_FIELDS: [AgentEditorField; 9] = [
    AgentEditorField::Scope,
    AgentEditorField::Name,
    AgentEditorField::Description,
    AgentEditorField::Tools,
    AgentEditorField::ModelProfile,
    AgentEditorField::Skills,
    AgentEditorField::Nicknames,
    AgentEditorField::Enabled,
    AgentEditorField::Instructions,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentDraft {
    pub(super) scope: AgentSettingsScope,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) tools_text: String,
    pub(super) model_profile: String,
    pub(super) skills_text: String,
    pub(super) nicknames_text: String,
    pub(super) enabled: bool,
    pub(super) instructions: String,
}

impl AgentDraft {
    pub(super) fn new(scope: AgentSettingsScope) -> Self {
        Self {
            scope,
            name: String::new(),
            description: String::new(),
            tools_text: String::new(),
            model_profile: String::new(),
            skills_text: String::new(),
            nicknames_text: String::new(),
            enabled: true,
            instructions: String::new(),
        }
    }

    pub(super) fn from_view(view: &AgentSettingsView) -> Self {
        Self {
            scope: if view.scope == AgentSettingsScope::Builtin {
                AgentSettingsScope::User
            } else {
                view.scope
            },
            name: view.name.clone(),
            description: view.description.clone(),
            tools_text: view.tools.join(", "),
            model_profile: view.model_profile.clone().unwrap_or_default(),
            skills_text: view.skills.join(", "),
            nicknames_text: view.nickname_candidates.join(", "),
            enabled: view.enabled,
            instructions: view.instructions.clone(),
        }
    }

    #[cfg(test)]
    pub(super) fn from_input(input: AgentSettingsInput) -> Self {
        Self {
            scope: input.scope,
            name: input.name,
            description: input.description,
            tools_text: input.tools.join(", "),
            model_profile: input.model_profile.unwrap_or_default(),
            skills_text: input.skills.join(", "),
            nicknames_text: input.nickname_candidates.join(", "),
            enabled: input.enabled,
            instructions: input.instructions,
        }
    }

    pub(super) fn to_input(&self) -> Option<AgentSettingsInput> {
        let name = self.name.trim();
        let description = self.description.trim();
        if name.is_empty() || description.is_empty() {
            return None;
        }

        Some(AgentSettingsInput {
            scope: self.scope,
            name: name.to_string(),
            description: description.to_string(),
            tools: split_csv(&self.tools_text),
            model_profile: trim_option(&self.model_profile),
            skills: split_csv(&self.skills_text),
            nickname_candidates: split_csv(&self.nicknames_text),
            enabled: self.enabled,
            instructions: self.instructions.trim_end().to_string(),
        })
    }

    pub(super) fn push_char(&mut self, field: AgentEditorField, ch: char) {
        match field {
            AgentEditorField::Scope => match ch {
                'u' | 'U' => self.scope = AgentSettingsScope::User,
                'p' | 'P' => self.scope = AgentSettingsScope::Project,
                _ => {}
            },
            AgentEditorField::Name => self.name.push(ch),
            AgentEditorField::Description => self.description.push(ch),
            AgentEditorField::Tools => self.tools_text.push(ch),
            AgentEditorField::ModelProfile => self.model_profile.push(ch),
            AgentEditorField::Skills => self.skills_text.push(ch),
            AgentEditorField::Nicknames => self.nicknames_text.push(ch),
            AgentEditorField::Enabled => match ch {
                'y' | 'Y' | '1' | 't' | 'T' => self.enabled = true,
                'n' | 'N' | '0' | 'f' | 'F' => self.enabled = false,
                ' ' => self.enabled = !self.enabled,
                _ => {}
            },
            AgentEditorField::Instructions => self.instructions.push(ch),
        }
    }

    pub(super) fn backspace(&mut self, field: AgentEditorField) {
        match field {
            AgentEditorField::Name => {
                self.name.pop();
            }
            AgentEditorField::Description => {
                self.description.pop();
            }
            AgentEditorField::Tools => {
                self.tools_text.pop();
            }
            AgentEditorField::ModelProfile => {
                self.model_profile.pop();
            }
            AgentEditorField::Skills => {
                self.skills_text.pop();
            }
            AgentEditorField::Nicknames => {
                self.nicknames_text.pop();
            }
            AgentEditorField::Instructions => {
                self.instructions.pop();
            }
            AgentEditorField::Scope | AgentEditorField::Enabled => {}
        }
    }

    pub(super) fn clear_field(&mut self, field: AgentEditorField) {
        match field {
            AgentEditorField::Name => self.name.clear(),
            AgentEditorField::Description => self.description.clear(),
            AgentEditorField::Tools => self.tools_text.clear(),
            AgentEditorField::ModelProfile => self.model_profile.clear(),
            AgentEditorField::Skills => self.skills_text.clear(),
            AgentEditorField::Nicknames => self.nicknames_text.clear(),
            AgentEditorField::Instructions => self.instructions.clear(),
            AgentEditorField::Scope | AgentEditorField::Enabled => {}
        }
    }
}

pub struct AgentOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) agents: Vec<AgentSettingsView>,
    pub(super) list_state: ListState,
    pub(super) mode: AgentOverlayMode,
    pub(super) draft: AgentDraft,
    pub(super) editor_field_index: usize,
}

impl Default for AgentOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            agents: Vec::new(),
            list_state: ListState::default(),
            mode: AgentOverlayMode::List,
            draft: AgentDraft::new(AgentSettingsScope::User),
            editor_field_index: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: AgentOverlaySnapshot) {
        let selected = if snapshot.agents.is_empty() {
            None
        } else {
            Some(
                self.list_state
                    .selected()
                    .unwrap_or(0)
                    .min(snapshot.agents.len().saturating_sub(1)),
            )
        };
        self.agents = snapshot.agents;
        self.list_state.select(selected);
        self.mode = AgentOverlayMode::List;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.agents.clear();
        self.list_state.select(None);
        self.mode = AgentOverlayMode::List;
        self.draft = AgentDraft::new(AgentSettingsScope::User);
        self.editor_field_index = 0;
    }

    #[allow(dead_code)]
    pub fn agents(&self) -> &[AgentSettingsView] {
        &self.agents
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub(super) fn selected_agent(&self) -> Option<&AgentSettingsView> {
        self.list_state
            .selected()
            .and_then(|index| self.agents.get(index))
    }

    pub(super) fn current_editor_field(&self) -> AgentEditorField {
        EDITOR_FIELDS[self.editor_field_index]
    }
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn trim_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
