use super::*;

fn project_id(id: &str) -> agent_core::ProjectId {
    agent_core::ProjectId::from_string(format!("prj_{id}"))
}

fn project(id: &str) -> ProjectInfo {
    ProjectInfo {
        id: project_id(id),
        display_name: id.to_string(),
        root_path: format!("/tmp/{id}"),
        expanded: true,
        git_status: None,
        instruction_summary: None,
    }
}

fn app_with_projects(ids: &[&str]) -> App {
    let mut app = App::new(
        "test-profile",
        agent_core::WorkspaceId::from_string("wrk_test".into()),
    );
    app.state.projects = ids.iter().map(|id| project(id)).collect();
    app
}

fn project_ids(app: &App) -> Vec<String> {
    app.state
        .projects
        .iter()
        .map(|project| project.id.to_string())
        .collect()
}

#[test]
fn reordered_project_ids_moves_one_step_and_returns_none_at_boundaries() {
    let mut app = app_with_projects(&["a", "b", "c"]);

    let moved_up = reordered_project_ids(&mut app, &project_id("b"), -1).expect("move b up");
    assert_eq!(
        moved_up,
        vec![project_id("b"), project_id("a"), project_id("c")]
    );

    let moved_down = reordered_project_ids(&mut app, &project_id("b"), 1).expect("move b down");
    assert_eq!(
        moved_down,
        vec![project_id("a"), project_id("c"), project_id("b")]
    );

    assert!(reordered_project_ids(&mut app, &project_id("a"), -1).is_none());
    assert!(reordered_project_ids(&mut app, &project_id("c"), 1).is_none());
    assert!(reordered_project_ids(&mut app, &project_id("b"), 0).is_none());
    assert!(reordered_project_ids(&mut app, &project_id("missing"), 1).is_none());
}

#[test]
fn apply_project_order_reorders_known_projects_and_appends_unchanged_projects() {
    let mut app = app_with_projects(&["a", "b", "c", "d"]);

    apply_project_order(&mut app, &[project_id("c"), project_id("a")]);

    assert_eq!(project_ids(&app), vec!["prj_c", "prj_a", "prj_b", "prj_d"]);
}

#[test]
fn project_git_status_message_formats_branchless_and_detail_states() {
    let missing = ProjectGitStatus {
        kind: ProjectGitStatusKind::MissingPath,
        branch: None,
        worktree_path: "/tmp/missing".to_string(),
        message: Some("directory not found".to_string()),
    };
    assert_eq!(
        project_git_status_message(&missing),
        "git status: missing path (/tmp/missing): directory not found"
    );

    let detached = ProjectGitStatus {
        kind: ProjectGitStatusKind::Detached,
        branch: None,
        worktree_path: "/tmp/detached".to_string(),
        message: Some("HEAD detached at abc123".to_string()),
    };
    assert_eq!(
        project_git_status_message(&detached),
        "git status: detached (/tmp/detached): HEAD detached at abc123"
    );

    let error = ProjectGitStatus {
        kind: ProjectGitStatusKind::Error,
        branch: Some("main".to_string()),
        worktree_path: "/tmp/repo".to_string(),
        message: Some("git failed".to_string()),
    };
    assert_eq!(
        project_git_status_message(&error),
        "git status: error on main (/tmp/repo): git failed"
    );
}

#[test]
fn project_instruction_message_formats_empty_warning_and_truncated_contents() {
    let empty = ProjectInstructionSummary {
        source_paths: Vec::new(),
        contents: None,
        warning: Some("not readable".to_string()),
    };
    assert_eq!(
        project_instruction_message(&empty),
        "project instructions: no instruction files\nwarning: not readable"
    );

    let long_contents = "a".repeat(4001);
    let summary = ProjectInstructionSummary {
        source_paths: vec![
            "AGENTS.md".to_string(),
            ".kairox/instructions.md".to_string(),
        ],
        contents: Some(long_contents),
        warning: None,
    };
    let message = project_instruction_message(&summary);
    assert!(message.starts_with("project instructions: AGENTS.md, .kairox/instructions.md\n\naaaa"));
    assert!(message.ends_with("\n\n[...truncated]"));
    let preview = message
        .split("\n\n")
        .nth(1)
        .expect("message should include contents preview");
    assert_eq!(preview.len(), 4000);
    assert!(preview.chars().all(|ch| ch == 'a'));
}
