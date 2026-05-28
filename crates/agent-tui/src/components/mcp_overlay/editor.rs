use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::components::Command;

pub(super) use super::editor_types::*;
use super::state::McpOverlay;
use super::types::McpOverlayMode;

impl McpOverlay {
    pub(super) fn start_server_create(&mut self) {
        self.mode = McpOverlayMode::ServerEditor;
        self.server_draft = ServerDraft::new();
        self.server_field_index = 0;
    }

    pub(super) fn start_server_edit_selected(&mut self) {
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

    pub(super) fn start_source_create(&mut self) {
        self.mode = McpOverlayMode::SourceEditor;
        self.source_draft = SourceDraft::new();
        self.source_field_index = 0;
    }

    pub(super) fn start_catalog_install_selected(&mut self) -> Vec<Command> {
        let Some(entry) = self.selected_catalog_entry().cloned() else {
            return Vec::new();
        };
        let config_items = catalog_config_items(&entry);
        if config_items.is_empty() {
            let request = install_request_for_entry(&entry, BTreeMap::new());
            self.mark_catalog_install_started(&request);
            return vec![Command::InstallMcpServer { request }];
        }

        self.mode = McpOverlayMode::CatalogInstallConfig;
        self.catalog_install_draft = CatalogInstallDraft::from_entry(&entry);
        self.catalog_install_field_index = 0;
        Vec::new()
    }

    pub(super) fn current_server_field(&self) -> ServerEditorField {
        SERVER_EDITOR_FIELDS[self.server_field_index]
    }

    pub(super) fn current_source_field(&self) -> SourceEditorField {
        SOURCE_EDITOR_FIELDS[self.source_field_index]
    }

    pub(super) fn move_server_field_down(&mut self) {
        self.server_field_index = (self.server_field_index + 1) % SERVER_EDITOR_FIELDS.len();
    }

    pub(super) fn move_server_field_up(&mut self) {
        self.server_field_index = if self.server_field_index == 0 {
            SERVER_EDITOR_FIELDS.len() - 1
        } else {
            self.server_field_index - 1
        };
    }

    pub(super) fn move_source_field_down(&mut self) {
        self.source_field_index = (self.source_field_index + 1) % SOURCE_EDITOR_FIELDS.len();
    }

    pub(super) fn move_source_field_up(&mut self) {
        self.source_field_index = if self.source_field_index == 0 {
            SOURCE_EDITOR_FIELDS.len() - 1
        } else {
            self.source_field_index - 1
        };
    }

    pub(super) fn move_catalog_install_field_down(&mut self) {
        let len = self.catalog_install_draft.items.len();
        if len > 0 {
            self.catalog_install_field_index = (self.catalog_install_field_index + 1) % len;
        }
    }

    pub(super) fn move_catalog_install_field_up(&mut self) {
        let len = self.catalog_install_draft.items.len();
        if len == 0 {
            return;
        }
        self.catalog_install_field_index = if self.catalog_install_field_index == 0 {
            len - 1
        } else {
            self.catalog_install_field_index - 1
        };
    }

    pub(super) fn handle_server_editor_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Vec<Command> {
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

    pub(super) fn handle_source_editor_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Vec<Command> {
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

    pub(super) fn handle_catalog_install_config_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Vec<Command> {
        match key {
            KeyCode::Tab | KeyCode::Down => self.move_catalog_install_field_down(),
            KeyCode::BackTab | KeyCode::Up => self.move_catalog_install_field_up(),
            KeyCode::Esc => self.mode = McpOverlayMode::List,
            KeyCode::Backspace => self
                .catalog_install_draft
                .backspace(self.catalog_install_field_index),
            KeyCode::Delete => self
                .catalog_install_draft
                .clear_field(self.catalog_install_field_index),
            KeyCode::Enter => {
                if let Some(request) = self.catalog_install_draft.to_request() {
                    self.mode = McpOverlayMode::List;
                    self.mark_catalog_install_started(&request);
                    return vec![Command::InstallMcpServer { request }];
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => self
                .catalog_install_draft
                .push_char(self.catalog_install_field_index, ch),
            _ => {}
        }
        Vec::new()
    }
}
