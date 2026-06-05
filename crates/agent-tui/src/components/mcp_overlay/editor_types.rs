use std::collections::{BTreeMap, BTreeSet};

use agent_core::facade::{
    AddCatalogSourceRequest, InstallRequest, McpServerSettingsInput, McpServerSettingsTransport,
    McpServerSettingsView, ServerEntry,
};
use agent_mcp::catalog::{EnvVarSpec, InstallSpec};

// ---------------------------------------------------------------------------
// Server editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ServerEditorField {
    Name,
    Transport,
    CommandOrUrl,
    Args,
    Description,
    Enabled,
}

pub(super) const SERVER_EDITOR_FIELDS: [ServerEditorField; 6] = [
    ServerEditorField::Name,
    ServerEditorField::Transport,
    ServerEditorField::CommandOrUrl,
    ServerEditorField::Args,
    ServerEditorField::Description,
    ServerEditorField::Enabled,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ServerTransportDraft {
    Stdio,
    Sse,
    StreamableHttp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ServerDraft {
    pub(super) name: String,
    pub(super) transport: ServerTransportDraft,
    pub(super) command: String,
    pub(super) args_text: String,
    pub(super) url: String,
    pub(super) description: String,
    pub(super) enabled: bool,
}

impl ServerDraft {
    pub(super) fn new() -> Self {
        Self {
            name: String::new(),
            transport: ServerTransportDraft::Stdio,
            command: String::new(),
            args_text: String::new(),
            url: String::new(),
            description: String::new(),
            enabled: true,
        }
    }

    pub(super) fn from_view(view: &McpServerSettingsView) -> Self {
        let transport = match view.transport.as_str() {
            "sse" => ServerTransportDraft::Sse,
            "streamable_http" => ServerTransportDraft::StreamableHttp,
            _ => ServerTransportDraft::Stdio,
        };
        Self {
            name: view.name.clone(),
            transport,
            command: String::new(),
            args_text: String::new(),
            url: String::new(),
            description: view.description.clone().unwrap_or_default(),
            enabled: view.enabled,
        }
    }

    pub(super) fn to_input(&self) -> Option<McpServerSettingsInput> {
        let name = self.name.trim();
        if name.is_empty() {
            return None;
        }

        let transport = match self.transport {
            ServerTransportDraft::Stdio => {
                let command = self.command.trim();
                if command.is_empty() {
                    return None;
                }
                McpServerSettingsTransport::Stdio {
                    command: command.to_string(),
                    args: split_args(&self.args_text),
                    env: BTreeMap::new(),
                }
            }
            ServerTransportDraft::Sse => {
                let url = self.url.trim();
                if url.is_empty() {
                    return None;
                }
                McpServerSettingsTransport::Sse {
                    url: url.to_string(),
                    headers: BTreeMap::new(),
                }
            }
            ServerTransportDraft::StreamableHttp => {
                let url = self.url.trim();
                if url.is_empty() {
                    return None;
                }
                McpServerSettingsTransport::StreamableHttp {
                    url: url.to_string(),
                    headers: BTreeMap::new(),
                }
            }
        };

        Some(McpServerSettingsInput {
            name: name.to_string(),
            transport,
            enabled: self.enabled,
            description: trim_option(&self.description),
        })
    }

    pub(super) fn push_char(&mut self, field: ServerEditorField, ch: char) {
        match field {
            ServerEditorField::Name => self.name.push(ch),
            ServerEditorField::Transport => match ch {
                's' | 'S' => self.transport = ServerTransportDraft::Stdio,
                'e' | 'E' => self.transport = ServerTransportDraft::Sse,
                'h' | 'H' => self.transport = ServerTransportDraft::StreamableHttp,
                _ => {}
            },
            ServerEditorField::CommandOrUrl => {
                if self.transport == ServerTransportDraft::Stdio {
                    self.command.push(ch);
                } else {
                    self.url.push(ch);
                }
            }
            ServerEditorField::Args => {
                if self.transport == ServerTransportDraft::Stdio {
                    self.args_text.push(ch);
                }
            }
            ServerEditorField::Description => self.description.push(ch),
            ServerEditorField::Enabled => match ch {
                ' ' => self.enabled = !self.enabled,
                'y' | 'Y' | '1' | 't' | 'T' => self.enabled = true,
                'n' | 'N' | '0' | 'f' | 'F' => self.enabled = false,
                _ => {}
            },
        }
    }

    pub(super) fn backspace(&mut self, field: ServerEditorField) {
        match field {
            ServerEditorField::Name => {
                self.name.pop();
            }
            ServerEditorField::CommandOrUrl => {
                if self.transport == ServerTransportDraft::Stdio {
                    self.command.pop();
                } else {
                    self.url.pop();
                }
            }
            ServerEditorField::Args => {
                self.args_text.pop();
            }
            ServerEditorField::Description => {
                self.description.pop();
            }
            ServerEditorField::Transport | ServerEditorField::Enabled => {}
        }
    }

    pub(super) fn clear_field(&mut self, field: ServerEditorField) {
        match field {
            ServerEditorField::Name => self.name.clear(),
            ServerEditorField::CommandOrUrl => {
                if self.transport == ServerTransportDraft::Stdio {
                    self.command.clear();
                } else {
                    self.url.clear();
                }
            }
            ServerEditorField::Args => self.args_text.clear(),
            ServerEditorField::Description => self.description.clear(),
            ServerEditorField::Transport | ServerEditorField::Enabled => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Source editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceEditorField {
    Id,
    DisplayName,
    Url,
    ApiKeyEnv,
    Priority,
    DefaultTrust,
    Enabled,
}

pub(super) const SOURCE_EDITOR_FIELDS: [SourceEditorField; 7] = [
    SourceEditorField::Id,
    SourceEditorField::DisplayName,
    SourceEditorField::Url,
    SourceEditorField::ApiKeyEnv,
    SourceEditorField::Priority,
    SourceEditorField::DefaultTrust,
    SourceEditorField::Enabled,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceDraft {
    pub(super) id: String,
    pub(super) display_name: String,
    pub(super) url: String,
    pub(super) api_key_env: String,
    pub(super) priority: String,
    pub(super) default_trust: String,
    pub(super) enabled: bool,
}

impl SourceDraft {
    pub(super) fn new() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            url: String::new(),
            api_key_env: String::new(),
            priority: "100".to_string(),
            default_trust: "community".to_string(),
            enabled: true,
        }
    }

    pub(super) fn to_request(&self) -> Option<AddCatalogSourceRequest> {
        let id = self.id.trim();
        let display_name = self.display_name.trim();
        let url = self.url.trim();
        if id.is_empty() || display_name.is_empty() || url.is_empty() {
            return None;
        }

        Some(AddCatalogSourceRequest {
            id: id.to_string(),
            display_name: display_name.to_string(),
            kind: "mcp_registry".to_string(),
            url: url.to_string(),
            api_key_env: trim_option(&self.api_key_env),
            priority: self.priority.trim().parse::<u32>().ok().or(Some(100)),
            default_trust: trim_option(&self.default_trust)
                .or_else(|| Some("community".to_string())),
            enabled: Some(self.enabled),
            cache_ttl_seconds: None,
        })
    }

    pub(super) fn push_char(&mut self, field: SourceEditorField, ch: char) {
        match field {
            SourceEditorField::Id => self.id.push(ch),
            SourceEditorField::DisplayName => self.display_name.push(ch),
            SourceEditorField::Url => self.url.push(ch),
            SourceEditorField::ApiKeyEnv => self.api_key_env.push(ch),
            SourceEditorField::Priority => {
                if ch.is_ascii_digit() {
                    self.priority.push(ch);
                }
            }
            SourceEditorField::DefaultTrust => self.default_trust.push(ch),
            SourceEditorField::Enabled => match ch {
                ' ' => self.enabled = !self.enabled,
                'y' | 'Y' | '1' | 't' | 'T' => self.enabled = true,
                'n' | 'N' | '0' | 'f' | 'F' => self.enabled = false,
                _ => {}
            },
        }
    }

    pub(super) fn backspace(&mut self, field: SourceEditorField) {
        match field {
            SourceEditorField::Id => {
                self.id.pop();
            }
            SourceEditorField::DisplayName => {
                self.display_name.pop();
            }
            SourceEditorField::Url => {
                self.url.pop();
            }
            SourceEditorField::ApiKeyEnv => {
                self.api_key_env.pop();
            }
            SourceEditorField::Priority => {
                self.priority.pop();
            }
            SourceEditorField::DefaultTrust => {
                self.default_trust.pop();
            }
            SourceEditorField::Enabled => {}
        }
    }

    pub(super) fn clear_field(&mut self, field: SourceEditorField) {
        match field {
            SourceEditorField::Id => self.id.clear(),
            SourceEditorField::DisplayName => self.display_name.clear(),
            SourceEditorField::Url => self.url.clear(),
            SourceEditorField::ApiKeyEnv => self.api_key_env.clear(),
            SourceEditorField::Priority => self.priority.clear(),
            SourceEditorField::DefaultTrust => self.default_trust.clear(),
            SourceEditorField::Enabled => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Catalog install
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CatalogConfigItem {
    pub(super) kind: &'static str,
    pub(super) key: String,
    pub(super) description: String,
    pub(super) required: bool,
    pub(super) secret: bool,
    pub(super) default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CatalogInstallDraft {
    pub(super) catalog_id: String,
    pub(super) source: String,
    pub(super) display_name: String,
    pub(super) items: Vec<CatalogConfigItem>,
    pub(super) values: BTreeMap<String, String>,
}

impl CatalogInstallDraft {
    pub(super) fn new() -> Self {
        Self {
            catalog_id: String::new(),
            source: String::new(),
            display_name: String::new(),
            items: Vec::new(),
            values: BTreeMap::new(),
        }
    }

    pub(super) fn from_entry(entry: &ServerEntry) -> Self {
        let items = catalog_config_items(entry);
        let values = items
            .iter()
            .map(|item| (item.key.clone(), item.default.clone().unwrap_or_default()))
            .collect();
        Self {
            catalog_id: entry.id.clone(),
            source: entry.source.clone(),
            display_name: entry.display_name.clone(),
            items,
            values,
        }
    }

    pub(super) fn to_request(&self) -> Option<InstallRequest> {
        let mut env_overrides = BTreeMap::new();
        for item in &self.items {
            let value = self.values.get(&item.key).cloned().unwrap_or_default();
            if item.required && value.trim().is_empty() {
                return None;
            }
            if !value.trim().is_empty() {
                env_overrides.insert(item.key.clone(), value);
            }
        }

        Some(InstallRequest {
            catalog_id: self.catalog_id.clone(),
            source: self.source.clone(),
            server_id_override: None,
            env_overrides,
            trust_grant: false,
            auto_start: true,
        })
    }

    pub(super) fn push_char(&mut self, index: usize, ch: char) {
        if let Some(key) = self.items.get(index).map(|item| item.key.clone()) {
            self.values.entry(key).or_default().push(ch);
        }
    }

    pub(super) fn backspace(&mut self, index: usize) {
        if let Some(key) = self.items.get(index).map(|item| item.key.clone()) {
            self.values.entry(key).or_default().pop();
        }
    }

    pub(super) fn clear_field(&mut self, index: usize) {
        if let Some(key) = self.items.get(index).map(|item| item.key.clone()) {
            self.values.entry(key).or_default().clear();
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers (used by both types and editor logic)
// ---------------------------------------------------------------------------

pub(super) fn catalog_config_items(entry: &ServerEntry) -> Vec<CatalogConfigItem> {
    let env = parse_default_env(entry);
    let mut header_keys = BTreeSet::new();
    let mut items = Vec::new();

    if let Some(InstallSpec::Sse { headers, .. } | InstallSpec::StreamableHttp { headers, .. }) =
        parse_install_spec(entry)
    {
        for key in headers.keys() {
            header_keys.insert(key.clone());
            let meta = env.iter().find(|spec| spec.key == *key);
            items.push(CatalogConfigItem {
                kind: "HTTP header",
                key: key.clone(),
                description: meta
                    .map(|spec| spec.description.clone())
                    .unwrap_or_default(),
                required: meta.map(|spec| spec.required).unwrap_or(false),
                secret: meta.map(|spec| spec.secret).unwrap_or(false),
                default: meta.and_then(|spec| spec.default.clone()),
            });
        }
    }

    for spec in env {
        if header_keys.contains(&spec.key) {
            continue;
        }
        items.push(CatalogConfigItem {
            kind: "env",
            key: spec.key,
            description: spec.description,
            required: spec.required,
            secret: spec.secret,
            default: spec.default,
        });
    }

    items
}

pub(super) fn install_request_for_entry(
    entry: &ServerEntry,
    env_overrides: BTreeMap<String, String>,
) -> InstallRequest {
    InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides,
        trust_grant: false,
        auto_start: true,
    }
}

pub(super) fn parse_install_spec(entry: &ServerEntry) -> Option<InstallSpec> {
    serde_json::from_str(&entry.install_spec_json).ok()
}

pub(super) fn parse_requirements(
    entry: &ServerEntry,
) -> Vec<agent_mcp::catalog::RuntimeRequirement> {
    serde_json::from_str(&entry.requirements_json).unwrap_or_default()
}

pub(super) fn parse_default_env(entry: &ServerEntry) -> Vec<EnvVarSpec> {
    serde_json::from_str(&entry.default_env_json).unwrap_or_default()
}

pub(super) fn split_args(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map(str::to_string)
        .collect()
}

pub(super) fn trim_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
#[path = "editor_types_tests.rs"]
mod tests;
