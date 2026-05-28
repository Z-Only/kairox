use agent_core::facade::{
    PluginCatalogEntry, PluginInstallTarget, PluginMarketplaceSourceView, PluginSettingsView,
};
use ratatui::widgets::ListState;

use super::types::{PluginOverlayMode, PluginTab};
use crate::components::{PluginCatalogFilters, PluginOverlaySnapshot};

pub struct PluginOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) mode: PluginOverlayMode,
    pub(super) tab: PluginTab,
    pub(super) plugins: Vec<PluginSettingsView>,
    pub(super) catalog: Vec<PluginCatalogEntry>,
    pub(super) sources: Vec<PluginMarketplaceSourceView>,
    pub(super) install_target: PluginInstallTarget,
    pub(super) catalog_keyword: String,
    pub(super) catalog_marketplace_filter: Option<String>,
    pub(super) plugins_state: ListState,
    pub(super) catalog_state: ListState,
    pub(super) sources_state: ListState,
}

impl Default for PluginOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            mode: PluginOverlayMode::List,
            tab: PluginTab::Installed,
            plugins: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
            install_target: PluginInstallTarget::User,
            catalog_keyword: String::new(),
            catalog_marketplace_filter: None,
            plugins_state: ListState::default(),
            catalog_state: ListState::default(),
            sources_state: ListState::default(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: PluginOverlaySnapshot) {
        self.plugins = snapshot.plugins;
        self.catalog = snapshot.catalog;
        self.sources = snapshot.sources;
        self.install_target = snapshot.install_target;
        self.visible = true;
        self.mode = PluginOverlayMode::List;
        self.reconcile_catalog_marketplace_filter();
        self.ensure_selection();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.mode = PluginOverlayMode::List;
        self.plugins.clear();
        self.catalog.clear();
        self.sources.clear();
        self.plugins_state.select(None);
        self.catalog_state.select(None);
        self.sources_state.select(None);
    }

    #[allow(dead_code)]
    pub fn plugins(&self) -> &[PluginSettingsView] {
        &self.plugins
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.current_selected()
    }

    pub fn catalog_filters(&self) -> PluginCatalogFilters {
        PluginCatalogFilters {
            marketplace_id: self.catalog_marketplace_filter.clone(),
            keyword: non_empty_trimmed(&self.catalog_keyword),
        }
    }

    pub(super) fn current_len(&self) -> usize {
        match self.tab {
            PluginTab::Installed => self.plugins.len(),
            PluginTab::Catalog => self.catalog.len(),
            PluginTab::Sources => self.sources.len(),
        }
    }

    pub(super) fn current_selected(&self) -> Option<usize> {
        match self.tab {
            PluginTab::Installed => self.plugins_state.selected(),
            PluginTab::Catalog => self.catalog_state.selected(),
            PluginTab::Sources => self.sources_state.selected(),
        }
    }

    pub(super) fn select_current(&mut self, selected: Option<usize>) {
        match self.tab {
            PluginTab::Installed => self.plugins_state.select(selected),
            PluginTab::Catalog => self.catalog_state.select(selected),
            PluginTab::Sources => self.sources_state.select(selected),
        }
    }

    pub(super) fn ensure_selection(&mut self) {
        for (len, state) in [
            (self.plugins.len(), &mut self.plugins_state),
            (self.catalog.len(), &mut self.catalog_state),
            (self.sources.len(), &mut self.sources_state),
        ] {
            let selected = if len == 0 {
                None
            } else {
                Some(state.selected().map_or(0, |i| i.min(len - 1)))
            };
            state.select(selected);
        }
    }

    pub(super) fn selected_plugin(&self) -> Option<&PluginSettingsView> {
        self.plugins_state
            .selected()
            .and_then(|index| self.plugins.get(index))
    }

    pub(super) fn selected_catalog_entry(&self) -> Option<&PluginCatalogEntry> {
        self.catalog_state
            .selected()
            .and_then(|index| self.catalog.get(index))
    }

    pub(super) fn selected_source(&self) -> Option<&PluginMarketplaceSourceView> {
        self.sources_state
            .selected()
            .and_then(|index| self.sources.get(index))
    }

    fn reconcile_catalog_marketplace_filter(&mut self) {
        if let Some(source_id) = &self.catalog_marketplace_filter {
            if !self.sources.iter().any(|source| &source.id == source_id) {
                self.catalog_marketplace_filter = None;
            }
        }
    }

    pub(super) fn cycle_catalog_marketplace_filter(&mut self) {
        if self.sources.is_empty() {
            self.catalog_marketplace_filter = None;
            return;
        }

        self.catalog_marketplace_filter = match self.catalog_marketplace_filter.as_deref() {
            None => self.sources.first().map(|source| source.id.clone()),
            Some(current) => {
                let next_index = self
                    .sources
                    .iter()
                    .position(|source| source.id == current)
                    .and_then(|index| index.checked_add(1))
                    .filter(|index| *index < self.sources.len());
                next_index.map(|index| self.sources[index].id.clone())
            }
        };
        self.ensure_selection();
    }

    pub(super) fn catalog_marketplace_label(&self) -> String {
        match self.catalog_marketplace_filter.as_deref() {
            Some(source_id) => self
                .sources
                .iter()
                .find(|source| source.id == source_id)
                .map(|source| {
                    if source.display_name.is_empty() {
                        source.id.clone()
                    } else {
                        source.display_name.clone()
                    }
                })
                .unwrap_or_else(|| source_id.to_string()),
            None => "all".to_string(),
        }
    }

    pub(super) fn catalog_filters_active(&self) -> bool {
        self.catalog_marketplace_filter.is_some() || !self.catalog_keyword.trim().is_empty()
    }
}

pub(super) fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
