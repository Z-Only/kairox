//! Per-row and per-label rendering helpers for the sessions panel.
//!
//! Each function produces a styled [`Line`], a label string, or a
//! small formatting decision. Extracted from [`super::render`] to keep
//! the orchestrator focused on layout and list/modal composition while
//! these helpers own the visual representation of individual rows.

use agent_core::{
    ProjectGitStatus, ProjectGitStatusKind, ProjectInstructionSummary, ProjectSessionVisibility,
};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::components::{ProjectInfo, SessionInfo, SessionState};

use super::state::SessionAction;

pub fn session_state_icon(state: &SessionState) -> (&'static str, Color) {
    match state {
        SessionState::Active => ("●", Color::Green),
        SessionState::Idle => ("○", Color::DarkGray),
        SessionState::Error(_) => ("✕", Color::Red),
        SessionState::AwaitingPermission => ("⚠", Color::Yellow),
    }
}

pub fn project_row_line(project: &ProjectInfo, sessions: &[SessionInfo]) -> Line<'static> {
    let session_count = sessions
        .iter()
        .filter(|session| !session.archived && session.project_id.as_ref() == Some(&project.id))
        .count();
    let expanded = if project.expanded { "▾" } else { "▸" };
    let mut spans = vec![
        Span::styled(format!("{expanded} "), Style::default().fg(Color::Cyan)),
        Span::styled(
            project.display_name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({session_count})"),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    if let Some(status) = &project.git_status {
        let (label, color) = project_git_status_label(status);
        spans.push(Span::styled(
            format!(" · {label}"),
            Style::default().fg(color),
        ));
    }

    if let Some(summary) = &project.instruction_summary {
        spans.push(Span::styled(
            format!(" · {}", project_instruction_label(summary)),
            Style::default().fg(Color::DarkGray),
        ));
    }

    Line::from(spans)
}

pub fn archive_project_label(projects: &[ProjectInfo], session: &SessionInfo) -> Option<String> {
    let project_id = session.project_id.as_ref()?;
    Some(
        projects
            .iter()
            .find(|project| &project.id == project_id)
            .map(|project| project.display_name.clone())
            .unwrap_or_else(|| project_id.to_string()),
    )
}

pub fn archived_session_row_line(session: &SessionInfo, projects: &[ProjectInfo]) -> Line<'static> {
    let mut spans = vec![
        Span::styled("○ ", Style::default().fg(Color::DarkGray)),
        Span::raw(session.title.clone()),
        Span::styled(
            format!(" [{}]", session.model_profile),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ];

    let mut metadata = Vec::new();
    if let Some(project) = archive_project_label(projects, session) {
        metadata.push(project);
    }
    if let Some(branch) = session
        .branch
        .as_deref()
        .filter(|branch| !branch.is_empty())
    {
        metadata.push(format!("branch {branch}"));
    }
    if let Some(path) = session
        .worktree_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .map(compact_worktree_path)
    {
        metadata.push(path);
    }
    if !metadata.is_empty() {
        spans.push(Span::styled(
            format!(" · {}", metadata.join(" · ")),
            Style::default().fg(Color::DarkGray),
        ));
    }

    Line::from(spans)
}

pub fn session_row_line(session: &SessionInfo, nested: bool) -> Line<'static> {
    let (icon, icon_color) = session_state_icon(&session.state);
    let pin = if session.pinned { "📌 " } else { "" };
    let archived = if session.archived { " [archived]" } else { "" };
    let metadata = session_metadata_label(session);
    let prefix = if nested { "  " } else { "" };
    let mut spans = vec![
        Span::raw(prefix.to_string()),
        Span::styled(format!("{pin}{icon} "), Style::default().fg(icon_color)),
        Span::raw(session.title.clone()),
        Span::styled(
            format!(" [{}]{archived}", session.model_profile),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ];
    if let Some(metadata) = metadata {
        spans.push(Span::styled(
            format!(" {metadata}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    if let SessionState::Error(e) = &session.state {
        spans.push(Span::styled(
            format!(" {e}"),
            Style::default().fg(Color::Red),
        ));
    }
    Line::from(spans)
}

pub fn project_git_status_label(status: &ProjectGitStatus) -> (String, Color) {
    match status.kind {
        ProjectGitStatusKind::NotInitialized => ("git not initialized".into(), Color::Yellow),
        ProjectGitStatusKind::Clean => (
            status
                .branch
                .as_deref()
                .map(|branch| format!("git {branch}"))
                .unwrap_or_else(|| "git clean".into()),
            Color::Green,
        ),
        ProjectGitStatusKind::Dirty => (
            status
                .branch
                .as_deref()
                .map(|branch| format!("dirty {branch}"))
                .unwrap_or_else(|| "dirty".into()),
            Color::Yellow,
        ),
        ProjectGitStatusKind::Detached => ("detached".into(), Color::Yellow),
        ProjectGitStatusKind::MissingPath => ("missing path".into(), Color::Red),
        ProjectGitStatusKind::Error => (
            status
                .message
                .as_deref()
                .filter(|message| !message.is_empty())
                .map(|message| format!("git error: {message}"))
                .unwrap_or_else(|| "git error".into()),
            Color::Red,
        ),
    }
}

pub fn project_instruction_label(summary: &ProjectInstructionSummary) -> String {
    if summary.source_paths.is_empty() {
        return "instructions none".into();
    }
    let names = summary
        .source_paths
        .iter()
        .filter_map(|path| std::path::Path::new(path).file_name())
        .filter_map(std::ffi::OsStr::to_str)
        .take(2)
        .collect::<Vec<_>>();
    if names.is_empty() {
        format!("instructions {}", summary.source_paths.len())
    } else {
        format!("instructions {}", names.join(", "))
    }
}

pub fn session_metadata_label(session: &SessionInfo) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(visibility) = &session.visibility {
        if matches!(visibility, ProjectSessionVisibility::DraftHidden) {
            parts.push("draft".to_string());
        }
    }
    if let Some(branch) = session
        .branch
        .as_deref()
        .filter(|branch| !branch.is_empty())
    {
        parts.push(format!("branch {branch}"));
    }
    if let Some(path) = session
        .worktree_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .map(compact_worktree_path)
    {
        parts.push(path);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

pub fn compact_worktree_path(path: &str) -> String {
    path.split_once(".kairox/")
        .map(|(_, suffix)| suffix.to_string())
        .unwrap_or_else(|| path.to_string())
}

pub fn action_label(action: SessionAction) -> &'static str {
    match action {
        SessionAction::Rename => "Rename",
        SessionAction::Archive => "Archive",
        SessionAction::Restore => "Restore",
        SessionAction::Delete => "Delete permanently",
        SessionAction::RemoveProject => "Remove project",
        SessionAction::MoveProjectUp => "Move project up",
        SessionAction::MoveProjectDown => "Move project down",
        SessionAction::ToggleExpanded => "Expand/collapse",
        SessionAction::NewDraft => "New draft session",
        SessionAction::NewWorktree => "New worktree session",
        SessionAction::GitStatus => "Refresh git status",
        SessionAction::InitGit => "Initialize git",
        SessionAction::Instructions => "Show instructions",
    }
}

pub fn action_key(action: SessionAction) -> &'static str {
    match action {
        SessionAction::Rename => "r",
        SessionAction::Archive => "a",
        SessionAction::Restore => "r",
        SessionAction::Delete => "d",
        SessionAction::RemoveProject => "d",
        SessionAction::MoveProjectUp => "↑",
        SessionAction::MoveProjectDown => "↓",
        SessionAction::ToggleExpanded => "e",
        SessionAction::NewDraft => "n",
        SessionAction::NewWorktree => "w",
        SessionAction::GitStatus => "g",
        SessionAction::InitGit => "i",
        SessionAction::Instructions => "I",
    }
}
