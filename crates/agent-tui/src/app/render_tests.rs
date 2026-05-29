use super::*;
use crate::components::{ProjectInfo, SessionInfo, SessionState};
use agent_core::{
    ProjectId, ProjectInstructionSummary, ProjectSessionVisibility, SessionId, WorkspaceId,
};

fn render_text(app: &mut App, width: u16, height: u16) -> String {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>()
        .join("")
}

fn project_session_app() -> App {
    let workspace_id = WorkspaceId::from_string("wrk_test".to_string());
    let project_id = ProjectId::from_string("prj_alpha".to_string());
    let session_id = SessionId::from_string("ses_active".to_string());
    let mut app = App::new("fast", workspace_id);
    app.current_session_id = Some(session_id.clone());
    app.state.sidebar_left_visible = false;
    app.state.projects = vec![ProjectInfo {
        id: project_id.clone(),
        display_name: "alpha".to_string(),
        root_path: "/tmp/alpha".to_string(),
        expanded: true,
        git_status: None,
        instruction_summary: Some(ProjectInstructionSummary {
            source_paths: vec!["/tmp/alpha/AGENTS.md".to_string()],
            contents: None,
            warning: None,
        }),
    }];
    app.state.sessions = vec![SessionInfo {
        id: session_id,
        title: "Worktree session".to_string(),
        model_profile: "fast".to_string(),
        state: SessionState::Active,
        pinned: false,
        archived: false,
        project_id: Some(project_id),
        worktree_path: Some("/tmp/alpha/.kairox/worktrees/feat-tui".to_string()),
        branch: Some("feat/tui".to_string()),
        visibility: Some(ProjectSessionVisibility::Visible),
    }];
    app.sync_status_bar();
    app
}

#[test]
fn session_git_meta_renders_in_current_chat_header_and_status() {
    let mut app = project_session_app();

    let rendered = render_text(&mut app, 120, 12);

    assert!(rendered.contains("worktree"), "{rendered}");
    assert!(rendered.contains("feat/tui"), "{rendered}");
    assert!(rendered.contains("worktrees/feat-tui"), "{rendered}");
}

#[test]
fn session_git_meta_renders_project_instruction_summary() {
    let mut app = project_session_app();

    let rendered = render_text(&mut app, 120, 12);

    assert!(rendered.contains("Loaded AGENTS.md"), "{rendered}");
}
