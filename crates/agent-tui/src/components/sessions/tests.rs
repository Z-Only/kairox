use super::render::session_state_icon;
use super::state::archive_stats;
use super::*;
use crate::components::{Command, Component, ProjectInfo, SessionInfo, SessionState};
use agent_core::{
    ProjectGitStatus, ProjectGitStatusKind, ProjectId, ProjectSessionVisibility, SessionId,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

fn make_session(title: &str, state: SessionState, pinned: bool) -> SessionInfo {
    SessionInfo {
        id: SessionId::new(),
        title: title.into(),
        model_profile: "fast".into(),
        state,
        pinned,
        archived: false,
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: None,
    }
}

fn make_archived_session(title: &str) -> SessionInfo {
    SessionInfo {
        archived: true,
        ..make_session(title, SessionState::Idle, false)
    }
}

fn make_project(title: &str) -> ProjectInfo {
    ProjectInfo {
        id: ProjectId::from_string(format!("prj_{title}")),
        display_name: title.into(),
        root_path: format!("/tmp/{title}"),
        expanded: true,
        git_status: None,
        instruction_summary: None,
    }
}

fn make_project_session(
    title: &str,
    project_id: ProjectId,
    branch: Option<&str>,
    worktree_path: Option<&str>,
) -> SessionInfo {
    SessionInfo {
        project_id: Some(project_id),
        branch: branch.map(str::to_string),
        worktree_path: worktree_path.map(str::to_string),
        visibility: Some(ProjectSessionVisibility::Visible),
        ..make_session(title, SessionState::Idle, false)
    }
}

fn ctx<'a>(
    projects: &'a [ProjectInfo],
    sessions: &'a [SessionInfo],
    current_session_id: &'a Option<SessionId>,
    workspace_id: &'a agent_core::WorkspaceId,
    projection: &'a agent_core::projection::SessionProjection,
) -> EventContext<'a> {
    EventContext {
        focus: crate::components::FocusTarget::Sessions,
        current_session: projection,
        projects,
        sessions,
        model_profile: "fast",
        permission_mode: agent_tools::PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id,
        current_session_id,
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn filtered_sessions_returns_all_when_no_query() {
    let panel = SessionsPanel::new();
    let sessions = vec![
        make_session("main", SessionState::Active, false),
        make_session("debug", SessionState::Idle, false),
    ];
    let filtered = panel.filtered_sessions(&sessions);
    assert_eq!(filtered.len(), 2);
}

#[test]
fn filtered_sessions_matches_title_case_insensitive() {
    let mut panel = SessionsPanel::new();
    panel.search_query = Some("MAIN".into());
    let sessions = vec![
        make_session("main session", SessionState::Active, false),
        make_session("debug", SessionState::Idle, false),
    ];
    let filtered = panel.filtered_sessions(&sessions);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "main session");
}

#[test]
fn filtered_sessions_excludes_archived_sessions() {
    let panel = SessionsPanel::new();
    let sessions = vec![
        make_session("visible", SessionState::Active, false),
        make_archived_session("archived"),
    ];

    let filtered = panel.filtered_sessions(&sessions);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "visible");
}

#[test]
fn selected_session_id_returns_correct_id() {
    let mut panel = SessionsPanel::new();
    let sessions = vec![
        make_session("first", SessionState::Active, false),
        make_session("second", SessionState::Idle, false),
    ];
    panel.state.select(Some(1));
    assert_eq!(
        panel.selected_session_id(&sessions),
        Some(sessions[1].id.clone())
    );
}

#[test]
fn session_state_icon_values() {
    assert_eq!(session_state_icon(&SessionState::Active).0, "●");
    assert_eq!(session_state_icon(&SessionState::Idle).0, "○");
    assert_eq!(
        session_state_icon(&SessionState::Error("err".into())).0,
        "✕"
    );
    assert_eq!(session_state_icon(&SessionState::AwaitingPermission).0, "⚠");
}

#[test]
fn context_menu_key_opens_session_actions_for_selection() {
    let mut panel = SessionsPanel::new();
    let sessions = vec![
        make_session("main", SessionState::Active, false),
        make_session("debug", SessionState::Idle, false),
    ];
    panel.state.select(Some(1));
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = Some(sessions[0].id.clone());

    let (effects, commands) = panel.handle_event(
        &ctx(&[], &sessions, &current, &workspace, &projection),
        &key(KeyCode::Char('x')),
    );

    assert!(effects.is_empty());
    assert!(commands.is_empty());
    assert!(panel.context_menu_open);
}

#[test]
fn action_overlay_emits_archive_for_visible_session() {
    let mut panel = SessionsPanel::new();
    let sessions = vec![
        make_session("main", SessionState::Active, false),
        make_session("debug", SessionState::Idle, false),
    ];
    panel.state.select(Some(1));
    panel.context_menu_open = true;
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = Some(sessions[0].id.clone());

    let (_, commands) = panel.handle_event(
        &ctx(&[], &sessions, &current, &workspace, &projection),
        &key(KeyCode::Char('a')),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::ArchiveSession { session_id }] if session_id == &sessions[1].id
    ));
    assert!(panel.context_menu_open);
}

#[test]
fn archive_manager_emits_restore_for_archived_session() {
    let mut panel = SessionsPanel::new();
    let sessions = vec![
        make_session("main", SessionState::Active, false),
        make_archived_session("old"),
    ];
    panel.open_archive_manager(&sessions);
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = Some(sessions[0].id.clone());

    let (_, commands) = panel.handle_event(
        &ctx(&[], &sessions, &current, &workspace, &projection),
        &key(KeyCode::Char('r')),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::RestoreSession { session_id }] if session_id == &sessions[1].id
    ));
    assert!(!panel.archive_manager_open);
}

#[test]
fn archive_manager_emits_delete_for_archived_session() {
    let mut panel = SessionsPanel::new();
    let sessions = vec![make_archived_session("old")];
    panel.open_archive_manager(&sessions);
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = None;

    let (_, commands) = panel.handle_event(
        &ctx(&[], &sessions, &current, &workspace, &projection),
        &key(KeyCode::Char('d')),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::DeleteSession { session_id }] if session_id == &sessions[0].id
    ));
    assert!(panel.archive_manager_open);
}

#[test]
fn rename_inline_mode_emits_rename_command() {
    let mut panel = SessionsPanel::new();
    let sessions = vec![make_session("main", SessionState::Active, false)];
    panel.context_menu_open = true;
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = Some(sessions[0].id.clone());

    let (_, commands) = panel.handle_event(
        &ctx(&[], &sessions, &current, &workspace, &projection),
        &key(KeyCode::Char('r')),
    );
    assert!(commands.is_empty());
    let (_, commands) = panel.handle_event(
        &ctx(&[], &sessions, &current, &workspace, &projection),
        &key(KeyCode::Char('!')),
    );
    assert!(commands.is_empty());
    let (_, commands) = panel.handle_event(
        &ctx(&[], &sessions, &current, &workspace, &projection),
        &key(KeyCode::Enter),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::RenameSession { session_id, title }]
            if session_id == &sessions[0].id && title == "main!"
    ));
    assert!(!panel.context_menu_open);
}

#[test]
fn render_sessions_shows_projects_and_branch_worktree_metadata() {
    let project = make_project("alpha");
    let sessions = vec![make_project_session(
        "Worktree session",
        project.id.clone(),
        Some("feat/tui"),
        Some("/tmp/alpha/.kairox/worktrees/feat-tui"),
    )];
    let mut panel_state = ListState::default();

    let backend = ratatui::backend::TestBackend::new(72, 8);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            render_sessions(
                frame.area(),
                frame,
                &[project],
                &sessions,
                true,
                &mut panel_state,
            );
        })
        .expect("draw");

    let buffer = terminal.backend().buffer();
    let text: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>()
        .join("");

    assert!(text.contains("Projects"));
    assert!(text.contains("alpha"));
    assert!(text.contains("Worktree session"));
    assert!(text.contains("feat/tui"));
    assert!(text.contains("worktrees/feat-tui"));
}

#[test]
fn session_list_rows_excludes_archived_sessions_from_primary_list() {
    let active = make_session("active", SessionState::Idle, false);
    let archived = make_archived_session("archived");
    let rows = session_list_rows(&[], &[active.clone(), archived]);

    assert_eq!(rows, vec![SessionListRow::Session(active.id)]);
}

#[test]
fn archive_stats_count_archived_sessions_and_projects() {
    let project = make_project("alpha");
    let mut project_archived =
        make_project_session("archived project", project.id.clone(), Some("main"), None);
    project_archived.archived = true;
    let sessions = vec![
        make_session("active", SessionState::Idle, false),
        project_archived,
        make_archived_session("loose archived"),
    ];

    let stats = archive_stats(&sessions);

    assert_eq!(stats.total, 2);
    assert_eq!(stats.projects, 1);
}

#[test]
fn render_archive_manager_shows_stats_and_archived_rows() {
    let project = make_project("alpha");
    let mut project_archived =
        make_project_session("archived project", project.id.clone(), Some("main"), None);
    project_archived.archived = true;
    let sessions = vec![project_archived, make_archived_session("loose archived")];
    let mut panel = SessionsPanel::new();
    panel.open_archive_manager(&sessions);

    let backend = ratatui::backend::TestBackend::new(80, 20);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            panel.render_action_overlay(
                frame.area(),
                frame,
                std::slice::from_ref(&project),
                &sessions,
            );
        })
        .expect("draw");

    let buffer = terminal.backend().buffer();
    let text: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>()
        .join("");

    assert!(text.contains("Archive Manager"));
    assert!(text.contains("Total: 2"));
    assert!(text.contains("Projects: 1"));
    assert!(text.contains("archived project"));
    assert!(text.contains("alpha"));
    assert!(text.contains("[Enter/r] restore"));
}

#[test]
fn archive_manager_restore_shortcut_emits_restore_for_selected_archived_session() {
    let mut panel = SessionsPanel::new();
    let sessions = vec![
        make_archived_session("first"),
        make_archived_session("second"),
    ];
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();

    assert!(panel.open_archive_manager(&sessions));
    panel.archive_cursor = 1;
    let (_, commands) = panel.handle_event(
        &ctx(&[], &sessions, &None, &workspace, &projection),
        &key(KeyCode::Char('r')),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::RestoreSession { session_id }] if session_id == &sessions[1].id
    ));
    assert!(!panel.archive_manager_open);
}

#[test]
fn archive_manager_delete_shortcut_emits_delete_for_selected_archived_session() {
    let mut panel = SessionsPanel::new();
    let sessions = vec![make_archived_session("archived")];
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();

    assert!(panel.open_archive_manager(&sessions));
    let (_, commands) = panel.handle_event(
        &ctx(&[], &sessions, &None, &workspace, &projection),
        &key(KeyCode::Char('d')),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::DeleteSession { session_id }] if session_id == &sessions[0].id
    ));
    assert!(panel.archive_manager_open);
}

#[test]
fn project_row_context_menu_emits_create_draft_for_empty_project() {
    let project = make_project("empty");
    let projects = vec![project.clone()];
    let sessions = Vec::new();
    let mut panel = SessionsPanel::new();
    panel.context_menu_open = true;
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = None;

    let (_, commands) = panel.handle_event(
        &ctx(&projects, &sessions, &current, &workspace, &projection),
        &key(KeyCode::Char('n')),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::CreateProjectDraftSession { project_id }]
            if project_id == &project.id
    ));
    assert!(!panel.context_menu_open);
}

#[test]
fn project_row_context_menu_emits_expand_persistence_command() {
    let mut project = make_project("collapsed");
    project.expanded = false;
    let projects = vec![project.clone()];
    let sessions = Vec::new();
    let mut panel = SessionsPanel::new();
    panel.context_menu_open = true;
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = None;

    let (_, commands) = panel.handle_event(
        &ctx(&projects, &sessions, &current, &workspace, &projection),
        &key(KeyCode::Char('e')),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::SetProjectExpanded { project_id, expanded }]
            if project_id == &project.id && *expanded
    ));
    assert!(!panel.context_menu_open);
}

#[test]
fn render_project_rows_show_git_branch_dirty_and_missing_path() {
    let mut clean = make_project("clean");
    clean.git_status = Some(ProjectGitStatus {
        kind: ProjectGitStatusKind::Clean,
        branch: Some("main".into()),
        worktree_path: clean.root_path.clone(),
        message: None,
    });
    let mut dirty = make_project("changed");
    dirty.git_status = Some(ProjectGitStatus {
        kind: ProjectGitStatusKind::Dirty,
        branch: Some("feat/tui".into()),
        worktree_path: dirty.root_path.clone(),
        message: None,
    });
    let mut missing = make_project("missing");
    missing.git_status = Some(ProjectGitStatus {
        kind: ProjectGitStatusKind::MissingPath,
        branch: None,
        worktree_path: missing.root_path.clone(),
        message: Some("path does not exist".into()),
    });
    let mut panel_state = ListState::default();

    let backend = ratatui::backend::TestBackend::new(96, 8);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            render_sessions(
                frame.area(),
                frame,
                &[clean, dirty, missing],
                &[],
                true,
                &mut panel_state,
            );
        })
        .expect("draw");

    let buffer = terminal.backend().buffer();
    let text: String = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>()
        .join("");

    assert!(text.contains("main"));
    assert!(text.contains("dirty"));
    assert!(text.contains("feat/tui"));
    assert!(text.contains("missing path"));
}

#[test]
fn action_overlay_emits_create_project_draft_for_project_session() {
    let project = make_project("alpha");
    let sessions = vec![make_project_session(
        "alpha session",
        project.id.clone(),
        Some("main"),
        Some("/tmp/alpha"),
    )];
    let mut panel = SessionsPanel::new();
    panel.context_menu_open = true;
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = Some(sessions[0].id.clone());

    let (_, commands) = panel.handle_event(
        &ctx(
            std::slice::from_ref(&project),
            &sessions,
            &current,
            &workspace,
            &projection,
        ),
        &key(KeyCode::Char('n')),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::CreateProjectDraftSession { project_id }]
            if project_id == &project.id
    ));
    assert!(!panel.context_menu_open);
}

#[test]
fn worktree_input_emits_create_worktree_session_with_branch() {
    let project = make_project("alpha");
    let sessions = vec![make_project_session(
        "alpha session",
        project.id.clone(),
        Some("main"),
        Some("/tmp/alpha"),
    )];
    let mut panel = SessionsPanel::new();
    panel.context_menu_open = true;
    let workspace = agent_core::WorkspaceId::new();
    let projection = agent_core::projection::SessionProjection::default();
    let current = Some(sessions[0].id.clone());

    let (_, commands) = panel.handle_event(
        &ctx(
            std::slice::from_ref(&project),
            &sessions,
            &current,
            &workspace,
            &projection,
        ),
        &key(KeyCode::Char('w')),
    );
    assert!(commands.is_empty());

    for ch in "feat/tui-parity".chars() {
        let (_, commands) = panel.handle_event(
            &ctx(
                std::slice::from_ref(&project),
                &sessions,
                &current,
                &workspace,
                &projection,
            ),
            &key(KeyCode::Char(ch)),
        );
        assert!(commands.is_empty());
    }

    let (_, commands) = panel.handle_event(
        &ctx(
            std::slice::from_ref(&project),
            &sessions,
            &current,
            &workspace,
            &projection,
        ),
        &key(KeyCode::Enter),
    );

    assert!(matches!(
        commands.as_slice(),
        [Command::CreateProjectWorktreeSession { project_id, branch_name }]
            if project_id == &project.id && branch_name == "feat/tui-parity"
    ));
    assert!(!panel.context_menu_open);
}
