//! Data types for the model overlay — focus/mode enums, editor field enum,
//! and profile draft struct used across the overlay submodules.

use agent_core::facade::ProfileSettingsInput;

use super::state::{format_optional, parse_optional, trim_option};
use crate::components::ModelProfileEntry;

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
    pub(super) client_identity: Option<String>,
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
            client_identity: None,
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
            client_identity: entry.client_identity.clone(),
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
            client_identity: input.client_identity,
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
            client_identity: self.client_identity.clone(),
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
