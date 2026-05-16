use super::*;

#[tauri::command]
#[specta::specta]
pub async fn list_projects(state: State<'_, GuiState>) -> Result<Vec<ProjectInfoResponse>, String> {
    let workspace_id = current_workspace_id(&state).await?;
    let projects = state
        .runtime
        .list_projects(&workspace_id)
        .await
        .map_err(|error| error.to_string())?;
    Ok(projects
        .into_iter()
        .map(ProjectInfoResponse::from)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn create_blank_project(
    state: State<'_, GuiState>,
    display_name: Option<String>,
) -> Result<ProjectInfoResponse, String> {
    let workspace_id = current_workspace_id(&state).await?;
    let project = state
        .runtime
        .create_blank_project(workspace_id, display_name)
        .await
        .map_err(|error| error.to_string())?;
    Ok(ProjectInfoResponse::from(project))
}

#[tauri::command]
#[specta::specta]
pub async fn add_existing_project(
    state: State<'_, GuiState>,
    path: String,
) -> Result<ProjectInfoResponse, String> {
    let workspace_id = current_workspace_id(&state).await?;
    let project = state
        .runtime
        .add_existing_project(workspace_id, path)
        .await
        .map_err(|error| error.to_string())?;
    Ok(ProjectInfoResponse::from(project))
}

#[tauri::command]
#[specta::specta]
pub async fn rename_project(
    state: State<'_, GuiState>,
    project_id: String,
    display_name: String,
) -> Result<(), String> {
    state
        .runtime
        .rename_project(ProjectId::from_string(project_id), display_name)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_project(state: State<'_, GuiState>, project_id: String) -> Result<(), String> {
    state
        .runtime
        .remove_project(ProjectId::from_string(project_id))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn restore_project_session(
    state: State<'_, GuiState>,
    session_id: String,
) -> Result<ProjectInfoResponse, String> {
    let project = state
        .runtime
        .restore_project_session(SessionId::from_string(session_id))
        .await
        .map_err(|error| error.to_string())?;
    Ok(ProjectInfoResponse::from(project))
}

#[tauri::command]
#[specta::specta]
pub async fn update_project_order(
    state: State<'_, GuiState>,
    project_ids: Vec<String>,
) -> Result<(), String> {
    let project_ids = project_ids
        .into_iter()
        .map(ProjectId::from_string)
        .collect();
    state
        .runtime
        .update_project_order(project_ids)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_project_expanded(
    state: State<'_, GuiState>,
    project_id: String,
    expanded: bool,
) -> Result<(), String> {
    state
        .runtime
        .update_project_expanded(ProjectId::from_string(project_id), expanded)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn create_project_draft_session(
    state: State<'_, GuiState>,
    project_id: String,
) -> Result<String, String> {
    let session_id = state
        .runtime
        .create_project_draft_session(ProjectId::from_string(project_id))
        .await
        .map_err(|error| error.to_string())?;
    Ok(session_id.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_project_sessions(
    state: State<'_, GuiState>,
    project_id: String,
) -> Result<Vec<SessionInfoResponse>, String> {
    let sessions = state
        .runtime
        .list_project_sessions(ProjectId::from_string(project_id))
        .await
        .map_err(|error| error.to_string())?;
    Ok(sessions
        .into_iter()
        .map(SessionInfoResponse::from)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn list_archived_sessions(
    state: State<'_, GuiState>,
) -> Result<Vec<SessionInfoResponse>, String> {
    let workspace_id = current_workspace_id(&state).await?;
    let sessions = state
        .runtime
        .list_archived_sessions(&workspace_id)
        .await
        .map_err(|error| error.to_string())?;
    Ok(sessions
        .into_iter()
        .map(SessionInfoResponse::from)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn create_project_worktree_session(
    state: State<'_, GuiState>,
    project_id: String,
    branch_name: String,
) -> Result<String, String> {
    let session_id = state
        .runtime
        .create_project_worktree_session(ProjectId::from_string(project_id), branch_name)
        .await
        .map_err(|error| error.to_string())?;
    Ok(session_id.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_project_git_status(
    state: State<'_, GuiState>,
    project_id: String,
) -> Result<ProjectGitStatusResponse, String> {
    let status = state
        .runtime
        .get_project_git_status(ProjectId::from_string(project_id))
        .await
        .map_err(|error| error.to_string())?;
    Ok(ProjectGitStatusResponse::from(status))
}

#[tauri::command]
#[specta::specta]
pub async fn get_session_git_status(
    state: State<'_, GuiState>,
    session_id: String,
) -> Result<ProjectGitStatusResponse, String> {
    let status = state
        .runtime
        .get_session_git_status(SessionId::from_string(session_id))
        .await
        .map_err(|error| error.to_string())?;
    Ok(ProjectGitStatusResponse::from(status))
}

#[tauri::command]
#[specta::specta]
pub async fn init_project_git(
    state: State<'_, GuiState>,
    project_id: String,
) -> Result<ProjectGitStatusResponse, String> {
    let status = state
        .runtime
        .init_project_git(ProjectId::from_string(project_id))
        .await
        .map_err(|error| error.to_string())?;
    Ok(ProjectGitStatusResponse::from(status))
}

#[tauri::command]
#[specta::specta]
pub async fn get_project_instruction_summary(
    state: State<'_, GuiState>,
    project_id: String,
) -> Result<ProjectInstructionSummaryResponse, String> {
    let summary = state
        .runtime
        .get_project_instruction_summary(ProjectId::from_string(project_id))
        .await
        .map_err(|error| error.to_string())?;
    Ok(ProjectInstructionSummaryResponse::from(summary))
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct WorkspaceFilesResponse {
    pub paths: Vec<String>,
}

fn walk_workspace_files(root: &std::path::Path, max: usize) -> Vec<String> {
    let mut paths = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    // Respect .gitignore / common ignores
    let skip_dirs: &[&str] = &[
        ".git",
        "node_modules",
        "target",
        ".claude",
        ".kairox",
        "__pycache__",
        ".venv",
        "venv",
        ".tox",
        ".eggs",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        "dist",
        "build",
        ".next",
        ".nuxt",
        ".output",
    ];
    while let Some(dir) = dirs.pop() {
        if paths.len() >= max {
            break;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if paths.len() >= max {
                break;
            }
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let is_hidden = name_str.starts_with('.');
            if ft.is_dir() {
                if skip_dirs.contains(&name_str.as_ref())
                    || (is_hidden && name_str != "." && name_str != "..")
                {
                    continue;
                }
                dirs.push(entry.path());
            } else if ft.is_file() || ft.is_symlink() {
                if is_hidden && !name_str.starts_with(".env") {
                    continue;
                }
                if let Ok(rel) = entry.path().strip_prefix(root) {
                    paths.push(rel.to_string_lossy().to_string());
                }
            }
        }
    }
    paths.sort();
    paths
}

#[tauri::command]
#[specta::specta]
pub async fn list_workspace_files(
    workspace_path: String,
) -> Result<WorkspaceFilesResponse, String> {
    let root = std::path::PathBuf::from(&workspace_path);
    if !root.exists() {
        return Err(format!("Path does not exist: {}", workspace_path));
    }
    let paths = tokio::task::spawn_blocking(move || walk_workspace_files(&root, 500))
        .await
        .map_err(|e| format!("Failed to walk files: {e}"))?;
    Ok(WorkspaceFilesResponse { paths })
}
