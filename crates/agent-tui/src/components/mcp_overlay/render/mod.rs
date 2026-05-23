use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::state::{McpOverlay, McpOverlayMode, McpOverlayTab};

mod catalog;
mod editors;
mod lists;
mod runtime;
mod tools;

struct McpOverlayRenderState<'a> {
    runtime: &'a mut ListState,
    settings: &'a mut ListState,
    installed: &'a mut ListState,
    catalog: &'a mut ListState,
    sources: &'a mut ListState,
    tools: &'a mut ListState,
    resources: &'a mut ListState,
    prompts: &'a mut ListState,
}

pub(super) fn render_mcp_overlay(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    if !overlay.visible {
        return;
    }
    let mut runtime_state = overlay.runtime_state;
    let mut settings_state = overlay.settings_state;
    let mut installed_state = overlay.installed_state;
    let mut catalog_state = overlay.catalog_state;
    let mut sources_state = overlay.sources_state;
    let mut tools_state = overlay.tools_state;
    let mut resources_state = overlay.resources_state;
    let mut prompts_state = overlay.prompts_state;
    let mut render_state = McpOverlayRenderState {
        runtime: &mut runtime_state,
        settings: &mut settings_state,
        installed: &mut installed_state,
        catalog: &mut catalog_state,
        sources: &mut sources_state,
        tools: &mut tools_state,
        resources: &mut resources_state,
        prompts: &mut prompts_state,
    };
    render_mcp_overlay_content(area, frame, overlay, &mut render_state);
}

fn render_mcp_overlay_content(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
    state: &mut McpOverlayRenderState<'_>,
) {
    let modal_width = 96.min(area.width.saturating_sub(4));
    let modal_height = 24.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " 🔌 MCP Servers ",
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

    match overlay.mode {
        McpOverlayMode::ServerEditor => {
            editors::render_server_editor(inner, frame, overlay);
            return;
        }
        McpOverlayMode::SourceEditor => {
            editors::render_source_editor(inner, frame, overlay);
            return;
        }
        McpOverlayMode::CatalogInstallConfig => {
            editors::render_catalog_install_config_editor(inner, frame, overlay);
            return;
        }
        McpOverlayMode::List | McpOverlayMode::CatalogFilter => {}
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
        McpOverlayTab::Runtime => {
            runtime::render_runtime(chunks[1], frame, overlay, state.runtime);
        }
        McpOverlayTab::Settings => {
            lists::render_settings(chunks[1], frame, &overlay.settings, state.settings);
        }
        McpOverlayTab::Installed => {
            lists::render_installed(chunks[1], frame, &overlay.installed, state.installed);
        }
        McpOverlayTab::Catalog => {
            catalog::render_catalog(chunks[1], frame, overlay, state.catalog);
        }
        McpOverlayTab::Sources => {
            lists::render_sources(chunks[1], frame, &overlay.sources, state.sources);
        }
        McpOverlayTab::Tools => {
            tools::render_tools(chunks[1], frame, overlay, state.tools);
        }
        McpOverlayTab::Resources => {
            tools::render_resources(chunks[1], frame, overlay, state.resources);
        }
        McpOverlayTab::Prompts => {
            tools::render_prompts(chunks[1], frame, overlay, state.prompts);
        }
    }
    render_hints(chunks[2], frame, overlay);
}

fn render_tabs(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let mut spans = Vec::new();
    for tab in [
        McpOverlayTab::Runtime,
        McpOverlayTab::Settings,
        McpOverlayTab::Installed,
        McpOverlayTab::Catalog,
        McpOverlayTab::Sources,
        McpOverlayTab::Tools,
        McpOverlayTab::Resources,
        McpOverlayTab::Prompts,
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
    if overlay.tab == McpOverlayTab::Catalog || overlay.catalog_filters_active() {
        let keyword = if overlay.catalog_keyword.trim().is_empty() {
            "*".to_string()
        } else {
            clip(overlay.catalog_keyword.trim(), 18)
        };
        spans.push(Span::styled(
            format!(
                "catalog: {}/{}  search:{keyword}  trust:{}",
                overlay.visible_catalog_len(),
                overlay.catalog.len(),
                overlay.catalog_trust_filter.label()
            ),
            Style::default().fg(Color::Cyan),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub(super) fn render_empty(area: Rect, frame: &mut Frame, label: &'static str) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            label,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}

pub(super) fn clip(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let clipped: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{clipped}...")
    } else {
        clipped
    }
}

pub(super) fn render_list(
    area: Rect,
    frame: &mut Frame,
    items: Vec<ListItem>,
    state: &mut ListState,
) {
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, state);
}

fn render_hints(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let action = if overlay.mode == McpOverlayMode::CatalogFilter {
        "[Enter/Esc] apply search  [Backspace] edit  "
    } else {
        match overlay.tab {
            McpOverlayTab::Runtime => {
                "[Enter] start/stop  [t] trust/revoke  [h] health  [c] test  [r] tools  "
            }
            McpOverlayTab::Settings => {
                "[n] new  [Enter] edit  [e] enable  [d/a] project off/on  [o] config  [x] delete  "
            }
            McpOverlayTab::Installed => "[x/u] uninstall  [r] reload  ",
            McpOverlayTab::Catalog => "[i] install  [/] search  [t] trust  [r] reload  ",
            McpOverlayTab::Sources => {
                "[n] new  [e] enable source  [x] remove  [o] config  [r] reload  "
            }
            McpOverlayTab::Tools => "[r] health  [e/Enter] enable tool  ",
            McpOverlayTab::Resources => "[r] list  [Enter] read  ",
            McpOverlayTab::Prompts => "[r] list  ",
        }
    };
    let hints = Line::from(vec![
        Span::styled("[Tab] tab  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled(action, Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), area);
}
