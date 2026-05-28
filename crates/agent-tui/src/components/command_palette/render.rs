//! Command palette rendering — modal layout, filter input, list, and hints.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::registry::PaletteEntry;
use super::state::CommandPalette;

pub fn render_command_palette(
    area: Rect,
    frame: &mut Frame,
    palette: &CommandPalette,
    entries: &[PaletteEntry],
    list_state: &mut ListState,
) {
    let modal_width = 72.min(area.width.saturating_sub(4));
    let modal_height = 18.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " ⌘ Command Palette ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if inner.height < 4 {
        return;
    }

    let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let list_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let hint_area = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width,
        1,
    );

    let filter_line = Line::from(vec![
        Span::styled(
            "> ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(palette.filter().to_string()),
        Span::styled("▌", Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(filter_line), filter_area);

    if entries.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No matching commands",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, list_area);
    } else {
        let items: Vec<ListItem> = entries
            .iter()
            .map(|e| {
                let line = Line::from(vec![
                    Span::styled(
                        e.label.as_ref(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(e.description.as_ref(), Style::default().fg(Color::Gray)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_stateful_widget(list, list_area, list_state);
    }

    let hints = Line::from(vec![
        Span::styled("[↑/↓] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter] run  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] close  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[type] filter", Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}
