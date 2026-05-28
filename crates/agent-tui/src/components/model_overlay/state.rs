//! Model overlay state — types, draft buffer, and query helpers.
//!
//! The data model and selection queries live here; interactive key-handling
//! logic lives in [`super::keys`]. Rendering lives in [`super::render`].

use std::collections::BTreeMap;

use agent_core::facade::ProfileSettingsInput;
use ratatui::widgets::ListState;

use crate::components::{ModelOverlaySnapshot, ModelProfileEntry, ModelProfileTestResult};

/// Effort presets exposed for reasoning-capable profiles. Mirrors the GUI's
/// `DEFAULT_REASONING_EFFORTS` constant in `apps/agent-gui/src/stores/session.ts`.
pub const REASONING_EFFORTS: [&str; 4] = ["low", "middle", "high", "xhigh"];

/// Which sub-panel currently consumes navigation keys inside the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OverlayFocus {
    ProfileList,
    EffortList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OverlayMode {
    List,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProfileEditorField {
    Alias,
    Provider,
    ModelId,
    BaseUrl,
    ApiKeyEnv,
    ContextWindow,
    OutputLimit,
    Temperature,
    TopP,
    TopK,
    MaxTokens,
    Enabled,
}

pub(super) const PROFILE_EDITOR_FIELDS: [ProfileEditorField; 12] = [
    ProfileEditorField::Alias,
    ProfileEditorField::Provider,
    ProfileEditorField::ModelId,
    ProfileEditorField::BaseUrl,
    ProfileEditorField::ApiKeyEnv,
    ProfileEditorField::ContextWindow,
    ProfileEditorField::OutputLimit,
    ProfileEditorField::Temperature,
    ProfileEditorField::TopP,
    ProfileEditorField::TopK,
    ProfileEditorField::MaxTokens,
    ProfileEditorField::Enabled,
];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProfileDraft {
    pub(super) alias: String,
    pub(super) provider: String,
    pub(super) model_id: String,
    pub(super) base_url: String,
    pub(super) api_key_env: String,
    pub(super) context_window: String,
    pub(super) output_limit: String,
    pub(super) temperature: String,
    pub(super) top_p: String,
    pub(super) top_k: String,
    pub(super) max_tokens: String,
    pub(super) enabled: bool,
    pub(super) alias_editable: bool,
}

impl ProfileDraft {
    pub(super) fn new() -> Self {
        Self {
            alias: String::new(),
            provider: String::new(),
            model_id: String::new(),
            base_url: String::new(),
            api_key_env: String::new(),
            context_window: String::new(),
            output_limit: String::new(),
            temperature: String::new(),
            top_p: String::new(),
            top_k: String::new(),
            max_tokens: String::new(),
            enabled: true,
            alias_editable: true,
        }
    }

    pub(super) fn from_entry(entry: &ModelProfileEntry) -> Self {
        Self {
            alias: entry.alias.clone(),
            provider: entry.provider_display.clone(),
            model_id: entry.model_display.clone(),
            base_url: entry.base_url.clone().unwrap_or_default(),
            api_key_env: entry.api_key_env.clone().unwrap_or_default(),
            context_window: format_optional(entry.context_window),
            output_limit: format_optional(entry.output_limit),
            temperature: format_optional(entry.temperature),
            top_p: format_optional(entry.top_p),
            top_k: format_optional(entry.top_k),
            max_tokens: format_optional(entry.max_tokens),
            enabled: entry.enabled,
            alias_editable: false,
        }
    }

    #[cfg(test)]
    pub(super) fn from_input(input: ProfileSettingsInput) -> Self {
        Self {
            alias: input.alias,
            provider: input.provider,
            model_id: input.model_id,
            base_url: input.base_url.unwrap_or_default(),
            api_key_env: input.api_key_env.unwrap_or_default(),
            context_window: format_optional(input.context_window),
            output_limit: format_optional(input.output_limit),
            temperature: format_optional(input.temperature),
            top_p: format_optional(input.top_p),
            top_k: format_optional(input.top_k),
            max_tokens: format_optional(input.max_tokens),
            enabled: input.enabled,
            alias_editable: true,
        }
    }

    pub(super) fn to_input(&self) -> Option<ProfileSettingsInput> {
        let alias = self.alias.trim();
        let provider = self.provider.trim();
        let model_id = self.model_id.trim();
        if alias.is_empty() || provider.is_empty() || model_id.is_empty() {
            return None;
        }

        Some(ProfileSettingsInput {
            alias: alias.to_string(),
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            enabled: self.enabled,
            context_window: parse_optional(&self.context_window),
            output_limit: parse_optional(&self.output_limit),
            temperature: parse_optional(&self.temperature),
            top_p: parse_optional(&self.top_p),
            top_k: parse_optional(&self.top_k),
            max_tokens: parse_optional(&self.max_tokens),
            base_url: trim_option(&self.base_url),
            api_key_env: trim_option(&self.api_key_env),
        })
    }

    pub(super) fn push_char(&mut self, field: ProfileEditorField, ch: char) {
        match field {
            ProfileEditorField::Alias if self.alias_editable => self.alias.push(ch),
            ProfileEditorField::Provider => self.provider.push(ch),
            ProfileEditorField::ModelId => self.model_id.push(ch),
            ProfileEditorField::BaseUrl => self.base_url.push(ch),
            ProfileEditorField::ApiKeyEnv => self.api_key_env.push(ch),
            ProfileEditorField::ContextWindow => self.context_window.push(ch),
            ProfileEditorField::OutputLimit => self.output_limit.push(ch),
            ProfileEditorField::Temperature => self.temperature.push(ch),
            ProfileEditorField::TopP => self.top_p.push(ch),
            ProfileEditorField::TopK => self.top_k.push(ch),
            ProfileEditorField::MaxTokens => self.max_tokens.push(ch),
            ProfileEditorField::Enabled => match ch {
                'y' | 'Y' | '1' | 't' | 'T' => self.enabled = true,
                'n' | 'N' | '0' | 'f' | 'F' => self.enabled = false,
                ' ' => self.enabled = !self.enabled,
                _ => {}
            },
            ProfileEditorField::Alias => {}
        }
    }

    pub(super) fn backspace(&mut self, field: ProfileEditorField) {
        match field {
            ProfileEditorField::Alias if self.alias_editable => {
                self.alias.pop();
            }
            ProfileEditorField::Provider => {
                self.provider.pop();
            }
            ProfileEditorField::ModelId => {
                self.model_id.pop();
            }
            ProfileEditorField::BaseUrl => {
                self.base_url.pop();
            }
            ProfileEditorField::ApiKeyEnv => {
                self.api_key_env.pop();
            }
            ProfileEditorField::ContextWindow => {
                self.context_window.pop();
            }
            ProfileEditorField::OutputLimit => {
                self.output_limit.pop();
            }
            ProfileEditorField::Temperature => {
                self.temperature.pop();
            }
            ProfileEditorField::TopP => {
                self.top_p.pop();
            }
            ProfileEditorField::TopK => {
                self.top_k.pop();
            }
            ProfileEditorField::MaxTokens => {
                self.max_tokens.pop();
            }
            ProfileEditorField::Alias | ProfileEditorField::Enabled => {}
        }
    }

    pub(super) fn clear_field(&mut self, field: ProfileEditorField) {
        match field {
            ProfileEditorField::Alias if self.alias_editable => self.alias.clear(),
            ProfileEditorField::Provider => self.provider.clear(),
            ProfileEditorField::ModelId => self.model_id.clear(),
            ProfileEditorField::BaseUrl => self.base_url.clear(),
            ProfileEditorField::ApiKeyEnv => self.api_key_env.clear(),
            ProfileEditorField::ContextWindow => self.context_window.clear(),
            ProfileEditorField::OutputLimit => self.output_limit.clear(),
            ProfileEditorField::Temperature => self.temperature.clear(),
            ProfileEditorField::TopP => self.top_p.clear(),
            ProfileEditorField::TopK => self.top_k.clear(),
            ProfileEditorField::MaxTokens => self.max_tokens.clear(),
            ProfileEditorField::Alias | ProfileEditorField::Enabled => {}
        }
    }
}

pub struct ModelOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) profiles: Vec<ModelProfileEntry>,
    pub(super) current_alias: Option<String>,
    pub(super) current_effort: Option<String>,
    pub(super) list_state: ListState,
    pub(super) effort_state: ListState,
    pub(super) overlay_focus: OverlayFocus,
    pub(super) mode: OverlayMode,
    pub(super) draft: ProfileDraft,
    pub(super) editor_field_index: usize,
    pub(super) test_results: BTreeMap<String, ModelProfileTestResult>,
}

impl Default for ModelOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            profiles: Vec::new(),
            current_alias: None,
            current_effort: None,
            list_state: ListState::default(),
            effort_state: ListState::default(),
            overlay_focus: OverlayFocus::ProfileList,
            mode: OverlayMode::List,
            draft: ProfileDraft::new(),
            editor_field_index: 0,
            test_results: BTreeMap::new(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: ModelOverlaySnapshot) {
        // Default selection: the current alias if it exists in the list, else 0.
        let select = if snapshot.profiles.is_empty() {
            None
        } else {
            snapshot
                .current_alias
                .as_ref()
                .and_then(|a| snapshot.profiles.iter().position(|p| &p.alias == a))
                .or(Some(0))
        };
        self.list_state.select(select);

        // Effort selection mirrors current_effort when present and the selected
        // profile supports reasoning; else default to "low" so the picker has
        // a visible cursor.
        self.current_alias = snapshot.current_alias;
        self.current_effort = snapshot.current_effort;
        self.profiles = snapshot.profiles;
        let initial_effort = self
            .current_effort
            .as_deref()
            .and_then(|e| REASONING_EFFORTS.iter().position(|x| *x == e))
            .unwrap_or(0);
        self.effort_state.select(Some(initial_effort));
        self.overlay_focus = OverlayFocus::ProfileList;
        self.mode = OverlayMode::List;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.profiles.clear();
        self.list_state.select(None);
        self.effort_state.select(None);
        self.current_alias = None;
        self.current_effort = None;
        self.overlay_focus = OverlayFocus::ProfileList;
        self.mode = OverlayMode::List;
        self.draft = ProfileDraft::new();
        self.editor_field_index = 0;
        self.test_results.clear();
    }

    #[allow(dead_code)]
    pub fn profiles(&self) -> &[ModelProfileEntry] {
        &self.profiles
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn selected_profile(&self) -> Option<&ModelProfileEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.profiles.get(i))
    }

    pub(super) fn current_editor_field(&self) -> ProfileEditorField {
        PROFILE_EDITOR_FIELDS[self.editor_field_index]
    }

    /// `true` when the selected profile is reasoning-capable, so the effort
    /// picker should be rendered.
    pub fn shows_effort_picker(&self) -> bool {
        self.selected_profile()
            .map(|p| p.enabled && p.supports_reasoning)
            .unwrap_or(false)
    }

    /// Currently highlighted effort string (only meaningful when the selected
    /// profile supports reasoning).
    pub fn selected_effort(&self) -> Option<&'static str> {
        if !self.shows_effort_picker() {
            return None;
        }
        self.effort_state
            .selected()
            .and_then(|i| REASONING_EFFORTS.get(i).copied())
    }

    /// Available effort options for the selected profile. Empty for
    /// non-reasoning models.
    #[allow(dead_code)]
    pub fn effort_options(&self) -> &'static [&'static str] {
        if self.shows_effort_picker() {
            &REASONING_EFFORTS
        } else {
            &[]
        }
    }

    pub(super) fn set_test_result(&mut self, result: ModelProfileTestResult) {
        self.test_results.insert(result.alias.clone(), result);
    }
}

pub(super) fn format_optional<T: ToString>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

pub(super) fn parse_optional<T: std::str::FromStr>(value: &str) -> Option<T> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse().ok()
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
