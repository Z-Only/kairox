use super::*;

pub(super) async fn dispatch<F>(runtime: &std::sync::Arc<F>, app: &mut App, command: Command)
where
    F: AppFacade + ?Sized,
{
    match command {
        Command::SetSettingsConfigSource { source } => {
            app.state.set_settings_config_source(source);
            let detail = match source {
                SettingsConfigSource::User => "user config".to_string(),
                SettingsConfigSource::Project => app
                    .state
                    .selected_settings_project()
                    .map(|project| format!("project config {}", project.display_name))
                    .unwrap_or_else(|| "project config".to_string()),
            };
            push_status_message(app, format!("settings source: {detail}"));
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::CycleSettingsProject { direction } => {
            app.state
                .set_settings_config_source(SettingsConfigSource::Project);
            match cycle_settings_project(app, direction) {
                Some(project) => {
                    let display_name = project.display_name.clone();
                    app.state.select_settings_project(project.id);
                    push_status_message(app, format!("settings project: {display_name}"));
                }
                None => {
                    push_status_message(app, "settings project unavailable".to_string());
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::OpenConfigDir => {
            match AppFacade::open_config_dir(runtime.as_ref()).await {
                Ok(Some(path)) => {
                    open_directory_path(app, &path, "config dir");
                }
                Ok(None) => {
                    push_status_message(app, "config dir path unavailable".to_string());
                }
                Err(error) => {
                    push_status_message(app, format!("[config dir error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::OpenAgentsDir => {
            match AppFacade::open_agents_dir(runtime.as_ref()).await {
                Ok(Some(path)) => {
                    open_directory_path(app, &path, "agents dir");
                }
                Ok(None) => {
                    push_status_message(app, "agents dir path unavailable".to_string());
                }
                Err(error) => {
                    push_status_message(app, format!("[agents dir error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::OpenSkillsDir => {
            match AppFacade::open_skills_dir(runtime.as_ref()).await {
                Ok(Some(path)) => {
                    open_directory_path(app, &path, "skills dir");
                }
                Ok(None) => {
                    push_status_message(app, "skills dir path unavailable".to_string());
                }
                Err(error) => {
                    push_status_message(app, format!("[skills dir error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::ClearSessionProjection => {
            clear_session_projection(app);
            push_status_message(app, "cleared local conversation projection".to_string());
            app.state.render_scheduler.mark_dirty_immediate();
        }
        _ => {}
    }
}

pub(super) fn push_status_message(app: &mut App, content: String) {
    if content.trim().is_empty() {
        return;
    }
    app.state.push_status_message(content);
    if let Some(entry) = app.state.latest_status_message() {
        app.status_bar.push_notification(entry.message.clone());
    }
    app.state.render_scheduler.mark_dirty();
}

pub fn clear_session_projection(app: &mut App) {
    app.state.current_session = agent_core::projection::SessionProjection::default();
    app.last_context_usage = None;
    app.compacting = false;
    app.state.render_scheduler.reset();
    app.sync_status_bar();
}

pub(super) fn user_config_path() -> std::path::PathBuf {
    std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".kairox")
        .join("config.toml")
}

fn project_config_path() -> Result<std::path::PathBuf, String> {
    std::env::current_dir()
        .map(|root| root.join(".kairox").join("config.toml"))
        .map_err(|error| format!("failed to resolve project config path: {error}"))
}

pub(super) fn selected_project_config_path(app: &App) -> Result<std::path::PathBuf, String> {
    app.state
        .selected_settings_project_config_path()
        .map(Ok)
        .unwrap_or_else(project_config_path)
}

pub(super) fn selected_project_config_path_for_source(
    app: &App,
) -> Result<Option<std::path::PathBuf>, String> {
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        selected_project_config_path(app).map(Some)
    } else {
        Ok(None)
    }
}

pub(super) fn selected_project_root_for_source(app: &App) -> Option<String> {
    app.state
        .selected_settings_project_root()
        .map(|root| root.display().to_string())
}

fn cycle_settings_project(app: &App, direction: i32) -> Option<ProjectInfo> {
    if app.state.projects.is_empty() {
        return None;
    }
    let current_index = app
        .state
        .selected_settings_project_id()
        .and_then(|project_id| {
            app.state
                .projects
                .iter()
                .position(|project| &project.id == project_id)
        })
        .unwrap_or(0);
    let last = app.state.projects.len() - 1;
    let next_index = if direction < 0 {
        if current_index == 0 {
            last
        } else {
            current_index - 1
        }
    } else if direction > 0 {
        if current_index >= last {
            0
        } else {
            current_index + 1
        }
    } else {
        current_index
    };
    app.state.projects.get(next_index).cloned()
}

fn open_directory_path(app: &mut App, path: &str, label: &str) {
    let path_buf = std::path::PathBuf::from(path);
    match std::fs::create_dir_all(&path_buf)
        .map_err(|error| format!("failed to create {label} {}: {error}", path_buf.display()))
        .and_then(|()| open_path_in_system_file_manager(&path_buf))
    {
        Ok(()) => {
            push_status_message(app, format!("opened {label} {}", path_buf.display()));
        }
        Err(error) => {
            push_status_message(app, format!("[{label} open error: {error}]"));
        }
    }
}

pub(super) fn open_path_in_system_file_manager(path: &std::path::Path) -> Result<(), String> {
    let mut command = system_file_manager_command(path);
    let status = command
        .status()
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;

    if status.success() {
        return Ok(());
    }

    Err(format!(
        "failed to open {}: system opener exited with {status}",
        path.display()
    ))
}

#[cfg(target_os = "macos")]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("explorer");
    command.arg(path);
    command
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(path);
    command
}

#[cfg(test)]
#[path = "common_tests.rs"]
mod tests;
