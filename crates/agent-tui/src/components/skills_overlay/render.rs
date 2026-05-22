use agent_core::facade::{
    SkillCatalogEntry, SkillInstallTarget, SkillSettingsView, SkillSourceView,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::SkillEntry;

use super::editor::{SkillSourceDraft, SkillSourceEditorField, SKILL_SOURCE_EDITOR_FIELDS};
use super::state::{SkillOverlayMode, SkillTab, SkillsOverlay};

struct SkillsOverlayRenderState<'a> {
    discovered: &'a mut ListState,
    installed: &'a mut ListState,
    catalog: &'a mut ListState,
    sources: &'a mut ListState,
}

pub(super) fn render_skills_overlay(area: Rect, frame: &mut Frame, overlay: &SkillsOverlay) {
    if !overlay.visible {
        return;
    }

    let mut discovered_state = overlay.discovered_state;
    let mut installed_state = overlay.installed_state;
    let mut catalog_state = overlay.catalog_state;
    let mut sources_state = overlay.sources_state;
    let mut render_state = SkillsOverlayRenderState {
        discovered: &mut discovered_state,
        installed: &mut installed_state,
        catalog: &mut catalog_state,
        sources: &mut sources_state,
    };
    render_skills_overlay_content(area, frame, overlay, &mut render_state);
}

fn render_skills_overlay_content(
    area: Rect,
    frame: &mut Frame,
    overlay: &SkillsOverlay,
    state: &mut SkillsOverlayRenderState<'_>,
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
            render_discovered(chunks[1], frame, &overlay.discovered, state.discovered)
        }
        SkillTab::Installed => {
            render_installed(chunks[1], frame, &overlay.installed, state.installed)
        }
        SkillTab::Catalog if overlay.mode == SkillOverlayMode::CatalogDetail => {
            let selected = state
                .catalog
                .selected()
                .and_then(|index| overlay.catalog.get(index));
            render_catalog_detail(chunks[1], frame, selected, overlay.install_target);
        }
        SkillTab::Catalog => {
            render_catalog(
                chunks[1],
                frame,
                &overlay.catalog,
                state.catalog,
                overlay.install_target,
            );
        }
        SkillTab::Sources => render_sources(chunks[1], frame, &overlay.sources, state.sources),
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

fn clip(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let clipped: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{clipped}...")
    } else {
        clipped
    }
}
