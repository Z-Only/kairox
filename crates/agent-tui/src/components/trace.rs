use crate::components::{Command, Component, CrossPanelEffect, EventContext};
use crate::keybindings::TraceDensity;
use agent_core::events::EventPayload;
use agent_core::facade::{TaskGraphSnapshot, TaskSnapshot};
use agent_core::TaskState;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use std::collections::{BTreeMap, BTreeSet};

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
    pending_delete_memory_id: Option<String>,
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
        let tree = build_task_tree_from_snapshot(snapshot);
        let rows = flatten_task_tree(&tree);
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

    fn label(self) -> &'static str {
        match self {
            Self::Trace => "Trace",
            Self::Tasks => "Tasks",
            Self::Memory => "Memory",
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct TaskTreeNode {
    pub id: String,
    pub title: String,
    pub role: String,
    pub state: TaskState,
    pub status: TraceStatus,
    pub error: Option<String>,
    pub retry_count: usize,
    pub max_retries: usize,
    pub children: Vec<TaskTreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListRow {
    pub node: TaskTreeNode,
    pub depth: usize,
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

pub fn extract_task_traces(events: &[agent_core::DomainEvent]) -> Vec<TaskTreeNode> {
    struct TaskInfo {
        id: String,
        title: String,
        role: String,
        state: TaskState,
        status: TraceStatus,
        error: Option<String>,
    }

    let mut tasks: Vec<TaskInfo> = Vec::new();

    for event in events {
        match &event.payload {
            EventPayload::AgentTaskCreated {
                task_id,
                title,
                role,
                dependencies: _,
            } => {
                let role_str = format!("{:?}", role);
                tasks.push(TaskInfo {
                    id: task_id.to_string(),
                    title: title.clone(),
                    role: role_str,
                    state: TaskState::Pending,
                    status: TraceStatus::Pending,
                    error: None,
                });
            }
            EventPayload::AgentTaskStarted { task_id } => {
                if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id.to_string()) {
                    t.state = TaskState::Running;
                    t.status = TraceStatus::Running;
                }
            }
            EventPayload::AgentTaskCompleted { task_id } => {
                if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id.to_string()) {
                    t.state = TaskState::Completed;
                    t.status = TraceStatus::Success;
                }
            }
            EventPayload::AgentTaskFailed { task_id, error } => {
                if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id.to_string()) {
                    t.state = TaskState::Failed;
                    t.status = TraceStatus::Failed;
                    t.error = Some(error.clone());
                }
            }
            _ => {}
        }
    }

    tasks
        .into_iter()
        .map(|t| TaskTreeNode {
            id: t.id,
            title: t.title,
            role: t.role,
            state: t.state,
            status: t.status,
            error: t.error,
            retry_count: 0,
            max_retries: 0,
            children: Vec::new(),
        })
        .collect()
}

pub fn build_task_tree_from_snapshot(snapshot: &TaskGraphSnapshot) -> Vec<TaskTreeNode> {
    let task_ids: BTreeSet<String> = snapshot
        .tasks
        .iter()
        .map(|task| task.id.to_string())
        .collect();
    let mut children_by_parent: BTreeMap<String, Vec<TaskTreeNode>> = BTreeMap::new();
    let mut roots = Vec::new();

    for task in &snapshot.tasks {
        let parent_id = task
            .dependencies
            .iter()
            .rev()
            .map(ToString::to_string)
            .find(|dependency_id| task_ids.contains(dependency_id));
        let node = task_node_from_snapshot(task);

        if let Some(parent_id) = parent_id {
            children_by_parent.entry(parent_id).or_default().push(node);
        } else {
            roots.push(node);
        }
    }

    for root in &mut roots {
        attach_task_children(root, &mut children_by_parent);
    }

    roots
}

pub fn flatten_task_tree(tasks: &[TaskTreeNode]) -> Vec<TaskListRow> {
    let mut rows = Vec::new();
    for task in tasks {
        flatten_task_tree_inner(task, 0, &mut rows);
    }
    rows
}

fn flatten_task_tree_inner(task: &TaskTreeNode, depth: usize, rows: &mut Vec<TaskListRow>) {
    rows.push(TaskListRow {
        node: task.clone(),
        depth,
    });
    for child in &task.children {
        flatten_task_tree_inner(child, depth + 1, rows);
    }
}

fn task_node_from_snapshot(task: &TaskSnapshot) -> TaskTreeNode {
    TaskTreeNode {
        id: task.id.to_string(),
        title: task.title.clone(),
        role: task.role.to_string(),
        state: task.state,
        status: trace_status_from_task_state(task.state),
        error: task.error.clone(),
        retry_count: task.retry_count,
        max_retries: task.max_retries,
        children: Vec::new(),
    }
}

fn attach_task_children(
    node: &mut TaskTreeNode,
    children_by_parent: &mut BTreeMap<String, Vec<TaskTreeNode>>,
) {
    node.children = children_by_parent.remove(&node.id).unwrap_or_default();
    for child in &mut node.children {
        attach_task_children(child, children_by_parent);
    }
}

fn trace_status_from_task_state(state: TaskState) -> TraceStatus {
    match state {
        TaskState::Running => TraceStatus::Running,
        TaskState::Completed | TaskState::Skipped => TraceStatus::Success,
        TaskState::Failed | TaskState::Cancelled => TraceStatus::Failed,
        TaskState::Pending | TaskState::Ready | TaskState::Blocked => TraceStatus::Pending,
    }
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

pub fn render_task_graph(
    area: Rect,
    frame: &mut Frame,
    tasks: &[TaskTreeNode],
    focused: bool,
    selected_index: usize,
) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let rows = flatten_task_tree(tasks);
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
            let role_label = match task.role.as_str() {
                "Planner" => "P",
                "Worker" => "W",
                "Reviewer" => "R",
                _ => "?",
            };
            let cursor = if focused && index == selected_index {
                ">"
            } else {
                " "
            };
            let indent = "  ".repeat(row.depth);
            let retry = if task.retry_count > 0 {
                format!(" ↻{}/{}", task.retry_count, task.max_retries)
            } else {
                String::new()
            };
            let line = Line::from(vec![
                Span::styled(cursor, Style::default().fg(Color::Cyan)),
                Span::raw(indent),
                Span::styled(format!("{} ", role_label), Style::default().fg(Color::Blue)),
                Span::styled(&task.title, Style::default()),
                Span::styled(
                    format!(" {}{}", task.status, retry),
                    Style::default().fg(status_color),
                ),
            ]);
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

pub fn render_memory_browser(area: Rect, frame: &mut Frame, panel: &TracePanel) {
    let memories = &panel.memory_rows;
    let focused = panel.focused();
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

fn right_panel_title(active: RightPanelTab) -> String {
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
        assert_eq!(
            TraceDensity::FullEventStream.next(),
            TraceDensity::TaskGraph
        );
        assert_eq!(TraceDensity::TaskGraph.next(), TraceDensity::Summary);
    }

    #[test]
    fn trace_status_display() {
        assert_eq!(format!("{}", TraceStatus::Running), "⏳");
        assert_eq!(format!("{}", TraceStatus::Success), "✓");
        assert_eq!(format!("{}", TraceStatus::Failed), "✕");
    }

    #[test]
    fn extract_task_traces_from_events() {
        use agent_core::{AgentRole, TaskId};

        let task_id = TaskId::new();
        let events = vec![
            make_event(EventPayload::AgentTaskCreated {
                task_id: task_id.clone(),
                title: "Plan features".into(),
                role: AgentRole::Planner,
                dependencies: vec![],
            }),
            make_event(EventPayload::AgentTaskStarted {
                task_id: task_id.clone(),
            }),
            make_event(EventPayload::AgentTaskCompleted {
                task_id: task_id.clone(),
            }),
        ];

        let tasks = extract_task_traces(&events);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Plan features");
        assert_eq!(tasks[0].role, "Planner");
        assert_eq!(tasks[0].status, TraceStatus::Success);
    }

    #[test]
    fn extract_task_traces_handles_failure() {
        use agent_core::{AgentRole, TaskId};

        let task_id = TaskId::new();
        let events = vec![
            make_event(EventPayload::AgentTaskCreated {
                task_id: task_id.clone(),
                title: "Run tests".into(),
                role: AgentRole::Worker,
                dependencies: vec![],
            }),
            make_event(EventPayload::AgentTaskStarted {
                task_id: task_id.clone(),
            }),
            make_event(EventPayload::AgentTaskFailed {
                task_id: task_id.clone(),
                error: "timeout".into(),
            }),
        ];

        let tasks = extract_task_traces(&events);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TraceStatus::Failed);
        assert_eq!(tasks[0].error, Some("timeout".into()));
    }

    #[test]
    fn builds_task_tree_from_snapshot_dependencies() {
        use agent_core::facade::{TaskGraphSnapshot, TaskSnapshot};
        use agent_core::{AgentRole, TaskId, TaskState};

        fn task_snapshot(
            id: TaskId,
            title: &str,
            role: AgentRole,
            state: TaskState,
            dependencies: Vec<TaskId>,
        ) -> TaskSnapshot {
            TaskSnapshot {
                id,
                title: title.into(),
                role,
                state,
                dependencies,
                error: None,
                retry_count: 0,
                max_retries: 3,
                assigned_agent_id: None,
                failure_reason: None,
            }
        }

        let root_id = TaskId::from_string("task_root".into());
        let child_id = TaskId::from_string("task_child".into());
        let grandchild_id = TaskId::from_string("task_grandchild".into());

        let snapshot = TaskGraphSnapshot {
            tasks: vec![
                task_snapshot(
                    root_id.clone(),
                    "Plan",
                    AgentRole::Planner,
                    TaskState::Completed,
                    vec![],
                ),
                task_snapshot(
                    child_id.clone(),
                    "Build",
                    AgentRole::Worker,
                    TaskState::Failed,
                    vec![root_id.clone()],
                ),
                task_snapshot(
                    grandchild_id,
                    "Review",
                    AgentRole::Reviewer,
                    TaskState::Blocked,
                    vec![child_id.clone()],
                ),
            ],
        };

        let tree = build_task_tree_from_snapshot(&snapshot);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].id, "task_root");
        assert_eq!(tree[0].children[0].id, "task_child");
        assert_eq!(tree[0].children[0].children[0].id, "task_grandchild");
        assert_eq!(tree[0].children[0].status, TraceStatus::Failed);
    }

    #[test]
    fn trace_panel_cycles_right_panel_tabs_without_changing_density() {
        let mut panel = TracePanel::new();

        assert_eq!(panel.active_tab, RightPanelTab::Trace);
        assert_eq!(panel.density, TraceDensity::Summary);

        panel.cycle_tab_next();
        assert_eq!(panel.active_tab, RightPanelTab::Tasks);
        assert_eq!(panel.density, TraceDensity::Summary);

        panel.cycle_tab_next();
        assert_eq!(panel.active_tab, RightPanelTab::Memory);

        panel.cycle_tab_next();
        assert_eq!(panel.active_tab, RightPanelTab::Trace);
    }

    #[test]
    fn memory_rows_render_scope_key_and_preview() {
        let mut row = MemoryRow::new(
            "mem_user".into(),
            "user".into(),
            Some("preferred-command".into()),
            "Use cargo test -p agent-tui trace task memory before opening the PR".into(),
        );
        row.accepted = false;

        assert_eq!(
            row.label(),
            "[user] pending preferred-command: Use cargo test -p agent-tui trace task memory before opening the PR"
        );
    }

    #[test]
    fn memory_scope_filter_cycles_through_all_gui_scopes() {
        assert_eq!(MemoryScopeFilter::All.next(), MemoryScopeFilter::Session);
        assert_eq!(MemoryScopeFilter::Session.next(), MemoryScopeFilter::User);
        assert_eq!(MemoryScopeFilter::User.next(), MemoryScopeFilter::Workspace);
        assert_eq!(MemoryScopeFilter::Workspace.next(), MemoryScopeFilter::All);
    }
}
