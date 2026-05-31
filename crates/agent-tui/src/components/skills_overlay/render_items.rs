//! Per-tab and per-item rendering helpers for the skills overlay.
//!
//! Each function draws one tab's content (discovered, installed,
//! catalog, catalog detail, or sources). Extracted from
//! [`super::render`] to keep the orchestrator focused on layout and
//! routing while these helpers own the visual representation of each
//! tab's list items.

use agent_core::facade::{
    RemoteSkillSearchResult, SkillCatalogEntry, SkillInstallTarget, SkillSettingsView,
    SkillSourceView,
};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{theme, SkillEntry};

use super::editor::{SkillSourceDraft, SkillSourceEditorField};

pub fn target_label(target: SkillInstallTarget) -> &'static str {
    match target {
        SkillInstallTarget::User => "user",
        SkillInstallTarget::Project => "project",
    }
}

pub fn render_discovered(
    area: Rect,
    frame: &mut Frame,
    skills: &[SkillEntry],
    state: &mut ListState,
) {
    if skills.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No skills discovered",
            theme::muted(),
        )));
        frame.render_widget(empty, area);
    } else {
        let items: Vec<ListItem> = skills
            .iter()
            .map(|s| {
                let (marker, marker_color) = if s.active {
                    ("● active ", theme::SUCCESS)
                } else {
                    ("○        ", theme::MUTED)
                };
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(marker_color)),
                    Span::styled(s.id.clone(), Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("  "),
                    Span::styled(s.description.clone(), theme::muted()),
                    Span::styled(
                        format!("  [{} / {}]", s.source, s.activation_mode),
                        theme::muted(),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).highlight_style(theme::selected());
        frame.render_stateful_widget(list, area, state);
    }
}

pub fn render_installed(
    area: Rect,
    frame: &mut Frame,
    installed: &[SkillSettingsView],
    state: &mut ListState,
) {
    if installed.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No skill settings installed",
                theme::muted(),
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
                theme::SUCCESS
            } else {
                theme::MUTED
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
                Span::styled(format!("  v{version}"), theme::muted()),
                Span::raw("  "),
                Span::styled(skill.description.clone(), theme::muted()),
                Span::styled(
                    format!(
                        "  [{:?} / {:?} / {:?}{effective}{valid}]",
                        skill.scope, skill.install_source, skill.update_state
                    ),
                    theme::muted(),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(theme::selected());
    frame.render_stateful_widget(list, area, state);
}

pub fn render_catalog(
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
                theme::muted(),
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
                    Style::default().fg(theme::ACCENT),
                ),
                Span::raw("  "),
                Span::styled(entry.description.clone(), theme::muted()),
                Span::styled(
                    format!(
                        "  installs:{installs} stars:{stars} -> {}",
                        target_label(install_target)
                    ),
                    theme::muted(),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(theme::selected());
    frame.render_stateful_widget(list, area, state);
}

pub fn render_catalog_detail(
    area: Rect,
    frame: &mut Frame,
    entry: Option<&SkillCatalogEntry>,
    install_target: SkillInstallTarget,
) {
    let Some(entry) = entry else {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No catalog skill selected",
                theme::muted(),
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
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(entry.description.clone()),
        Line::from(""),
        Line::from(vec![
            Span::styled("Catalog: ", theme::muted()),
            Span::raw(entry.source.clone()),
        ]),
        Line::from(vec![
            Span::styled("Source: ", theme::muted()),
            Span::raw(source_url.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Package: ", theme::muted()),
            Span::raw(entry.package.clone()),
        ]),
        Line::from(vec![
            Span::styled("Download: ", theme::muted()),
            Span::raw(package_url.to_string()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Installs: ", theme::muted()),
            Span::raw(installs),
            Span::styled("  Stars: ", theme::muted()),
            Span::raw(stars),
            Span::styled("  Security: ", theme::muted()),
            Span::raw(security),
            Span::styled("  Rating: ", theme::muted()),
            Span::raw(rating),
        ]),
        Line::from(vec![
            Span::styled("Target: ", theme::muted()),
            Span::styled(
                target_label(install_target),
                Style::default()
                    .fg(theme::WARNING)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

pub fn render_search_results(
    area: Rect,
    frame: &mut Frame,
    results: &[RemoteSkillSearchResult],
    state: &mut ListState,
    install_target: SkillInstallTarget,
) {
    if results.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No search results — press / to search",
                theme::muted(),
            ))),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = results
        .iter()
        .map(|result| {
            let installs = result
                .install_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".to_string());
            let line = Line::from(vec![
                Span::styled(
                    result.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(clip(&result.description, 40), theme::muted()),
                Span::styled(
                    format!("  installs:{installs} -> {}", target_label(install_target)),
                    theme::muted(),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(theme::selected());
    frame.render_stateful_widget(list, area, state);
}

pub fn render_sources(
    area: Rect,
    frame: &mut Frame,
    sources: &[SkillSourceView],
    state: &mut ListState,
) {
    if sources.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No skill sources configured",
                theme::muted(),
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
                theme::SUCCESS
            } else {
                theme::MUTED
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
                    theme::muted(),
                ),
                Span::raw("  "),
                Span::styled(source.url.clone(), theme::muted()),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(theme::selected());
    frame.render_stateful_widget(list, area, state);
}

pub fn skill_source_field_label(field: SkillSourceEditorField) -> &'static str {
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

pub fn skill_source_field_value(draft: &SkillSourceDraft, field: SkillSourceEditorField) -> String {
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

pub fn clip(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let clipped: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{clipped}...")
    } else {
        clipped
    }
}
