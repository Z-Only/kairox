//! Trace panel — right-side overlay tab that shows tool traces, the task
//! graph, and the memory browser.
//!
//! State is owned by [`TracePanel`]; the App layer mutates the panel based on
//! incoming events and effects, and dispatches commands derived from selection.

mod memory;
mod render;
mod task_tree;

#[cfg(test)]
mod tests;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};
use crate::keybindings::TraceDensity;
use agent_core::facade::{TaskGraphSnapshot, TaskSnapshot};
use agent_core::TaskState;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;
use std::collections::BTreeSet;

pub use memory::{render_memory_browser, MemoryRow, MemoryScopeFilter};
#[allow(unused_imports)]
pub use render::{
    extract_tool_traces, render_task_graph_placeholder, render_task_graph_with_collapsed,
    render_trace_l1, TraceEntry,
};
#[allow(unused_imports)]
pub use task_tree::{
    build_task_tree_from_snapshot, extract_task_traces, flatten_task_tree_with_collapsed,
    TaskListRow, TaskTreeNode,
};

#[allow(dead_code)]
pub struct TracePanel {
    focused: bool,
    pub active_tab: RightPanelTab,
    pub density: TraceDensity,
    pub expanded_index: Option<usize>,
    pub scroll_offset: usize,
    pub selected_task_index: usize,
    pub selected_memory_index: usize,
    pub memory_scope_filter: MemoryScopeFilter,
    pub memory_search_query: String,
    pub memory_search_active: bool,
    pub memory_rows: Vec<MemoryRow>,
    collapsed_task_ids: BTreeSet<String>,
    pub(super) pending_delete_memory_id: Option<String>,
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
            active_tab: RightPanelTab::Trace,
            density: TraceDensity::default(),
            expanded_index: None,
            scroll_offset: 0,
            selected_task_index: 0,
            selected_memory_index: 0,
            memory_scope_filter: MemoryScopeFilter::All,
            memory_search_query: String::new(),
            memory_search_active: false,
            memory_rows: Vec::new(),
            collapsed_task_ids: BTreeSet::new(),
            pending_delete_memory_id: None,
        }
    }

    pub fn cycle_tab_next(&mut self) {
        self.active_tab = self.active_tab.next();
        self.clear_memory_transient_state_if_hidden();
        self.clamp_selection();
    }

    pub fn cycle_tab_previous(&mut self) {
        self.active_tab = self.active_tab.previous();
        self.clear_memory_transient_state_if_hidden();
        self.clamp_selection();
    }

    pub fn cycle_density(&mut self) {
        self.density = self.density.next();
        self.active_tab = if self.density == TraceDensity::TaskGraph {
            RightPanelTab::Tasks
        } else {
            RightPanelTab::Trace
        };
        self.clear_memory_transient_state_if_hidden();
    }

    pub fn select_next(&mut self, row_count: usize) {
        if row_count == 0 {
            self.set_selected_index(0);
            return;
        }
        let next = self.selected_index().saturating_add(1).min(row_count - 1);
        self.set_selected_index(next);
    }

    pub fn select_previous(&mut self) {
        let previous = self.selected_index().saturating_sub(1);
        self.set_selected_index(previous);
    }

    pub fn selected_retry_task_id(
        &self,
        snapshot: &TaskGraphSnapshot,
    ) -> Option<agent_core::TaskId> {
        if self.active_tab != RightPanelTab::Tasks {
            return None;
        }
        let task = self.selected_task(snapshot)?;
        if task.state == TaskState::Failed && task.retry_count < task.max_retries {
            Some(task.id.clone())
        } else {
            None
        }
    }

    pub fn selected_cancel_task_id(
        &self,
        snapshot: &TaskGraphSnapshot,
    ) -> Option<agent_core::TaskId> {
        if self.active_tab != RightPanelTab::Tasks {
            return None;
        }
        let task = self.selected_task(snapshot)?;
        if matches!(task.state, TaskState::Failed | TaskState::Blocked) {
            Some(task.id.clone())
        } else {
            None
        }
    }

    pub fn visible_task_rows(&self, snapshot: &TaskGraphSnapshot) -> Vec<TaskListRow> {
        let tree = build_task_tree_from_snapshot(snapshot);
        flatten_task_tree_with_collapsed(&tree, &self.collapsed_task_ids)
    }

    pub fn visible_task_row_count(&self, snapshot: &TaskGraphSnapshot) -> usize {
        self.visible_task_rows(snapshot).len()
    }

    pub fn collapsed_task_ids(&self) -> &BTreeSet<String> {
        &self.collapsed_task_ids
    }

    pub fn toggle_selected_task_expansion(&mut self, snapshot: &TaskGraphSnapshot) -> bool {
        if self.active_tab != RightPanelTab::Tasks {
            return false;
        }

        let selected_id = {
            let rows = self.visible_task_rows(snapshot);
            let Some(row) = rows.get(self.selected_task_index) else {
                return false;
            };
            if row.node.children.is_empty() {
                return false;
            }
            row.node.id.clone()
        };

        if !self.collapsed_task_ids.insert(selected_id.clone()) {
            self.collapsed_task_ids.remove(&selected_id);
        }
        self.clamp_task_selection(snapshot);
        true
    }

    pub fn set_memory_rows(&mut self, rows: Vec<MemoryRow>) {
        self.memory_rows = rows;
        self.pending_delete_memory_id = None;
        self.clamp_selection();
    }

    pub fn remove_memory_row(&mut self, memory_id: &str) {
        self.memory_rows.retain(|row| row.id != memory_id);
        if self.pending_delete_memory_id.as_deref() == Some(memory_id) {
            self.pending_delete_memory_id = None;
        }
        self.clamp_selection();
    }

    pub fn selected_memory_id(&self) -> Option<String> {
        if self.active_tab != RightPanelTab::Memory {
            return None;
        }
        self.memory_rows
            .get(self.selected_memory_index)
            .map(|row| row.id.clone())
    }

    pub fn memory_keywords(&self) -> Vec<String> {
        self.memory_search_query
            .split_whitespace()
            .map(str::to_string)
            .collect()
    }

    pub fn memory_load_command(&self) -> Command {
        Command::LoadMemories {
            scope: self.memory_scope_filter.scope(),
            keywords: self.memory_keywords(),
            limit: 100,
        }
    }

    pub fn cycle_memory_scope_filter(&mut self) {
        self.memory_scope_filter = self.memory_scope_filter.next();
        self.pending_delete_memory_id = None;
    }

    pub fn start_memory_search(&mut self) {
        self.memory_search_active = true;
        self.pending_delete_memory_id = None;
    }

    pub fn apply_memory_search(&mut self) {
        self.memory_search_active = false;
        self.pending_delete_memory_id = None;
    }

    pub fn push_memory_search_char(&mut self, ch: char) {
        self.memory_search_query.push(ch);
        self.pending_delete_memory_id = None;
    }

    pub fn pop_memory_search_char(&mut self) {
        self.memory_search_query.pop();
        self.pending_delete_memory_id = None;
    }

    pub fn clear_memory_transient_state(&mut self) {
        self.memory_search_active = false;
        self.pending_delete_memory_id = None;
    }

    pub fn begin_memory_delete_confirmation(&mut self) -> Option<String> {
        let memory_id = self.selected_memory_id()?;
        if self.pending_delete_memory_id.as_deref() == Some(memory_id.as_str()) {
            self.pending_delete_memory_id = None;
            return Some(memory_id);
        }
        self.memory_search_active = false;
        self.pending_delete_memory_id = Some(memory_id);
        None
    }

    pub fn confirm_memory_delete(&mut self) -> Option<String> {
        self.pending_delete_memory_id.take()
    }

    pub fn pending_delete_memory_id(&self) -> Option<String> {
        self.pending_delete_memory_id.clone()
    }

    fn selected_task<'a>(&self, snapshot: &'a TaskGraphSnapshot) -> Option<&'a TaskSnapshot> {
        let rows = self.visible_task_rows(snapshot);
        let selected = rows.get(self.selected_task_index)?;
        snapshot
            .tasks
            .iter()
            .find(|task| task.id.to_string() == selected.node.id)
    }

    fn selected_index(&self) -> usize {
        match self.active_tab {
            RightPanelTab::Trace | RightPanelTab::Tasks => self.selected_task_index,
            RightPanelTab::Memory => self.selected_memory_index,
        }
    }

    fn set_selected_index(&mut self, index: usize) {
        match self.active_tab {
            RightPanelTab::Trace | RightPanelTab::Tasks => self.selected_task_index = index,
            RightPanelTab::Memory => self.selected_memory_index = index,
        }
    }

    fn clamp_task_selection(&mut self, snapshot: &TaskGraphSnapshot) {
        let row_count = self.visible_task_row_count(snapshot);
        if row_count == 0 {
            self.selected_task_index = 0;
        } else if self.selected_task_index >= row_count {
            self.selected_task_index = row_count - 1;
        }
    }

    fn clamp_selection(&mut self) {
        if self.memory_rows.is_empty() {
            self.selected_memory_index = 0;
            self.pending_delete_memory_id = None;
        } else if self.selected_memory_index >= self.memory_rows.len() {
            self.selected_memory_index = self.memory_rows.len() - 1;
        }
    }

    fn clear_memory_transient_state_if_hidden(&mut self) {
        if self.active_tab != RightPanelTab::Memory {
            self.clear_memory_transient_state();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelTab {
    Trace,
    Tasks,
    Memory,
}

impl RightPanelTab {
    fn next(self) -> Self {
        match self {
            Self::Trace => Self::Tasks,
            Self::Tasks => Self::Memory,
            Self::Memory => Self::Trace,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Trace => Self::Memory,
            Self::Tasks => Self::Trace,
            Self::Memory => Self::Tasks,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Trace => "Trace",
            Self::Tasks => "Tasks",
            Self::Memory => "Memory",
        }
    }
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
