use std::collections::BTreeMap;

use agent_core::facade::{
    CatalogSourceView, InstallOutcomeView, InstallRequest, InstalledEntry, McpServerSettingsView,
    ServerEntry,
};
use ratatui::widgets::ListState;

use crate::components::{
    McpConnectivityEntry, McpOverlaySnapshot, McpPromptEntry, McpResourceEntry, McpServerEntry,
    McpToolEntry,
};

use super::editor::{CatalogInstallDraft, ServerDraft, SourceDraft};
use super::types::{
    catalog_install_key, trust_rank, CatalogInstallStatus, CatalogTrustFilter, McpHealthState,
    McpOverlayMode, McpOverlayTab,
};

pub struct McpOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) mode: McpOverlayMode,
    pub(super) tab: McpOverlayTab,
    pub(super) runtime_servers: Vec<McpServerEntry>,
    pub(super) settings: Vec<McpServerSettingsView>,
    pub(super) installed: Vec<InstalledEntry>,
    pub(super) catalog: Vec<ServerEntry>,
    pub(super) sources: Vec<CatalogSourceView>,
    pub(super) tools: BTreeMap<String, Vec<McpToolEntry>>,
    pub(super) resources: BTreeMap<String, Vec<McpResourceEntry>>,
    pub(super) prompts: BTreeMap<String, Vec<McpPromptEntry>>,
    pub(super) health: BTreeMap<String, McpHealthState>,
    pub(super) connectivity: BTreeMap<String, McpConnectivityEntry>,
    pub(super) resource_previews: BTreeMap<String, String>,
    pub(super) catalog_install_statuses: BTreeMap<String, CatalogInstallStatus>,
    pub(super) catalog_keyword: String,
    pub(super) catalog_trust_filter: CatalogTrustFilter,
    pub(super) runtime_state: ListState,
    pub(super) settings_state: ListState,
    pub(super) installed_state: ListState,
    pub(super) catalog_state: ListState,
    pub(super) sources_state: ListState,
    pub(super) tools_state: ListState,
    pub(super) resources_state: ListState,
    pub(super) prompts_state: ListState,
    pub(super) server_draft: ServerDraft,
    pub(super) server_field_index: usize,
    pub(super) source_draft: SourceDraft,
    pub(super) source_field_index: usize,
    pub(super) catalog_install_draft: CatalogInstallDraft,
    pub(super) catalog_install_field_index: usize,
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
            catalog_install_statuses: BTreeMap::new(),
            catalog_keyword: String::new(),
            catalog_trust_filter: CatalogTrustFilter::All,
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
            catalog_install_draft: CatalogInstallDraft::new(),
            catalog_install_field_index: 0,
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
        self.catalog_install_statuses.clear();
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
        self.catalog_install_draft = CatalogInstallDraft::new();
        self.catalog_install_field_index = 0;
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
    pub(super) fn server_draft_name_for_test(&self) -> Option<&str> {
        if self.mode == McpOverlayMode::ServerEditor {
            Some(self.server_draft.name.as_str())
        } else {
            None
        }
    }

    pub(super) fn current_len(&self) -> usize {
        match self.tab {
            McpOverlayTab::Runtime => self.runtime_servers.len(),
            McpOverlayTab::Settings => self.settings.len(),
            McpOverlayTab::Installed => self.installed.len(),
            McpOverlayTab::Catalog => self.visible_catalog_len(),
            McpOverlayTab::Sources => self.sources.len(),
            McpOverlayTab::Tools => self.current_tools().len(),
            McpOverlayTab::Resources => self.current_resources().len(),
            McpOverlayTab::Prompts => self.current_prompts().len(),
        }
    }

    pub(super) fn current_selected(&self) -> Option<usize> {
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

    pub(super) fn select_current(&mut self, selected: Option<usize>) {
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

    pub(super) fn ensure_selection(&mut self) {
        let tools_len = self.current_tools().len();
        let resources_len = self.current_resources().len();
        let prompts_len = self.current_prompts().len();
        let catalog_len = self.visible_catalog_len();
        for (len, state) in [
            (self.runtime_servers.len(), &mut self.runtime_state),
            (self.settings.len(), &mut self.settings_state),
            (self.installed.len(), &mut self.installed_state),
            (catalog_len, &mut self.catalog_state),
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

    pub(super) fn selected_runtime_server(&self) -> Option<&McpServerEntry> {
        self.runtime_state
            .selected()
            .and_then(|index| self.runtime_servers.get(index))
    }

    pub(super) fn selected_setting(&self) -> Option<&McpServerSettingsView> {
        self.settings_state
            .selected()
            .and_then(|index| self.settings.get(index))
    }

    pub(super) fn selected_installed(&self) -> Option<&InstalledEntry> {
        self.installed_state
            .selected()
            .and_then(|index| self.installed.get(index))
    }

    pub(super) fn selected_catalog_entry(&self) -> Option<&ServerEntry> {
        let visible_index = self.catalog_state.selected()?;
        let catalog_index = self.visible_catalog_indices().get(visible_index).copied()?;
        self.catalog.get(catalog_index)
    }

    pub(super) fn install_status_for_entry(
        &self,
        entry: &ServerEntry,
    ) -> Option<&CatalogInstallStatus> {
        self.catalog_install_statuses
            .get(&catalog_install_key(&entry.source, &entry.id))
    }

    pub(crate) fn mark_catalog_install_started(&mut self, request: &InstallRequest) {
        self.catalog_install_statuses.insert(
            catalog_install_key(&request.source, &request.catalog_id),
            CatalogInstallStatus::Installing,
        );
    }

    pub(crate) fn mark_catalog_install_outcome(
        &mut self,
        request: &InstallRequest,
        outcome: &InstallOutcomeView,
    ) {
        let status = match outcome.kind.as_str() {
            "installed" => CatalogInstallStatus::Installed {
                server_id: outcome
                    .server_id
                    .clone()
                    .or_else(|| request.server_id_override.clone())
                    .unwrap_or_else(|| request.catalog_id.clone()),
                started: outcome.started.unwrap_or(false),
            },
            "already_installed" => CatalogInstallStatus::AlreadyInstalled {
                server_id: outcome
                    .server_id
                    .clone()
                    .or_else(|| request.server_id_override.clone())
                    .unwrap_or_else(|| request.catalog_id.clone()),
            },
            "runtime_missing" => CatalogInstallStatus::RuntimeMissing {
                missing_runtimes: outcome.missing_runtimes.clone(),
            },
            "invalid_env" => CatalogInstallStatus::MissingEnv {
                missing_env_keys: outcome.missing_env_keys.clone(),
            },
            other => CatalogInstallStatus::Failed {
                message: format!("unexpected outcome {other}"),
            },
        };
        self.catalog_install_statuses.insert(
            catalog_install_key(&request.source, &request.catalog_id),
            status,
        );
    }

    pub(crate) fn mark_catalog_install_failed(
        &mut self,
        request: &InstallRequest,
        message: String,
    ) {
        self.catalog_install_statuses.insert(
            catalog_install_key(&request.source, &request.catalog_id),
            CatalogInstallStatus::Failed { message },
        );
    }

    pub(super) fn visible_catalog_len(&self) -> usize {
        self.catalog
            .iter()
            .filter(|entry| self.catalog_entry_visible(entry))
            .count()
    }

    pub(super) fn visible_catalog_indices(&self) -> Vec<usize> {
        self.catalog
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| self.catalog_entry_visible(entry).then_some(index))
            .collect()
    }

    pub(super) fn visible_catalog_entries(&self) -> Vec<&ServerEntry> {
        self.visible_catalog_indices()
            .into_iter()
            .filter_map(|index| self.catalog.get(index))
            .collect()
    }

    pub(super) fn catalog_entry_visible(&self, entry: &ServerEntry) -> bool {
        if !self.catalog_source_enabled(&entry.source) {
            return false;
        }

        if let Some(min_rank) = self.catalog_trust_filter.min_rank() {
            if trust_rank(&entry.trust) < min_rank {
                return false;
            }
        }

        let keyword = self.catalog_keyword.trim().to_lowercase();
        if keyword.is_empty() {
            return true;
        }

        let haystack = format!(
            "{} {} {} {} {} {}",
            entry.id,
            entry.display_name,
            entry.summary,
            entry.description,
            entry.categories.join(" "),
            entry.tags.join(" ")
        )
        .to_lowercase();
        haystack.contains(&keyword)
    }

    pub(super) fn catalog_source_enabled(&self, source_id: &str) -> bool {
        self.sources
            .iter()
            .find(|source| source.id == source_id)
            .map(|source| source.enabled)
            .unwrap_or(source_id == "builtin")
    }

    pub(super) fn catalog_filters_active(&self) -> bool {
        !self.catalog_keyword.trim().is_empty()
            || self.catalog_trust_filter != CatalogTrustFilter::All
            || self
                .catalog
                .iter()
                .any(|entry| !self.catalog_source_enabled(&entry.source))
    }

    pub(super) fn cycle_catalog_trust_filter(&mut self) {
        self.catalog_trust_filter = self.catalog_trust_filter.next();
        self.ensure_selection();
    }

    pub(super) fn selected_source(&self) -> Option<&CatalogSourceView> {
        self.sources_state
            .selected()
            .and_then(|index| self.sources.get(index))
    }

    pub(super) fn selected_server_id(&self) -> Option<&str> {
        self.selected_runtime_server()
            .map(|entry| entry.server_id.as_str())
    }

    pub(super) fn current_tools(&self) -> &[McpToolEntry] {
        self.selected_server_id()
            .and_then(|server_id| self.tools.get(server_id))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn current_resources(&self) -> &[McpResourceEntry] {
        self.selected_server_id()
            .and_then(|server_id| self.resources.get(server_id))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn current_prompts(&self) -> &[McpPromptEntry] {
        self.selected_server_id()
            .and_then(|server_id| self.prompts.get(server_id))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn selected_tool(&self) -> Option<&McpToolEntry> {
        self.tools_state
            .selected()
            .and_then(|index| self.current_tools().get(index))
    }

    pub(super) fn selected_resource(&self) -> Option<&McpResourceEntry> {
        self.resources_state
            .selected()
            .and_then(|index| self.current_resources().get(index))
    }
}
