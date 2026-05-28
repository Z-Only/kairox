//! Data types for the hooks overlay -- tab/mode/field enums and draft structs
//! used across the overlay submodules.

use agent_core::facade::{HookSettingsInput, HookSettingsView, HookTemplateView};
use agent_core::ConfigScope;

pub(super) const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

/// Active tab within the hooks overlay.
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
    pub(super) fn from_input(input: HookSettingsInput) -> Self {
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

pub(super) fn trim_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
