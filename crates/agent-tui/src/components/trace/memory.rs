//! Memory tab — types for the saved-memory list and its renderer.
//!
//! Storage and search lifecycle live on [`crate::components::trace::TracePanel`].
//! This module owns the row types, the scope filter, and the list/empty-state
//! rendering surface.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use super::render::right_panel_title;
use super::{RightPanelTab, TracePanel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScopeFilter {
    All,
    Session,
    User,
    Workspace,
}

impl MemoryScopeFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Session,
            Self::Session => Self::User,
            Self::User => Self::Workspace,
            Self::Workspace => Self::All,
        }
    }

    pub fn scope(self) -> Option<agent_memory::MemoryScope> {
        match self {
            Self::All => None,
            Self::Session => Some(agent_memory::MemoryScope::Session),
            Self::User => Some(agent_memory::MemoryScope::User),
            Self::Workspace => Some(agent_memory::MemoryScope::Workspace),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Session => "session",
            Self::User => "user",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRow {
    pub id: String,
    pub scope: String,
    pub key: Option<String>,
    pub content: String,
    pub accepted: bool,
}

impl MemoryRow {
    pub fn new(id: String, scope: String, key: Option<String>, content: String) -> Self {
        Self {
            id,
            scope,
            key,
            content,
            accepted: true,
        }
    }

    pub fn label(&self) -> String {
        let pending = if self.accepted { "" } else { " pending" };
        match &self.key {
            Some(key) => format!("[{}]{} {}: {}", self.scope, pending, key, self.content),
            None => format!("[{}]{} {}", self.scope, pending, self.content),
        }
    }
}

impl From<agent_memory::MemoryEntry> for MemoryRow {
    fn from(entry: agent_memory::MemoryEntry) -> Self {
        let scope = match entry.scope {
            agent_memory::MemoryScope::User => "user",
            agent_memory::MemoryScope::Workspace => "workspace",
            agent_memory::MemoryScope::Session => "session",
        };
        let mut row = Self::new(entry.id, scope.into(), entry.key, entry.content);
        row.accepted = entry.accepted;
        row
    }
}

pub fn render_memory_browser(area: Rect, frame: &mut Frame, panel: &TracePanel) {
    let memories = &panel.memory_rows;
    let focused = crate::components::Component::focused(panel);
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title = memory_browser_title(panel);

    if memories.is_empty() {
        let paragraph = Paragraph::new(memory_browser_empty_text(panel)).block(
            Block::default()
                .borders(Borders::LEFT)
                .title(title)
                .border_style(border_style),
        );
        frame.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = memories
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let cursor = if focused && index == panel.selected_memory_index {
                ">"
            } else {
                " "
            };
            let delete_hint = if panel.pending_delete_memory_id.as_deref() == Some(row.id.as_str())
            {
                " delete? y/N"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(cursor, Style::default().fg(Color::Cyan)),
                Span::raw(row.label()),
                Span::styled(delete_hint, Style::default().fg(Color::Red)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::LEFT)
            .title(title)
            .border_style(border_style),
    );
    frame.render_widget(list, area);
}

fn memory_browser_title(panel: &TracePanel) -> String {
    let search = if panel.memory_search_query.is_empty() {
        String::from("-")
    } else if panel.memory_search_active {
        format!("{}_", panel.memory_search_query)
    } else {
        panel.memory_search_query.clone()
    };
    format!(
        "{} · scope:{} · search:{}",
        right_panel_title(RightPanelTab::Memory),
        panel.memory_scope_filter.label(),
        search
    )
}

fn memory_browser_empty_text(panel: &TracePanel) -> String {
    if panel.memory_search_query.is_empty() && panel.memory_scope_filter == MemoryScopeFilter::All {
        "No saved memories\n\n[s] scope  [/] search  [r] refresh".to_string()
    } else {
        format!(
            "No saved memories for scope:{} search:{}\n\n[s] scope  [/] search  [r] refresh",
            panel.memory_scope_filter.label(),
            if panel.memory_search_query.is_empty() {
                "-"
            } else {
                panel.memory_search_query.as_str()
            }
        )
    }
}
