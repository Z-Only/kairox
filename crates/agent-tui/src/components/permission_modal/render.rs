use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::RiskLevel;

use super::state::PermissionModal;

pub fn render_permission_modal(area: Rect, frame: &mut Frame, modal: &PermissionModal) {
    let Some(request) = modal.request.as_ref() else {
        return;
    };

    let modal_width = 76.min(area.width.saturating_sub(4));
    let modal_height = 19.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let (title, risk_label, risk_color, warning) = match &request.risk_level {
        RiskLevel::Destructive => (
            "⛔ Destructive Operation",
            "Destructive",
            Color::Red,
            "This operation cannot be undone.",
        ),
        RiskLevel::Write => (
            "🧠 Memory Write",
            "Write",
            Color::Yellow,
            "This will save a memory entry.",
        ),
        RiskLevel::McpTool { server_id: _ } => (
            "🔌 MCP Tool",
            "MCP",
            Color::Magenta,
            "",
            // Use server_id below for the tool label
        ),
    };

    // For MCP tools, show [MCP] server/tool in the tool label
    let tool_label = match &request.risk_level {
        RiskLevel::McpTool { server_id } => {
            format!("[MCP] {}/{}", server_id, request.tool_id)
        }
        _ => request.tool_id.clone(),
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "Permission Center",
                Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} pending", modal.pending_requests.len().max(1)),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(title, Style::default().fg(risk_color))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tool: ", Style::default().fg(Color::Gray)),
            Span::raw(&tool_label),
        ]),
        Line::from(vec![
            Span::styled("Command: ", Style::default().fg(Color::Gray)),
            Span::raw(&request.tool_preview),
        ]),
        Line::from(vec![
            Span::styled("Risk: ", Style::default().fg(Color::Gray)),
            Span::styled(risk_label, Style::default().fg(risk_color)),
        ]),
    ];
    if !warning.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(warning));
    }

    if modal.pending_requests.len() > 1 {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Pending queue",
            Style::default().fg(Color::Cyan),
        )));
        for (index, pending) in modal.pending_requests.iter().take(4).enumerate() {
            let marker = if index == 0 { ">" } else { " " };
            let label = match &pending.risk_level {
                RiskLevel::McpTool { server_id } => {
                    format!("[MCP] {server_id}/{}", pending.tool_id)
                }
                RiskLevel::Destructive => format!("[destructive] {}", pending.tool_id),
                RiskLevel::Write => format!("[write] {}", pending.tool_id),
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(risk_color)),
                Span::raw(format!(" {} ", index + 1)),
                Span::raw(label),
            ]));
        }
    }

    if !modal.history.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recent",
            Style::default().fg(Color::DarkGray),
        )));
        for entry in modal.history.iter().rev().take(2) {
            let status = if entry.approved { "allowed" } else { "denied" };
            lines.push(Line::from(vec![
                Span::styled(status, Style::default().fg(Color::DarkGray)),
                Span::raw(format!(" {}", entry.request.tool_id)),
            ]));
        }
    }
    lines.push(Line::from(""));

    // Key hints — add (T) Trust option for MCP tools
    let mut key_hints = vec![
        Span::styled("[Y] Allow once  ", Style::default().fg(Color::Yellow)),
        Span::styled("[N] Deny  ", Style::default().fg(Color::Gray)),
    ];
    if matches!(request.risk_level, RiskLevel::McpTool { .. }) {
        key_hints.push(Span::styled(
            "[T] Trust server  ",
            Style::default().fg(Color::Magenta),
        ));
    }
    key_hints.push(Span::styled(
        "[Esc] Cancel",
        Style::default().fg(Color::DarkGray),
    ));
    lines.push(Line::from(key_hints));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(risk_color)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, modal_area);
}
