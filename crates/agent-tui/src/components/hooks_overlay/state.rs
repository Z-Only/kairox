//! State types and behaviour for [`HooksOverlay`].
//!
//! The overlay tracks tab/mode selection plus an in-progress hook draft, and
//! exposes high-level helpers used by the [`Component`](crate::components::Component)
//! implementation in [`super::mod`](super) and the rendering helpers in
//! [`super::render`](super::render).
//!
//! Key-event handling lives in [`super::keys`].

use agent_core::facade::{
    HookSettingsInput, HookSettingsView, HookTemplateView, HooksSettingsView,
};
use agent_core::ConfigScope;
use ratatui::widgets::ListState;

pub(super) const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HooksTab {
    User,
    Project,
    Templates,
}

impl HooksTab {
    pub(super) fn next(self) -> Self {
        match self {
            Self::User => Self::Project,
            Self::Project => Self::Templates,
            Self::Templates => Self::User,
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::User => Self::Templates,
            Self::Project => Self::User,
            Self::Templates => Self::Project,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::Project => "Project",
            Self::Templates => "Templates",
        }
    }

    pub(super) fn scope(self) -> Option<ConfigScope> {
        match self {
            Self::User => Some(ConfigScope::User),
            Self::Project => Some(ConfigScope::Project),
            Self::Templates => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HooksMode {
    List,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HookEditorField {
    Scope,
    Id,
    Event,
    Matcher,
    Command,
    StatusMessage,
    TimeoutSecs,
    Enabled,
}

pub(super) const EDITOR_FIELDS: [HookEditorField; 8] = [
    HookEditorField::Scope,
    HookEditorField::Id,
    HookEditorField::Event,
    HookEditorField::Matcher,
    HookEditorField::Command,
    HookEditorField::StatusMessage,
    HookEditorField::TimeoutSecs,
    HookEditorField::Enabled,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HookDraft {
    pub(super) scope: ConfigScope,
    pub(super) id: String,
    pub(super) event: String,
    pub(super) matcher: String,
    pub(super) command: String,
    pub(super) status_message: String,
    pub(super) timeout_secs: String,
    pub(super) enabled: bool,
}

impl HookDraft {
    pub(super) fn new(scope: ConfigScope) -> Self {
        Self {
            scope,
            id: String::new(),
            event: "Stop".into(),
            matcher: "*".into(),
            command: String::new(),
            status_message: String::new(),
            timeout_secs: "600".into(),
            enabled: true,
        }
    }

    pub(super) fn from_hook(hook: &HookSettingsView, fallback_scope: ConfigScope) -> Self {
        Self {
            scope: hook.source,
            id: hook.id.clone(),
            event: hook.event.clone(),
            matcher: hook.matcher.clone().unwrap_or_default(),
            command: hook.command.clone(),
            status_message: hook.status_message.clone().unwrap_or_default(),
            timeout_secs: hook
                .timeout_secs
                .map(|value| value.to_string())
                .unwrap_or_default(),
            enabled: hook.enabled,
        }
        .with_scope_if_read_only(fallback_scope)
    }

    pub(super) fn from_template(template: &HookTemplateView, scope: ConfigScope) -> Self {
        Self {
            scope,
            id: template.id.clone(),
            event: template.event.clone(),
            matcher: template.matcher.clone().unwrap_or_default(),
            command: template.command.clone(),
            status_message: template.status_message.clone().unwrap_or_default(),
            timeout_secs: template
                .timeout_secs
                .map(|value| value.to_string())
                .unwrap_or_default(),
            enabled: true,
        }
    }

    #[cfg(test)]
    fn from_input(input: HookSettingsInput) -> Self {
        Self {
            scope: input.scope,
            id: input.id,
            event: input.event,
            matcher: input.matcher.unwrap_or_default(),
            command: input.command,
            status_message: input.status_message.unwrap_or_default(),
            timeout_secs: input
                .timeout_secs
                .map(|value| value.to_string())
                .unwrap_or_default(),
            enabled: input.enabled,
        }
    }

    fn with_scope_if_read_only(mut self, fallback_scope: ConfigScope) -> Self {
        if !matches!(self.scope, ConfigScope::User | ConfigScope::Project) {
            self.scope = fallback_scope;
        }
        self
    }

    pub(super) fn to_input(&self) -> Option<HookSettingsInput> {
        let id = self.id.trim();
        let event = self.event.trim();
        let command = self.command.trim();
        if id.is_empty() || event.is_empty() || command.is_empty() {
            return None;
        }

        Some(HookSettingsInput {
            scope: self.scope,
            id: id.to_string(),
            event: event.to_string(),
            matcher: trim_option(&self.matcher),
            command: command.to_string(),
            status_message: trim_option(&self.status_message),
            timeout_secs: self
                .timeout_secs
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0),
            enabled: self.enabled,
        })
    }

    pub(super) fn push_char(&mut self, field: HookEditorField, ch: char) {
        match field {
            HookEditorField::Scope => match ch {
                'u' | 'U' => self.scope = ConfigScope::User,
                'p' | 'P' => self.scope = ConfigScope::Project,
                _ => {}
            },
            HookEditorField::Id => self.id.push(ch),
            HookEditorField::Event => self.event.push(ch),
            HookEditorField::Matcher => self.matcher.push(ch),
            HookEditorField::Command => self.command.push(ch),
            HookEditorField::StatusMessage => self.status_message.push(ch),
            HookEditorField::TimeoutSecs if ch.is_ascii_digit() => self.timeout_secs.push(ch),
            HookEditorField::TimeoutSecs => {}
            HookEditorField::Enabled => match ch {
                ' ' | 't' | 'T' => self.enabled = !self.enabled,
                'y' | 'Y' | '1' => self.enabled = true,
                'n' | 'N' | '0' => self.enabled = false,
                _ => {}
            },
        }
    }

    pub(super) fn backspace(&mut self, field: HookEditorField) {
        match field {
            HookEditorField::Id => {
                self.id.pop();
            }
            HookEditorField::Event => {
                self.event.pop();
            }
            HookEditorField::Matcher => {
                self.matcher.pop();
            }
            HookEditorField::Command => {
                self.command.pop();
            }
            HookEditorField::StatusMessage => {
                self.status_message.pop();
            }
            HookEditorField::TimeoutSecs => {
                self.timeout_secs.pop();
            }
            HookEditorField::Scope | HookEditorField::Enabled => {}
        }
    }

    pub(super) fn clear_field(&mut self, field: HookEditorField) {
        match field {
            HookEditorField::Id => self.id.clear(),
            HookEditorField::Event => self.event.clear(),
            HookEditorField::Matcher => self.matcher.clear(),
            HookEditorField::Command => self.command.clear(),
            HookEditorField::StatusMessage => self.status_message.clear(),
            HookEditorField::TimeoutSecs => self.timeout_secs.clear(),
            HookEditorField::Scope | HookEditorField::Enabled => {}
        }
    }

    pub(super) fn cycle_event(&mut self, direction: i32) {
        let current = HOOK_EVENTS
            .iter()
            .position(|event| *event == self.event)
            .unwrap_or(0);
        let next = if direction < 0 {
            current.checked_sub(1).unwrap_or(HOOK_EVENTS.len() - 1)
        } else {
            (current + 1) % HOOK_EVENTS.len()
        };
        self.event = HOOK_EVENTS[next].to_string();
    }
}

pub struct HooksOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) tab: HooksTab,
    pub(super) mode: HooksMode,
    pub(super) user: Vec<HookSettingsView>,
    pub(super) project: Vec<HookSettingsView>,
    pub(super) templates: Vec<HookTemplateView>,
    pub(super) user_config_path: String,
    pub(super) project_config_path: Option<String>,
    pub(super) user_state: ListState,
    pub(super) project_state: ListState,
    pub(super) template_state: ListState,
    pub(super) draft: HookDraft,
    pub(super) editor_field_index: usize,
}

impl Default for HooksOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl HooksOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            tab: HooksTab::User,
            mode: HooksMode::List,
            user: Vec::new(),
            project: Vec::new(),
            templates: Vec::new(),
            user_config_path: String::new(),
            project_config_path: None,
            user_state: ListState::default(),
            project_state: ListState::default(),
            template_state: ListState::default(),
            draft: HookDraft::new(ConfigScope::User),
            editor_field_index: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, view: HooksSettingsView) {
        self.user = view.user;
        self.project = view.project;
        self.templates = view.templates;
        self.user_config_path = view.user_config_path;
        self.project_config_path = view.project_config_path;
        self.mode = HooksMode::List;
        self.visible = true;
        self.ensure_selection();
    }

    pub fn set_active_scope(&mut self, scope: ConfigScope) {
        self.tab = match scope {
            ConfigScope::Project => HooksTab::Project,
            _ => HooksTab::User,
        };
        self.ensure_selection();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.mode = HooksMode::List;
        self.tab = HooksTab::User;
        self.user.clear();
        self.project.clear();
        self.templates.clear();
        self.user_state.select(None);
        self.project_state.select(None);
        self.template_state.select(None);
        self.draft = HookDraft::new(ConfigScope::User);
        self.editor_field_index = 0;
    }

    #[allow(dead_code)]
    pub fn user_hooks(&self) -> &[HookSettingsView] {
        &self.user
    }

    #[allow(dead_code)]
    pub fn project_hooks(&self) -> &[HookSettingsView] {
        &self.project
    }

    pub(super) fn current_len(&self) -> usize {
        match self.tab {
            HooksTab::User => self.user.len(),
            HooksTab::Project => self.project.len(),
            HooksTab::Templates => self.templates.len(),
        }
    }

    pub(super) fn current_selected(&self) -> Option<usize> {
        match self.tab {
            HooksTab::User => self.user_state.selected(),
            HooksTab::Project => self.project_state.selected(),
            HooksTab::Templates => self.template_state.selected(),
        }
    }

    pub(super) fn current_state_mut(&mut self) -> &mut ListState {
        match self.tab {
            HooksTab::User => &mut self.user_state,
            HooksTab::Project => &mut self.project_state,
            HooksTab::Templates => &mut self.template_state,
        }
    }

    pub(super) fn selected_hook(&self) -> Option<&HookSettingsView> {
        let index = self.current_selected()?;
        match self.tab {
            HooksTab::User => self.user.get(index),
            HooksTab::Project => self.project.get(index),
            HooksTab::Templates => None,
        }
    }

    pub(super) fn selected_template(&self) -> Option<&HookTemplateView> {
        self.template_state
            .selected()
            .and_then(|index| self.templates.get(index))
    }

    pub(super) fn ensure_selection(&mut self) {
        for (len, state) in [
            (self.user.len(), &mut self.user_state),
            (self.project.len(), &mut self.project_state),
            (self.templates.len(), &mut self.template_state),
        ] {
            if len == 0 {
                state.select(None);
            } else {
                let selected = state.selected().unwrap_or(0).min(len.saturating_sub(1));
                state.select(Some(selected));
            }
        }
    }

    pub(super) fn current_editor_field(&self) -> HookEditorField {
        EDITOR_FIELDS[self.editor_field_index]
    }

    #[cfg(test)]
    pub(super) fn draft_for_test(&self) -> &HookDraft {
        &self.draft
    }

    #[cfg(test)]
    pub(super) fn replace_draft_for_test(&mut self, input: HookSettingsInput) {
        self.draft = HookDraft::from_input(input);
        self.mode = HooksMode::Editor;
        self.visible = true;
    }
}

pub(super) fn trim_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
