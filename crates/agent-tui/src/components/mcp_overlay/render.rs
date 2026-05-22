use agent_core::facade::{CatalogSourceView, InstalledEntry, McpServerSettingsView, ServerEntry};
use agent_mcp::catalog::InstallSpec;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::McpServerStatusView;

use super::editor::{
    catalog_config_items, parse_install_spec, parse_requirements, ServerDraft, ServerEditorField,
    ServerTransportDraft, SourceDraft, SourceEditorField, SERVER_EDITOR_FIELDS,
    SOURCE_EDITOR_FIELDS,
};
use super::state::{
    resource_preview_key, CatalogInstallStatus, McpOverlay, McpOverlayMode, McpOverlayTab,
};

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
            render_server_editor(inner, frame, overlay);
            return;
        }
        McpOverlayMode::SourceEditor => {
            render_source_editor(inner, frame, overlay);
            return;
        }
        McpOverlayMode::CatalogInstallConfig => {
            render_catalog_install_config_editor(inner, frame, overlay);
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
            render_runtime(chunks[1], frame, overlay, state.runtime);
        }
        McpOverlayTab::Settings => {
            render_settings(chunks[1], frame, &overlay.settings, state.settings);
        }
        McpOverlayTab::Installed => {
            render_installed(chunks[1], frame, &overlay.installed, state.installed);
        }
        McpOverlayTab::Catalog => {
            render_catalog(chunks[1], frame, overlay, state.catalog);
        }
        McpOverlayTab::Sources => {
            render_sources(chunks[1], frame, &overlay.sources, state.sources);
        }
        McpOverlayTab::Tools => {
            render_tools(chunks[1], frame, overlay, state.tools);
        }
        McpOverlayTab::Resources => {
            render_resources(chunks[1], frame, overlay, state.resources);
        }
        McpOverlayTab::Prompts => {
            render_prompts(chunks[1], frame, overlay, state.prompts);
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

fn render_empty(area: Rect, frame: &mut Frame, label: &'static str) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            label,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}

fn render_runtime(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    let servers = &overlay.runtime_servers;
    if servers.is_empty() {
        render_empty(area, frame, "No MCP runtime servers configured");
        return;
    }
    let items: Vec<ListItem> = servers
        .iter()
        .map(|s| {
            let (status_label, status_color) = match s.status {
                McpServerStatusView::Running => ("running ", Color::Green),
                McpServerStatusView::Starting => ("starting", Color::Yellow),
                McpServerStatusView::Stopped => ("stopped ", Color::Gray),
                McpServerStatusView::Failed => ("failed  ", Color::Red),
            };
            let trust_label = if s.trusted { " trusted" } else { "" };
            let health = overlay.health.get(&s.server_id);
            let health_label = health
                .map(|state| {
                    if state.healthy {
                        format!(" health:ok({})", state.tool_count)
                    } else if let Some(error) = &state.error {
                        format!(" health:fail({})", clip(error, 18))
                    } else {
                        " health:fail".to_string()
                    }
                })
                .unwrap_or_default();
            let health_color = match health {
                Some(state) if state.healthy => Color::Green,
                Some(_) => Color::Red,
                None => Color::DarkGray,
            };
            let connectivity = overlay.connectivity.get(&s.server_id);
            let connectivity_label = connectivity
                .map(|state| {
                    if state.connected {
                        let count = state
                            .tool_count
                            .map(|tool_count| format!("({tool_count})"))
                            .unwrap_or_default();
                        format!(" conn:ok{count}")
                    } else {
                        " conn:fail".to_string()
                    }
                })
                .unwrap_or_default();
            let connectivity_color = match connectivity {
                Some(state) if state.connected => Color::Green,
                Some(_) => Color::Red,
                None => Color::DarkGray,
            };
            ListItem::new(Line::from(vec![
                Span::styled(status_label, Style::default().fg(status_color)),
                Span::raw("  "),
                Span::styled(
                    s.server_id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  tools:{}", s.tool_count),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(trust_label, Style::default().fg(Color::Magenta)),
                Span::styled(health_label, Style::default().fg(health_color)),
                Span::styled(connectivity_label, Style::default().fg(connectivity_color)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_settings(
    area: Rect,
    frame: &mut Frame,
    settings: &[McpServerSettingsView],
    state: &mut ListState,
) {
    if settings.is_empty() {
        render_empty(area, frame, "No MCP server settings configured");
        return;
    }
    let items: Vec<ListItem> = settings
        .iter()
        .map(|setting| {
            let enabled_label = if setting.enabled {
                "enabled "
            } else {
                "disabled"
            };
            let enabled_color = if setting.enabled {
                Color::Green
            } else {
                Color::DarkGray
            };
            let writable = if setting.writable {
                " writable"
            } else {
                " read-only"
            };
            let tools = setting
                .tool_count
                .map(|count| format!(" tools:{count}"))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    setting.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", setting.runtime_status),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("  [{}{}]", setting.source, writable),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(tools, Style::default().fg(Color::Cyan)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_installed(
    area: Rect,
    frame: &mut Frame,
    installed: &[InstalledEntry],
    state: &mut ListState,
) {
    if installed.is_empty() {
        render_empty(area, frame, "No MCP marketplace servers installed");
        return;
    }
    let items: Vec<ListItem> = installed
        .iter()
        .map(|entry| {
            let running = if entry.running { "running" } else { "stopped" };
            let source = entry.source.as_deref().unwrap_or("manual");
            let catalog = entry.catalog_id.as_deref().unwrap_or("unknown");
            ListItem::new(Line::from(vec![
                Span::styled(
                    running,
                    Style::default().fg(if entry.running {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
                Span::raw("  "),
                Span::styled(
                    entry.server_id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {catalog}@{source}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_catalog(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(area);
    let list_area = chunks[0];
    let detail_area = chunks[1];

    let catalog = overlay.visible_catalog_entries();
    if catalog.is_empty() {
        let label = if overlay.catalog_filters_active() {
            "No MCP catalog entries match filters"
        } else {
            "No MCP catalog entries available"
        };
        render_empty(list_area, frame, label);
        render_catalog_detail(detail_area, frame, overlay, None);
        return;
    }

    let items: Vec<ListItem> = catalog
        .iter()
        .map(|entry| {
            let mut spans = vec![
                Span::styled(
                    entry.display_name.as_str(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  @{}", entry.source),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("  {}", entry.trust),
                    Style::default().fg(trust_color(&entry.trust)),
                ),
                Span::raw("  "),
                Span::styled(entry.summary.as_str(), Style::default().fg(Color::Gray)),
            ];
            if let Some(status) = overlay.install_status_for_entry(entry) {
                let (label, color) = install_status_list_label(status);
                spans.push(Span::styled(
                    format!("  {label}"),
                    Style::default().fg(color),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    render_list(list_area, frame, items, state);
    render_catalog_detail(
        detail_area,
        frame,
        overlay,
        overlay.selected_catalog_entry(),
    );
}

fn render_catalog_detail(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
    entry: Option<&ServerEntry>,
) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Detail ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(entry) = entry else {
        render_empty(inner, frame, "Select a catalog entry to view details");
        return;
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            entry.display_name.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(clip(&entry.description, 96)),
        Line::from(""),
        Line::from(vec![
            Span::styled("id: ", Style::default().fg(Color::DarkGray)),
            Span::raw(entry.id.clone()),
            Span::styled("  source: ", Style::default().fg(Color::DarkGray)),
            Span::raw(entry.source.clone()),
        ]),
        Line::from(vec![
            Span::styled("trust: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                entry.trust.clone(),
                Style::default().fg(trust_color(&entry.trust)),
            ),
            Span::styled("  version: ", Style::default().fg(Color::DarkGray)),
            Span::raw(entry.version.as_deref().unwrap_or("unknown").to_string()),
        ]),
    ];

    if let Some(author) = &entry.author {
        lines.push(Line::from(vec![
            Span::styled("author: ", Style::default().fg(Color::DarkGray)),
            Span::raw(author.clone()),
        ]));
    }
    if let Some(homepage) = &entry.homepage {
        lines.push(Line::from(vec![
            Span::styled("home: ", Style::default().fg(Color::DarkGray)),
            Span::raw(clip(homepage, 72)),
        ]));
    }
    if !entry.categories.is_empty() || !entry.tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("categories: ", Style::default().fg(Color::DarkGray)),
            Span::raw(join_or_dash(&entry.categories)),
            Span::styled("  tags: ", Style::default().fg(Color::DarkGray)),
            Span::raw(join_or_dash(&entry.tags)),
        ]));
    }

    lines.push(Line::from(""));
    lines.extend(render_install_lines(entry));
    if let Some(status) = overlay.install_status_for_entry(entry) {
        lines.extend(render_install_status_lines(status));
    }
    lines.push(Line::from(""));
    lines.extend(render_requirement_lines(entry));
    lines.push(Line::from(""));
    lines.extend(render_config_lines(entry));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_install_lines(entry: &ServerEntry) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    match parse_install_spec(entry) {
        Some(InstallSpec::Stdio { command, args, .. }) => {
            let mut command_line = command;
            if !args.is_empty() {
                command_line.push(' ');
                command_line.push_str(&args.join(" "));
            }
            lines.push(Line::from(vec![
                Span::styled("install: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("stdio {}", clip(&command_line, 76))),
            ]));
        }
        Some(InstallSpec::Sse { url, headers }) => {
            lines.push(Line::from(vec![
                Span::styled("install: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("sse {}", clip(&url, 76))),
            ]));
            if !headers.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("headers: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(headers.keys().cloned().collect::<Vec<_>>().join(", ")),
                ]));
            }
        }
        Some(InstallSpec::StreamableHttp { url, headers }) => {
            lines.push(Line::from(vec![
                Span::styled("install: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("streamable_http {}", clip(&url, 68))),
            ]));
            if !headers.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("headers: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(headers.keys().cloned().collect::<Vec<_>>().join(", ")),
                ]));
            }
        }
        None => lines.push(Line::from(vec![
            Span::styled("install: ", Style::default().fg(Color::DarkGray)),
            Span::raw("unknown"),
        ])),
    }
    lines
}

fn render_install_status_lines(status: &CatalogInstallStatus) -> Vec<Line<'static>> {
    let (label, color) = install_status_detail(status);
    vec![Line::from(vec![
        Span::styled("install status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(label, Style::default().fg(color)),
    ])]
}

fn install_status_list_label(status: &CatalogInstallStatus) -> (&'static str, Color) {
    match status {
        CatalogInstallStatus::Installing => ("installing", Color::Yellow),
        CatalogInstallStatus::Installed { .. } => ("installed", Color::Green),
        CatalogInstallStatus::AlreadyInstalled { .. } => ("already installed", Color::Green),
        CatalogInstallStatus::RuntimeMissing { .. }
        | CatalogInstallStatus::MissingEnv { .. }
        | CatalogInstallStatus::Failed { .. } => ("install failed", Color::Red),
    }
}

fn install_status_detail(status: &CatalogInstallStatus) -> (String, Color) {
    match status {
        CatalogInstallStatus::Installing => ("installing".to_string(), Color::Yellow),
        CatalogInstallStatus::Installed { server_id, started } => {
            let suffix = if *started { " (started)" } else { "" };
            (
                format!("installed as {}{suffix}", clip(server_id, 48)),
                Color::Green,
            )
        }
        CatalogInstallStatus::AlreadyInstalled { server_id } => (
            format!("already installed as {}", clip(server_id, 48)),
            Color::Green,
        ),
        CatalogInstallStatus::RuntimeMissing { missing_runtimes } => {
            let missing = if missing_runtimes.is_empty() {
                "unknown runtime".to_string()
            } else {
                clip(&missing_runtimes.join(", "), 56)
            };
            (format!("missing runtime {missing}"), Color::Red)
        }
        CatalogInstallStatus::MissingEnv { missing_env_keys } => {
            let missing = if missing_env_keys.is_empty() {
                "unknown key".to_string()
            } else {
                clip(&missing_env_keys.join(", "), 56)
            };
            (format!("missing env {missing}"), Color::Red)
        }
        CatalogInstallStatus::Failed { message } => {
            (format!("failed {}", clip(message, 60)), Color::Red)
        }
    }
}

fn render_requirement_lines(entry: &ServerEntry) -> Vec<Line<'static>> {
    let requirements = parse_requirements(entry);
    if requirements.is_empty() {
        return vec![Line::from(vec![
            Span::styled("requirements: ", Style::default().fg(Color::DarkGray)),
            Span::raw("none"),
        ])];
    }

    let mut lines = vec![Line::from(Span::styled(
        "requirements:",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    ))];
    for requirement in requirements.into_iter().take(4) {
        let mut label = requirement.kind.as_str().to_string();
        if let Some(version) = requirement.min_version {
            label.push_str(" >=");
            label.push_str(&version);
        }
        if let Some(hint) = requirement.install_hint {
            label.push_str(" - ");
            label.push_str(&clip(&hint, 52));
        }
        lines.push(Line::from(format!("  {label}")));
    }
    lines
}

fn render_config_lines(entry: &ServerEntry) -> Vec<Line<'static>> {
    let config = catalog_config_items(entry);
    if config.is_empty() {
        return vec![Line::from(vec![
            Span::styled("configuration: ", Style::default().fg(Color::DarkGray)),
            Span::raw("none"),
        ])];
    }

    let required_count = config.iter().filter(|item| item.required).count();
    let mut lines = vec![Line::from(vec![
        Span::styled(
            "configuration:",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " {} item{}",
            config.len(),
            if config.len() == 1 { "" } else { "s" }
        )),
        Span::styled(
            format!(" required:{required_count}"),
            Style::default().fg(if required_count == 0 {
                Color::DarkGray
            } else {
                Color::Yellow
            }),
        ),
    ])];

    for item in config.into_iter().take(5) {
        let mut meta = format!(
            "{} {}{}",
            item.kind,
            if item.required {
                "required"
            } else {
                "optional"
            },
            if item.secret { " secret" } else { "" }
        );
        if let Some(default) = item.default.as_ref().filter(|value| !value.is_empty()) {
            if item.secret {
                meta.push_str(" default:set");
            } else {
                meta.push_str(" default:");
                meta.push_str(&clip(default, 20));
            }
        }

        let description = if item.description.trim().is_empty() {
            String::new()
        } else {
            format!(" - {}", clip(&item.description, 42))
        };

        lines.push(Line::from(vec![
            Span::raw(format!("  {}  ", item.key)),
            Span::styled(meta, Style::default().fg(Color::Cyan)),
            Span::styled(description, Style::default().fg(Color::Gray)),
        ]));
    }
    lines
}

fn join_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn trust_color(value: &str) -> Color {
    match value {
        "verified" => Color::Green,
        "community" => Color::Yellow,
        _ => Color::Gray,
    }
}

fn render_sources(
    area: Rect,
    frame: &mut Frame,
    sources: &[CatalogSourceView],
    state: &mut ListState,
) {
    if sources.is_empty() {
        render_empty(area, frame, "No MCP catalog sources configured");
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
            let location = if source.url.is_empty() {
                "builtin".to_string()
            } else {
                source.url.clone()
            };
            ListItem::new(Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    source.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", source.kind),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(location, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_tools(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    let Some(server_id) = overlay.selected_server_id() else {
        render_empty(area, frame, "Select a runtime server before browsing tools");
        return;
    };
    let tools = overlay.current_tools();
    if tools.is_empty() {
        let label = if overlay.health.contains_key(server_id) {
            "No MCP tools discovered for selected server"
        } else {
            "Press [r] to health-check selected server and load tools"
        };
        render_empty(area, frame, label);
        return;
    }
    let items: Vec<ListItem> = tools
        .iter()
        .map(|tool| {
            let state_label = if tool.disabled {
                "disabled"
            } else {
                "enabled "
            };
            let state_color = if tool.disabled {
                Color::DarkGray
            } else {
                Color::Green
            };
            let description = tool
                .description
                .as_ref()
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(state_label, Style::default().fg(state_color)),
                Span::raw("  "),
                Span::styled(
                    tool.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(description, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_resources(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    if overlay.selected_server_id().is_none() {
        render_empty(
            area,
            frame,
            "Select a runtime server before browsing resources",
        );
        return;
    }
    let resources = overlay.current_resources();
    if resources.is_empty() {
        render_empty(
            area,
            frame,
            "Press [r] to list resources for selected server",
        );
        return;
    }
    let items: Vec<ListItem> = resources
        .iter()
        .map(|resource| {
            let mime = resource
                .mime_type
                .as_ref()
                .map(|value| format!("  {value}"))
                .unwrap_or_default();
            let preview = overlay
                .resource_previews
                .get(&resource_preview_key(&resource.server_id, &resource.uri))
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(
                    resource.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", clip(&resource.uri, 42)),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(mime, Style::default().fg(Color::DarkGray)),
                Span::styled(preview, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_prompts(area: Rect, frame: &mut Frame, overlay: &McpOverlay, state: &mut ListState) {
    if overlay.selected_server_id().is_none() {
        render_empty(
            area,
            frame,
            "Select a runtime server before browsing prompts",
        );
        return;
    }
    let prompts = overlay.current_prompts();
    if prompts.is_empty() {
        render_empty(area, frame, "Press [r] to list prompts for selected server");
        return;
    }
    let items: Vec<ListItem> = prompts
        .iter()
        .map(|prompt| {
            let description = prompt
                .description
                .as_ref()
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(
                    prompt.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  args:{}", prompt.argument_count),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(description, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

fn render_server_editor(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let list_height = area.height.saturating_sub(1);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );
    let items = SERVER_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.server_field_index {
                "> "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("{:<12}", server_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    server_field_value(&overlay.server_draft, *field),
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
            Span::styled("[s/e/h] transport  ", Style::default().fg(Color::Cyan)),
            Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
            Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
        ])),
        hint_area,
    );
}

fn render_source_editor(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let list_height = area.height.saturating_sub(1);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );
    let items = SOURCE_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.source_field_index {
                "> "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("{:<12}", source_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    source_field_value(&overlay.source_draft, *field),
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

fn render_catalog_install_config_editor(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    let draft = &overlay.catalog_install_draft;

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "Install configuration: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(clip(&draft.display_name, 64)),
            ]),
            Line::from(Span::styled(
                "Fill required MCP catalog values before installing",
                Style::default().fg(Color::DarkGray),
            )),
        ]),
        chunks[0],
    );

    let items = draft
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let marker = if index == overlay.catalog_install_field_index {
                "> "
            } else {
                "  "
            };
            let value = draft.values.get(&item.key).cloned().unwrap_or_default();
            let missing = item.required && value.trim().is_empty();
            let value_label = if item.secret && !value.is_empty() {
                "*".repeat(value.chars().count().min(12))
            } else {
                value
            };
            let required = if item.required {
                "required"
            } else {
                "optional"
            };
            let required_color = if missing {
                Color::Yellow
            } else {
                Color::DarkGray
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("{:<18}", item.key),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<12}", item.kind),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{required:<8} "),
                    Style::default().fg(required_color),
                ),
                Span::styled(
                    if item.secret { "secret " } else { "       " },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(value_label),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), chunks[1]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "[Tab/Up/Down] field  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("[Enter] install  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
        ])),
        chunks[2],
    );
}

fn server_field_label(field: ServerEditorField) -> &'static str {
    match field {
        ServerEditorField::Name => "Name",
        ServerEditorField::Transport => "Transport",
        ServerEditorField::CommandOrUrl => "Command/URL",
        ServerEditorField::Args => "Args",
        ServerEditorField::Description => "Description",
        ServerEditorField::Enabled => "Enabled",
    }
}

fn server_field_value(draft: &ServerDraft, field: ServerEditorField) -> String {
    match field {
        ServerEditorField::Name => draft.name.clone(),
        ServerEditorField::Transport => server_transport_label(draft.transport).to_string(),
        ServerEditorField::CommandOrUrl if draft.transport == ServerTransportDraft::Stdio => {
            draft.command.clone()
        }
        ServerEditorField::CommandOrUrl => draft.url.clone(),
        ServerEditorField::Args => {
            if draft.transport == ServerTransportDraft::Stdio {
                draft.args_text.clone()
            } else {
                "n/a".to_string()
            }
        }
        ServerEditorField::Description => draft.description.clone(),
        ServerEditorField::Enabled => draft.enabled.to_string(),
    }
}

fn source_field_label(field: SourceEditorField) -> &'static str {
    match field {
        SourceEditorField::Id => "ID",
        SourceEditorField::DisplayName => "Name",
        SourceEditorField::Url => "URL",
        SourceEditorField::ApiKeyEnv => "API key env",
        SourceEditorField::Priority => "Priority",
        SourceEditorField::DefaultTrust => "Trust",
        SourceEditorField::Enabled => "Enabled",
    }
}

fn source_field_value(draft: &SourceDraft, field: SourceEditorField) -> String {
    match field {
        SourceEditorField::Id => draft.id.clone(),
        SourceEditorField::DisplayName => draft.display_name.clone(),
        SourceEditorField::Url => draft.url.clone(),
        SourceEditorField::ApiKeyEnv => draft.api_key_env.clone(),
        SourceEditorField::Priority => draft.priority.clone(),
        SourceEditorField::DefaultTrust => draft.default_trust.clone(),
        SourceEditorField::Enabled => draft.enabled.to_string(),
    }
}

fn server_transport_label(transport: ServerTransportDraft) -> &'static str {
    match transport {
        ServerTransportDraft::Stdio => "stdio",
        ServerTransportDraft::Sse => "sse",
        ServerTransportDraft::StreamableHttp => "streamable_http",
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

fn render_list(area: Rect, frame: &mut Frame, items: Vec<ListItem>, state: &mut ListState) {
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
