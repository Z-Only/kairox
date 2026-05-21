//! Skills overlay — pop-up modal listing native skills with an active marker,
//! supporting per-session activation/deactivation and inline body preview.
//!
//! The TUI surface for the same data the GUI's `SkillSettingsPane` shows,
//! minus remote-marketplace search. The App constructs a snapshot of
//! [`SkillEntry`] values before opening the overlay; the overlay produces
//! [`Command`] values that the main loop dispatches back to `AppFacade`.

use agent_core::facade::{
    InstallRemoteSkillRequest, SkillCatalogEntry, SkillFieldMappingView, SkillInstallSource,
    SkillInstallTarget, SkillSettingsScope, SkillSettingsView, SkillSourceView,
};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, SkillEntry, SkillOverlaySnapshot,
};

/// Inline detail view shown when the user presses Enter on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyView {
    pub skill_id: String,
    pub body: String,
}

const DEFAULT_SKILL_SEARCH_TEMPLATE: &str =
    "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc";
const DEFAULT_SKILL_DOWNLOAD_TEMPLATE: &str = "/api/v1/download?slug={{slug}}";
const DEFAULT_SKILL_LIST_TEMPLATE: &str =
    "/api/skills?page=1&pageSize={{limit}}&sortBy=downloads&order=desc";
const DEFAULT_SKILL_DETAIL_TEMPLATE: &str = "/api/v1/skills/{{slug}}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillTab {
    Discovered,
    Installed,
    Catalog,
    Sources,
}

impl SkillTab {
    fn next(self) -> Self {
        match self {
            Self::Discovered => Self::Installed,
            Self::Installed => Self::Catalog,
            Self::Catalog => Self::Sources,
            Self::Sources => Self::Discovered,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Discovered => Self::Sources,
            Self::Installed => Self::Discovered,
            Self::Catalog => Self::Installed,
            Self::Sources => Self::Catalog,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Discovered => "Discovered",
            Self::Installed => "Installed",
            Self::Catalog => "Catalog",
            Self::Sources => "Sources",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillOverlayMode {
    List,
    CatalogDetail,
    CatalogFilter,
    SourceEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillSourceEditorField {
    Id,
    DisplayName,
    Url,
    Kind,
    SearchTemplate,
    DownloadTemplate,
    ListTemplate,
    DetailTemplate,
    Priority,
    CacheTtlSeconds,
    Enabled,
}

const SKILL_SOURCE_EDITOR_FIELDS: [SkillSourceEditorField; 11] = [
    SkillSourceEditorField::Id,
    SkillSourceEditorField::DisplayName,
    SkillSourceEditorField::Url,
    SkillSourceEditorField::Kind,
    SkillSourceEditorField::SearchTemplate,
    SkillSourceEditorField::DownloadTemplate,
    SkillSourceEditorField::ListTemplate,
    SkillSourceEditorField::DetailTemplate,
    SkillSourceEditorField::Priority,
    SkillSourceEditorField::CacheTtlSeconds,
    SkillSourceEditorField::Enabled,
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillSourceDraft {
    id: String,
    display_name: String,
    kind: String,
    url: String,
    search_template: String,
    download_template: String,
    list_template: String,
    detail_template: String,
    priority: String,
    cache_ttl_seconds: String,
    enabled: bool,
}

impl SkillSourceDraft {
    fn new() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            kind: "skillhub".to_string(),
            url: String::new(),
            search_template: DEFAULT_SKILL_SEARCH_TEMPLATE.to_string(),
            download_template: DEFAULT_SKILL_DOWNLOAD_TEMPLATE.to_string(),
            list_template: DEFAULT_SKILL_LIST_TEMPLATE.to_string(),
            detail_template: DEFAULT_SKILL_DETAIL_TEMPLATE.to_string(),
            priority: "100".to_string(),
            cache_ttl_seconds: "900".to_string(),
            enabled: true,
        }
    }

    fn to_view(&self) -> Option<SkillSourceView> {
        let id = self.id.trim();
        let display_name = self.display_name.trim();
        let kind = self.kind.trim();
        let url = self.url.trim();
        let search_template = self.search_template.trim();
        let download_template = self.download_template.trim();
        if id.is_empty()
            || display_name.is_empty()
            || kind.is_empty()
            || search_template.is_empty()
            || download_template.is_empty()
            || !(url.starts_with("http://") || url.starts_with("https://"))
        {
            return None;
        }

        Some(SkillSourceView {
            id: id.to_string(),
            display_name: display_name.to_string(),
            kind: kind.to_string(),
            url: url.to_string(),
            search_template: search_template.to_string(),
            download_template: download_template.to_string(),
            list_template: trim_option(&self.list_template),
            detail_template: trim_option(&self.detail_template),
            field_mapping: SkillFieldMappingView::default(),
            enabled: self.enabled,
            priority: self.priority.trim().parse::<u32>().unwrap_or(100),
            cache_ttl_seconds: self.cache_ttl_seconds.trim().parse::<u64>().unwrap_or(900),
            last_error: None,
        })
    }

    fn push_char(&mut self, field: SkillSourceEditorField, ch: char) {
        match field {
            SkillSourceEditorField::Id => self.id.push(ch),
            SkillSourceEditorField::DisplayName => self.display_name.push(ch),
            SkillSourceEditorField::Kind => self.kind.push(ch),
            SkillSourceEditorField::Url => self.url.push(ch),
            SkillSourceEditorField::SearchTemplate => self.search_template.push(ch),
            SkillSourceEditorField::DownloadTemplate => self.download_template.push(ch),
            SkillSourceEditorField::ListTemplate => self.list_template.push(ch),
            SkillSourceEditorField::DetailTemplate => self.detail_template.push(ch),
            SkillSourceEditorField::Priority => {
                if ch.is_ascii_digit() {
                    self.priority.push(ch);
                }
            }
            SkillSourceEditorField::CacheTtlSeconds => {
                if ch.is_ascii_digit() {
                    self.cache_ttl_seconds.push(ch);
                }
            }
            SkillSourceEditorField::Enabled => match ch {
                ' ' => self.enabled = !self.enabled,
                'y' | 'Y' | '1' | 't' | 'T' => self.enabled = true,
                'n' | 'N' | '0' | 'f' | 'F' => self.enabled = false,
                _ => {}
            },
        }
    }

    fn backspace(&mut self, field: SkillSourceEditorField) {
        match field {
            SkillSourceEditorField::Id => {
                self.id.pop();
            }
            SkillSourceEditorField::DisplayName => {
                self.display_name.pop();
            }
            SkillSourceEditorField::Kind => {
                self.kind.pop();
            }
            SkillSourceEditorField::Url => {
                self.url.pop();
            }
            SkillSourceEditorField::SearchTemplate => {
                self.search_template.pop();
            }
            SkillSourceEditorField::DownloadTemplate => {
                self.download_template.pop();
            }
            SkillSourceEditorField::ListTemplate => {
                self.list_template.pop();
            }
            SkillSourceEditorField::DetailTemplate => {
                self.detail_template.pop();
            }
            SkillSourceEditorField::Priority => {
                self.priority.pop();
            }
            SkillSourceEditorField::CacheTtlSeconds => {
                self.cache_ttl_seconds.pop();
            }
            SkillSourceEditorField::Enabled => {}
        }
    }

    fn clear_field(&mut self, field: SkillSourceEditorField) {
        match field {
            SkillSourceEditorField::Id => self.id.clear(),
            SkillSourceEditorField::DisplayName => self.display_name.clear(),
            SkillSourceEditorField::Kind => self.kind.clear(),
            SkillSourceEditorField::Url => self.url.clear(),
            SkillSourceEditorField::SearchTemplate => self.search_template.clear(),
            SkillSourceEditorField::DownloadTemplate => self.download_template.clear(),
            SkillSourceEditorField::ListTemplate => self.list_template.clear(),
            SkillSourceEditorField::DetailTemplate => self.detail_template.clear(),
            SkillSourceEditorField::Priority => self.priority.clear(),
            SkillSourceEditorField::CacheTtlSeconds => self.cache_ttl_seconds.clear(),
            SkillSourceEditorField::Enabled => {}
        }
    }
}

pub struct SkillsOverlay {
    focused: bool,
    visible: bool,
    mode: SkillOverlayMode,
    tab: SkillTab,
    discovered: Vec<SkillEntry>,
    installed: Vec<SkillSettingsView>,
    catalog: Vec<SkillCatalogEntry>,
    sources: Vec<SkillSourceView>,
    catalog_keyword: String,
    catalog_keyword_draft: String,
    catalog_source_filter: Option<String>,
    install_target: SkillInstallTarget,
    discovered_state: ListState,
    installed_state: ListState,
    catalog_state: ListState,
    sources_state: ListState,
    source_draft: SkillSourceDraft,
    source_field_index: usize,
    body: Option<BodyView>,
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

    fn selected_discovered(&self) -> Option<&SkillEntry> {
        self.discovered_state
            .selected()
            .and_then(|i| self.discovered.get(i))
    }

    fn selected_installed(&self) -> Option<&SkillSettingsView> {
        self.installed_state
            .selected()
            .and_then(|i| self.installed.get(i))
    }

    fn selected_catalog_entry(&self) -> Option<&SkillCatalogEntry> {
        self.catalog_state
            .selected()
            .and_then(|i| self.catalog.get(i))
    }

    fn selected_source(&self) -> Option<&SkillSourceView> {
        self.sources_state
            .selected()
            .and_then(|i| self.sources.get(i))
    }

    fn current_len(&self) -> usize {
        match self.tab {
            SkillTab::Discovered => self.discovered.len(),
            SkillTab::Installed => self.installed.len(),
            SkillTab::Catalog => self.catalog.len(),
            SkillTab::Sources => self.sources.len(),
        }
    }

    fn current_selected(&self) -> Option<usize> {
        match self.tab {
            SkillTab::Discovered => self.discovered_state.selected(),
            SkillTab::Installed => self.installed_state.selected(),
            SkillTab::Catalog => self.catalog_state.selected(),
            SkillTab::Sources => self.sources_state.selected(),
        }
    }

    fn select_current(&mut self, selected: Option<usize>) {
        match self.tab {
            SkillTab::Discovered => self.discovered_state.select(selected),
            SkillTab::Installed => self.installed_state.select(selected),
            SkillTab::Catalog => self.catalog_state.select(selected),
            SkillTab::Sources => self.sources_state.select(selected),
        }
    }

    fn ensure_selection(&mut self) {
        for (len, state) in [
            (self.discovered.len(), &mut self.discovered_state),
            (self.installed.len(), &mut self.installed_state),
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

    fn toggle_install_target(&mut self) {
        self.install_target = match self.install_target {
            SkillInstallTarget::User => SkillInstallTarget::Project,
            SkillInstallTarget::Project => SkillInstallTarget::User,
        };
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

    fn start_source_create(&mut self) {
        self.mode = SkillOverlayMode::SourceEditor;
        self.source_draft = SkillSourceDraft::new();
        self.source_field_index = 0;
    }

    fn prune_catalog_source_filter(&mut self) {
        if let Some(source_id) = self.catalog_source_filter.as_ref() {
            if !self.sources.iter().any(|source| source.id == *source_id) {
                self.catalog_source_filter = None;
            }
        }
    }

    fn catalog_filters_active(&self) -> bool {
        self.mode == SkillOverlayMode::CatalogFilter
            || !self.catalog_keyword.trim().is_empty()
            || self.catalog_source_filter.is_some()
    }

    fn catalog_keyword_for_display(&self) -> &str {
        if self.mode == SkillOverlayMode::CatalogFilter {
            &self.catalog_keyword_draft
        } else {
            &self.catalog_keyword
        }
    }

    fn catalog_source_filter_label(&self) -> String {
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

    fn cycle_catalog_source_filter(&mut self) {
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

    fn list_catalog_command(&self) -> Command {
        let (keyword, sources) = self.catalog_query();
        Command::ListSkillCatalog { keyword, sources }
    }

    fn refresh_catalog_command(&self) -> Command {
        let (keyword, sources) = self.catalog_query();
        Command::RefreshSkillCatalog { keyword, sources }
    }

    fn current_source_field(&self) -> SkillSourceEditorField {
        SKILL_SOURCE_EDITOR_FIELDS[self.source_field_index]
    }

    fn move_source_field_down(&mut self) {
        self.source_field_index = (self.source_field_index + 1) % SKILL_SOURCE_EDITOR_FIELDS.len();
    }

    fn move_source_field_up(&mut self) {
        self.source_field_index = if self.source_field_index == 0 {
            SKILL_SOURCE_EDITOR_FIELDS.len() - 1
        } else {
            self.source_field_index - 1
        };
    }

    fn handle_source_editor_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Vec<Command> {
        match key {
            KeyCode::Tab | KeyCode::Down => self.move_source_field_down(),
            KeyCode::BackTab | KeyCode::Up => self.move_source_field_up(),
            KeyCode::Esc => self.mode = SkillOverlayMode::List,
            KeyCode::Backspace => self.source_draft.backspace(self.current_source_field()),
            KeyCode::Delete => self.source_draft.clear_field(self.current_source_field()),
            KeyCode::Enter => {
                if let Some(config) = self.source_draft.to_view() {
                    self.mode = SkillOverlayMode::List;
                    return vec![Command::AddSkillSource { config }];
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.source_draft.push_char(self.current_source_field(), ch);
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_catalog_filter_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Vec<Command> {
        match key {
            KeyCode::Enter => {
                self.catalog_keyword = self.catalog_keyword_draft.trim().to_string();
                self.mode = SkillOverlayMode::List;
                return vec![self.list_catalog_command()];
            }
            KeyCode::Esc => {
                self.catalog_keyword_draft = self.catalog_keyword.clone();
                self.mode = SkillOverlayMode::List;
            }
            KeyCode::Backspace => {
                self.catalog_keyword_draft.pop();
            }
            KeyCode::Delete => {
                self.catalog_keyword_draft.clear();
            }
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.catalog_keyword_draft.clear();
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.catalog_keyword_draft.push(ch);
            }
            _ => {}
        }
        Vec::new()
    }

    fn install_selected_catalog_command(&self) -> Option<Command> {
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

    fn handle_catalog_detail_key(&mut self, key: KeyCode) -> Vec<Command> {
        match key {
            KeyCode::Esc => self.mode = SkillOverlayMode::List,
            KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char('I') => {
                return self
                    .install_selected_catalog_command()
                    .into_iter()
                    .collect();
            }
            KeyCode::Char('t') | KeyCode::Char('T') => self.toggle_install_target(),
            _ => {}
        }
        Vec::new()
    }

    fn command_for_current_tab(&mut self, key: KeyCode) -> Option<Command> {
        match (self.tab, key) {
            (SkillTab::Installed, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_installed()
                .filter(|skill| skill.editable && skill.scope != SkillSettingsScope::Builtin)
                .map(|skill| Command::SetSkillEnabled {
                    skill_id: skill.id.clone(),
                    enabled: !skill.enabled,
                }),
            (SkillTab::Installed, KeyCode::Char('u') | KeyCode::Char('U')) => self
                .selected_installed()
                .filter(|skill| skill.install_source != SkillInstallSource::Builtin)
                .map(|skill| Command::UpdateSkillSettings {
                    skill_id: skill.id.clone(),
                }),
            (SkillTab::Installed, KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete) => {
                self.selected_installed()
                    .filter(|skill| skill.deletable)
                    .map(|skill| Command::DeleteSkillSettings {
                        skill_id: skill.id.clone(),
                    })
            }
            (SkillTab::Catalog, KeyCode::Char('i') | KeyCode::Char('I')) => {
                self.install_selected_catalog_command()
            }
            (SkillTab::Catalog, KeyCode::Char('t') | KeyCode::Char('T')) => {
                self.toggle_install_target();
                None
            }
            (SkillTab::Sources, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_source()
                .map(|source| Command::SetSkillSourceEnabled {
                    source_id: source.id.clone(),
                    enabled: !source.enabled,
                }),
            (SkillTab::Sources, KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete) => self
                .selected_source()
                .map(|source| Command::RemoveSkillSource {
                    source_id: source.id.clone(),
                }),
            _ => None,
        }
    }
}

pub fn render_skills_overlay(
    area: Rect,
    frame: &mut Frame,
    overlay: &SkillsOverlay,
    discovered_state: &mut ListState,
    installed_state: &mut ListState,
    catalog_state: &mut ListState,
    sources_state: &mut ListState,
) {
    let modal_width = 92.min(area.width.saturating_sub(4));
    let modal_height = 24.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let title = match (overlay.body.is_some(), overlay.mode) {
        (true, _) => " 🧠 Skill detail ",
        (false, SkillOverlayMode::CatalogDetail) => " 🧠 Skill catalog detail ",
        (false, _) => " 🧠 Skills Manager ",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if let Some(detail) = overlay.body.as_ref() {
        let body_area = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(1),
        );
        let hint_area = Rect::new(
            inner.x,
            inner.y + body_area.height,
            inner.width,
            inner.height.saturating_sub(body_area.height),
        );
        let header = Line::from(vec![Span::styled(
            detail.skill_id.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]);
        let mut lines = vec![header, Line::from("")];
        for raw in detail.body.lines() {
            lines.push(Line::from(raw.to_string()));
        }
        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(para, body_area);

        let hints = Line::from(vec![
            Span::styled("[Esc] back  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Ctrl+S] close", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(hints), hint_area);
        return;
    }

    if inner.height < 5 {
        return;
    }

    if overlay.mode == SkillOverlayMode::SourceEditor {
        render_source_editor(inner, frame, overlay);
        return;
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
        SkillTab::Discovered => {
            render_discovered(chunks[1], frame, &overlay.discovered, discovered_state)
        }
        SkillTab::Installed => {
            render_installed(chunks[1], frame, &overlay.installed, installed_state)
        }
        SkillTab::Catalog if overlay.mode == SkillOverlayMode::CatalogDetail => {
            let selected = catalog_state
                .selected()
                .and_then(|index| overlay.catalog.get(index));
            render_catalog_detail(chunks[1], frame, selected, overlay.install_target);
        }
        SkillTab::Catalog => {
            render_catalog(
                chunks[1],
                frame,
                &overlay.catalog,
                catalog_state,
                overlay.install_target,
            );
        }
        SkillTab::Sources => render_sources(chunks[1], frame, &overlay.sources, sources_state),
    }
    render_hints(chunks[2], frame, overlay);
}

fn target_label(target: SkillInstallTarget) -> &'static str {
    match target {
        SkillInstallTarget::User => "user",
        SkillInstallTarget::Project => "project",
    }
}

fn render_tabs(area: Rect, frame: &mut Frame, overlay: &SkillsOverlay) {
    let mut spans = Vec::new();
    for tab in [
        SkillTab::Discovered,
        SkillTab::Installed,
        SkillTab::Catalog,
        SkillTab::Sources,
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
    spans.push(Span::styled(
        format!("target: {}", target_label(overlay.install_target)),
        Style::default().fg(Color::Cyan),
    ));
    if overlay.tab == SkillTab::Catalog || overlay.catalog_filters_active() {
        let keyword_value = overlay.catalog_keyword_for_display().trim();
        let keyword = if keyword_value.is_empty() {
            "*".to_string()
        } else {
            clip(keyword_value, 18)
        };
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!(
                "catalog:{} search:{} source:{}",
                overlay.catalog.len(),
                keyword,
                clip(&overlay.catalog_source_filter_label(), 18)
            ),
            Style::default().fg(Color::Cyan),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_discovered(area: Rect, frame: &mut Frame, skills: &[SkillEntry], state: &mut ListState) {
    if skills.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No skills discovered",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, area);
    } else {
        let items: Vec<ListItem> = skills
            .iter()
            .map(|s| {
                let (marker, marker_color) = if s.active {
                    ("● active ", Color::Green)
                } else {
                    ("○        ", Color::DarkGray)
                };
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(marker_color)),
                    Span::styled(s.id.clone(), Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("  "),
                    Span::styled(s.description.clone(), Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("  [{} / {}]", s.source, s.activation_mode),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_stateful_widget(list, area, state);
    }
}

fn render_installed(
    area: Rect,
    frame: &mut Frame,
    installed: &[SkillSettingsView],
    state: &mut ListState,
) {
    if installed.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No skill settings installed",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = installed
        .iter()
        .map(|skill| {
            let enabled_label = if skill.enabled {
                "enabled "
            } else {
                "disabled"
            };
            let enabled_color = if skill.enabled {
                Color::Green
            } else {
                Color::DarkGray
            };
            let version = skill.version.as_deref().unwrap_or("unknown");
            let effective = if skill.effective { " effective" } else { "" };
            let valid = if skill.valid { "" } else { " invalid" };
            let line = Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    skill.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  v{version}"),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(skill.description.clone(), Style::default().fg(Color::Gray)),
                Span::styled(
                    format!(
                        "  [{:?} / {:?} / {:?}{effective}{valid}]",
                        skill.scope, skill.install_source, skill.update_state
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, state);
}

fn render_catalog(
    area: Rect,
    frame: &mut Frame,
    catalog: &[SkillCatalogEntry],
    state: &mut ListState,
    install_target: SkillInstallTarget,
) {
    if catalog.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No catalog skills available",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = catalog
        .iter()
        .map(|entry| {
            let installs = entry
                .install_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let stars = entry
                .github_stars
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".to_string());
            let line = Line::from(vec![
                Span::styled(
                    entry.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  @{}", entry.source),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("  "),
                Span::styled(entry.description.clone(), Style::default().fg(Color::Gray)),
                Span::styled(
                    format!(
                        "  installs:{installs} stars:{stars} -> {}",
                        target_label(install_target)
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, state);
}

fn render_catalog_detail(
    area: Rect,
    frame: &mut Frame,
    entry: Option<&SkillCatalogEntry>,
    install_target: SkillInstallTarget,
) {
    let Some(entry) = entry else {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No catalog skill selected",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
        return;
    };

    let source_url = if entry.source_url.trim().is_empty() {
        "unknown"
    } else {
        entry.source_url.as_str()
    };
    let package_url = entry.package_url.as_deref().unwrap_or("unknown");
    let installs = entry
        .install_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let stars = entry
        .github_stars
        .map(|count| count.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let security = entry
        .security_score
        .map(|score| score.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let rating = entry
        .rating
        .map(|rating| format!("{rating:.1}"))
        .unwrap_or_else(|| "unknown".to_string());

    let lines = vec![
        Line::from(vec![Span::styled(
            entry.name.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(entry.description.clone()),
        Line::from(""),
        Line::from(vec![
            Span::styled("Catalog: ", Style::default().fg(Color::DarkGray)),
            Span::raw(entry.source.clone()),
        ]),
        Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::DarkGray)),
            Span::raw(source_url.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Package: ", Style::default().fg(Color::DarkGray)),
            Span::raw(entry.package.clone()),
        ]),
        Line::from(vec![
            Span::styled("Download: ", Style::default().fg(Color::DarkGray)),
            Span::raw(package_url.to_string()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Installs: ", Style::default().fg(Color::DarkGray)),
            Span::raw(installs),
            Span::styled("  Stars: ", Style::default().fg(Color::DarkGray)),
            Span::raw(stars),
            Span::styled("  Security: ", Style::default().fg(Color::DarkGray)),
            Span::raw(security),
            Span::styled("  Rating: ", Style::default().fg(Color::DarkGray)),
            Span::raw(rating),
        ]),
        Line::from(vec![
            Span::styled("Target: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                target_label(install_target),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn render_sources(
    area: Rect,
    frame: &mut Frame,
    sources: &[SkillSourceView],
    state: &mut ListState,
) {
    if sources.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No skill sources configured",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
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
            let line = Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    source.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {} p{}", source.kind, source.priority),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(source.url.clone(), Style::default().fg(Color::Gray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, state);
}

fn render_hints(area: Rect, frame: &mut Frame, overlay: &SkillsOverlay) {
    let action = if overlay.mode == SkillOverlayMode::CatalogFilter {
        "[Enter] search  [Esc] close search  [Backspace] edit  "
    } else if overlay.mode == SkillOverlayMode::CatalogDetail {
        "[Enter/i] install  [t] target  [Esc] back  "
    } else {
        match overlay.tab {
            SkillTab::Discovered => "[Enter] body  [a] activate  [d] deactivate  ",
            SkillTab::Installed => "[e] enable  [u] update  [x] delete  ",
            SkillTab::Catalog => {
                "[Enter] detail  [i] install  [/] search  [s] source  [t] target  "
            }
            SkillTab::Sources => "[n] new  [e] enable source  [x] remove  ",
        }
    };
    let hints = Line::from(vec![
        Span::styled("[Tab] tab  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled(action, Style::default().fg(Color::Yellow)),
        Span::styled("[r] refresh  ", Style::default().fg(Color::Cyan)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), area);
}

fn render_source_editor(area: Rect, frame: &mut Frame, overlay: &SkillsOverlay) {
    let list_height = area.height.saturating_sub(1);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );
    let items = SKILL_SOURCE_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.source_field_index {
                "> "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{:<14}", skill_source_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    skill_source_field_value(&overlay.source_draft, *field),
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

fn skill_source_field_label(field: SkillSourceEditorField) -> &'static str {
    match field {
        SkillSourceEditorField::Id => "ID",
        SkillSourceEditorField::DisplayName => "Name",
        SkillSourceEditorField::Url => "URL",
        SkillSourceEditorField::Kind => "Kind",
        SkillSourceEditorField::SearchTemplate => "Search",
        SkillSourceEditorField::DownloadTemplate => "Download",
        SkillSourceEditorField::ListTemplate => "List",
        SkillSourceEditorField::DetailTemplate => "Detail",
        SkillSourceEditorField::Priority => "Priority",
        SkillSourceEditorField::CacheTtlSeconds => "TTL",
        SkillSourceEditorField::Enabled => "Enabled",
    }
}

fn skill_source_field_value(draft: &SkillSourceDraft, field: SkillSourceEditorField) -> String {
    match field {
        SkillSourceEditorField::Id => draft.id.clone(),
        SkillSourceEditorField::DisplayName => draft.display_name.clone(),
        SkillSourceEditorField::Url => draft.url.clone(),
        SkillSourceEditorField::Kind => draft.kind.clone(),
        SkillSourceEditorField::SearchTemplate => draft.search_template.clone(),
        SkillSourceEditorField::DownloadTemplate => draft.download_template.clone(),
        SkillSourceEditorField::ListTemplate => draft.list_template.clone(),
        SkillSourceEditorField::DetailTemplate => draft.detail_template.clone(),
        SkillSourceEditorField::Priority => draft.priority.clone(),
        SkillSourceEditorField::CacheTtlSeconds => draft.cache_ttl_seconds.clone(),
        SkillSourceEditorField::Enabled => draft.enabled.to_string(),
    }
}

fn trim_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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

impl Component for SkillsOverlay {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
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

        // In body view Esc returns to the list; any other key is ignored
        // so the parent activate/deactivate keys don't fire by accident.
        if self.body.is_some() {
            if matches!(key.code, KeyCode::Esc) {
                self.body = None;
            }
            return (effects, commands);
        }

        match self.mode {
            SkillOverlayMode::SourceEditor => {
                commands.extend(self.handle_source_editor_key(key.code, key.modifiers));
                return (effects, commands);
            }
            SkillOverlayMode::CatalogDetail => {
                commands.extend(self.handle_catalog_detail_key(key.code));
                return (effects, commands);
            }
            SkillOverlayMode::CatalogFilter => {
                commands.extend(self.handle_catalog_filter_key(key.code, key.modifiers));
                return (effects, commands);
            }
            SkillOverlayMode::List => {}
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
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissSkillsOverlay);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                commands.push(self.refresh_catalog_command());
            }
            KeyCode::Char('/') if self.tab == SkillTab::Catalog => {
                self.catalog_keyword_draft = self.catalog_keyword.clone();
                self.mode = SkillOverlayMode::CatalogFilter;
            }
            KeyCode::Char('s') | KeyCode::Char('S') if self.tab == SkillTab::Catalog => {
                self.cycle_catalog_source_filter();
                commands.push(self.list_catalog_command());
            }
            KeyCode::Char('n') | KeyCode::Char('N') if self.tab == SkillTab::Sources => {
                self.start_source_create();
            }
            KeyCode::Enter => match self.tab {
                SkillTab::Discovered => {
                    if let Some(entry) = self.selected_discovered() {
                        commands.push(Command::ShowSkill {
                            skill_id: entry.id.clone(),
                        });
                    }
                }
                SkillTab::Catalog => {
                    if self.selected_catalog_entry().is_some() {
                        self.mode = SkillOverlayMode::CatalogDetail;
                    }
                }
                SkillTab::Installed | SkillTab::Sources => {}
            },
            KeyCode::Char('a') | KeyCode::Char('A') => {
                if let (Some(entry), Some(session_id)) =
                    (self.selected_discovered(), ctx.current_session_id.as_ref())
                {
                    if self.tab == SkillTab::Discovered && !entry.active {
                        commands.push(Command::ActivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id: entry.id.clone(),
                        });
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let (Some(entry), Some(session_id)) =
                    (self.selected_discovered(), ctx.current_session_id.as_ref())
                {
                    if self.tab == SkillTab::Discovered && entry.active {
                        commands.push(Command::DeactivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id: entry.id.clone(),
                        });
                    }
                }
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
            CrossPanelEffect::ShowSkillsOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissSkillsOverlay => self.hide(),
            CrossPanelEffect::ShowSkillBody { skill_id, body } if self.visible => {
                self.body = Some(BodyView {
                    skill_id: skill_id.clone(),
                    body: body.clone(),
                });
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut discovered_state = self.discovered_state;
        let mut installed_state = self.installed_state;
        let mut catalog_state = self.catalog_state;
        let mut sources_state = self.sources_state;
        render_skills_overlay(
            area,
            frame,
            self,
            &mut discovered_state,
            &mut installed_state,
            &mut catalog_state,
            &mut sources_state,
        );
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
    use crate::components::{FocusTarget, SessionInfo, SkillOverlaySnapshot};
    use agent_core::facade::{
        SkillCatalogEntry, SkillInstallSource, SkillInstallTarget, SkillSettingsScope,
        SkillSettingsView, SkillSourceView, SkillUpdateState,
    };

    fn entry(id: &str, active: bool) -> SkillEntry {
        SkillEntry {
            id: id.to_string(),
            name: id.to_string(),
            description: format!("{id} description"),
            source: "user".to_string(),
            activation_mode: "manual".to_string(),
            active,
        }
    }

    fn installed_skill(skill_id: &str, enabled: bool) -> SkillSettingsView {
        SkillSettingsView {
            settings_id: format!("user:{skill_id}"),
            id: skill_id.to_string(),
            name: skill_id.to_string(),
            description: format!("{skill_id} settings"),
            version: Some("1.0.0".to_string()),
            scope: SkillSettingsScope::User,
            path: format!("/tmp/{skill_id}/SKILL.md"),
            enabled,
            activation_mode: "manual".to_string(),
            install_source: SkillInstallSource::Registry,
            update_state: SkillUpdateState::UpdateAvailable,
            effective: enabled,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            editable: true,
            deletable: true,
        }
    }

    fn catalog_entry(name: &str) -> SkillCatalogEntry {
        SkillCatalogEntry {
            catalog_id: "skillhub".to_string(),
            name: name.to_string(),
            description: format!("{name} catalog skill"),
            source: "skillhub".to_string(),
            source_url: format!("https://example.test/{name}"),
            install_count: Some(42),
            github_stars: Some(7),
            security_score: Some(95),
            rating: Some(4.8),
            package: name.to_string(),
            package_url: Some(format!("https://example.test/{name}.zip")),
        }
    }

    fn source(id: &str, enabled: bool) -> SkillSourceView {
        SkillSourceView {
            id: id.to_string(),
            display_name: id.to_string(),
            kind: "skillhub".to_string(),
            url: format!("https://example.test/{id}"),
            search_template: "/api/skills?q={{query}}".to_string(),
            download_template: "/api/download/{{slug}}".to_string(),
            list_template: Some("/api/skills".to_string()),
            detail_template: None,
            field_mapping: agent_core::facade::SkillFieldMappingView::default(),
            enabled,
            priority: 10,
            cache_ttl_seconds: 900,
            last_error: None,
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn type_text(overlay: &mut SkillsOverlay, text: &str) {
        for ch in text.chars() {
            let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char(ch)));
        }
    }

    fn render_overlay_text(overlay: &SkillsOverlay) -> String {
        let backend = ratatui::backend::TestBackend::new(140, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| overlay.render(f.area(), f))
            .expect("render");
        let buf = terminal.backend().buffer().clone();
        let mut rendered = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        rendered
    }

    fn test_ctx_session(
        session_id: &Option<agent_core::SessionId>,
        workspace_id: &agent_core::WorkspaceId,
    ) -> EventContext<'static> {
        use agent_core::projection::SessionProjection;
        static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
        let projection = PROJECTION.get_or_init(SessionProjection::default);
        static SESSIONS: std::sync::OnceLock<Vec<SessionInfo>> = std::sync::OnceLock::new();
        let sessions = SESSIONS.get_or_init(Vec::new);
        // The component only reads `workspace_id` and `current_session_id` —
        // leak owned copies so the static-lifetime EventContext compiles for
        // tests without us having to thread a runtime through.
        let ws: &'static agent_core::WorkspaceId = Box::leak(Box::new(workspace_id.clone()));
        let sid: &'static Option<agent_core::SessionId> = Box::leak(Box::new(session_id.clone()));
        EventContext {
            focus: FocusTarget::SkillsOverlay,
            current_session: projection,
            projects: &[],
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: true,
            workspace_id: ws,
            current_session_id: sid,
        }
    }

    fn test_ctx() -> EventContext<'static> {
        let ws = agent_core::WorkspaceId::new();
        let sid: Option<agent_core::SessionId> = Some(agent_core::SessionId::new());
        test_ctx_session(&sid, &ws)
    }

    #[test]
    fn lists_skills_with_active_marker() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", true), entry("beta", false)]);
        assert!(overlay.is_visible());
        assert_eq!(overlay.skills().len(), 2);
        assert_eq!(overlay.selected_index(), Some(0));

        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| overlay.render(f.area(), f))
            .expect("render");
        let buf = terminal.backend().buffer().clone();
        let mut rendered = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        assert!(rendered.contains("alpha"), "alpha row missing: {rendered}");
        assert!(rendered.contains("beta"), "beta row missing: {rendered}");
        assert!(
            rendered.contains("active"),
            "active marker missing for active skill: {rendered}"
        );
    }

    #[test]
    fn overlay_invisible_by_default() {
        let overlay = SkillsOverlay::new();
        assert!(!overlay.is_visible());
        assert!(overlay.skills().is_empty());
    }

    #[test]
    fn j_and_k_navigate_selection() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![
            entry("alpha", false),
            entry("beta", true),
            entry("gamma", false),
        ]);
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(2));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Down));
        assert_eq!(overlay.selected_index(), Some(2));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Up));
        assert_eq!(overlay.selected_index(), Some(0));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(0));
    }

    #[test]
    fn enter_emits_show_skill_for_selected() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false), entry("beta", false)]);
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::ShowSkill { skill_id } if skill_id == "beta"
        ));
    }

    #[test]
    fn body_effect_switches_to_detail_view() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false)]);
        overlay.handle_effect(&CrossPanelEffect::ShowSkillBody {
            skill_id: "alpha".to_string(),
            body: "## Body\n\nDoc text".to_string(),
        });
        assert_eq!(overlay.body_skill_id(), Some("alpha"));

        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| overlay.render(f.area(), f))
            .expect("render");
        let buf = terminal.backend().buffer().clone();
        let mut rendered = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        assert!(rendered.contains("Doc text"), "body text missing");

        // Esc in body view returns to the list, not dismiss.
        let (effects, _) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
        assert!(effects.is_empty());
        assert!(overlay.is_visible());
        assert_eq!(overlay.body_skill_id(), None);
    }

    #[test]
    fn a_emits_activate_for_inactive_skill() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));
        assert!(matches!(
            &commands[0],
            Command::ActivateSkill { skill_id, .. } if skill_id == "alpha"
        ));
    }

    #[test]
    fn a_is_no_op_for_already_active_skill() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", true)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));
        assert!(commands.is_empty());
    }

    #[test]
    fn d_emits_deactivate_for_active_skill() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", true)]);
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('d')));
        assert!(matches!(
            &commands[0],
            Command::DeactivateSkill { skill_id, .. } if skill_id == "alpha"
        ));
    }

    #[test]
    fn a_without_session_emits_nothing() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false)]);
        let ws = agent_core::WorkspaceId::new();
        let ctx = test_ctx_session(&None, &ws);
        let (_, commands) = overlay.handle_event(&ctx, &key(KeyCode::Char('a')));
        assert!(commands.is_empty());
    }

    #[test]
    fn esc_hides_and_emits_dismiss_effect() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false)]);
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
        assert!(commands.is_empty());
        assert!(effects.contains(&CrossPanelEffect::DismissSkillsOverlay));
        assert!(!overlay.is_visible());
    }

    #[test]
    fn ignores_keys_when_hidden() {
        let mut overlay = SkillsOverlay::new();
        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
    }

    #[test]
    fn show_effect_makes_visible() {
        let mut overlay = SkillsOverlay::new();
        overlay.handle_effect(&CrossPanelEffect::ShowSkillsOverlay(
            vec![entry("alpha", false)].into(),
        ));
        assert!(overlay.is_visible());
        assert_eq!(overlay.skills().len(), 1);
    }

    #[test]
    fn show_preserves_selection_across_refresh() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(vec![entry("alpha", false), entry("beta", false)]);
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));
        // Same list, beta now active — selection should stay on beta.
        overlay.show(vec![entry("alpha", false), entry("beta", true)]);
        assert_eq!(overlay.selected_index(), Some(1));
        assert!(overlay.skills()[1].active);
    }

    #[test]
    fn installed_tab_dispatches_enable_update_and_delete_commands() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(SkillOverlaySnapshot {
            discovered: vec![entry("alpha", false)],
            installed: vec![installed_skill("review", true)],
            catalog: vec![],
            sources: vec![],
            install_target: SkillInstallTarget::User,
        });
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (_, enable_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
        assert!(matches!(
            &enable_commands[..],
            [Command::SetSkillEnabled { skill_id, enabled }]
                if skill_id == "review" && !enabled
        ));

        let (_, update_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('u')));
        assert!(matches!(
            &update_commands[..],
            [Command::UpdateSkillSettings { skill_id }] if skill_id == "review"
        ));

        let (_, delete_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
        assert!(matches!(
            &delete_commands[..],
            [Command::DeleteSkillSettings { skill_id }] if skill_id == "review"
        ));
    }

    #[test]
    fn catalog_tab_installs_selected_entry_to_current_target() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(SkillOverlaySnapshot {
            discovered: vec![],
            installed: vec![],
            catalog: vec![catalog_entry("review")],
            sources: vec![],
            install_target: SkillInstallTarget::User,
        });
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (_, install_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
        assert!(matches!(
            &install_commands[..],
            [Command::InstallRemoteSkill { request }]
                if request.package == "review"
                    && request.source == "skillhub"
                    && request.target == SkillInstallTarget::User
                    && request.package_url.as_deref() == Some("https://example.test/review.zip")
        ));

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
        let (_, project_install_commands) =
            overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
        assert!(matches!(
            &project_install_commands[..],
            [Command::InstallRemoteSkill { request }]
                if request.package == "review" && request.target == SkillInstallTarget::Project
        ));
    }

    #[test]
    fn catalog_enter_opens_detail_and_esc_returns_to_list() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(SkillOverlaySnapshot {
            discovered: vec![],
            installed: vec![],
            catalog: vec![catalog_entry("review")],
            sources: vec![],
            install_target: SkillInstallTarget::User,
        });
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
        let rendered = render_overlay_text(&overlay);
        assert!(
            rendered.contains("Source: https://example.test/review"),
            "source URL missing from detail: {rendered}"
        );
        assert!(
            rendered.contains("Package: review"),
            "package missing from detail: {rendered}"
        );
        assert!(
            rendered.contains("Download: https://example.test/review.zip"),
            "download URL missing from detail: {rendered}"
        );
        assert!(
            rendered.contains("Installs: 42"),
            "install stats missing from detail: {rendered}"
        );
        assert!(
            rendered.contains("Target: user"),
            "target confirmation missing from detail: {rendered}"
        );

        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
        assert!(overlay.is_visible());
        let rendered = render_overlay_text(&overlay);
        assert!(
            rendered.contains("review catalog skill"),
            "Esc should return to catalog list: {rendered}"
        );
    }

    #[test]
    fn catalog_detail_installs_selected_entry_to_current_target() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(SkillOverlaySnapshot {
            discovered: vec![],
            installed: vec![],
            catalog: vec![catalog_entry("review")],
            sources: vec![],
            install_target: SkillInstallTarget::User,
        });
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
        let rendered = render_overlay_text(&overlay);
        assert!(
            rendered.contains("Target: project"),
            "target toggle should update detail confirmation: {rendered}"
        );

        let (_, install_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));
        assert!(matches!(
            &install_commands[..],
            [Command::InstallRemoteSkill { request }]
                if request.package == "review"
                    && request.source == "skillhub"
                    && request.target == SkillInstallTarget::Project
                    && request.package_url.as_deref() == Some("https://example.test/review.zip")
        ));
    }

    #[test]
    fn catalog_tab_searches_and_refreshes_with_active_source_filter() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(SkillOverlaySnapshot {
            discovered: vec![],
            installed: vec![],
            catalog: vec![catalog_entry("review")],
            sources: vec![source("skillhub", true), source("corp", true)],
            install_target: SkillInstallTarget::User,
        });
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (_, source_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('s')));
        assert!(matches!(
            &source_commands[..],
            [Command::ListSkillCatalog { keyword: None, sources: Some(sources) }]
                if sources == &vec!["skillhub".to_string()]
        ));

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('/')));
        type_text(&mut overlay, "review");
        let (_, search_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(
            &search_commands[..],
            [Command::ListSkillCatalog { keyword: Some(keyword), sources: Some(sources) }]
                if keyword == "review" && sources == &vec!["skillhub".to_string()]
        ));

        let (_, refresh_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('r')));
        assert!(matches!(
            &refresh_commands[..],
            [Command::RefreshSkillCatalog { keyword: Some(keyword), sources: Some(sources) }]
                if keyword == "review" && sources == &vec!["skillhub".to_string()]
        ));
    }

    #[test]
    fn sources_tab_toggles_selected_source() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(SkillOverlaySnapshot {
            discovered: vec![],
            installed: vec![],
            catalog: vec![],
            sources: vec![source("skillhub", true)],
            install_target: SkillInstallTarget::User,
        });
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));
        assert!(matches!(
            &commands[..],
            [Command::SetSkillSourceEnabled { source_id, enabled }]
                if source_id == "skillhub" && !enabled
        ));
    }

    #[test]
    fn sources_tab_adds_and_removes_skill_sources() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(SkillOverlaySnapshot {
            discovered: vec![],
            installed: vec![],
            catalog: vec![],
            sources: vec![source("skillhub", true)],
            install_target: SkillInstallTarget::User,
        });
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));

        let (_, remove_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));
        assert!(matches!(
            &remove_commands[..],
            [Command::RemoveSkillSource { source_id }] if source_id == "skillhub"
        ));

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('n')));
        assert!(commands.is_empty());
        type_text(&mut overlay, "corp");
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        type_text(&mut overlay, "Corporate Skills");
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));
        type_text(&mut overlay, "https://skills.example.com");
        let (_, add_commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Enter));

        assert!(matches!(
            &add_commands[..],
            [Command::AddSkillSource { config }]
                if config.id == "corp"
                    && config.display_name == "Corporate Skills"
                    && config.kind == "skillhub"
                    && config.url == "https://skills.example.com"
                    && config.search_template == "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc"
                    && config.download_template == "/api/v1/download?slug={{slug}}"
                    && config.list_template.as_deref() == Some("/api/skills?page=1&pageSize={{limit}}&sortBy=downloads&order=desc")
                    && config.detail_template.as_deref() == Some("/api/v1/skills/{{slug}}")
                    && config.enabled
                    && config.priority == 100
                    && config.cache_ttl_seconds == 900
                    && config.last_error.is_none()
        ));
    }

    #[test]
    fn discovered_tab_keeps_session_activation_commands() {
        let mut overlay = SkillsOverlay::new();
        overlay.show(SkillOverlaySnapshot {
            discovered: vec![entry("alpha", false)],
            installed: vec![installed_skill("alpha", true)],
            catalog: vec![catalog_entry("alpha")],
            sources: vec![source("skillhub", true)],
            install_target: SkillInstallTarget::User,
        });

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('a')));

        assert!(matches!(
            &commands[..],
            [Command::ActivateSkill { skill_id, .. }] if skill_id == "alpha"
        ));
    }
}
