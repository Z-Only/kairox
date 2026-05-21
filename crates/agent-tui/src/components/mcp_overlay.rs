//! MCP server overlay — pop-up modal listing runtime servers, settings,
//! installed marketplace entries, catalog entries, and catalog sources.
//!
//! The App constructs a snapshot before opening the overlay; the overlay owns
//! tab and selection state, then emits [`Command`] values that the main loop
//! dispatches to the runtime manager or MCP facade.

use std::collections::BTreeMap;

use agent_core::facade::{
    AddCatalogSourceRequest, CatalogSourceView, InstalledEntry, McpServerSettingsInput,
    McpServerSettingsTransport, McpServerSettingsView, ServerEntry,
};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, McpConnectivityEntry, McpOverlaySnapshot,
    McpPromptEntry, McpResourceEntry, McpServerEntry, McpServerStatusView, McpToolEntry,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpOverlayTab {
    Runtime,
    Settings,
    Installed,
    Catalog,
    Sources,
    Tools,
    Resources,
    Prompts,
}

impl McpOverlayTab {
    fn next(self) -> Self {
        match self {
            Self::Runtime => Self::Settings,
            Self::Settings => Self::Installed,
            Self::Installed => Self::Catalog,
            Self::Catalog => Self::Sources,
            Self::Sources => Self::Tools,
            Self::Tools => Self::Resources,
            Self::Resources => Self::Prompts,
            Self::Prompts => Self::Runtime,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Runtime => Self::Prompts,
            Self::Settings => Self::Runtime,
            Self::Installed => Self::Settings,
            Self::Catalog => Self::Installed,
            Self::Sources => Self::Catalog,
            Self::Tools => Self::Sources,
            Self::Resources => Self::Tools,
            Self::Prompts => Self::Resources,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Runtime => "Runtime",
            Self::Settings => "Settings",
            Self::Installed => "Installed",
            Self::Catalog => "Catalog",
            Self::Sources => "Sources",
            Self::Tools => "Tools",
            Self::Resources => "Resources",
            Self::Prompts => "Prompts",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct McpHealthState {
    healthy: bool,
    tool_count: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpOverlayMode {
    List,
    ServerEditor,
    SourceEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerEditorField {
    Name,
    Transport,
    CommandOrUrl,
    Args,
    Description,
    Enabled,
}

const SERVER_EDITOR_FIELDS: [ServerEditorField; 6] = [
    ServerEditorField::Name,
    ServerEditorField::Transport,
    ServerEditorField::CommandOrUrl,
    ServerEditorField::Args,
    ServerEditorField::Description,
    ServerEditorField::Enabled,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerTransportDraft {
    Stdio,
    Sse,
    StreamableHttp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerDraft {
    name: String,
    transport: ServerTransportDraft,
    command: String,
    args_text: String,
    url: String,
    description: String,
    enabled: bool,
}

impl ServerDraft {
    fn new() -> Self {
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

    fn from_view(view: &McpServerSettingsView) -> Self {
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

    fn to_input(&self) -> Option<McpServerSettingsInput> {
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

    fn push_char(&mut self, field: ServerEditorField, ch: char) {
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

    fn backspace(&mut self, field: ServerEditorField) {
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

    fn clear_field(&mut self, field: ServerEditorField) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceEditorField {
    Id,
    DisplayName,
    Url,
    ApiKeyEnv,
    Priority,
    DefaultTrust,
    Enabled,
}

const SOURCE_EDITOR_FIELDS: [SourceEditorField; 7] = [
    SourceEditorField::Id,
    SourceEditorField::DisplayName,
    SourceEditorField::Url,
    SourceEditorField::ApiKeyEnv,
    SourceEditorField::Priority,
    SourceEditorField::DefaultTrust,
    SourceEditorField::Enabled,
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceDraft {
    id: String,
    display_name: String,
    url: String,
    api_key_env: String,
    priority: String,
    default_trust: String,
    enabled: bool,
}

impl SourceDraft {
    fn new() -> Self {
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

    fn to_request(&self) -> Option<AddCatalogSourceRequest> {
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

    fn push_char(&mut self, field: SourceEditorField, ch: char) {
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

    fn backspace(&mut self, field: SourceEditorField) {
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

    fn clear_field(&mut self, field: SourceEditorField) {
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

pub struct McpOverlay {
    focused: bool,
    visible: bool,
    mode: McpOverlayMode,
    tab: McpOverlayTab,
    runtime_servers: Vec<McpServerEntry>,
    settings: Vec<McpServerSettingsView>,
    installed: Vec<InstalledEntry>,
    catalog: Vec<ServerEntry>,
    sources: Vec<CatalogSourceView>,
    tools: BTreeMap<String, Vec<McpToolEntry>>,
    resources: BTreeMap<String, Vec<McpResourceEntry>>,
    prompts: BTreeMap<String, Vec<McpPromptEntry>>,
    health: BTreeMap<String, McpHealthState>,
    connectivity: BTreeMap<String, McpConnectivityEntry>,
    resource_previews: BTreeMap<String, String>,
    runtime_state: ListState,
    settings_state: ListState,
    installed_state: ListState,
    catalog_state: ListState,
    sources_state: ListState,
    tools_state: ListState,
    resources_state: ListState,
    prompts_state: ListState,
    server_draft: ServerDraft,
    server_field_index: usize,
    source_draft: SourceDraft,
    source_field_index: usize,
}

struct McpOverlayRenderState<'a> {
    runtime: &'a mut ListState,
    settings: &'a mut ListState,
    installed: &'a mut ListState,
    catalog: &'a mut ListState,
    sources: &'a mut ListState,
    tools: &'a mut ListState,
    resources: &'a mut ListState,
    prompts: &'a mut ListState,
}

impl Default for McpOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl McpOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            mode: McpOverlayMode::List,
            tab: McpOverlayTab::Runtime,
            runtime_servers: Vec::new(),
            settings: Vec::new(),
            installed: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
            tools: BTreeMap::new(),
            resources: BTreeMap::new(),
            prompts: BTreeMap::new(),
            health: BTreeMap::new(),
            connectivity: BTreeMap::new(),
            resource_previews: BTreeMap::new(),
            runtime_state: ListState::default(),
            settings_state: ListState::default(),
            installed_state: ListState::default(),
            catalog_state: ListState::default(),
            sources_state: ListState::default(),
            tools_state: ListState::default(),
            resources_state: ListState::default(),
            prompts_state: ListState::default(),
            server_draft: ServerDraft::new(),
            server_field_index: 0,
            source_draft: SourceDraft::new(),
            source_field_index: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: McpOverlaySnapshot) {
        self.runtime_servers = snapshot.runtime_servers;
        self.settings = snapshot.settings;
        self.installed = snapshot.installed;
        self.catalog = snapshot.catalog;
        self.sources = snapshot.sources;
        self.visible = true;
        self.mode = McpOverlayMode::List;
        self.ensure_selection();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.runtime_servers.clear();
        self.settings.clear();
        self.installed.clear();
        self.catalog.clear();
        self.sources.clear();
        self.tools.clear();
        self.resources.clear();
        self.prompts.clear();
        self.health.clear();
        self.connectivity.clear();
        self.resource_previews.clear();
        self.mode = McpOverlayMode::List;
        self.runtime_state.select(None);
        self.settings_state.select(None);
        self.installed_state.select(None);
        self.catalog_state.select(None);
        self.sources_state.select(None);
        self.tools_state.select(None);
        self.resources_state.select(None);
        self.prompts_state.select(None);
        self.server_draft = ServerDraft::new();
        self.server_field_index = 0;
        self.source_draft = SourceDraft::new();
        self.source_field_index = 0;
    }

    #[allow(dead_code)]
    pub fn servers(&self) -> &[McpServerEntry] {
        &self.runtime_servers
    }

    #[allow(dead_code)]
    pub fn settings_len(&self) -> usize {
        self.settings.len()
    }

    #[allow(dead_code)]
    pub fn catalog_len(&self) -> usize {
        self.catalog.len()
    }

    #[allow(dead_code)]
    pub fn sources_len(&self) -> usize {
        self.sources.len()
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
    }

    #[cfg(test)]
    fn server_draft_name_for_test(&self) -> Option<&str> {
        if self.mode == McpOverlayMode::ServerEditor {
            Some(self.server_draft.name.as_str())
        } else {
            None
        }
    }

    fn current_len(&self) -> usize {
        match self.tab {
            McpOverlayTab::Runtime => self.runtime_servers.len(),
            McpOverlayTab::Settings => self.settings.len(),
            McpOverlayTab::Installed => self.installed.len(),
            McpOverlayTab::Catalog => self.catalog.len(),
            McpOverlayTab::Sources => self.sources.len(),
            McpOverlayTab::Tools => self.current_tools().len(),
            McpOverlayTab::Resources => self.current_resources().len(),
            McpOverlayTab::Prompts => self.current_prompts().len(),
        }
    }

    fn current_selected(&self) -> Option<usize> {
        match self.tab {
            McpOverlayTab::Runtime => self.runtime_state.selected(),
            McpOverlayTab::Settings => self.settings_state.selected(),
            McpOverlayTab::Installed => self.installed_state.selected(),
            McpOverlayTab::Catalog => self.catalog_state.selected(),
            McpOverlayTab::Sources => self.sources_state.selected(),
            McpOverlayTab::Tools => self.tools_state.selected(),
            McpOverlayTab::Resources => self.resources_state.selected(),
            McpOverlayTab::Prompts => self.prompts_state.selected(),
        }
    }

    fn select_current(&mut self, selected: Option<usize>) {
        match self.tab {
            McpOverlayTab::Runtime => self.runtime_state.select(selected),
            McpOverlayTab::Settings => self.settings_state.select(selected),
            McpOverlayTab::Installed => self.installed_state.select(selected),
            McpOverlayTab::Catalog => self.catalog_state.select(selected),
            McpOverlayTab::Sources => self.sources_state.select(selected),
            McpOverlayTab::Tools => self.tools_state.select(selected),
            McpOverlayTab::Resources => self.resources_state.select(selected),
            McpOverlayTab::Prompts => self.prompts_state.select(selected),
        }
    }

    fn ensure_selection(&mut self) {
        let tools_len = self.current_tools().len();
        let resources_len = self.current_resources().len();
        let prompts_len = self.current_prompts().len();
        for (len, state) in [
            (self.runtime_servers.len(), &mut self.runtime_state),
            (self.settings.len(), &mut self.settings_state),
            (self.installed.len(), &mut self.installed_state),
            (self.catalog.len(), &mut self.catalog_state),
            (self.sources.len(), &mut self.sources_state),
            (tools_len, &mut self.tools_state),
            (resources_len, &mut self.resources_state),
            (prompts_len, &mut self.prompts_state),
        ] {
            let selected = if len == 0 {
                None
            } else {
                Some(state.selected().map_or(0, |index| index.min(len - 1)))
            };
            state.select(selected);
        }
    }

    fn selected_runtime_server(&self) -> Option<&McpServerEntry> {
        self.runtime_state
            .selected()
            .and_then(|index| self.runtime_servers.get(index))
    }

    fn selected_setting(&self) -> Option<&McpServerSettingsView> {
        self.settings_state
            .selected()
            .and_then(|index| self.settings.get(index))
    }

    fn selected_installed(&self) -> Option<&InstalledEntry> {
        self.installed_state
            .selected()
            .and_then(|index| self.installed.get(index))
    }

    fn selected_catalog_entry(&self) -> Option<&ServerEntry> {
        self.catalog_state
            .selected()
            .and_then(|index| self.catalog.get(index))
    }

    fn selected_source(&self) -> Option<&CatalogSourceView> {
        self.sources_state
            .selected()
            .and_then(|index| self.sources.get(index))
    }

    fn selected_server_id(&self) -> Option<&str> {
        self.selected_runtime_server()
            .map(|entry| entry.server_id.as_str())
    }

    fn current_tools(&self) -> &[McpToolEntry] {
        self.selected_server_id()
            .and_then(|server_id| self.tools.get(server_id))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn current_resources(&self) -> &[McpResourceEntry] {
        self.selected_server_id()
            .and_then(|server_id| self.resources.get(server_id))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn current_prompts(&self) -> &[McpPromptEntry] {
        self.selected_server_id()
            .and_then(|server_id| self.prompts.get(server_id))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn selected_tool(&self) -> Option<&McpToolEntry> {
        self.tools_state
            .selected()
            .and_then(|index| self.current_tools().get(index))
    }

    fn selected_resource(&self) -> Option<&McpResourceEntry> {
        self.resources_state
            .selected()
            .and_then(|index| self.current_resources().get(index))
    }

    fn move_down(&mut self) {
        let len = self.current_len();
        if len == 0 {
            return;
        }
        let next = match self.current_selected() {
            Some(i) if i + 1 < len => i + 1,
            Some(_) => len - 1,
            None => 0,
        };
        self.select_current(Some(next));
    }

    fn move_up(&mut self) {
        if self.current_len() == 0 {
            return;
        }
        let next = match self.current_selected() {
            Some(i) if i > 0 => i - 1,
            _ => 0,
        };
        self.select_current(Some(next));
    }

    fn start_server_create(&mut self) {
        self.mode = McpOverlayMode::ServerEditor;
        self.server_draft = ServerDraft::new();
        self.server_field_index = 0;
    }

    fn start_server_edit_selected(&mut self) {
        let Some(setting) = self
            .selected_setting()
            .filter(|setting| setting.writable)
            .cloned()
        else {
            return;
        };
        self.mode = McpOverlayMode::ServerEditor;
        self.server_draft = ServerDraft::from_view(&setting);
        self.server_field_index = 0;
    }

    fn start_source_create(&mut self) {
        self.mode = McpOverlayMode::SourceEditor;
        self.source_draft = SourceDraft::new();
        self.source_field_index = 0;
    }

    fn current_server_field(&self) -> ServerEditorField {
        SERVER_EDITOR_FIELDS[self.server_field_index]
    }

    fn current_source_field(&self) -> SourceEditorField {
        SOURCE_EDITOR_FIELDS[self.source_field_index]
    }

    fn move_server_field_down(&mut self) {
        self.server_field_index = (self.server_field_index + 1) % SERVER_EDITOR_FIELDS.len();
    }

    fn move_server_field_up(&mut self) {
        self.server_field_index = if self.server_field_index == 0 {
            SERVER_EDITOR_FIELDS.len() - 1
        } else {
            self.server_field_index - 1
        };
    }

    fn move_source_field_down(&mut self) {
        self.source_field_index = (self.source_field_index + 1) % SOURCE_EDITOR_FIELDS.len();
    }

    fn move_source_field_up(&mut self) {
        self.source_field_index = if self.source_field_index == 0 {
            SOURCE_EDITOR_FIELDS.len() - 1
        } else {
            self.source_field_index - 1
        };
    }

    fn handle_server_editor_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Vec<Command> {
        match key {
            KeyCode::Tab | KeyCode::Down => self.move_server_field_down(),
            KeyCode::BackTab | KeyCode::Up => self.move_server_field_up(),
            KeyCode::Esc => self.mode = McpOverlayMode::List,
            KeyCode::Backspace => self.server_draft.backspace(self.current_server_field()),
            KeyCode::Delete => self.server_draft.clear_field(self.current_server_field()),
            KeyCode::Enter => {
                if let Some(input) = self.server_draft.to_input() {
                    self.mode = McpOverlayMode::List;
                    return vec![Command::SaveMcpServerSettings { input }];
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.server_draft.push_char(self.current_server_field(), ch);
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_source_editor_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Vec<Command> {
        match key {
            KeyCode::Tab | KeyCode::Down => self.move_source_field_down(),
            KeyCode::BackTab | KeyCode::Up => self.move_source_field_up(),
            KeyCode::Esc => self.mode = McpOverlayMode::List,
            KeyCode::Backspace => self.source_draft.backspace(self.current_source_field()),
            KeyCode::Delete => self.source_draft.clear_field(self.current_source_field()),
            KeyCode::Enter => {
                if let Some(request) = self.source_draft.to_request() {
                    self.mode = McpOverlayMode::List;
                    return vec![Command::AddMcpCatalogSource { request }];
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.source_draft.push_char(self.current_source_field(), ch);
            }
            _ => {}
        }
        Vec::new()
    }

    fn command_for_current_tab(&self, key: KeyCode) -> Option<Command> {
        match (self.tab, key) {
            (McpOverlayTab::Runtime, KeyCode::Enter) => {
                self.selected_runtime_server().map(|entry| {
                    let server_id = entry.server_id.clone();
                    match entry.status {
                        McpServerStatusView::Running | McpServerStatusView::Starting => {
                            Command::StopMcpServer { server_id }
                        }
                        McpServerStatusView::Stopped | McpServerStatusView::Failed => {
                            Command::StartMcpServer { server_id }
                        }
                    }
                })
            }
            (McpOverlayTab::Runtime, KeyCode::Char('t') | KeyCode::Char('T')) => {
                self.selected_runtime_server().map(|entry| {
                    let server_id = entry.server_id.clone();
                    if entry.trusted {
                        Command::RevokeMcpTrust { server_id }
                    } else {
                        Command::TrustMcpServer { server_id }
                    }
                })
            }
            (McpOverlayTab::Runtime, KeyCode::Char('h') | KeyCode::Char('H')) => self
                .selected_runtime_server()
                .map(|entry| Command::CheckMcpHealth {
                    server_id: entry.server_id.clone(),
                }),
            (McpOverlayTab::Runtime, KeyCode::Char('c') | KeyCode::Char('C')) => self
                .selected_runtime_server()
                .map(|entry| Command::TestMcpConnectivity {
                    server_id: entry.server_id.clone(),
                }),
            (McpOverlayTab::Runtime, KeyCode::Char('r') | KeyCode::Char('R')) => self
                .selected_runtime_server()
                .map(|entry| Command::RefreshMcpTools {
                    server_id: entry.server_id.clone(),
                }),
            (McpOverlayTab::Tools, KeyCode::Char('r') | KeyCode::Char('R')) => self
                .selected_server_id()
                .map(|server_id| Command::CheckMcpHealth {
                    server_id: server_id.to_string(),
                }),
            (McpOverlayTab::Tools, KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::Enter) => {
                self.selected_tool()
                    .map(|tool| Command::SetMcpToolDisabled {
                        server_id: tool.server_id.clone(),
                        tool_name: tool.name.clone(),
                        disabled: !tool.disabled,
                    })
            }
            (McpOverlayTab::Resources, KeyCode::Char('r') | KeyCode::Char('R')) => self
                .selected_server_id()
                .map(|server_id| Command::ListMcpResources {
                    server_id: server_id.to_string(),
                }),
            (McpOverlayTab::Resources, KeyCode::Enter) => {
                self.selected_resource()
                    .map(|resource| Command::ReadMcpResource {
                        server_id: resource.server_id.clone(),
                        uri: resource.uri.clone(),
                    })
            }
            (McpOverlayTab::Prompts, KeyCode::Char('r') | KeyCode::Char('R')) => self
                .selected_server_id()
                .map(|server_id| Command::ListMcpPrompts {
                    server_id: server_id.to_string(),
                }),
            (McpOverlayTab::Settings, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_setting()
                .filter(|setting| setting.writable)
                .map(|setting| Command::SetMcpServerEnabled {
                    server_id: setting.id.clone(),
                    enabled: !setting.enabled,
                }),
            (McpOverlayTab::Settings, KeyCode::Char('o') | KeyCode::Char('O')) => {
                Some(Command::OpenMcpConfig)
            }
            (McpOverlayTab::Settings, KeyCode::Char('d') | KeyCode::Char('D')) => self
                .selected_setting()
                .map(|setting| Command::DisableMcpServerAtScope {
                    server_id: setting.id.clone(),
                }),
            (McpOverlayTab::Settings, KeyCode::Char('a') | KeyCode::Char('A')) => self
                .selected_setting()
                .map(|setting| Command::EnableMcpServerAtScope {
                    server_id: setting.id.clone(),
                }),
            (
                McpOverlayTab::Settings,
                KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete,
            ) => self
                .selected_setting()
                .filter(|setting| setting.writable)
                .map(|setting| Command::DeleteMcpServerSettings {
                    server_id: setting.id.clone(),
                }),
            (
                McpOverlayTab::Installed,
                KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Char('u') | KeyCode::Char('U'),
            ) => self
                .selected_installed()
                .map(|entry| Command::UninstallMcpServer {
                    server_id: entry.server_id.clone(),
                }),
            (McpOverlayTab::Catalog, KeyCode::Char('i') | KeyCode::Char('I')) => self
                .selected_catalog_entry()
                .map(|entry| Command::InstallMcpServer {
                    request: agent_core::facade::InstallRequest {
                        catalog_id: entry.id.clone(),
                        source: entry.source.clone(),
                        server_id_override: None,
                        env_overrides: BTreeMap::new(),
                        trust_grant: false,
                        auto_start: true,
                    },
                }),
            (McpOverlayTab::Sources, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_source()
                .map(|source| Command::SetMcpCatalogSourceEnabled {
                    source_id: source.id.clone(),
                    enabled: !source.enabled,
                }),
            (McpOverlayTab::Sources, KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete) => {
                self.selected_source()
                    .filter(|source| source.id != "builtin")
                    .map(|source| Command::RemoveMcpCatalogSource {
                        source_id: source.id.clone(),
                    })
            }
            (McpOverlayTab::Sources, KeyCode::Char('o') | KeyCode::Char('O')) => {
                Some(Command::OpenMcpConfig)
            }
            _ => None,
        }
    }
}

fn render_mcp_overlay(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
    state: &mut McpOverlayRenderState<'_>,
) {
    let modal_width = 96.min(area.width.saturating_sub(4));
    let modal_height = 24.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " 🔌 MCP Servers ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if inner.height < 5 {
        return;
    }

    match overlay.mode {
        McpOverlayMode::ServerEditor => {
            render_server_editor(inner, frame, overlay);
            return;
        }
        McpOverlayMode::SourceEditor => {
            render_source_editor(inner, frame, overlay);
            return;
        }
        McpOverlayMode::List => {}
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    render_tabs(chunks[0], frame, overlay);
    match overlay.tab {
        McpOverlayTab::Runtime => {
            render_runtime(chunks[1], frame, overlay, state.runtime);
        }
        McpOverlayTab::Settings => {
            render_settings(chunks[1], frame, &overlay.settings, state.settings);
        }
        McpOverlayTab::Installed => {
            render_installed(chunks[1], frame, &overlay.installed, state.installed);
        }
        McpOverlayTab::Catalog => {
            render_catalog(chunks[1], frame, &overlay.catalog, state.catalog);
        }
        McpOverlayTab::Sources => {
            render_sources(chunks[1], frame, &overlay.sources, state.sources);
        }
        McpOverlayTab::Tools => {
            render_tools(chunks[1], frame, overlay, state.tools);
        }
        McpOverlayTab::Resources => {
            render_resources(chunks[1], frame, overlay, state.resources);
        }
        McpOverlayTab::Prompts => {
            render_prompts(chunks[1], frame, overlay, state.prompts);
        }
    }
    render_hints(chunks[2], frame, overlay.tab);
}

fn render_tabs(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let mut spans = Vec::new();
    for tab in [
        McpOverlayTab::Runtime,
        McpOverlayTab::Settings,
        McpOverlayTab::Installed,
        McpOverlayTab::Catalog,
        McpOverlayTab::Sources,
        McpOverlayTab::Tools,
        McpOverlayTab::Resources,
        McpOverlayTab::Prompts,
    ] {
        let style = if tab == overlay.tab {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!(" {} ", tab.label()), style));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_empty(area: Rect, frame: &mut Frame, label: &'static str) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            label,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}

fn render_runtime(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    let servers = &overlay.runtime_servers;
    if servers.is_empty() {
        render_empty(area, frame, "No MCP runtime servers configured");
        return;
    }
    let items: Vec<ListItem> = servers
        .iter()
        .map(|s| {
            let (status_label, status_color) = match s.status {
                McpServerStatusView::Running => ("running ", Color::Green),
                McpServerStatusView::Starting => ("starting", Color::Yellow),
                McpServerStatusView::Stopped => ("stopped ", Color::Gray),
                McpServerStatusView::Failed => ("failed  ", Color::Red),
            };
            let trust_label = if s.trusted { " trusted" } else { "" };
            let health = overlay.health.get(&s.server_id);
            let health_label = health
                .map(|state| {
                    if state.healthy {
                        format!(" health:ok({})", state.tool_count)
                    } else if let Some(error) = &state.error {
                        format!(" health:fail({})", clip(error, 18))
                    } else {
                        " health:fail".to_string()
                    }
                })
                .unwrap_or_default();
            let health_color = match health {
                Some(state) if state.healthy => Color::Green,
                Some(_) => Color::Red,
                None => Color::DarkGray,
            };
            let connectivity = overlay.connectivity.get(&s.server_id);
            let connectivity_label = connectivity
                .map(|state| {
                    if state.connected {
                        let count = state
                            .tool_count
                            .map(|tool_count| format!("({tool_count})"))
                            .unwrap_or_default();
                        format!(" conn:ok{count}")
                    } else {
                        " conn:fail".to_string()
                    }
                })
                .unwrap_or_default();
            let connectivity_color = match connectivity {
                Some(state) if state.connected => Color::Green,
                Some(_) => Color::Red,
                None => Color::DarkGray,
            };
            ListItem::new(Line::from(vec![
                Span::styled(status_label, Style::default().fg(status_color)),
                Span::raw("  "),
                Span::styled(
                    s.server_id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  tools:{}", s.tool_count),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(trust_label, Style::default().fg(Color::Magenta)),
                Span::styled(health_label, Style::default().fg(health_color)),
                Span::styled(connectivity_label, Style::default().fg(connectivity_color)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_settings(
    area: Rect,
    frame: &mut Frame,
    settings: &[McpServerSettingsView],
    state: &mut ListState,
) {
    if settings.is_empty() {
        render_empty(area, frame, "No MCP server settings configured");
        return;
    }
    let items: Vec<ListItem> = settings
        .iter()
        .map(|setting| {
            let enabled_label = if setting.enabled {
                "enabled "
            } else {
                "disabled"
            };
            let enabled_color = if setting.enabled {
                Color::Green
            } else {
                Color::DarkGray
            };
            let writable = if setting.writable {
                " writable"
            } else {
                " read-only"
            };
            let tools = setting
                .tool_count
                .map(|count| format!(" tools:{count}"))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    setting.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", setting.runtime_status),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("  [{}{}]", setting.source, writable),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(tools, Style::default().fg(Color::Cyan)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_installed(
    area: Rect,
    frame: &mut Frame,
    installed: &[InstalledEntry],
    state: &mut ListState,
) {
    if installed.is_empty() {
        render_empty(area, frame, "No MCP marketplace servers installed");
        return;
    }
    let items: Vec<ListItem> = installed
        .iter()
        .map(|entry| {
            let running = if entry.running { "running" } else { "stopped" };
            let source = entry.source.as_deref().unwrap_or("manual");
            let catalog = entry.catalog_id.as_deref().unwrap_or("unknown");
            ListItem::new(Line::from(vec![
                Span::styled(
                    running,
                    Style::default().fg(if entry.running {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
                Span::raw("  "),
                Span::styled(
                    entry.server_id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {catalog}@{source}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_catalog(area: Rect, frame: &mut Frame, catalog: &[ServerEntry], state: &mut ListState) {
    if catalog.is_empty() {
        render_empty(area, frame, "No MCP catalog entries available");
        return;
    }
    let items: Vec<ListItem> = catalog
        .iter()
        .map(|entry| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    entry.display_name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  @{}", entry.source),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("  {}", entry.trust),
                    Style::default().fg(Color::Magenta),
                ),
                Span::raw("  "),
                Span::styled(entry.summary.clone(), Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_sources(
    area: Rect,
    frame: &mut Frame,
    sources: &[CatalogSourceView],
    state: &mut ListState,
) {
    if sources.is_empty() {
        render_empty(area, frame, "No MCP catalog sources configured");
        return;
    }
    let items: Vec<ListItem> = sources
        .iter()
        .map(|source| {
            let enabled_label = if source.enabled {
                "enabled "
            } else {
                "disabled"
            };
            let enabled_color = if source.enabled {
                Color::Green
            } else {
                Color::DarkGray
            };
            let location = if source.url.is_empty() {
                "builtin".to_string()
            } else {
                source.url.clone()
            };
            ListItem::new(Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    source.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", source.kind),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(location, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_tools(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    let Some(server_id) = overlay.selected_server_id() else {
        render_empty(area, frame, "Select a runtime server before browsing tools");
        return;
    };
    let tools = overlay.current_tools();
    if tools.is_empty() {
        let label = if overlay.health.contains_key(server_id) {
            "No MCP tools discovered for selected server"
        } else {
            "Press [r] to health-check selected server and load tools"
        };
        render_empty(area, frame, label);
        return;
    }
    let items: Vec<ListItem> = tools
        .iter()
        .map(|tool| {
            let state_label = if tool.disabled {
                "disabled"
            } else {
                "enabled "
            };
            let state_color = if tool.disabled {
                Color::DarkGray
            } else {
                Color::Green
            };
            let description = tool
                .description
                .as_ref()
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(state_label, Style::default().fg(state_color)),
                Span::raw("  "),
                Span::styled(
                    tool.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(description, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_resources(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    if overlay.selected_server_id().is_none() {
        render_empty(
            area,
            frame,
            "Select a runtime server before browsing resources",
        );
        return;
    }
    let resources = overlay.current_resources();
    if resources.is_empty() {
        render_empty(
            area,
            frame,
            "Press [r] to list resources for selected server",
        );
        return;
    }
    let items: Vec<ListItem> = resources
        .iter()
        .map(|resource| {
            let mime = resource
                .mime_type
                .as_ref()
                .map(|value| format!("  {value}"))
                .unwrap_or_default();
            let preview = overlay
                .resource_previews
                .get(&resource_preview_key(&resource.server_id, &resource.uri))
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(
                    resource.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", clip(&resource.uri, 42)),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(mime, Style::default().fg(Color::DarkGray)),
                Span::styled(preview, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_prompts(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    if overlay.selected_server_id().is_none() {
        render_empty(
            area,
            frame,
            "Select a runtime server before browsing prompts",
        );
        return;
    }
    let prompts = overlay.current_prompts();
    if prompts.is_empty() {
        render_empty(area, frame, "Press [r] to list prompts for selected server");
        return;
    }
    let items: Vec<ListItem> = prompts
        .iter()
        .map(|prompt| {
            let description = prompt
                .description
                .as_ref()
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(
                    prompt.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  args:{}", prompt.argument_count),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(description, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_server_editor(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let list_height = area.height.saturating_sub(1);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );
    let items = SERVER_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.server_field_index {
                "> "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("{:<12}", server_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    server_field_value(&overlay.server_draft, *field),
                    Style::default().fg(Color::Gray),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), list_area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "[Tab/Up/Down] field  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("[s/e/h] transport  ", Style::default().fg(Color::Cyan)),
            Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
            Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
        ])),
        hint_area,
    );
}

fn render_source_editor(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let list_height = area.height.saturating_sub(1);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );
    let items = SOURCE_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.source_field_index {
                "> "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("{:<12}", source_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    source_field_value(&overlay.source_draft, *field),
                    Style::default().fg(Color::Gray),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), list_area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "[Tab/Up/Down] field  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
            Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
        ])),
        hint_area,
    );
}

fn server_field_label(field: ServerEditorField) -> &'static str {
    match field {
        ServerEditorField::Name => "Name",
        ServerEditorField::Transport => "Transport",
        ServerEditorField::CommandOrUrl => "Command/URL",
        ServerEditorField::Args => "Args",
        ServerEditorField::Description => "Description",
        ServerEditorField::Enabled => "Enabled",
    }
}

fn server_field_value(draft: &ServerDraft, field: ServerEditorField) -> String {
    match field {
        ServerEditorField::Name => draft.name.clone(),
        ServerEditorField::Transport => server_transport_label(draft.transport).to_string(),
        ServerEditorField::CommandOrUrl if draft.transport == ServerTransportDraft::Stdio => {
            draft.command.clone()
        }
        ServerEditorField::CommandOrUrl => draft.url.clone(),
        ServerEditorField::Args => {
            if draft.transport == ServerTransportDraft::Stdio {
                draft.args_text.clone()
            } else {
                "n/a".to_string()
            }
        }
        ServerEditorField::Description => draft.description.clone(),
        ServerEditorField::Enabled => draft.enabled.to_string(),
    }
}

fn source_field_label(field: SourceEditorField) -> &'static str {
    match field {
        SourceEditorField::Id => "ID",
        SourceEditorField::DisplayName => "Name",
        SourceEditorField::Url => "URL",
        SourceEditorField::ApiKeyEnv => "API key env",
        SourceEditorField::Priority => "Priority",
        SourceEditorField::DefaultTrust => "Trust",
        SourceEditorField::Enabled => "Enabled",
    }
}

fn source_field_value(draft: &SourceDraft, field: SourceEditorField) -> String {
    match field {
        SourceEditorField::Id => draft.id.clone(),
        SourceEditorField::DisplayName => draft.display_name.clone(),
        SourceEditorField::Url => draft.url.clone(),
        SourceEditorField::ApiKeyEnv => draft.api_key_env.clone(),
        SourceEditorField::Priority => draft.priority.clone(),
        SourceEditorField::DefaultTrust => draft.default_trust.clone(),
        SourceEditorField::Enabled => draft.enabled.to_string(),
    }
}

fn server_transport_label(transport: ServerTransportDraft) -> &'static str {
    match transport {
        ServerTransportDraft::Stdio => "stdio",
        ServerTransportDraft::Sse => "sse",
        ServerTransportDraft::StreamableHttp => "streamable_http",
    }
}

fn split_args(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
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

fn resource_preview_key(server_id: &str, uri: &str) -> String {
    format!("{server_id}\n{uri}")
}

fn clip(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let clipped: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{clipped}...")
    } else {
        clipped
    }
}

fn render_list(area: Rect, frame: &mut Frame, items: Vec<ListItem>, state: &mut ListState) {
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, state);
}

fn render_hints(area: Rect, frame: &mut Frame, tab: McpOverlayTab) {
    let action = match tab {
        McpOverlayTab::Runtime => {
            "[Enter] start/stop  [t] trust/revoke  [h] health  [c] test  [r] tools  "
        }
        McpOverlayTab::Settings => {
            "[n] new  [Enter] edit  [e] enable  [d/a] project off/on  [o] config  [x] delete  "
        }
        McpOverlayTab::Installed => "[x/u] uninstall  [r] reload  ",
        McpOverlayTab::Catalog => "[i] install  [r] reload  ",
        McpOverlayTab::Sources => {
            "[n] new  [e] enable source  [x] remove  [o] config  [r] reload  "
        }
        McpOverlayTab::Tools => "[r] health  [e/Enter] enable tool  ",
        McpOverlayTab::Resources => "[r] list  [Enter] read  ",
        McpOverlayTab::Prompts => "[r] list  ",
    };
    let hints = Line::from(vec![
        Span::styled("[Tab] tab  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled(action, Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), area);
}

impl Component for McpOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match self.mode {
            McpOverlayMode::ServerEditor => {
                commands.extend(self.handle_server_editor_key(key.code, key.modifiers));
                return (effects, commands);
            }
            McpOverlayMode::SourceEditor => {
                commands.extend(self.handle_source_editor_key(key.code, key.modifiers));
                return (effects, commands);
            }
            McpOverlayMode::List => {}
        }

        match key.code {
            KeyCode::Tab => {
                self.tab = self.tab.next();
                self.ensure_selection();
            }
            KeyCode::BackTab => {
                self.tab = self.tab.previous();
                self.ensure_selection();
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('n') | KeyCode::Char('N') if self.tab == McpOverlayTab::Settings => {
                self.start_server_create();
            }
            KeyCode::Enter if self.tab == McpOverlayTab::Settings => {
                self.start_server_edit_selected();
            }
            KeyCode::Char('n') | KeyCode::Char('N') if self.tab == McpOverlayTab::Sources => {
                self.start_source_create();
            }
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissMcpOverlay);
            }
            KeyCode::Char('r') | KeyCode::Char('R')
                if matches!(
                    self.tab,
                    McpOverlayTab::Settings
                        | McpOverlayTab::Installed
                        | McpOverlayTab::Catalog
                        | McpOverlayTab::Sources
                ) =>
            {
                commands.push(Command::OpenMcpOverlay);
            }
            key => {
                if let Some(command) = self.command_for_current_tab(key) {
                    commands.push(command);
                }
            }
        }

        (effects, commands)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowMcpOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissMcpOverlay => self.hide(),
            CrossPanelEffect::McpToolsLoaded {
                server_id,
                tools,
                healthy,
                error,
            } => {
                self.tools.insert(server_id.clone(), tools.clone());
                self.health.insert(
                    server_id.clone(),
                    McpHealthState {
                        healthy: *healthy,
                        tool_count: tools.len(),
                        error: error.clone(),
                    },
                );
                self.ensure_selection();
            }
            CrossPanelEffect::McpConnectivityChecked(entry) => {
                self.connectivity
                    .insert(entry.server_id.clone(), entry.clone());
            }
            CrossPanelEffect::McpResourcesLoaded {
                server_id,
                resources,
            } => {
                self.resources.insert(server_id.clone(), resources.clone());
                self.ensure_selection();
            }
            CrossPanelEffect::McpPromptsLoaded { server_id, prompts } => {
                self.prompts.insert(server_id.clone(), prompts.clone());
                self.ensure_selection();
            }
            CrossPanelEffect::McpResourceRead {
                server_id,
                uri,
                preview,
            } => {
                self.resource_previews
                    .insert(resource_preview_key(server_id, uri), preview.clone());
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut runtime_state = self.runtime_state;
        let mut settings_state = self.settings_state;
        let mut installed_state = self.installed_state;
        let mut catalog_state = self.catalog_state;
        let mut sources_state = self.sources_state;
        let mut tools_state = self.tools_state;
        let mut resources_state = self.resources_state;
        let mut prompts_state = self.prompts_state;
        let mut render_state = McpOverlayRenderState {
            runtime: &mut runtime_state,
            settings: &mut settings_state,
            installed: &mut installed_state,
            catalog: &mut catalog_state,
            sources: &mut sources_state,
            tools: &mut tools_state,
            resources: &mut resources_state,
            prompts: &mut prompts_state,
        };
        render_mcp_overlay(area, frame, self, &mut render_state);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use agent_core::facade::{
        CatalogSourceView, InstalledEntry, McpServerSettingsView, ServerEntry,
    };

    use crate::components::{
        FocusTarget, McpOverlaySnapshot, McpPromptEntry, McpResourceEntry, McpServerStatusView,
        McpToolEntry,
    };

    fn entry(id: &str, status: McpServerStatusView, trusted: bool, tools: usize) -> McpServerEntry {
        McpServerEntry {
            server_id: id.to_string(),
            status,
            trusted,
            tool_count: tools,
        }
    }

    fn setting(id: &str, enabled: bool) -> McpServerSettingsView {
        McpServerSettingsView {
            id: id.to_string(),
            name: id.to_string(),
            transport: "stdio".to_string(),
            enabled,
            runtime_status: "stopped".to_string(),
            trusted: false,
            tool_count: Some(2),
            last_error: None,
            writable: true,
            config_path: Some("/tmp/kairox/config.toml".to_string()),
            description: Some(format!("{id} settings")),
            source: "user".to_string(),
            verified: false,
        }
    }

    fn installed(id: &str, running: bool) -> InstalledEntry {
        InstalledEntry {
            server_id: id.to_string(),
            catalog_id: Some(format!("{id}-catalog")),
            source: Some("builtin".to_string()),
            display_name: id.to_string(),
            installed_at: "2026-05-21T00:00:00Z".to_string(),
            running,
        }
    }

    fn catalog_entry(id: &str, source: &str) -> ServerEntry {
        ServerEntry {
            id: id.to_string(),
            source: source.to_string(),
            display_name: format!("{id} MCP"),
            summary: format!("{id} summary"),
            description: format!("{id} description"),
            categories: vec!["dev".to_string()],
            tags: vec!["local".to_string()],
            author: Some("Kairox".to_string()),
            homepage: None,
            version: Some("1.0.0".to_string()),
            trust: "verified".to_string(),
            verified: true,
            icon: None,
            install_spec_json: "{}".to_string(),
            requirements_json: "[]".to_string(),
            default_env_json: "[]".to_string(),
        }
    }

    fn source(id: &str, enabled: bool) -> CatalogSourceView {
        CatalogSourceView {
            id: id.to_string(),
            display_name: id.to_string(),
            kind: "mcp_registry".to_string(),
            url: format!("https://example.com/{id}"),
            api_key_env: None,
            priority: 10,
            default_trust: "community".to_string(),
            enabled,
            cache_ttl_seconds: Some(300),
            last_error: None,
        }
    }

    fn tool(name: &str, disabled: bool) -> McpToolEntry {
        McpToolEntry {
            server_id: "alpha".to_string(),
            name: name.to_string(),
            description: Some(format!("{name} tool")),
            input_schema: None,
            disabled,
        }
    }

    fn resource(uri: &str) -> McpResourceEntry {
        McpResourceEntry {
            server_id: "alpha".to_string(),
            uri: uri.to_string(),
            name: "App log".to_string(),
            description: Some("Application log".to_string()),
            mime_type: Some("text/plain".to_string()),
        }
    }

    fn prompt(name: &str) -> McpPromptEntry {
        McpPromptEntry {
            server_id: "alpha".to_string(),
            name: name.to_string(),
            description: Some(format!("{name} prompt")),
            argument_count: 2,
        }
    }

    fn advance_tabs(overlay: &mut McpOverlay, count: usize) {
        for _ in 0..count {
            let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        }
    }

    fn snapshot() -> McpOverlaySnapshot {
        McpOverlaySnapshot {
            runtime_servers: vec![
                entry("alpha", McpServerStatusView::Running, true, 3),
                entry("beta", McpServerStatusView::Stopped, false, 0),
            ],
            settings: vec![setting("alpha", true), setting("beta", false)],
            installed: vec![installed("alpha", true)],
            catalog: vec![catalog_entry("filesystem", "builtin")],
            sources: vec![source("registry", true)],
        }
    }

    fn runtime_snapshot(runtime_servers: Vec<McpServerEntry>) -> McpOverlaySnapshot {
        McpOverlaySnapshot {
            runtime_servers,
            settings: Vec::new(),
            installed: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
        }
    }

    fn test_ctx() -> EventContext<'static> {
        use agent_core::projection::SessionProjection;
        static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
        let projection = PROJECTION.get_or_init(SessionProjection::default);
        static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
            std::sync::OnceLock::new();
        let sessions = SESSIONS.get_or_init(Vec::new);
        EventContext {
            focus: FocusTarget::McpOverlay,
            current_session: projection,
            projects: &[],
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: true,
            workspace_id: Box::leak(Box::new(agent_core::WorkspaceId::new())),
            current_session_id: Box::leak(Box::new(None)),
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn type_text(overlay: &mut McpOverlay, text: &str) {
        for ch in text.chars() {
            let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char(ch)));
        }
    }

    #[test]
    fn overlay_invisible_by_default() {
        let overlay = McpOverlay::new();
        assert!(!overlay.is_visible());
        assert!(overlay.servers().is_empty());
    }

    #[test]
    fn renders_server_list_from_runtime() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![
            entry("alpha", McpServerStatusView::Running, true, 3),
            entry("beta", McpServerStatusView::Stopped, false, 0),
        ]));
        assert!(overlay.is_visible());
        assert_eq!(overlay.servers().len(), 2);
        assert_eq!(overlay.selected_index(), Some(0));
        // Render into a test buffer to ensure no panic and selection drawn.
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| overlay.render(f.area(), f))
            .expect("render");
    }

    #[test]
    fn j_and_k_navigate_selection() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![
            entry("alpha", McpServerStatusView::Running, false, 1),
            entry("beta", McpServerStatusView::Stopped, false, 0),
            entry("gamma", McpServerStatusView::Failed, false, 0),
        ]));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(2));
        // Down again clamps at last index.
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Down));
        assert_eq!(overlay.selected_index(), Some(2));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Up));
        assert_eq!(overlay.selected_index(), Some(0));
        // Up at top stays at 0.
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(0));
    }

    #[test]
    fn enter_starts_stopped_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "beta",
            McpServerStatusView::Stopped,
            false,
            0,
        )]));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            Command::StartMcpServer { server_id } if server_id == "beta"
        ));
    }

    #[test]
    fn enter_stops_running_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            true,
            5,
        )]));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            Command::StopMcpServer { server_id } if server_id == "alpha"
        ));
    }

    #[test]
    fn enter_starts_failed_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "crash",
            McpServerStatusView::Failed,
            false,
            0,
        )]));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::StartMcpServer { server_id } if server_id == "crash"
        ));
    }

    #[test]
    fn t_emits_trust_command_for_selected_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![
            entry("alpha", McpServerStatusView::Running, false, 1),
            entry("beta", McpServerStatusView::Running, false, 1),
        ]));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
        assert!(matches!(
            &commands[0],
            Command::TrustMcpServer { server_id } if server_id == "beta"
        ));
    }

    #[test]
    fn t_emits_revoke_command_for_trusted_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            true,
            1,
        )]));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
        assert!(matches!(
            &commands[..],
            [Command::RevokeMcpTrust { server_id }] if server_id == "alpha"
        ));
    }

    #[test]
    fn r_emits_refresh_tools_command() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            false,
            1,
        )]));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
        assert!(matches!(
            &commands[0],
            Command::RefreshMcpTools { server_id } if server_id == "alpha"
        ));
    }

    #[test]
    fn runtime_tab_emits_health_and_connectivity_commands() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            false,
            1,
        )]));

        let (_, health_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('h')));
        assert!(matches!(
            &health_commands[..],
            [Command::CheckMcpHealth { server_id }] if server_id == "alpha"
        ));

        let (_, connectivity_commands) =
            overlay.handle_event(&test_ctx(), &key(KeyCode::Char('c')));
        assert!(matches!(
            &connectivity_commands[..],
            [Command::TestMcpConnectivity { server_id }] if server_id == "alpha"
        ));
    }

    #[test]
    fn tools_tab_toggles_selected_tool_disabled_state() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            false,
            2,
        )]));
        overlay.handle_effect(&CrossPanelEffect::McpToolsLoaded {
            server_id: "alpha".to_string(),
            tools: vec![tool("search", false), tool("write", true)],
            healthy: true,
            error: None,
        });
        advance_tabs(&mut overlay, 5);

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
        assert!(matches!(
            &commands[..],
            [Command::SetMcpToolDisabled {
                server_id,
                tool_name,
                disabled,
            }] if server_id == "alpha" && tool_name == "write" && !disabled
        ));
    }

    #[test]
    fn resources_tab_lists_and_reads_selected_resource() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            false,
            1,
        )]));
        advance_tabs(&mut overlay, 6);

        let (_, list_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
        assert!(matches!(
            &list_commands[..],
            [Command::ListMcpResources { server_id }] if server_id == "alpha"
        ));

        overlay.handle_effect(&CrossPanelEffect::McpResourcesLoaded {
            server_id: "alpha".to_string(),
            resources: vec![resource("file://logs/app.log")],
        });
        let (_, read_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(
            &read_commands[..],
            [Command::ReadMcpResource { server_id, uri }]
                if server_id == "alpha" && uri == "file://logs/app.log"
        ));
    }

    #[test]
    fn prompts_tab_lists_prompts_for_selected_runtime_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            false,
            1,
        )]));
        advance_tabs(&mut overlay, 7);

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
        assert!(matches!(
            &commands[..],
            [Command::ListMcpPrompts { server_id }] if server_id == "alpha"
        ));

        overlay.handle_effect(&CrossPanelEffect::McpPromptsLoaded {
            server_id: "alpha".to_string(),
            prompts: vec![prompt("summarize")],
        });
        assert_eq!(overlay.selected_index(), Some(0));
    }

    #[test]
    fn esc_hides_and_emits_dismiss_effect() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(vec![entry(
            "alpha",
            McpServerStatusView::Running,
            false,
            1,
        )]));
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
        assert!(commands.is_empty());
        assert!(effects.contains(&CrossPanelEffect::DismissMcpOverlay));
        assert!(!overlay.is_visible());
    }

    #[test]
    fn ignores_keys_when_hidden() {
        let mut overlay = McpOverlay::new();
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
    }

    #[test]
    fn show_effect_makes_visible() {
        let mut overlay = McpOverlay::new();
        overlay.handle_effect(&CrossPanelEffect::ShowMcpOverlay(runtime_snapshot(vec![
            entry("alpha", McpServerStatusView::Running, false, 1),
        ])));
        assert!(overlay.is_visible());
        assert_eq!(overlay.servers().len(), 1);
    }

    #[test]
    fn enter_with_no_servers_emits_nothing() {
        let mut overlay = McpOverlay::new();
        overlay.show(runtime_snapshot(Vec::new()));
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
    }

    #[test]
    fn tabs_preserve_independent_selection() {
        let mut overlay = McpOverlay::new();
        overlay.show(snapshot());

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        assert_eq!(overlay.selected_index(), Some(0));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));
        assert_eq!(overlay.selected_index(), Some(1));
    }

    #[test]
    fn settings_tab_emits_enable_and_delete_commands() {
        let mut overlay = McpOverlay::new();
        overlay.show(snapshot());
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (_, enable_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
        assert!(matches!(
            &enable_commands[..],
            [Command::SetMcpServerEnabled { server_id, enabled }]
                if server_id == "alpha" && !enabled
        ));

        let (_, delete_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
        assert!(matches!(
            &delete_commands[..],
            [Command::DeleteMcpServerSettings { server_id }] if server_id == "alpha"
        ));
    }

    #[test]
    fn settings_tab_opens_config_and_updates_project_scope_disablement() {
        let mut overlay = McpOverlay::new();
        overlay.show(snapshot());
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (_, open_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('o')));
        assert!(matches!(&open_commands[..], [Command::OpenMcpConfig]));

        let (_, disable_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('d')));
        assert!(matches!(
            &disable_commands[..],
            [Command::DisableMcpServerAtScope { server_id }] if server_id == "alpha"
        ));

        let (_, enable_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));
        assert!(matches!(
            &enable_commands[..],
            [Command::EnableMcpServerAtScope { server_id }] if server_id == "alpha"
        ));
    }

    #[test]
    fn settings_tab_server_editor_saves_new_stdio_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(snapshot());
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('n')));
        assert!(commands.is_empty());
        type_text(&mut overlay, "gamma");
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        type_text(&mut overlay, "npx");
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        type_text(&mut overlay, "-y @modelcontextprotocol/server-filesystem");
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

        assert!(matches!(
            &commands[..],
            [Command::SaveMcpServerSettings { input }]
                if input.name == "gamma"
                    && input.enabled
                    && input.description.is_none()
                    && matches!(
                        &input.transport,
                        agent_core::facade::McpServerSettingsTransport::Stdio { command, args, env }
                            if command == "npx"
                                && args.as_slice() == [
                                    "-y".to_string(),
                                    "@modelcontextprotocol/server-filesystem".to_string()
                                ]
                                && env.is_empty()
                    )
        ));
    }

    #[test]
    fn settings_tab_enter_edits_selected_server() {
        let mut overlay = McpOverlay::new();
        overlay.show(snapshot());
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(commands.is_empty());

        assert_eq!(overlay.server_draft_name_for_test(), Some("alpha"));
    }

    #[test]
    fn catalog_and_installed_tabs_emit_install_uninstall_commands() {
        let mut overlay = McpOverlay::new();
        overlay.show(snapshot());

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        let (_, uninstall_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
        assert!(matches!(
            &uninstall_commands[..],
            [Command::UninstallMcpServer { server_id }] if server_id == "alpha"
        ));

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        let (_, install_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
        assert!(matches!(
            &install_commands[..],
            [Command::InstallMcpServer { request }]
                if request.catalog_id == "filesystem"
                    && request.source == "builtin"
                    && request.server_id_override.is_none()
                    && request.env_overrides == BTreeMap::new()
                    && request.auto_start
                    && !request.trust_grant
        ));
    }

    #[test]
    fn sources_tab_emits_source_enable_command() {
        let mut overlay = McpOverlay::new();
        overlay.show(snapshot());
        advance_tabs(&mut overlay, 4);

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
        assert!(matches!(
            &commands[..],
            [Command::SetMcpCatalogSourceEnabled { source_id, enabled }]
                if source_id == "registry" && !enabled
        ));
    }

    #[test]
    fn sources_tab_adds_and_removes_catalog_sources() {
        let mut overlay = McpOverlay::new();
        overlay.show(snapshot());
        advance_tabs(&mut overlay, 4);

        let (_, remove_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
        assert!(matches!(
            &remove_commands[..],
            [Command::RemoveMcpCatalogSource { source_id }] if source_id == "registry"
        ));

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('n')));
        assert!(commands.is_empty());
        type_text(&mut overlay, "corp");
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        type_text(&mut overlay, "Corporate Registry");
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        type_text(&mut overlay, "https://registry.example.com/catalog.json");
        let (_, add_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

        assert!(matches!(
            &add_commands[..],
            [Command::AddMcpCatalogSource { request }]
                if request.id == "corp"
                    && request.display_name == "Corporate Registry"
                    && request.kind == "mcp_registry"
                    && request.url == "https://registry.example.com/catalog.json"
                    && request.api_key_env.is_none()
                    && request.priority == Some(100)
                    && request.default_trust.as_deref() == Some("community")
                    && request.enabled == Some(true)
                    && request.cache_ttl_seconds.is_none()
        ));
    }
}
