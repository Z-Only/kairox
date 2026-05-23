use agent_core::facade::{
    InstallPluginRequest, PluginCatalogEntry, PluginInstallTarget, PluginMarketplaceSourceView,
    PluginSettingsView,
};
use agent_core::ConfigScope;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::widgets::ListState;

use crate::components::{Command, PluginCatalogFilters, PluginOverlaySnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PluginTab {
    Installed,
    Catalog,
    Sources,
}

impl PluginTab {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Installed => Self::Catalog,
            Self::Catalog => Self::Sources,
            Self::Sources => Self::Installed,
        }
    }

    pub(super) fn previous(self) -> Self {
        match self {
            Self::Installed => Self::Sources,
            Self::Catalog => Self::Installed,
            Self::Sources => Self::Catalog,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::Catalog => "Catalog",
            Self::Sources => "Sources",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PluginOverlayMode {
    List,
    CatalogSearch,
}

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

    fn current_len(&self) -> usize {
        match self.tab {
            PluginTab::Installed => self.plugins.len(),
            PluginTab::Catalog => self.catalog.len(),
            PluginTab::Sources => self.sources.len(),
        }
    }

    fn current_selected(&self) -> Option<usize> {
        match self.tab {
            PluginTab::Installed => self.plugins_state.selected(),
            PluginTab::Catalog => self.catalog_state.selected(),
            PluginTab::Sources => self.sources_state.selected(),
        }
    }

    fn select_current(&mut self, selected: Option<usize>) {
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

    fn selected_plugin(&self) -> Option<&PluginSettingsView> {
        self.plugins_state
            .selected()
            .and_then(|index| self.plugins.get(index))
    }

    fn selected_catalog_entry(&self) -> Option<&PluginCatalogEntry> {
        self.catalog_state
            .selected()
            .and_then(|index| self.catalog.get(index))
    }

    fn selected_source(&self) -> Option<&PluginMarketplaceSourceView> {
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

    pub(super) fn handle_catalog_search_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> bool {
        match code {
            KeyCode::Enter | KeyCode::Esc => {
                self.mode = PluginOverlayMode::List;
                true
            }
            KeyCode::Backspace => {
                self.catalog_keyword.pop();
                false
            }
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.catalog_keyword.clear();
                false
            }
            KeyCode::Char(ch)
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.catalog_keyword.push(ch);
                false
            }
            _ => false,
        }
    }

    fn toggle_install_target(&mut self) {
        self.install_target = match self.install_target {
            PluginInstallTarget::User => PluginInstallTarget::Project,
            PluginInstallTarget::Project => PluginInstallTarget::User,
        };
    }

    pub(super) fn command_for_current_tab(&mut self, key: KeyCode) -> Option<Command> {
        match (self.tab, key) {
            (PluginTab::Installed, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_plugin()
                .filter(|plugin| plugin.scope != ConfigScope::Builtin)
                .map(|plugin| Command::SetPluginEnabled {
                    settings_id: plugin.settings_id.clone(),
                    enabled: !plugin.enabled,
                }),
            (PluginTab::Installed, KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete) => {
                self.selected_plugin()
                    .filter(|plugin| plugin.scope != ConfigScope::Builtin)
                    .map(|plugin| Command::DeletePluginSettings {
                        settings_id: plugin.settings_id.clone(),
                    })
            }
            (PluginTab::Catalog, KeyCode::Char('i') | KeyCode::Char('I')) => self
                .selected_catalog_entry()
                .map(|entry| Command::InstallPlugin {
                    request: InstallPluginRequest {
                        marketplace_id: entry.marketplace_id.clone(),
                        plugin_name: entry.name.clone(),
                        target: self.install_target,
                    },
                }),
            (PluginTab::Catalog, KeyCode::Char('t') | KeyCode::Char('T')) => {
                self.toggle_install_target();
                None
            }
            (PluginTab::Sources, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_source()
                .map(|source| Command::SetPluginMarketplaceSourceEnabled {
                    source_id: source.id.clone(),
                    enabled: !source.enabled,
                }),
            _ => None,
        }
    }
}

pub(super) fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
