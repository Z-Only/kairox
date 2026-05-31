//! State types and behaviour for [`SkillsOverlay`].
//!
//! Key-event handling lives in [`super::keys`].

use agent_core::facade::{
    InstallRemoteSkillRequest, RemoteSkillSearchResult, SkillCatalogEntry, SkillInstallTarget,
    SkillSettingsView, SkillSourceView,
};
use ratatui::widgets::ListState;

use crate::components::{Command, SkillEntry, SkillOverlaySnapshot};

use super::editor::SkillSourceDraft;
use super::types::{BodyView, SkillOverlayMode, SkillTab};
pub struct SkillsOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) mode: SkillOverlayMode,
    pub(super) tab: SkillTab,
    pub(super) discovered: Vec<SkillEntry>,
    pub(super) installed: Vec<SkillSettingsView>,
    pub(super) catalog: Vec<SkillCatalogEntry>,
    pub(super) sources: Vec<SkillSourceView>,
    pub(super) catalog_keyword: String,
    pub(super) catalog_keyword_draft: String,
    pub(super) catalog_source_filter: Option<String>,
    pub(super) install_target: SkillInstallTarget,
    pub(super) discovered_state: ListState,
    pub(super) installed_state: ListState,
    pub(super) catalog_state: ListState,
    pub(super) sources_state: ListState,
    pub(super) source_draft: SkillSourceDraft,
    pub(super) source_field_index: usize,
    pub(super) body: Option<BodyView>,
    pub(super) search_results: Vec<RemoteSkillSearchResult>,
    pub(super) search_query: String,
    pub(super) search_query_draft: String,
    pub(super) search_state: ListState,
}

impl Default for SkillsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillsOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            mode: SkillOverlayMode::List,
            tab: SkillTab::Discovered,
            discovered: Vec::new(),
            installed: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
            catalog_keyword: String::new(),
            catalog_keyword_draft: String::new(),
            catalog_source_filter: None,
            install_target: SkillInstallTarget::User,
            discovered_state: ListState::default(),
            installed_state: ListState::default(),
            catalog_state: ListState::default(),
            sources_state: ListState::default(),
            source_draft: SkillSourceDraft::new(),
            source_field_index: 0,
            body: None,
            search_results: Vec::new(),
            search_query: String::new(),
            search_query_draft: String::new(),
            search_state: ListState::default(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: impl Into<SkillOverlaySnapshot>) {
        let snapshot = snapshot.into();
        let prior_selected_id = self
            .discovered_state
            .selected()
            .and_then(|i| self.discovered.get(i))
            .map(|s| s.id.clone());

        let select = if snapshot.discovered.is_empty() {
            None
        } else if let Some(id) = prior_selected_id {
            snapshot
                .discovered
                .iter()
                .position(|s| s.id == id)
                .or(Some(0))
        } else {
            Some(0)
        };

        self.discovered = snapshot.discovered;
        self.installed = snapshot.installed;
        self.catalog = snapshot.catalog;
        self.sources = snapshot.sources;
        self.install_target = snapshot.install_target;
        self.prune_catalog_source_filter();
        self.discovered_state.select(select);
        self.ensure_selection();
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.discovered.clear();
        self.installed.clear();
        self.catalog.clear();
        self.sources.clear();
        self.catalog_keyword.clear();
        self.catalog_keyword_draft.clear();
        self.catalog_source_filter = None;
        self.discovered_state.select(None);
        self.installed_state.select(None);
        self.catalog_state.select(None);
        self.sources_state.select(None);
        self.mode = SkillOverlayMode::List;
        self.source_draft = SkillSourceDraft::new();
        self.source_field_index = 0;
        self.body = None;
        self.search_results.clear();
        self.search_query.clear();
        self.search_query_draft.clear();
        self.search_state.select(None);
    }

    #[allow(dead_code)]
    pub fn skills(&self) -> &[SkillEntry] {
        &self.discovered
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
    }

    #[allow(dead_code)]
    pub fn body_skill_id(&self) -> Option<&str> {
        self.body.as_ref().map(|b| b.skill_id.as_str())
    }

    pub fn catalog_query(&self) -> (Option<String>, Option<Vec<String>>) {
        (
            trim_option(&self.catalog_keyword),
            self.catalog_source_filter
                .as_ref()
                .map(|source| vec![source.clone()]),
        )
    }

    pub(super) fn selected_discovered(&self) -> Option<&SkillEntry> {
        self.discovered_state
            .selected()
            .and_then(|i| self.discovered.get(i))
    }

    pub(super) fn selected_installed(&self) -> Option<&SkillSettingsView> {
        self.installed_state
            .selected()
            .and_then(|i| self.installed.get(i))
    }

    pub(super) fn selected_catalog_entry(&self) -> Option<&SkillCatalogEntry> {
        self.catalog_state
            .selected()
            .and_then(|i| self.catalog.get(i))
    }

    pub(super) fn selected_source(&self) -> Option<&SkillSourceView> {
        self.sources_state
            .selected()
            .and_then(|i| self.sources.get(i))
    }

    pub(super) fn current_len(&self) -> usize {
        match self.tab {
            SkillTab::Discovered => self.discovered.len(),
            SkillTab::Installed => self.installed.len(),
            SkillTab::Catalog => self.catalog.len(),
            SkillTab::Sources => self.sources.len(),
            SkillTab::Search => self.search_results.len(),
        }
    }

    pub(super) fn current_selected(&self) -> Option<usize> {
        match self.tab {
            SkillTab::Discovered => self.discovered_state.selected(),
            SkillTab::Installed => self.installed_state.selected(),
            SkillTab::Catalog => self.catalog_state.selected(),
            SkillTab::Sources => self.sources_state.selected(),
            SkillTab::Search => self.search_state.selected(),
        }
    }

    pub(super) fn select_current(&mut self, selected: Option<usize>) {
        match self.tab {
            SkillTab::Discovered => self.discovered_state.select(selected),
            SkillTab::Installed => self.installed_state.select(selected),
            SkillTab::Catalog => self.catalog_state.select(selected),
            SkillTab::Sources => self.sources_state.select(selected),
            SkillTab::Search => self.search_state.select(selected),
        }
    }

    pub(super) fn ensure_selection(&mut self) {
        for (len, state) in [
            (self.discovered.len(), &mut self.discovered_state),
            (self.installed.len(), &mut self.installed_state),
            (self.catalog.len(), &mut self.catalog_state),
            (self.sources.len(), &mut self.sources_state),
            (self.search_results.len(), &mut self.search_state),
        ] {
            let selected = if len == 0 {
                None
            } else {
                Some(state.selected().map_or(0, |i| i.min(len - 1)))
            };
            state.select(selected);
        }
    }

    pub(super) fn prune_catalog_source_filter(&mut self) {
        if let Some(source_id) = self.catalog_source_filter.as_ref() {
            if !self.sources.iter().any(|source| source.id == *source_id) {
                self.catalog_source_filter = None;
            }
        }
    }

    pub(super) fn catalog_filters_active(&self) -> bool {
        self.mode == SkillOverlayMode::CatalogFilter
            || !self.catalog_keyword.trim().is_empty()
            || self.catalog_source_filter.is_some()
    }

    pub(super) fn catalog_keyword_for_display(&self) -> &str {
        if self.mode == SkillOverlayMode::CatalogFilter {
            &self.catalog_keyword_draft
        } else {
            &self.catalog_keyword
        }
    }

    pub(super) fn catalog_source_filter_label(&self) -> String {
        self.catalog_source_filter
            .as_ref()
            .map(|source_id| {
                self.sources
                    .iter()
                    .find(|source| source.id == *source_id)
                    .map(|source| source.display_name.as_str())
                    .unwrap_or(source_id)
            })
            .unwrap_or("*")
            .to_string()
    }

    pub(super) fn cycle_catalog_source_filter(&mut self) {
        if self.sources.is_empty() {
            self.catalog_source_filter = None;
            return;
        }
        let next_index = self
            .catalog_source_filter
            .as_ref()
            .and_then(|source_id| {
                self.sources
                    .iter()
                    .position(|source| source.id == *source_id)
            })
            .map_or(Some(0), |index| {
                let next = index + 1;
                (next < self.sources.len()).then_some(next)
            });
        self.catalog_source_filter = next_index.map(|index| self.sources[index].id.clone());
        self.ensure_selection();
    }

    pub(super) fn list_catalog_command(&self) -> Command {
        let (keyword, sources) = self.catalog_query();
        Command::ListSkillCatalog { keyword, sources }
    }

    pub(super) fn refresh_catalog_command(&self) -> Command {
        let (keyword, sources) = self.catalog_query();
        Command::RefreshSkillCatalog { keyword, sources }
    }

    pub(super) fn install_selected_catalog_command(&self) -> Option<Command> {
        self.selected_catalog_entry()
            .map(|entry| Command::InstallRemoteSkill {
                request: InstallRemoteSkillRequest {
                    package: entry.package.clone(),
                    source: entry.source.clone(),
                    target: self.install_target,
                    package_url: entry.package_url.clone(),
                },
            })
    }

    pub(super) fn selected_search_result(&self) -> Option<&RemoteSkillSearchResult> {
        self.search_state
            .selected()
            .and_then(|i| self.search_results.get(i))
    }

    pub(super) fn install_selected_search_result_command(&self) -> Option<Command> {
        self.selected_search_result()
            .map(|result| Command::InstallRemoteSkill {
                request: InstallRemoteSkillRequest {
                    package: result.package.clone(),
                    source: "registry".to_string(),
                    target: self.install_target,
                    package_url: None,
                },
            })
    }

    pub(super) fn search_query_for_display(&self) -> &str {
        if self.mode == SkillOverlayMode::RemoteSearchInput {
            &self.search_query_draft
        } else {
            &self.search_query
        }
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
