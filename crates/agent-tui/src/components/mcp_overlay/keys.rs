//! Keyboard input handling for the MCP overlay — methods that map key events
//! to navigation, mode changes, or [`Command`] values.
//!
//! Separated from [`super::state`] to keep the data model and selection queries
//! in one file and the interactive keyboard-dispatch logic in another.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::components::{Command, McpServerStatusView};

use super::state::McpOverlay;
use super::types::{McpOverlayMode, McpOverlayTab};

impl McpOverlay {
    pub(super) fn move_down(&mut self) {
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

    pub(super) fn move_up(&mut self) {
        if self.current_len() == 0 {
            return;
        }
        let next = match self.current_selected() {
            Some(i) if i > 0 => i - 1,
            _ => 0,
        };
        self.select_current(Some(next));
    }

    pub(super) fn handle_catalog_filter_key(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        match key {
            KeyCode::Enter | KeyCode::Esc => {
                self.mode = McpOverlayMode::List;
            }
            KeyCode::Backspace => {
                self.catalog_keyword.pop();
                self.ensure_selection();
            }
            KeyCode::Delete => {
                self.catalog_keyword.clear();
                self.ensure_selection();
            }
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.catalog_keyword.clear();
                self.ensure_selection();
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.catalog_keyword.push(ch);
                self.ensure_selection();
            }
            _ => {}
        }
    }

    pub(super) fn command_for_current_tab(&self, key: KeyCode) -> Option<Command> {
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
