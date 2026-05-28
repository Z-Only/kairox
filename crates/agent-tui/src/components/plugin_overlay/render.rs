use agent_core::facade::{
    PluginCatalogEntry, PluginInstallTarget, PluginMarketplaceSourceView, PluginSettingsView,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::state::{non_empty_trimmed, PluginOverlay};
use super::types::{PluginOverlayMode, PluginTab};

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
    if overlay.tab == PluginTab::Catalog || overlay.catalog_filters_active() {
        let keyword = non_empty_trimmed(&overlay.catalog_keyword).unwrap_or_else(|| "*".into());
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!(
                "catalog:{}  search:{}  source:{}",
                overlay.catalog.len(),
                keyword,
                overlay.catalog_marketplace_label()
            ),
            Style::default().fg(Color::Cyan),
        ));
    }
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
    let action = if overlay.mode == PluginOverlayMode::CatalogSearch {
        "[Enter/Esc] apply search  [Backspace] edit  "
    } else {
        match overlay.tab {
            PluginTab::Installed => "[e] enable  [x] delete  ",
            PluginTab::Catalog => "[/] search  [s] source  [i] install  [t] target  ",
            PluginTab::Sources => "[e] enable source  ",
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

fn target_label(target: PluginInstallTarget) -> &'static str {
    match target {
        PluginInstallTarget::User => "user",
        PluginInstallTarget::Project => "project",
    }
}
