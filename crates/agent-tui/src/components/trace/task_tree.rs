//! Task hierarchy types and helpers used by the trace panel's "Tasks" tab.
//!
//! Builds the tree from a runtime [`TaskGraphSnapshot`] or a stream of domain
//! events, flattens it into a list of rows honouring user-collapsed nodes, and
//! formats one row's label with state, role/agent, retry, and failure details.

use agent_core::events::EventPayload;
use agent_core::facade::{TaskGraphSnapshot, TaskSnapshot};
use agent_core::{TaskFailureReason, TaskState};
use std::collections::{BTreeMap, BTreeSet};

use super::TraceStatus;

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
    pub assigned_agent_id: Option<String>,
    pub failure_reason: Option<TaskFailureReason>,
    pub children: Vec<TaskTreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListRow {
    pub node: TaskTreeNode,
    pub depth: usize,
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
            assigned_agent_id: None,
            failure_reason: None,
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

pub fn flatten_task_tree_with_collapsed(
    tasks: &[TaskTreeNode],
    collapsed_task_ids: &BTreeSet<String>,
) -> Vec<TaskListRow> {
    let mut rows = Vec::new();
    for task in tasks {
        flatten_task_tree_inner(task, 0, collapsed_task_ids, &mut rows);
    }
    rows
}

fn flatten_task_tree_inner(
    task: &TaskTreeNode,
    depth: usize,
    collapsed_task_ids: &BTreeSet<String>,
    rows: &mut Vec<TaskListRow>,
) {
    rows.push(TaskListRow {
        node: task.clone(),
        depth,
    });
    if collapsed_task_ids.contains(&task.id) {
        return;
    }
    for child in &task.children {
        flatten_task_tree_inner(child, depth + 1, collapsed_task_ids, rows);
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
        assigned_agent_id: task.assigned_agent_id.clone(),
        failure_reason: task.failure_reason.clone(),
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

pub(super) fn trace_status_from_task_state(state: TaskState) -> TraceStatus {
    match state {
        TaskState::Running => TraceStatus::Running,
        TaskState::Completed | TaskState::Skipped => TraceStatus::Success,
        TaskState::Failed | TaskState::Cancelled => TraceStatus::Failed,
        TaskState::Pending | TaskState::Ready | TaskState::Blocked => TraceStatus::Pending,
    }
}

#[cfg(test)]
pub(super) fn task_row_label(row: &TaskListRow, selected: bool) -> String {
    task_row_label_with_collapsed(row, selected, false)
}

pub(super) fn task_row_label_with_collapsed(
    row: &TaskListRow,
    selected: bool,
    collapsed: bool,
) -> String {
    let task = &row.node;
    let cursor = if selected { ">" } else { " " };
    let branch = if row.depth == 0 {
        String::new()
    } else {
        format!("{}├─ ", "│ ".repeat(row.depth.saturating_sub(1)))
    };
    let spacer = if row.depth == 0 {
        String::new()
    } else {
        "  ".repeat(row.depth)
    };
    let expander = if task.children.is_empty() {
        " "
    } else if collapsed {
        "▸"
    } else {
        "▾"
    };
    let retry = if task.retry_count > 0 {
        format!(" ↻{}/{}", task.retry_count, task.max_retries)
    } else {
        String::new()
    };
    let summary = if collapsed && !task.children.is_empty() {
        let summary = child_status_summary(&task.children);
        if summary.is_empty() {
            String::new()
        } else {
            format!(" {{{summary}}}")
        }
    } else {
        String::new()
    };
    let error = task
        .error
        .as_ref()
        .map(|error| format!(" error: {error}"))
        .unwrap_or_default();
    let failure_reason = task
        .failure_reason
        .as_ref()
        .map(|reason| format!(" reason: {}", task_failure_reason_label(reason)))
        .unwrap_or_default();
    let (state_icon, state_label) = task_state_display(task.state);

    format!(
        "{cursor}{spacer}{branch}{expander} {} {} {state_icon} {state_label}{retry}{summary}{error}{failure_reason}",
        task_agent_badge(task),
        task.title,
    )
}

fn task_agent_badge(task: &TaskTreeNode) -> String {
    if let Some(agent_id) = &task.assigned_agent_id {
        return format!("[{agent_id}]");
    }
    let role_label = match task.role.as_str() {
        "Planner" => "P",
        "Worker" => "W",
        "Reviewer" => "R",
        _ => "?",
    };
    format!("[{role_label}]")
}

fn task_state_display(state: TaskState) -> (&'static str, &'static str) {
    match state {
        TaskState::Pending => ("⏳", "Pending"),
        TaskState::Ready => ("⏳", "Ready"),
        TaskState::Running => ("🔄", "Running"),
        TaskState::Blocked => ("⏸", "Blocked"),
        TaskState::Completed => ("✅", "Completed"),
        TaskState::Failed => ("❌", "Failed"),
        TaskState::Skipped => ("⏭", "Skipped"),
        TaskState::Cancelled => ("🚫", "Cancelled"),
    }
}

fn child_status_summary(children: &[TaskTreeNode]) -> String {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for child in children {
        let (icon, _) = task_state_display(child.state);
        *counts.entry(icon).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(icon, count)| format!("{icon} {count}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn task_failure_reason_label(reason: &TaskFailureReason) -> String {
    match reason {
        TaskFailureReason::ModelError { retries } => format!("model error after {retries} retries"),
        TaskFailureReason::ToolExhausted {
            tool_id,
            attempts,
            last_error,
        } => format!("tool {tool_id} exhausted after {attempts} attempts: {last_error}"),
        TaskFailureReason::PermissionDenied { tool_id } => {
            format!("permission denied for {tool_id}")
        }
        TaskFailureReason::Cancelled => "cancelled".to_string(),
        TaskFailureReason::MaxIterations => "max iterations reached".to_string(),
    }
}
