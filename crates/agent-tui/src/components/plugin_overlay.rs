//! Plugin manager overlay — compact keyboard surface over installed plugins,
//! catalog entries, and marketplace sources.
//!
//! The App builds a [`PluginOverlaySnapshot`] from the existing plugin facade;
//! the overlay only owns selection state and emits mutation commands.

use agent_core::facade::{
    InstallPluginRequest, PluginCatalogEntry, PluginInstallTarget, PluginMarketplaceSourceView,
    PluginSettingsView,
};
use agent_core::ConfigScope;
use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, PluginOverlaySnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginTab {
    Installed,
    Catalog,
    Sources,
}

impl PluginTab {
    fn next(self) -> Self {
        match self {
            Self::Installed => Self::Catalog,
            Self::Catalog => Self::Sources,
            Self::Sources => Self::Installed,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Installed => Self::Sources,
            Self::Catalog => Self::Installed,
            Self::Sources => Self::Catalog,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::Catalog => "Catalog",
            Self::Sources => "Sources",
        }
    }
}

pub struct PluginOverlay {
    focused: bool,
    visible: bool,
    tab: PluginTab,
    plugins: Vec<PluginSettingsView>,
    catalog: Vec<PluginCatalogEntry>,
    sources: Vec<PluginMarketplaceSourceView>,
    install_target: PluginInstallTarget,
    plugins_state: ListState,
    catalog_state: ListState,
    sources_state: ListState,
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
            tab: PluginTab::Installed,
            plugins: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
            install_target: PluginInstallTarget::User,
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
        self.ensure_selection();
    }

    pub fn hide(&mut self) {
        self.visible = false;
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

    fn ensure_selection(&mut self) {
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

    fn toggle_install_target(&mut self) {
        self.install_target = match self.install_target {
            PluginInstallTarget::User => PluginInstallTarget::Project,
            PluginInstallTarget::Project => PluginInstallTarget::User,
        };
    }

    fn command_for_current_tab(&mut self, key: KeyCode) -> Option<Command> {
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

fn target_label(target: PluginInstallTarget) -> &'static str {
    match target {
        PluginInstallTarget::User => "user",
        PluginInstallTarget::Project => "project",
    }
}

pub fn render_plugin_overlay(
    area: Rect,
    frame: &mut Frame,
    overlay: &PluginOverlay,
    plugins_state: &mut ListState,
    catalog_state: &mut ListState,
    sources_state: &mut ListState,
) {
    let modal_width = 88.min(area.width.saturating_sub(4));
    let modal_height = 24.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Plugin Manager ",
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
        PluginTab::Installed => render_installed(chunks[1], frame, &overlay.plugins, plugins_state),
        PluginTab::Catalog => render_catalog(
            chunks[1],
            frame,
            &overlay.catalog,
            catalog_state,
            overlay.install_target,
        ),
        PluginTab::Sources => render_sources(chunks[1], frame, &overlay.sources, sources_state),
    }

    render_hints(chunks[2], frame, overlay);
}

fn render_tabs(area: Rect, frame: &mut Frame, overlay: &PluginOverlay) {
    let mut spans = Vec::new();
    for tab in [PluginTab::Installed, PluginTab::Catalog, PluginTab::Sources] {
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
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_installed(
    area: Rect,
    frame: &mut Frame,
    plugins: &[PluginSettingsView],
    state: &mut ListState,
) {
    if plugins.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No plugins installed",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = plugins
        .iter()
        .map(|plugin| {
            let enabled_label = if plugin.enabled {
                "enabled "
            } else {
                "disabled"
            };
            let enabled_color = if plugin.enabled {
                Color::Green
            } else {
                Color::DarkGray
            };
            let effective = if plugin.effective { " effective" } else { "" };
            let validity = if plugin.valid { "" } else { " invalid" };
            let inventory = format!(
                " s:{} mcp:{} a:{} h:{}",
                plugin.inventory.skill_count,
                plugin.inventory.mcp_server_count,
                plugin.inventory.agent_count,
                plugin.inventory.hook_count
            );
            let line = Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    plugin.settings_id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(plugin.description.clone(), Style::default().fg(Color::Gray)),
                Span::styled(
                    format!(" [{}{}{}{}]", plugin.scope, effective, validity, inventory),
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
    catalog: &[PluginCatalogEntry],
    state: &mut ListState,
    install_target: PluginInstallTarget,
) {
    if catalog.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No catalog plugins available",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = catalog
        .iter()
        .map(|entry| {
            let version = entry.version.as_deref().unwrap_or("unknown");
            let line = Line::from(vec![
                Span::styled(
                    entry.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  v{version}"),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  @{}", entry.marketplace_id),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("  "),
                Span::styled(entry.description.clone(), Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("  -> {}", target_label(install_target)),
                    Style::default().fg(Color::Magenta),
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

fn render_sources(
    area: Rect,
    frame: &mut Frame,
    sources: &[PluginMarketplaceSourceView],
    state: &mut ListState,
) {
    if sources.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No marketplace sources configured",
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
            let kind = if source.builtin { "builtin" } else { "user" };
            let line = Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    source.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {kind}"), Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(source.source.clone(), Style::default().fg(Color::Gray)),
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

fn render_hints(area: Rect, frame: &mut Frame, overlay: &PluginOverlay) {
    let action = match overlay.tab {
        PluginTab::Installed => "[e] enable  [x] delete  ",
        PluginTab::Catalog => "[i] install  [t] target  ",
        PluginTab::Sources => "[e] enable source  ",
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

impl Component for PluginOverlay {
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
                effects.push(CrossPanelEffect::DismissPluginsOverlay);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => commands.push(Command::OpenPluginsOverlay),
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
            CrossPanelEffect::ShowPluginsOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissPluginsOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut plugins_state = self.plugins_state;
        let mut catalog_state = self.catalog_state;
        let mut sources_state = self.sources_state;
        render_plugin_overlay(
            area,
            frame,
            self,
            &mut plugins_state,
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
    use super::PluginOverlay;
    use agent_core::facade::{
        PluginCatalogEntry, PluginComponentInventoryView, PluginInstallTarget,
        PluginMarketplaceSourceView, PluginSettingsView,
    };
    use agent_core::ConfigScope;
    use crossterm::event::{Event, KeyCode};

    use crate::components::{
        Command, Component, CrossPanelEffect, EventContext, FocusTarget, PluginOverlaySnapshot,
    };

    fn installed_plugin(settings_id: &str, enabled: bool) -> PluginSettingsView {
        PluginSettingsView {
            settings_id: settings_id.to_string(),
            id: settings_id.replace(':', "-"),
            name: settings_id.to_string(),
            description: format!("{settings_id} plugin"),
            version: Some("1.2.3".to_string()),
            scope: ConfigScope::User,
            path: format!("/tmp/{settings_id}"),
            enabled,
            install_source: Some("local".to_string()),
            marketplace: Some("local-market".to_string()),
            effective: true,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            inventory: PluginComponentInventoryView {
                skill_count: 2,
                skill_names: vec!["alpha".to_string(), "beta".to_string()],
                mcp_server_count: 1,
                app_count: 0,
                agent_count: 1,
                hook_count: 0,
            },
            manifest_kind: "kairox".to_string(),
        }
    }

    fn catalog_entry(name: &str) -> PluginCatalogEntry {
        PluginCatalogEntry {
            marketplace_id: "local-market".to_string(),
            name: name.to_string(),
            description: format!("{name} catalog plugin"),
            version: Some("0.1.0".to_string()),
            source: format!("/tmp/catalog/{name}"),
        }
    }

    fn source(id: &str, enabled: bool) -> PluginMarketplaceSourceView {
        PluginMarketplaceSourceView {
            id: id.to_string(),
            display_name: id.to_string(),
            source: format!("/tmp/{id}"),
            enabled,
            builtin: false,
        }
    }

    fn snapshot() -> PluginOverlaySnapshot {
        PluginOverlaySnapshot {
            plugins: vec![
                installed_plugin("user:alpha", true),
                installed_plugin("user:beta", false),
            ],
            catalog: vec![catalog_entry("delta")],
            sources: vec![source("local-market", true)],
            install_target: PluginInstallTarget::User,
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
            focus: FocusTarget::PluginOverlay,
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

    #[test]
    fn lists_installed_plugins_from_snapshot() {
        let mut overlay = PluginOverlay::new();
        overlay.show(snapshot());

        assert!(overlay.is_visible());
        assert_eq!(overlay.selected_index(), Some(0));
        assert_eq!(overlay.plugins().len(), 2);

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
        assert!(
            rendered.contains("user:alpha"),
            "installed plugin missing: {rendered}"
        );
        assert!(
            rendered.contains("enabled"),
            "enabled marker missing: {rendered}"
        );
    }

    #[test]
    fn e_toggles_selected_installed_plugin_enabled_state() {
        let mut overlay = PluginOverlay::new();
        overlay.show(snapshot());

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));

        assert!(matches!(
            &commands[..],
            [Command::SetPluginEnabled { settings_id, enabled }]
                if settings_id == "user:alpha" && !enabled
        ));
    }

    #[test]
    fn x_deletes_selected_installed_plugin() {
        let mut overlay = PluginOverlay::new();
        overlay.show(snapshot());

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('x')));

        assert!(matches!(
            &commands[..],
            [Command::DeletePluginSettings { settings_id }] if settings_id == "user:alpha"
        ));
    }

    #[test]
    fn i_installs_selected_catalog_plugin_to_current_target() {
        let mut overlay = PluginOverlay::new();
        overlay.show(snapshot());
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));

        assert!(matches!(
            &commands[..],
            [Command::InstallPlugin { request }]
                if request.marketplace_id == "local-market"
                    && request.plugin_name == "delta"
                    && request.target == PluginInstallTarget::User
        ));
    }

    #[test]
    fn t_changes_catalog_install_target() {
        let mut overlay = PluginOverlay::new();
        overlay.show(snapshot());
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Tab));

        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('t')));
        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('i')));

        assert!(matches!(
            &commands[..],
            [Command::InstallPlugin { request }]
                if request.plugin_name == "delta" && request.target == PluginInstallTarget::Project
        ));
    }

    #[test]
    fn e_toggles_selected_marketplace_source() {
        let mut overlay = PluginOverlay::new();
        overlay.show(snapshot());
        let _ = overlay.handle_event(&test_ctx(), &key(KeyCode::BackTab));

        let (_, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Char('e')));

        assert!(matches!(
            &commands[..],
            [Command::SetPluginMarketplaceSourceEnabled { source_id, enabled }]
                if source_id == "local-market" && !enabled
        ));
    }

    #[test]
    fn esc_hides_and_emits_dismiss_effect() {
        let mut overlay = PluginOverlay::new();
        overlay.show(snapshot());

        let (effects, commands) = overlay.handle_event(&test_ctx(), &key(KeyCode::Esc));

        assert!(commands.is_empty());
        assert!(effects.contains(&CrossPanelEffect::DismissPluginsOverlay));
        assert!(!overlay.is_visible());
    }
}
