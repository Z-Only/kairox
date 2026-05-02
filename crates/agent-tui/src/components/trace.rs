use crate::components::{Command, Component, CrossPanelEffect, EventContext};
use crate::keybindings::TraceDensity;
use agent_core::events::EventPayload;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

#[allow(dead_code)]
pub struct TracePanel {
    focused: bool,
    pub density: TraceDensity,
    pub expanded_index: Option<usize>,
    pub scroll_offset: usize,
}

impl Default for TracePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl TracePanel {
    pub fn new() -> Self {
        Self {
            focused: false,
            density: TraceDensity::default(),
            expanded_index: None,
            scroll_offset: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    pub tool_id: String,
    pub status: TraceStatus,
    pub kind: TraceKind,
    pub duration_ms: Option<u64>,
    pub args_preview: Option<String>,
    pub output_preview: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceStatus {
    Running,
    Success,
    Failed,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceKind {
    Tool,
    Memory,
}

impl std::fmt::Display for TraceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "⏳"),
            Self::Success => write!(f, "✓"),
            Self::Failed => write!(f, "✕"),
            Self::Pending => write!(f, "?"),
        }
    }
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
            .title(" Trace ")
            .border_style(border_style),
    );
    frame.render_widget(list, area);
}

impl Component for TracePanel {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        _event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        (Vec::new(), Vec::new())
    }

    fn handle_effect(&mut self, _effect: &CrossPanelEffect) {}

    fn render(&self, area: Rect, frame: &mut Frame) {
        let _ = (area, frame);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AgentId, DomainEvent, PrivacyClassification, SessionId, WorkspaceId};

    fn make_event(payload: EventPayload) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
    }

    #[test]
    fn extract_tool_traces_from_events() {
        let events = vec![
            make_event(EventPayload::ToolInvocationStarted {
                invocation_id: "inv1".into(),
                tool_id: "shell.exec".into(),
            }),
            make_event(EventPayload::ToolInvocationCompleted {
                invocation_id: "inv1".into(),
                tool_id: "shell.exec".into(),
                output_preview: "ok".into(),
                exit_code: None,
                duration_ms: 1200,
                truncated: false,
            }),
            make_event(EventPayload::ToolInvocationStarted {
                invocation_id: "inv2".into(),
                tool_id: "patch.apply".into(),
            }),
        ];

        let traces = extract_tool_traces(&events);
        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].tool_id, "shell.exec");
        assert_eq!(traces[0].status, TraceStatus::Success);
        assert_eq!(traces[0].duration_ms, Some(1200));
        assert_eq!(traces[1].tool_id, "patch.apply");
        assert_eq!(traces[1].status, TraceStatus::Running);
        assert!(traces[1].duration_ms.is_none());
    }

    #[test]
    fn extract_tool_traces_handles_failure() {
        let events = vec![
            make_event(EventPayload::ToolInvocationStarted {
                invocation_id: "inv1".into(),
                tool_id: "shell.exec".into(),
            }),
            make_event(EventPayload::ToolInvocationFailed {
                invocation_id: "inv1".into(),
                tool_id: "shell.exec".into(),
                error: "permission denied".into(),
            }),
        ];

        let traces = extract_tool_traces(&events);
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].status, TraceStatus::Failed);
        assert_eq!(traces[0].output_preview, Some("permission denied".into()));
    }

    #[test]
    fn trace_density_cycles() {
        assert_eq!(TraceDensity::Summary.next(), TraceDensity::Expanded);
        assert_eq!(TraceDensity::Expanded.next(), TraceDensity::FullEventStream);
        assert_eq!(TraceDensity::FullEventStream.next(), TraceDensity::Summary);
    }

    #[test]
    fn trace_status_display() {
        assert_eq!(format!("{}", TraceStatus::Running), "⏳");
        assert_eq!(format!("{}", TraceStatus::Success), "✓");
        assert_eq!(format!("{}", TraceStatus::Failed), "✕");
    }
}
