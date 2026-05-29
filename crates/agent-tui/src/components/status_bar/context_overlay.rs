//! Context-details popup overlay rendered above the status bar when toggled.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{theme, StatusInfo};

use super::context_line::render_context_details_lines;

pub(super) fn render_context_details_overlay(
    status_area: Rect,
    frame: &mut Frame,
    info: &StatusInfo,
) {
    let detail_lines = render_context_details_lines(info);
    let Some(area) = context_details_overlay_area(status_area, detail_lines.len()) else {
        return;
    };

    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Context Details ")
        .borders(Borders::ALL)
        .border_style(theme::border(true));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = detail_lines.into_iter().map(Line::from).collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn context_details_overlay_area(status_area: Rect, line_count: usize) -> Option<Rect> {
    if status_area.width == 0 || status_area.y < 3 {
        return None;
    }

    let desired_height = (line_count as u16).saturating_add(2);
    let height = desired_height.min(status_area.y).max(3);
    let width = status_area.width.min(78);
    let x = status_area.x + status_area.width.saturating_sub(width);
    let y = status_area.y.saturating_sub(height);

    Some(Rect {
        x,
        y,
        width,
        height,
    })
}
