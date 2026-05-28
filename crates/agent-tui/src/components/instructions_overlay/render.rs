//! Rendering functions for the instructions overlay.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::state::{InstructionsOverlay, InstructionsTab};

pub fn render_instructions_overlay(area: Rect, frame: &mut Frame, overlay: &InstructionsOverlay) {
    let modal_width = 84.min(area.width.saturating_sub(4));
    let modal_height = 24.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Instructions ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
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

    let tabs = Line::from(vec![
        tab_span("System", overlay.tab == InstructionsTab::System),
        Span::raw("  "),
        tab_span("User", overlay.tab == InstructionsTab::User),
        Span::raw("  "),
        tab_span("Project", overlay.tab == InstructionsTab::Project),
        Span::raw("  "),
        tab_span("Effective", overlay.tab == InstructionsTab::Effective),
    ]);
    frame.render_widget(Paragraph::new(tabs), chunks[0]);

    let (title, content, editable) = match overlay.tab {
        InstructionsTab::System => ("System prompt", overlay.system_text(), false),
        InstructionsTab::User => ("User instructions", overlay.user_text(), true),
        InstructionsTab::Project => ("Project instructions", overlay.project_text(), true),
        InstructionsTab::Effective => ("Effective preview", "", false),
    };
    let content = if overlay.tab == InstructionsTab::Effective {
        overlay.effective_text()
    } else {
        content.to_string()
    };
    let empty_hint = if editable {
        "<empty - save an empty value to clear this scope>"
    } else {
        "<empty>"
    };
    let shown = if content.trim().is_empty() {
        empty_hint.to_string()
    } else {
        content
    };
    let mut body_lines = vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(if editable {
                    Color::Yellow
                } else {
                    Color::Magenta
                })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    body_lines.extend(shown.lines().map(Line::from));
    let body = Text::from(body_lines);
    let body_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if editable {
            Color::Yellow
        } else {
            Color::DarkGray
        }));
    frame.render_widget(
        Paragraph::new(body)
            .block(body_block)
            .wrap(Wrap { trim: false }),
        chunks[1],
    );

    let hints = Line::from(vec![
        Span::styled("[Tab] scope  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[F2] save  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Enter] newline  ", Style::default().fg(Color::Cyan)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[2]);
}

fn tab_span(label: &'static str, active: bool) -> Span<'static> {
    if active {
        Span::styled(
            format!("[{label}]"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!(" {label} "), Style::default().fg(Color::DarkGray))
    }
}
