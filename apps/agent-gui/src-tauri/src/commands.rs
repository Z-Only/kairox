#![allow(dead_code)]
#![allow(clippy::new_without_default)]
use crate::app_state::GuiState;
use crate::event_forwarder::spawn_event_forwarder;
use agent_config::ProfileInfo;
use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, McpServerSettingsInput,
    McpServerSettingsView, RemoteSkillSearchResult, SkillCatalogEntry, SkillCatalogQuery,
    SkillSettingsDetail, SkillSettingsView, SkillSourceView,
};
use agent_core::{
    AppFacade, PermissionDecision, ProjectGitStatus, ProjectGitStatusKind, ProjectId,
    ProjectInstructionSummary, ProjectMeta, ProjectSessionVisibility, SessionId, SessionMeta,
};
use agent_memory::{MemoryEntry, MemoryQuery, MemoryScope};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct WorkspaceInfoResponse {
    pub workspace_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SessionInfoResponse {
    pub id: String,
    pub title: String,
    pub profile: String,
    pub project_id: Option<String>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct MemoryEntryResponse {
    pub id: String,
    pub scope: String,
    pub key: Option<String>,
    pub content: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileDetailResponse {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TaskSnapshotResponse {
    pub id: String,
    pub title: String,
    pub role: String,
    pub state: String,
    pub dependencies: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BuildInfoResponse {
    pub version: String,
    pub git_hash: String,
    pub build_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProjectInfoResponse {
    pub project_id: String,
    pub display_name: String,
    pub root_path: String,
    pub removed_at: Option<String>,
    pub sort_order: i64,
    pub expanded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProjectGitStatusResponse {
    pub kind: String,
    pub branch: Option<String>,
    pub worktree_path: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProjectInstructionSummaryResponse {
    pub source_paths: Vec<String>,
    pub warning: Option<String>,
}

impl From<ProjectMeta> for ProjectInfoResponse {
    fn from(project: ProjectMeta) -> Self {
        Self {
            project_id: project.project_id.to_string(),
            display_name: project.display_name,
            root_path: project.root_path,
            removed_at: project.removed_at,
            sort_order: project.sort_order,
            expanded: project.expanded,
        }
    }
}

impl From<ProjectGitStatus> for ProjectGitStatusResponse {
    fn from(status: ProjectGitStatus) -> Self {
        Self {
            kind: project_git_status_kind_to_string(status.kind),
            branch: status.branch,
            worktree_path: status.worktree_path,
            message: status.message,
        }
    }
}

impl From<ProjectInstructionSummary> for ProjectInstructionSummaryResponse {
    fn from(summary: ProjectInstructionSummary) -> Self {
        Self {
            source_paths: summary.source_paths,
            warning: summary.warning,
        }
    }
}

impl From<SessionMeta> for SessionInfoResponse {
    fn from(session: SessionMeta) -> Self {
        Self {
            id: session.session_id.to_string(),
            title: session.title,
            profile: session.model_profile,
            project_id: session.project_id.map(|project_id| project_id.to_string()),
            worktree_path: session.worktree_path,
            branch: session.branch,
            visibility: session.visibility.map(project_visibility_to_string),
        }
    }
}

impl From<MemoryEntry> for MemoryEntryResponse {
    fn from(e: MemoryEntry) -> Self {
        Self {
            id: e.id,
            scope: match e.scope {
                MemoryScope::User => "user".into(),
                MemoryScope::Workspace => "workspace".into(),
                MemoryScope::Session => "session".into(),
            },
            key: e.key,
            content: e.content,
            accepted: e.accepted,
        }
    }
}

// ---------------------------------------------------------------------------
// MCP response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpServerStatusResponse {
    pub id: String,
    pub status: agent_mcp::McpServerStatus,
    pub tool_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpToolDefResponse {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpResourceDefResponse {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpPromptDefResponse {
    pub name: String,
    pub description: Option<String>,
    pub argument_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContentBlockResponse {
    Text {
        text: String,
    },
    Image {
        data: String,
        mime_type: String,
    },
    Resource {
        uri: String,
        name: String,
        mime_type: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Original commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn list_profiles(state: State<'_, GuiState>) -> Result<Vec<String>, String> {
    Ok(state.config.read().unwrap().profile_names())
}

#[tauri::command]
#[specta::specta]
pub async fn get_profile_info(state: State<'_, GuiState>) -> Result<Vec<ProfileInfo>, String> {
    Ok(state.config.read().unwrap().profile_info())
}

#[tauri::command]
#[specta::specta]
pub async fn initialize_workspace(
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<WorkspaceInfoResponse, String> {
    // Prevent double initialization
    {
        let ws = state.workspace_id.lock().await;
        if ws.is_some() {
            return Err("Workspace already initialized".into());
        }
    }

    let workspace_path = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {e}"))?
        .display()
        .to_string();

    // Try to reuse an existing workspace for this path
    let workspace = {
        let workspaces = state
            .runtime
            .list_workspaces()
            .await
            .map_err(|e| format!("Failed to list workspaces: {e}"))?;
        if let Some(existing) = workspaces.iter().find(|w| w.path == workspace_path) {
            existing.clone()
        } else {
            state
                .runtime
                .open_workspace(workspace_path)
                .await
                .map_err(|e| format!("Failed to open workspace: {e}"))?
        }
    };

    let workspace_id = workspace.workspace_id.clone();
    let profile = state.config.read().unwrap().default_profile();

    // Try to restore an existing session, or create a new one
    let session_id = {
        let sessions = state
            .runtime
            .list_sessions(&workspace_id)
            .await
            .map_err(|e| format!("Failed to list sessions: {e}"))?;
        if let Some(last) = sessions.last() {
            last.session_id.clone()
        } else {
            state
                .runtime
                .start_session(agent_core::StartSessionRequest {
                    workspace_id: workspace_id.clone(),
                    model_profile: profile.clone(),
                })
                .await
                .map_err(|e| format!("Failed to start session: {e}"))?
        }
    };

    // Spawn event forwarder for all sessions
    {
        let mut handle = state.forwarder_handle.lock().await;
        *handle = Some(spawn_event_forwarder(&state.runtime, &app_handle));
    }

    // Store workspace and session info
    {
        let mut ws = state.workspace_id.lock().await;
        *ws = Some(workspace_id.clone());
    }
    {
        let mut current = state.current_session_id.lock().await;
        *current = Some(session_id.clone());
    }

    Ok(WorkspaceInfoResponse {
        workspace_id: workspace_id.to_string(),
        path: workspace.path,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn start_session(
    profile: String,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<SessionInfoResponse, String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };

    let session_id = state
        .runtime
        .start_session(agent_core::StartSessionRequest {
            workspace_id: workspace_id.clone(),
            model_profile: profile.clone(),
        })
        .await
        .map_err(|e| format!("Failed to start session: {e}"))?;

    let title = format!("Session using {profile}");

    // Switch to the new session (no forwarder respawn needed with subscribe_all)
    switch_session_inner(&state, session_id.clone(), &app_handle).await?;

    Ok(SessionInfoResponse {
        id: session_id.to_string(),
        title,
        profile,
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: None,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn send_message(
    content: String,
    attachments: Vec<agent_core::AttachmentInfo>,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current = state.current_session_id.lock().await;
        current.clone().ok_or("No active session")?
    };

    let enriched = enrich_content_with_attachments(&content, &attachments);

    let session_id_str = session_id.to_string();
    let runtime = state.runtime.clone();
    tokio::spawn(async move {
        let result = runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id,
                session_id,
                content: enriched,
                attachments,
            })
            .await;

        if let Err(e) = result {
            eprintln!("[commands] send_message failed: {e}");
            let payload = serde_json::json!({
                "type": "SendMessageError",
                "error": e.to_string(),
                "session_id": session_id_str
            });
            let _ = app_handle.emit("session-error", &payload);
        }
    });

    Ok(())
}

const MAX_TEXT_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
const MAX_IMAGE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB

/// Read attachment files and format their content into the message.
///
/// - Images: base64-encoded data URIs appended to the content.
/// - Text files: content wrapped in markdown code blocks with filename headers.
/// - Other binaries: filename reference only.
fn enrich_content_with_attachments(
    content: &str,
    attachments: &[agent_core::AttachmentInfo],
) -> String {
    let mut parts: Vec<String> = Vec::new();

    for att in attachments {
        let mime = att.mime_type.as_str();
        if mime.starts_with("image/") {
            match std::fs::metadata(&att.path) {
                Ok(meta) if meta.len() > MAX_IMAGE_BYTES => {
                    parts.push(format!("[image: {} (file too large, >50MB)]", att.name));
                    continue;
                }
                Err(e) => {
                    eprintln!("[commands] failed to stat image {}: {e}", att.path);
                    parts.push(format!("[image: {} (read error)]", att.name));
                    continue;
                }
                _ => {}
            }
            match std::fs::read(&att.path) {
                Ok(bytes) => {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    parts.push(format!("![{}](data:{};base64,{})", att.name, mime, b64));
                }
                Err(e) => {
                    eprintln!("[commands] failed to read image {}: {e}", att.path);
                    parts.push(format!("[image: {} (read error)]", att.name));
                }
            }
        } else if is_text_mime(mime) {
            match std::fs::metadata(&att.path) {
                Ok(meta) if meta.len() > MAX_TEXT_BYTES => {
                    parts.push(format!("[file: {} (file too large, >10MB)]", att.name));
                    continue;
                }
                Err(e) => {
                    eprintln!("[commands] failed to stat file {}: {e}", att.path);
                    parts.push(format!("[file: {} (read error)]", att.name));
                    continue;
                }
                _ => {}
            }
            match std::fs::read_to_string(&att.path) {
                Ok(text) => {
                    let ext = std::path::Path::new(&att.path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    parts.push(format!("```{}\n// file: {}\n{}\n```", ext, att.name, text));
                }
                Err(e) => {
                    eprintln!("[commands] failed to read file {}: {e}", att.path);
                    parts.push(format!("[file: {} (read error)]", att.name));
                }
            }
        } else {
            parts.push(format!("[attached file: {}]", att.name));
        }
    }

    if parts.is_empty() {
        content.to_string()
    } else if content.trim().is_empty() {
        parts.join("\n\n")
    } else {
        format!("{}\n\n{}", parts.join("\n\n"), content)
    }
}

fn is_text_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/xhtml+xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/toml"
                | "application/x-sh"
                | "application/x-shellscript"
        )
}

#[tauri::command]
pub async fn switch_session(
    session_id: String,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let sid: agent_core::SessionId = session_id.into();
    switch_session_inner(&state, sid.clone(), &app_handle).await?;

    let projection = state
        .runtime
        .get_session_projection(sid)
        .await
        .map_err(|e| format!("Failed to get session projection: {e}"))?;

    Ok(serde_json::to_value(&projection).unwrap_or_default())
}

/// Returns historical trace events for a session as a JSON array.
/// Used by the frontend to repopulate the trace panel when switching sessions.
#[tauri::command]
pub async fn get_trace(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<String>, String> {
    let sid: agent_core::SessionId = session_id.into();
    let trace = state
        .runtime
        .get_trace(sid)
        .await
        .map_err(|e| format!("Failed to get trace: {e}"))?;
    Ok(trace
        .into_iter()
        .filter_map(|entry| serde_json::to_string(&entry.event).ok())
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn list_sessions(state: State<'_, GuiState>) -> Result<Vec<SessionInfoResponse>, String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };

    let sessions = state
        .runtime
        .list_sessions(&workspace_id)
        .await
        .map_err(|e| format!("Failed to list sessions: {e}"))?;

    let current_session_id = state.current_session_id.lock().await;

    let mut result: Vec<SessionInfoResponse> = sessions
        .into_iter()
        .map(SessionInfoResponse::from)
        .collect();

    // Sort: current session first
    if let Some(ref current_id) = *current_session_id {
        let current_str = current_id.to_string();
        result.sort_by(|a, b| {
            if a.id == current_str {
                std::cmp::Ordering::Less
            } else if b.id == current_str {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
    }

    Ok(result)
}

async fn current_workspace_id(
    state: &State<'_, GuiState>,
) -> Result<agent_core::WorkspaceId, String> {
    let workspace_id = state.workspace_id.lock().await;
    workspace_id
        .clone()
        .ok_or_else(|| "Workspace not initialized".to_string())
}

fn project_visibility_to_string(visibility: ProjectSessionVisibility) -> String {
    match visibility {
        ProjectSessionVisibility::DraftHidden => "draft_hidden".into(),
        ProjectSessionVisibility::Visible => "visible".into(),
        ProjectSessionVisibility::Archived => "archived".into(),
    }
}

fn project_git_status_kind_to_string(kind: ProjectGitStatusKind) -> String {
    match kind {
        ProjectGitStatusKind::NotInitialized => "not_initialized".into(),
        ProjectGitStatusKind::Clean => "clean".into(),
        ProjectGitStatusKind::Dirty => "dirty".into(),
        ProjectGitStatusKind::Detached => "detached".into(),
        ProjectGitStatusKind::MissingPath => "missing_path".into(),
        ProjectGitStatusKind::Error => "error".into(),
    }
}

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

#[tauri::command]
#[specta::specta]
pub async fn resolve_permission(
    state: State<'_, GuiState>,
    request_id: String,
    decision: String,
    reason: Option<String>,
) -> Result<(), String> {
    let perm_decision = match decision.as_str() {
        "grant" => PermissionDecision {
            request_id: request_id.clone(),
            approve: true,
            reason: None,
        },
        "deny" => PermissionDecision {
            request_id: request_id.clone(),
            approve: false,
            reason: reason.or_else(|| Some("User denied".into())),
        },
        _ => return Err("Invalid decision: must be 'grant' or 'deny'".into()),
    };
    state
        .runtime
        .resolve_permission(&request_id, perm_decision)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn query_memories(
    state: State<'_, GuiState>,
    scope: Option<String>,
    keywords: Option<Vec<String>>,
    limit: Option<usize>,
) -> Result<Vec<MemoryEntryResponse>, String> {
    let scope = scope.map(|s| match s.as_str() {
        "user" => MemoryScope::User,
        "workspace" => MemoryScope::Workspace,
        _ => MemoryScope::Session,
    });
    let entries = state
        .memory_store
        .query(MemoryQuery {
            scope,
            keywords: keywords.unwrap_or_default(),
            limit: limit.unwrap_or(50),
            session_id: None,
            workspace_id: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(MemoryEntryResponse::from).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_memory(state: State<'_, GuiState>, id: String) -> Result<(), String> {
    state
        .memory_store
        .delete(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_workspaces(
    state: State<'_, GuiState>,
) -> Result<Vec<WorkspaceInfoResponse>, String> {
    let workspaces = state
        .runtime
        .list_workspaces()
        .await
        .map_err(|e| format!("Failed to list workspaces: {e}"))?;
    Ok(workspaces
        .into_iter()
        .map(|w| WorkspaceInfoResponse {
            workspace_id: w.workspace_id.to_string(),
            path: w.path,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn rename_session(
    session_id: String,
    title: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .rename_session(&sid, title)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_session(session_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .soft_delete_session(&sid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_session(state: State<'_, GuiState>) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current = state.current_session_id.lock().await;
        current.clone().ok_or("No active session")?
    };
    state
        .runtime
        .cancel_session(workspace_id, session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_permission_mode(state: State<'_, GuiState>) -> Result<String, String> {
    Ok(format!("{:?}", state.runtime.permission_mode().await))
}

#[tauri::command]
#[specta::specta]
pub async fn get_profile_detail(
    profile: String,
    state: State<'_, GuiState>,
) -> Result<ProfileDetailResponse, String> {
    let info = state
        .config
        .read()
        .unwrap()
        .profile_info()
        .into_iter()
        .find(|p| p.alias == profile)
        .ok_or_else(|| format!("Profile '{}' not found", profile))?;
    Ok(ProfileDetailResponse {
        alias: info.alias,
        provider: info.provider,
        model_id: info.model_id,
        local: info.local,
        has_api_key: info.has_api_key,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn get_task_graph(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<TaskSnapshotResponse>, String> {
    let sid: agent_core::SessionId = session_id.into();
    let snapshot = state
        .runtime
        .get_task_graph(sid)
        .await
        .map_err(|e| format!("Failed to get task graph: {e}"))?;
    Ok(snapshot
        .tasks
        .into_iter()
        .map(|t| TaskSnapshotResponse {
            id: t.id.to_string(),
            title: t.title,
            role: format!("{:?}", t.role),
            state: format!("{:?}", t.state),
            dependencies: t.dependencies.iter().map(|d| d.to_string()).collect(),
            error: t.error,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn restore_workspace(
    workspace_id: String,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let wid: agent_core::WorkspaceId = workspace_id.into();
    {
        let mut ws = state.workspace_id.lock().await;
        *ws = Some(wid);
    }

    // Spawn event forwarder if not already running.
    // This is needed because restore_workspace is called on app restart
    // (via recoverSessions), which bypasses initialize_workspace where
    // the forwarder is normally started.
    {
        let handle = state.forwarder_handle.lock().await;
        if handle.is_none() {
            drop(handle); // Release lock before spawning
            let mut handle = state.forwarder_handle.lock().await;
            if handle.is_none() {
                *handle = Some(spawn_event_forwarder(&state.runtime, &app_handle));
            }
        }
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_build_info() -> BuildInfoResponse {
    let info = agent_core::build_info::BuildInfo::from_env();
    BuildInfoResponse {
        version: info.version.to_string(),
        git_hash: info.git_hash.to_string(),
        build_time: info.build_time.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Skill commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn list_skills(state: State<'_, GuiState>) -> Result<Vec<agent_core::SkillView>, String> {
    state
        .runtime
        .list_skills()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_skill_detail(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<agent_core::SkillDetail, String> {
    state
        .runtime
        .get_skill(skill_id.clone())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Skill not found: {skill_id}"))
}

#[tauri::command]
#[specta::specta]
pub async fn activate_skill(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<agent_core::ActiveSkillView, String> {
    let workspace_id = {
        let workspace_id = state.workspace_id.lock().await;
        workspace_id.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current_session_id = state.current_session_id.lock().await;
        current_session_id.clone().ok_or("No active session")?
    };

    state
        .runtime
        .activate_skill(agent_core::ActivateSkillRequest {
            workspace_id,
            session_id,
            skill_id,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn deactivate_skill(state: State<'_, GuiState>, skill_id: String) -> Result<(), String> {
    let workspace_id = {
        let workspace_id = state.workspace_id.lock().await;
        workspace_id.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current_session_id = state.current_session_id.lock().await;
        current_session_id.clone().ok_or("No active session")?
    };

    state
        .runtime
        .deactivate_skill(agent_core::DeactivateSkillRequest {
            workspace_id,
            session_id,
            skill_id,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_active_skills(
    state: State<'_, GuiState>,
) -> Result<Vec<agent_core::ActiveSkillView>, String> {
    let session_id = {
        let current_session_id = state.current_session_id.lock().await;
        current_session_id.clone().ok_or("No active session")?
    };

    state
        .runtime
        .list_active_skills(session_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_mcp_server_settings(
    state: State<'_, GuiState>,
) -> Result<Vec<McpServerSettingsView>, String> {
    state
        .runtime
        .list_mcp_server_settings()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_mcp_server_settings(
    state: State<'_, GuiState>,
    input: McpServerSettingsInput,
) -> Result<McpServerSettingsView, String> {
    state
        .runtime
        .upsert_mcp_server_settings(input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_mcp_server_enabled(
    state: State<'_, GuiState>,
    server_id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_mcp_server_enabled(server_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_mcp_server_settings(
    state: State<'_, GuiState>,
    server_id: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_mcp_server_settings(server_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn open_mcp_config_file(state: State<'_, GuiState>) -> Result<Option<String>, String> {
    let Some(config_file_path) = state
        .runtime
        .open_mcp_config_file()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };

    let config_file_path = std::path::PathBuf::from(config_file_path);
    let config_folder_path = config_file_path
        .parent()
        .unwrap_or(config_file_path.as_path())
        .to_path_buf();

    open_path_in_system_file_manager(&config_folder_path)?;
    Ok(Some(config_folder_path.display().to_string()))
}

fn open_path_in_system_file_manager(path: &std::path::Path) -> Result<(), String> {
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

#[tauri::command]
#[specta::specta]
pub async fn list_skill_settings(
    state: State<'_, GuiState>,
) -> Result<Vec<SkillSettingsView>, String> {
    state
        .runtime
        .list_skill_settings()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_skill_settings_detail(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<SkillSettingsDetail, String> {
    state
        .runtime
        .get_skill_settings_detail(skill_id.clone())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Skill not found: {skill_id}"))
}

#[tauri::command]
#[specta::specta]
pub async fn set_skill_enabled(
    state: State<'_, GuiState>,
    skill_id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_skill_enabled(skill_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_skill_settings(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_skill_settings(skill_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn search_remote_skills(
    state: State<'_, GuiState>,
    query: String,
) -> Result<Vec<RemoteSkillSearchResult>, String> {
    state
        .runtime
        .search_remote_skills(query)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_remote_skill(
    state: State<'_, GuiState>,
    request: InstallRemoteSkillRequest,
) -> Result<SkillSettingsView, String> {
    state
        .runtime
        .install_remote_skill(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_github_skill(
    state: State<'_, GuiState>,
    request: InstallGithubSkillRequest,
) -> Result<SkillSettingsView, String> {
    state
        .runtime
        .install_github_skill(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_skill(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<SkillSettingsView, String> {
    state
        .runtime
        .update_skill(skill_id)
        .await
        .map_err(|error| error.to_string())
}

// ── Skill catalog ────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn list_skill_catalog(
    state: State<'_, GuiState>,
    query: SkillCatalogQuery,
) -> Result<Vec<SkillCatalogEntry>, String> {
    state
        .runtime
        .list_skill_catalog(query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_skill_sources(
    state: State<'_, GuiState>,
) -> Result<Vec<SkillSourceView>, String> {
    state
        .runtime
        .list_skill_sources()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn add_skill_source(
    state: State<'_, GuiState>,
    config: SkillSourceView,
) -> Result<(), String> {
    state
        .runtime
        .add_skill_source(config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_skill_source(state: State<'_, GuiState>, id: String) -> Result<(), String> {
    state
        .runtime
        .remove_skill_source(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_skill_source_enabled(
    state: State<'_, GuiState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_skill_source_enabled(id, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_skill_catalog(state: State<'_, GuiState>) -> Result<(), String> {
    state
        .runtime
        .refresh_skill_catalog()
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// MCP commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn list_mcp_servers(
    state: State<'_, GuiState>,
) -> Result<Vec<McpServerStatusResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            Ok(manager
                .server_statuses()
                .into_iter()
                .map(|(id, status)| McpServerStatusResponse {
                    id,
                    status,
                    tool_count: None,
                })
                .collect())
        }
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn start_mcp_server(server_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let mut manager = manager.lock().await;
            manager
                .ensure_server(&server_id)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn stop_mcp_server(server_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let mut manager = manager.lock().await;
            manager
                .shutdown_server(&server_id)
                .await
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_mcp_tools(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<McpToolDefResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let mut manager = manager.lock().await;
            manager
                .refresh_tools(&server_id)
                .await
                .map(|tools| {
                    tools
                        .into_iter()
                        .map(|t| McpToolDefResponse {
                            name: t.name,
                            description: t.description,
                            input_schema: t.input_schema,
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn trust_mcp_server(server_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .trust_server(&server_id)
                .await
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn revoke_mcp_trust(server_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .revoke_trust(&server_id)
                .await
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn list_mcp_resources(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<McpResourceDefResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .list_resources(&server_id)
                .await
                .map(|r| {
                    r.into_iter()
                        .map(|r| McpResourceDefResponse {
                            uri: r.uri,
                            name: r.name,
                            description: r.description,
                            mime_type: r.mime_type,
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn list_mcp_prompts(
    server_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<McpPromptDefResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .list_prompts(&server_id)
                .await
                .map(|p| {
                    p.into_iter()
                        .map(|p| McpPromptDefResponse {
                            name: p.name,
                            description: p.description,
                            argument_count: p.arguments.len(),
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn read_mcp_resource(
    server_id: String,
    uri: String,
    state: State<'_, GuiState>,
) -> Result<Vec<McpContentBlockResponse>, String> {
    let runtime = state.runtime.clone();
    match runtime.mcp_manager() {
        Some(manager) => {
            let manager = manager.lock().await;
            manager
                .read_resource(&server_id, &uri)
                .await
                .map(|blocks| {
                    blocks
                        .into_iter()
                        .map(|b| match b {
                            agent_mcp::McpContentBlock::Text { text } => {
                                McpContentBlockResponse::Text { text }
                            }
                            agent_mcp::McpContentBlock::Image { data, mime_type } => {
                                McpContentBlockResponse::Image { data, mime_type }
                            }
                            agent_mcp::McpContentBlock::Resource { resource } => {
                                McpContentBlockResponse::Resource {
                                    uri: resource.uri,
                                    name: String::new(),
                                    mime_type: resource.mime_type,
                                }
                            }
                        })
                        .collect()
                })
                .map_err(|e| e.to_string())
        }
        None => Err("No MCP servers configured".into()),
    }
}

/// Inner helper: update current session.
/// No forwarder respawning needed since we use subscribe_all().
async fn switch_session_inner(
    state: &GuiState,
    session_id: agent_core::SessionId,
    _app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    // Update current session
    {
        let mut current = state.current_session_id.lock().await;
        *current = Some(session_id);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Marketplace (catalog) response & request DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct CatalogQueryRequest {
    pub keyword: Option<String>,
    pub category: Option<String>,
    /// "unverified" | "community" | "verified"
    pub trust_min: Option<String>,
    pub source: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ServerEntryResponse {
    pub id: String,
    pub source: String,
    pub display_name: String,
    pub summary: String,
    pub description: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    /// Lower-case trust level: "unverified" | "community" | "verified".
    pub trust: String,
    pub icon: Option<String>,
    /// JSON-encoded `agent_mcp::catalog::InstallSpec`.
    pub install_spec_json: String,
    /// JSON-encoded `Vec<agent_mcp::catalog::RuntimeRequirement>`.
    pub requirements_json: String,
    /// JSON-encoded `Vec<agent_mcp::catalog::EnvVarSpec>`.
    pub default_env_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstallRequestPayload {
    pub catalog_id: String,
    pub source: String,
    pub server_id_override: Option<String>,
    pub env_overrides: std::collections::BTreeMap<String, String>,
    pub trust_grant: bool,
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstallOutcomeResponse {
    /// "installed" | "runtime_missing" | "already_installed" | "invalid_env"
    pub kind: String,
    pub server_id: Option<String>,
    pub started: Option<bool>,
    pub missing_runtimes: Vec<String>,
    pub missing_env_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstalledEntryResponse {
    pub server_id: String,
    pub catalog_id: Option<String>,
    pub source: Option<String>,
    pub display_name: String,
    pub installed_at: String,
    pub running: bool,
}

// ---------------------------------------------------------------------------
// Marketplace commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn list_catalog(
    state: State<'_, GuiState>,
    query: Option<CatalogQueryRequest>,
) -> Result<Vec<ServerEntryResponse>, String> {
    let q = into_core_query(query.unwrap_or_default());
    let entries = state
        .runtime
        .list_catalog(q)
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(into_response_entry).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn get_catalog_entry(
    state: State<'_, GuiState>,
    id: String,
    source: Option<String>,
) -> Result<Option<ServerEntryResponse>, String> {
    let e = state
        .runtime
        .get_catalog_entry(id, source)
        .await
        .map_err(|e| e.to_string())?;
    Ok(e.map(into_response_entry))
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_catalog(
    state: State<'_, GuiState>,
    source: Option<String>,
) -> Result<(), String> {
    state
        .runtime
        .refresh_catalog(source)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_catalog_entry(
    state: State<'_, GuiState>,
    request: InstallRequestPayload,
) -> Result<InstallOutcomeResponse, String> {
    let outcome = state
        .runtime
        .install_catalog_entry(into_core_install_request(request))
        .await
        .map_err(|e| e.to_string())?;
    Ok(into_response_outcome(outcome))
}

#[tauri::command]
#[specta::specta]
pub async fn uninstall_catalog_entry(
    state: State<'_, GuiState>,
    server_id: String,
) -> Result<(), String> {
    state
        .runtime
        .uninstall_catalog_entry(server_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_installed_entries(
    state: State<'_, GuiState>,
) -> Result<Vec<InstalledEntryResponse>, String> {
    let v = state
        .runtime
        .list_installed_entries()
        .await
        .map_err(|e| e.to_string())?;
    Ok(v.into_iter()
        .map(|e| InstalledEntryResponse {
            server_id: e.server_id,
            catalog_id: e.catalog_id,
            source: e.source,
            display_name: e.display_name,
            installed_at: e.installed_at,
            running: e.running,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Marketplace helper conversions
// ---------------------------------------------------------------------------

fn into_core_query(q: CatalogQueryRequest) -> agent_core::CatalogQuery {
    agent_core::CatalogQuery {
        keyword: q.keyword,
        category: q.category,
        trust_min: q.trust_min,
        source: q.source,
        limit: q.limit,
    }
}

fn into_response_entry(e: agent_core::ServerEntry) -> ServerEntryResponse {
    ServerEntryResponse {
        id: e.id,
        source: e.source,
        display_name: e.display_name,
        summary: e.summary,
        description: e.description,
        categories: e.categories,
        tags: e.tags,
        author: e.author,
        homepage: e.homepage,
        version: e.version,
        trust: e.trust,
        icon: e.icon,
        install_spec_json: e.install_spec_json,
        requirements_json: e.requirements_json,
        default_env_json: e.default_env_json,
    }
}

fn into_core_install_request(p: InstallRequestPayload) -> agent_core::InstallRequest {
    agent_core::InstallRequest {
        catalog_id: p.catalog_id,
        source: p.source,
        server_id_override: p.server_id_override,
        env_overrides: p.env_overrides,
        trust_grant: p.trust_grant,
        auto_start: p.auto_start,
    }
}

fn into_response_outcome(o: agent_core::InstallOutcomeView) -> InstallOutcomeResponse {
    InstallOutcomeResponse {
        kind: o.kind,
        server_id: o.server_id,
        started: o.started,
        missing_runtimes: o.missing_runtimes,
        missing_env_keys: o.missing_env_keys,
    }
}

#[cfg(test)]
mod marketplace_command_tests {
    use super::*;

    #[test]
    fn install_outcome_response_serializes_kind_string() {
        let r = InstallOutcomeResponse {
            kind: "installed".into(),
            server_id: Some("filesystem".into()),
            started: Some(true),
            missing_runtimes: vec![],
            missing_env_keys: vec![],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"installed\""));
        assert!(json.contains("\"server_id\":\"filesystem\""));
    }

    #[test]
    fn catalog_query_request_default_is_all_none() {
        let q = CatalogQueryRequest::default();
        assert!(q.keyword.is_none() && q.category.is_none() && q.trust_min.is_none());
    }
}

// ---------------------------------------------------------------------------
// Phase 2: catalog source commands + types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CatalogSourceViewResponse {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: u32,
    pub default_trust: String,
    pub enabled: bool,
    pub cache_ttl_seconds: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AddCatalogSourceRequestPayload {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: Option<u32>,
    pub default_trust: Option<String>,
    pub enabled: Option<bool>,
    pub cache_ttl_seconds: Option<u64>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_catalog_sources(
    state: State<'_, GuiState>,
) -> Result<Vec<CatalogSourceViewResponse>, String> {
    let v = state
        .runtime
        .list_catalog_sources()
        .await
        .map_err(|e| e.to_string())?;
    Ok(v.into_iter().map(into_source_view_response).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn add_catalog_source(
    state: State<'_, GuiState>,
    request: AddCatalogSourceRequestPayload,
) -> Result<(), String> {
    state
        .runtime
        .add_catalog_source(into_core_add_catalog_source_request(request))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_catalog_source(state: State<'_, GuiState>, id: String) -> Result<(), String> {
    state
        .runtime
        .remove_catalog_source(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_catalog_source_enabled(
    state: State<'_, GuiState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_catalog_source_enabled(id, enabled)
        .await
        .map_err(|e| e.to_string())
}

fn into_source_view_response(s: agent_core::CatalogSourceView) -> CatalogSourceViewResponse {
    CatalogSourceViewResponse {
        id: s.id,
        display_name: s.display_name,
        kind: s.kind,
        url: s.url,
        api_key_env: s.api_key_env,
        priority: s.priority,
        default_trust: s.default_trust,
        enabled: s.enabled,
        cache_ttl_seconds: s.cache_ttl_seconds,
        last_error: s.last_error,
    }
}

fn into_core_add_catalog_source_request(
    p: AddCatalogSourceRequestPayload,
) -> agent_core::AddCatalogSourceRequest {
    agent_core::AddCatalogSourceRequest {
        id: p.id,
        display_name: p.display_name,
        kind: p.kind,
        url: p.url,
        api_key_env: p.api_key_env,
        priority: p.priority,
        default_trust: p.default_trust,
        enabled: p.enabled,
        cache_ttl_seconds: p.cache_ttl_seconds,
    }
}

#[cfg(test)]
mod catalog_sources_command_tests {
    use super::*;

    #[test]
    fn source_view_response_serializes_kind_and_last_error() {
        let r = CatalogSourceViewResponse {
            id: "smithery".into(),
            display_name: "Smithery".into(),
            kind: "smithery".into(),
            url: "https://x".into(),
            api_key_env: None,
            priority: 50,
            default_trust: "community".into(),
            enabled: true,
            cache_ttl_seconds: None,
            last_error: Some("timeout".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"smithery\""));
        assert!(json.contains("\"last_error\":\"timeout\""));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileWithLimits {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub context_window: u64,
    pub output_limit: u64,
    /// Snake-case `LimitSource`: "user_config" | "builtin_registry" | "runtime_probe" | "fallback".
    pub limit_source: String,
    pub has_api_key: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_config_for_project(
    project_root: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let path = std::path::Path::new(&project_root);
    state.refresh_config_for_project(path)?;
    eprintln!(
        "Config refreshed for project: profiles={:?}",
        state.config.read().unwrap().profile_names()
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_profiles_with_limits(
    state: State<'_, GuiState>,
) -> Result<Vec<ProfileWithLimits>, String> {
    let config = state.config.read().unwrap();
    let mut out = Vec::with_capacity(config.profiles.len());
    for (alias, profile) in &config.profiles {
        let limits = agent_config::resolve_limits(profile);
        let limit_source = match limits.source {
            agent_models::LimitSource::UserConfig => "user_config",
            agent_models::LimitSource::BuiltinRegistry => "builtin_registry",
            agent_models::LimitSource::RuntimeProbe => "runtime_probe",
            agent_models::LimitSource::Fallback => "fallback",
        };
        let has_api_key = profile.api_key.is_some()
            || profile
                .api_key_env
                .as_deref()
                .map(|env| std::env::var(env).is_ok())
                .unwrap_or(false)
            || matches!(profile.provider.as_str(), "ollama" | "fake");
        out.push(ProfileWithLimits {
            alias: alias.clone(),
            provider: profile.provider.clone(),
            model_id: profile.model_id.clone(),
            context_window: limits.context_window,
            output_limit: limits.output_limit,
            limit_source: limit_source.into(),
            has_api_key,
        });
    }
    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub async fn compact_session(state: State<'_, GuiState>) -> Result<(), String> {
    let session_id = {
        let current = state.current_session_id.lock().await;
        current
            .clone()
            .ok_or_else(|| "No active session to compact".to_string())?
    };

    state
        .runtime
        .compact_session(session_id, agent_core::CompactionReason::UserRequested)
        .await
        .map_err(|e| e.to_string())
}

/// P4: swap the active model profile for the current session.
///
/// The switch takes effect at the next `send_message` — in-flight
/// streams keep using the old profile end-to-end. Returns an error
/// when the alias is unknown or the session is currently compacting.
#[tauri::command]
#[specta::specta]
pub async fn switch_model(
    state: State<'_, GuiState>,
    session_id: String,
    profile_alias: String,
) -> Result<(), String> {
    let session_id = agent_core::SessionId::from_string(session_id);
    state
        .runtime
        .switch_model(session_id, profile_alias)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod compact_session_command_tests {
    use super::compact_session;

    #[test]
    fn compact_session_command_function_exists() {
        // Compile-time presence check — if `compact_session` is renamed or
        // removed this fails to compile, which is exactly the signal we want
        // before `collect_commands![]` / `generate_handler![]` blow up.
        let _ = compact_session;
    }
}

#[cfg(test)]
mod switch_model_command_tests {
    use super::switch_model;

    #[test]
    fn switch_model_command_function_exists() {
        // Compile-time presence check — if `switch_model` is renamed or
        // removed this fails to compile before `collect_commands!` /
        // `generate_handler!` get a chance to blow up at runtime.
        let _ = switch_model;
    }
}

#[cfg(test)]
mod profile_with_limits_tests {
    use super::*;

    #[test]
    fn profile_with_limits_serializes_expected_shape() {
        let p = ProfileWithLimits {
            alias: "fast".into(),
            provider: "openai".into(),
            model_id: "gpt-4o-mini".into(),
            context_window: 128_000,
            output_limit: 16_384,
            limit_source: "builtin_registry".into(),
            has_api_key: true,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"alias\":\"fast\""));
        assert!(json.contains("\"context_window\":128000"));
        assert!(json.contains("\"limit_source\":\"builtin_registry\""));
        assert!(json.contains("\"has_api_key\":true"));
    }
}
