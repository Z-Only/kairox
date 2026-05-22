use super::*;

pub(super) async fn dispatch<F>(runtime: &std::sync::Arc<F>, app: &mut App, command: Command)
where
    F: AppFacade + ?Sized,
{
    match command {
        Command::CreateBlankProject { display_name } => {
            match ProjectFacade::create_blank_project(
                runtime.as_ref(),
                app.workspace_id.clone(),
                display_name.clone(),
            )
            .await
            {
                Ok(project) => {
                    let project_info = project_info_from_meta(project.clone());
                    upsert_project(app, project_info);
                    refresh_project_status(runtime, app, project.project_id.clone()).await;
                    common::push_status_message(
                        app,
                        format!("created project {}", project.display_name),
                    );
                }
                Err(error) => {
                    common::push_status_message(app, format!("[project create error: {error}]"));
                }
            }
        }
        Command::AddExistingProject { path } => {
            match ProjectFacade::add_existing_project(
                runtime.as_ref(),
                app.workspace_id.clone(),
                path.clone(),
            )
            .await
            {
                Ok(project) => {
                    let project_info = project_info_from_meta(project.clone());
                    upsert_project(app, project_info);
                    refresh_project_status(runtime, app, project.project_id.clone()).await;
                    common::push_status_message(
                        app,
                        format!("imported project {}", project.display_name),
                    );
                }
                Err(error) => {
                    common::push_status_message(app, format!("[project import error: {error}]"));
                }
            }
        }
        Command::RenameProject {
            project_id,
            display_name,
        } => {
            match ProjectFacade::rename_project(
                runtime.as_ref(),
                project_id.clone(),
                display_name.clone(),
            )
            .await
            {
                Ok(()) => {
                    if let Some(project) = app
                        .state
                        .projects
                        .iter_mut()
                        .find(|project| project.id == project_id)
                    {
                        project.display_name = display_name;
                    }
                    app.state.render_scheduler.mark_dirty();
                }
                Err(error) => {
                    common::push_status_message(app, format!("[project rename error: {error}]"));
                }
            }
        }
        Command::RemoveProject { project_id } => {
            match ProjectFacade::remove_project(runtime.as_ref(), project_id.clone()).await {
                Ok(()) => {
                    app.state
                        .projects
                        .retain(|project| project.id != project_id);
                    for session in &mut app.state.sessions {
                        if session.project_id.as_ref() == Some(&project_id) {
                            session.archived = true;
                            session.visibility =
                                Some(agent_core::ProjectSessionVisibility::Archived);
                        }
                    }
                    app.state.render_scheduler.mark_dirty();
                }
                Err(error) => {
                    common::push_status_message(app, format!("[project remove error: {error}]"));
                }
            }
        }
        Command::MoveProject {
            project_id,
            direction,
        } => {
            if let Some(project_ids) = reordered_project_ids(app, &project_id, direction) {
                match ProjectFacade::update_project_order(runtime.as_ref(), project_ids.clone())
                    .await
                {
                    Ok(()) => {
                        apply_project_order(app, &project_ids);
                        app.state.render_scheduler.mark_dirty();
                    }
                    Err(error) => {
                        common::push_status_message(
                            app,
                            format!("[project reorder error: {error}]"),
                        );
                    }
                }
            }
        }
        Command::SetProjectExpanded {
            project_id,
            expanded,
        } => {
            match ProjectFacade::update_project_expanded(
                runtime.as_ref(),
                project_id.clone(),
                expanded,
            )
            .await
            {
                Ok(()) => {
                    if let Some(project) = app
                        .state
                        .projects
                        .iter_mut()
                        .find(|project| project.id == project_id)
                    {
                        project.expanded = expanded;
                    }
                    app.state.render_scheduler.mark_dirty();
                }
                Err(error) => {
                    common::push_status_message(app, format!("[project expanded error: {error}]"));
                }
            }
        }
        Command::RefreshProjectGitStatus { project_id } => {
            match ProjectFacade::get_project_git_status(runtime.as_ref(), project_id.clone()).await
            {
                Ok(status) => {
                    set_project_status(app, &project_id, status.clone());
                    common::push_status_message(app, project_git_status_message(&status));
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[project git status error: {error}]"),
                    );
                }
            }
        }
        Command::InitProjectGit { project_id } => {
            match ProjectFacade::init_project_git(runtime.as_ref(), project_id.clone()).await {
                Ok(status) => {
                    set_project_status(app, &project_id, status.clone());
                    common::push_status_message(app, project_git_status_message(&status));
                }
                Err(error) => {
                    common::push_status_message(app, format!("[project git init error: {error}]"));
                }
            }
        }
        Command::ShowProjectInstructions { project_id } => {
            match ProjectFacade::get_project_instruction_summary(
                runtime.as_ref(),
                project_id.clone(),
            )
            .await
            {
                Ok(summary) => {
                    set_project_instruction_summary(app, &project_id, summary.clone());
                    common::push_status_message(app, project_instruction_message(&summary));
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[project instructions error: {error}]"),
                    );
                }
            }
        }
        _ => {}
    }
}

fn project_info_from_meta(project: ProjectMeta) -> ProjectInfo {
    ProjectInfo {
        id: project.project_id,
        display_name: project.display_name,
        root_path: project.root_path,
        expanded: project.expanded,
        git_status: None,
        instruction_summary: None,
    }
}

fn upsert_project(app: &mut App, project: ProjectInfo) {
    if let Some(existing) = app
        .state
        .projects
        .iter_mut()
        .find(|existing| existing.id == project.id)
    {
        *existing = project;
    } else {
        app.state.projects.push(project);
    }
    app.state.render_scheduler.mark_dirty();
}

async fn refresh_project_status<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    project_id: agent_core::ProjectId,
) where
    F: AppFacade + ?Sized,
{
    if let Ok(status) =
        ProjectFacade::get_project_git_status(runtime.as_ref(), project_id.clone()).await
    {
        set_project_status(app, &project_id, status);
    }
}

fn set_project_status(app: &mut App, project_id: &agent_core::ProjectId, status: ProjectGitStatus) {
    if let Some(project) = app
        .state
        .projects
        .iter_mut()
        .find(|project| &project.id == project_id)
    {
        project.git_status = Some(status);
    }
    app.state.render_scheduler.mark_dirty();
}

fn set_project_instruction_summary(
    app: &mut App,
    project_id: &agent_core::ProjectId,
    summary: ProjectInstructionSummary,
) {
    if let Some(project) = app
        .state
        .projects
        .iter_mut()
        .find(|project| &project.id == project_id)
    {
        project.instruction_summary = Some(summary);
    }
    app.state.render_scheduler.mark_dirty();
}

fn reordered_project_ids(
    app: &mut App,
    project_id: &agent_core::ProjectId,
    direction: i32,
) -> Option<Vec<agent_core::ProjectId>> {
    let index = app
        .state
        .projects
        .iter()
        .position(|project| &project.id == project_id)?;
    if app.state.projects.is_empty() {
        return None;
    }
    let last = app.state.projects.len() - 1;
    let next = if direction < 0 {
        index.saturating_sub(1)
    } else if direction > 0 {
        (index + 1).min(last)
    } else {
        index
    };
    if next == index {
        return None;
    }
    let mut project_ids = app
        .state
        .projects
        .iter()
        .map(|project| project.id.clone())
        .collect::<Vec<_>>();
    project_ids.swap(index, next);
    Some(project_ids)
}

fn apply_project_order(app: &mut App, project_ids: &[agent_core::ProjectId]) {
    let mut next_projects = Vec::with_capacity(app.state.projects.len());
    for project_id in project_ids {
        if let Some(project) = app
            .state
            .projects
            .iter()
            .find(|project| &project.id == project_id)
            .cloned()
        {
            next_projects.push(project);
        }
    }
    next_projects.extend(
        app.state
            .projects
            .iter()
            .filter(|project| {
                !project_ids
                    .iter()
                    .any(|project_id| project_id == &project.id)
            })
            .cloned(),
    );
    app.state.projects = next_projects;
}

fn project_git_status_message(status: &ProjectGitStatus) -> String {
    let branch = status
        .branch
        .as_deref()
        .map(|branch| format!(" on {branch}"))
        .unwrap_or_default();
    let kind = match status.kind {
        ProjectGitStatusKind::NotInitialized => "not initialized",
        ProjectGitStatusKind::Clean => "clean",
        ProjectGitStatusKind::Dirty => "dirty",
        ProjectGitStatusKind::Detached => "detached",
        ProjectGitStatusKind::MissingPath => "missing path",
        ProjectGitStatusKind::Error => "error",
    };
    let mut message = format!("git status: {kind}{branch} ({})", status.worktree_path);
    if let Some(detail) = status
        .message
        .as_deref()
        .filter(|detail| !detail.is_empty())
    {
        message.push_str(&format!(": {detail}"));
    }
    message
}

fn project_instruction_message(summary: &ProjectInstructionSummary) -> String {
    let sources = if summary.source_paths.is_empty() {
        "no instruction files".to_string()
    } else {
        summary.source_paths.join(", ")
    };
    let mut message = format!("project instructions: {sources}");
    if let Some(warning) = summary
        .warning
        .as_deref()
        .filter(|warning| !warning.is_empty())
    {
        message.push_str(&format!("\nwarning: {warning}"));
    }
    if let Some(contents) = summary
        .contents
        .as_deref()
        .filter(|contents| !contents.is_empty())
    {
        let preview: String = contents.chars().take(4000).collect();
        message.push_str("\n\n");
        message.push_str(&preview);
        if contents.chars().count() > preview.chars().count() {
            message.push_str("\n\n[...truncated]");
        }
    }
    message
}
