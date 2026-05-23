use agent_core::facade::ServerEntry;
use agent_mcp::catalog::InstallSpec;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::super::editor::{catalog_config_items, parse_install_spec, parse_requirements};
use super::super::state::{CatalogInstallStatus, McpOverlay};
use super::{clip, render_empty, render_list};

pub(super) fn render_catalog(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
    state: &mut ListState,
) {
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
