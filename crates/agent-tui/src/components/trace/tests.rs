use super::task_tree::task_row_label;
use super::*;
use crate::keybindings::TraceDensity;
use agent_core::events::EventPayload;
use agent_core::{
    AgentId, DomainEvent, PrivacyClassification, SessionId, TaskFailureReason, WorkspaceId,
};

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
            input_preview: String::new(),
        }),
        make_event(EventPayload::ToolInvocationCompleted {
            invocation_id: "inv1".into(),
            tool_id: "shell.exec".into(),
            output_preview: "ok".into(),
            exit_code: None,
            duration_ms: 1200,
            truncated: false,
            images: vec![],
        }),
        make_event(EventPayload::ToolInvocationStarted {
            invocation_id: "inv2".into(),
            tool_id: "patch.apply".into(),
            input_preview: String::new(),
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
            input_preview: String::new(),
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
fn task_row_label_distinguishes_blocked_skipped_and_cancelled_states() {
    use super::task_tree::trace_status_from_task_state;
    use agent_core::TaskState;

    for (state, expected) in [
        (TaskState::Blocked, "⏸ Blocked"),
        (TaskState::Skipped, "⏭ Skipped"),
        (TaskState::Cancelled, "🚫 Cancelled"),
    ] {
        let row = TaskListRow {
            node: TaskTreeNode {
                id: format!("task_{state:?}"),
                title: "Task state".into(),
                role: "Worker".into(),
                state,
                status: trace_status_from_task_state(state),
                error: None,
                retry_count: 0,
                max_retries: 3,
                assigned_agent_id: None,
                failure_reason: None,
                children: Vec::new(),
            },
            depth: 0,
        };

        assert!(task_row_label(&row, false).contains(expected));
    }
}

#[test]
fn task_row_label_includes_failure_reason_and_error_details() {
    use agent_core::TaskState;

    let row = TaskListRow {
        node: TaskTreeNode {
            id: "task_failed".into(),
            title: "Run tests".into(),
            role: "Worker".into(),
            state: TaskState::Failed,
            status: TraceStatus::Failed,
            error: Some("cargo test failed".into()),
            retry_count: 2,
            max_retries: 3,
            assigned_agent_id: None,
            failure_reason: Some(TaskFailureReason::ToolExhausted {
                tool_id: "shell.exec".into(),
                attempts: 2,
                last_error: "exit 101".into(),
            }),
            children: Vec::new(),
        },
        depth: 0,
    };

    let label = task_row_label(&row, false);

    assert!(label.contains("error: cargo test failed"));
    assert!(label.contains("tool shell.exec exhausted after 2 attempts: exit 101"));
}

#[test]
fn task_row_label_prefers_assigned_agent_badge_over_role_initial() {
    use agent_core::TaskState;

    let row = TaskListRow {
        node: TaskTreeNode {
            id: "task_assigned".into(),
            title: "Implement task".into(),
            role: "Worker".into(),
            state: TaskState::Running,
            status: TraceStatus::Running,
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: Some("agent_worker_1".into()),
            failure_reason: None,
            children: Vec::new(),
        },
        depth: 0,
    };

    let label = task_row_label(&row, false);

    assert!(label.contains("[agent_worker_1]"));
    assert!(!label.contains("[W]"));
}

#[test]
fn visible_task_rows_skip_descendants_of_collapsed_nodes() {
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
                TaskState::Running,
                vec![root_id.clone()],
            ),
            task_snapshot(
                grandchild_id,
                "Review",
                AgentRole::Reviewer,
                TaskState::Pending,
                vec![child_id.clone()],
            ),
        ],
    };
    let mut panel = TracePanel::new();
    panel.active_tab = RightPanelTab::Tasks;

    assert_eq!(panel.visible_task_row_count(&snapshot), 3);

    assert!(panel.toggle_selected_task_expansion(&snapshot));
    assert_eq!(panel.visible_task_row_count(&snapshot), 1);

    assert!(panel.toggle_selected_task_expansion(&snapshot));
    panel.selected_task_index = 1;
    assert!(panel.toggle_selected_task_expansion(&snapshot));
    assert_eq!(panel.visible_task_row_count(&snapshot), 2);
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
