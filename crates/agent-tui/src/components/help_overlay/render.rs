//! Rendering functions for the help overlay.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{theme, FocusTarget};

use super::shortcuts::{
    common_commands, context_shortcuts, current_label, current_label_prefix, global_shortcuts,
    is_overlay_focus,
};
use super::state::HelpOverlay;
use super::types::Shortcut;

pub fn render_help_overlay(area: Rect, frame: &mut Frame, overlay: &HelpOverlay) {
    let modal_width = 84.min(area.width.saturating_sub(4));
    let modal_height = 26.min(area.height.saturating_sub(2));
    if modal_width == 0 || modal_height == 0 {
        return;
    }
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Help / Keybindings ", theme::title()))
        .border_style(theme::border(true));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let paragraph = Paragraph::new(help_lines(overlay.snapshot.focus))
        .wrap(Wrap { trim: true })
        .style(theme::muted());
    frame.render_widget(paragraph, inner);
}

fn help_lines(focus: FocusTarget) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            current_label_prefix(focus),
            Style::default()
                .fg(theme::WARNING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            current_label(focus),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::default());
    lines.push(section_line("Global shortcuts"));
    lines.extend(shortcut_lines(global_shortcuts()));
    lines.push(Line::default());
    lines.push(section_line(if is_overlay_focus(focus) {
        "Current overlay shortcuts"
    } else {
        "Current focus shortcuts"
    }));
    lines.extend(shortcut_lines(context_shortcuts(focus)));
    lines.push(Line::default());
    lines.push(section_line("Common commands"));
    lines.extend(shortcut_lines(common_commands()));
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("F1", key_style()),
        Span::raw(" or "),
        Span::styled("Esc", key_style()),
        Span::raw(" closes help"),
    ]));
    lines
}

fn section_line(label: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        label,
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    ))
}

fn shortcut_lines(shortcuts: &[Shortcut]) -> Vec<Line<'static>> {
    shortcuts
        .chunks(2)
        .map(|chunk| {
            let mut spans = Vec::new();
            for (index, shortcut) in chunk.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::raw("    "));
                }
                spans.push(Span::styled(shortcut.key, key_style()));
                spans.push(Span::raw(" "));
                spans.push(Span::raw(shortcut.label));
            }
            Line::from(spans)
        })
        .collect()
}

fn key_style() -> Style {
    Style::default()
        .fg(theme::WARNING)
        .add_modifier(Modifier::BOLD)
}
