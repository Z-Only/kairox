//! Skills overlay — pop-up modal listing native skills with an active marker,
//! supporting per-session activation/deactivation and inline body preview.
//!
//! The TUI surface for the same data the GUI's `SkillSettingsPane` shows,
//! minus remote-marketplace search. The App constructs a snapshot of
//! [`SkillEntry`] values before opening the overlay; the overlay produces
//! [`Command`] values that the main loop dispatches back to `AppFacade`.

use agent_core::facade::{
    InstallRemoteSkillRequest, SkillCatalogEntry, SkillInstallSource, SkillInstallTarget,
    SkillSettingsScope, SkillSettingsView, SkillSourceView,
};
use crossterm::event::{Event, KeyCode};
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

pub struct SkillsOverlay {
    focused: bool,
    visible: bool,
    tab: SkillTab,
    discovered: Vec<SkillEntry>,
    installed: Vec<SkillSettingsView>,
    catalog: Vec<SkillCatalogEntry>,
    sources: Vec<SkillSourceView>,
    install_target: SkillInstallTarget,
    discovered_state: ListState,
    installed_state: ListState,
    catalog_state: ListState,
    sources_state: ListState,
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
            tab: SkillTab::Discovered,
            discovered: Vec::new(),
            installed: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
            install_target: SkillInstallTarget::User,
            discovered_state: ListState::default(),
            installed_state: ListState::default(),
            catalog_state: ListState::default(),
            sources_state: ListState::default(),
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
        self.discovered_state.select(None);
        self.installed_state.select(None);
        self.catalog_state.select(None);
        self.sources_state.select(None);
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
            (SkillTab::Catalog, KeyCode::Char('i') | KeyCode::Char('I')) => self
                .selected_catalog_entry()
                .map(|entry| Command::InstallRemoteSkill {
                    request: InstallRemoteSkillRequest {
                        package: entry.package.clone(),
                        source: entry.source.clone(),
                        target: self.install_target,
                        package_url: entry.package_url.clone(),
                    },
                }),
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

    let title = if overlay.body.is_some() {
        " 🧠 Skill detail "
    } else {
        " 🧠 Skills Manager "
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
        SkillTab::Catalog => render_catalog(
            chunks[1],
            frame,
            &overlay.catalog,
            catalog_state,
            overlay.install_target,
        ),
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
    let action = match overlay.tab {
        SkillTab::Discovered => "[Enter] body  [a] activate  [d] deactivate  ",
        SkillTab::Installed => "[e] enable  [u] update  [x] delete  ",
        SkillTab::Catalog => "[i] install  [t] target  ",
        SkillTab::Sources => "[e] enable source  ",
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
            KeyCode::Char('r') | KeyCode::Char('R') => commands.push(Command::RefreshSkillCatalog),
            KeyCode::Enter => {
                if let Some(entry) = self
                    .selected_discovered()
                    .filter(|_| self.tab == SkillTab::Discovered)
                {
                    commands.push(Command::ShowSkill {
                        skill_id: entry.id.clone(),
                    });
                }
            }
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
