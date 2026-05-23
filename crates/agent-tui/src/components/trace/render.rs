//! Rendering and tool-trace extraction for the trace panel.
//!
//! Tool traces and memory proposals are reconstructed from a stream of domain
//! events; the L1 list, the task graph, and the placeholder are then drawn
//! against a [`ratatui::Frame`].

use agent_core::events::EventPayload;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use std::collections::BTreeSet;

use super::task_tree::{
    flatten_task_tree_with_collapsed, task_row_label_with_collapsed, TaskTreeNode,
};
use super::{RightPanelTab, TraceKind, TraceStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    pub tool_id: String,
    pub status: TraceStatus,
    pub kind: TraceKind,
    pub duration_ms: Option<u64>,
    pub args_preview: Option<String>,
    pub output_preview: Option<String>,
}

pub fn extract_tool_traces(events: &[agent_core::DomainEvent]) -> Vec<TraceEntry> {
    let mut traces = Vec::new();
    for event in events {
        match &event.payload {
            EventPayload::ToolInvocationStarted { tool_id, .. } => {
                traces.push(TraceEntry {
                    tool_id: tool_id.clone(),
                    status: TraceStatus::Running,
                    kind: TraceKind::Tool,
                    duration_ms: None,
                    args_preview: None,
                    output_preview: None,
                });
            }
            EventPayload::ToolInvocationCompleted {
                tool_id,
                duration_ms,
                output_preview,
                ..
            } => {
                if let Some(entry) = traces
                    .iter_mut()
                    .rev()
                    .find(|t| t.tool_id == *tool_id && t.status == TraceStatus::Running)
                {
                    entry.status = TraceStatus::Success;
                    entry.duration_ms = Some(*duration_ms);
                    entry.output_preview = Some(output_preview.clone());
                }
            }
            EventPayload::ToolInvocationFailed { tool_id, error, .. } => {
                if let Some(entry) = traces
                    .iter_mut()
                    .rev()
                    .find(|t| t.tool_id == *tool_id && t.status == TraceStatus::Running)
                {
                    entry.status = TraceStatus::Failed;
                    entry.output_preview = Some(error.clone());
                }
            }
            EventPayload::MemoryProposed {
                memory_id: _,
                scope,
                key,
                content,
            } => {
                let label = match key {
                    Some(k) => format!("memory[{scope}:{k}]"),
                    None => format!("memory[{scope}]"),
                };
                traces.push(TraceEntry {
                    tool_id: label,
                    status: TraceStatus::Pending,
                    kind: TraceKind::Memory,
                    duration_ms: None,
                    args_preview: Some(content.clone()),
                    output_preview: None,
                });
            }
            EventPayload::MemoryAccepted { memory_id: _, .. } => {
                if let Some(entry) = traces.iter_mut().rev().find(|t| {
                    matches!(t.kind, TraceKind::Memory) && t.status == TraceStatus::Pending
                }) {
                    entry.status = TraceStatus::Success;
                    entry.output_preview = Some("accepted".to_string());
                }
            }
            EventPayload::MemoryRejected {
                memory_id: _,
                reason,
                ..
            } => {
                if let Some(entry) = traces.iter_mut().rev().find(|t| {
                    matches!(t.kind, TraceKind::Memory) && t.status == TraceStatus::Pending
                }) {
                    entry.status = TraceStatus::Failed;
                    entry.output_preview = Some(reason.clone());
                }
            }
            _ => {}
        }
    }
    traces
}

pub fn render_trace_l1(area: Rect, frame: &mut Frame, traces: &[TraceEntry], focused: bool) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = traces
        .iter()
        .map(|entry| {
            let status_color = match entry.status {
                TraceStatus::Running => Color::Yellow,
                TraceStatus::Success => Color::Green,
                TraceStatus::Failed => Color::Red,
                TraceStatus::Pending => Color::Magenta,
            };
            let icon = match entry.kind {
                TraceKind::Tool => "▶ ",
                TraceKind::Memory => "🧠",
            };
            let duration = entry
                .duration_ms
                .map(|d| format!(" {:.1}s", d as f64 / 1000.0))
                .unwrap_or_default();
            let line = Line::from(vec![
                Span::styled(icon, Style::default()),
                Span::styled(&entry.tool_id, Style::default()),
                Span::styled(
                    format!(" {}{}", entry.status, duration),
                    Style::default().fg(status_color),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::LEFT)
            .title(right_panel_title(RightPanelTab::Trace))
            .border_style(border_style),
    );
    frame.render_widget(list, area);
}

pub fn render_task_graph_with_collapsed(
    area: Rect,
    frame: &mut Frame,
    tasks: &[TaskTreeNode],
    focused: bool,
    selected_index: usize,
    collapsed_task_ids: &BTreeSet<String>,
) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let rows = flatten_task_tree_with_collapsed(tasks, collapsed_task_ids);
    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let task = &row.node;
            let status_color = match task.status {
                TraceStatus::Running => Color::Yellow,
                TraceStatus::Success => Color::Green,
                TraceStatus::Failed => Color::Red,
                TraceStatus::Pending => Color::Magenta,
            };
            let selected = focused && index == selected_index;
            let collapsed = collapsed_task_ids.contains(&task.id);
            let line = Line::from(Span::styled(
                task_row_label_with_collapsed(row, selected, collapsed),
                Style::default().fg(status_color),
            ));
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::LEFT)
            .title(right_panel_title(RightPanelTab::Tasks))
            .border_style(border_style),
    );
    frame.render_widget(list, area);
}

pub fn render_task_graph_placeholder(area: Rect, frame: &mut Frame, focused: bool) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let paragraph = Paragraph::new("No tasks yet\n\nUse F5 to cycle trace density.").block(
        Block::default()
            .borders(Borders::LEFT)
            .title(right_panel_title(RightPanelTab::Tasks))
            .border_style(border_style),
    );
    frame.render_widget(paragraph, area);
}

pub(super) fn right_panel_title(active: RightPanelTab) -> String {
    [
        RightPanelTab::Trace,
        RightPanelTab::Tasks,
        RightPanelTab::Memory,
    ]
    .into_iter()
    .map(|tab| {
        if tab == active {
            format!("[{}]", tab.label())
        } else {
            tab.label().to_string()
        }
    })
    .collect::<Vec<_>>()
    .join(" | ")
}
