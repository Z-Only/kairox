use crate::app_state::GuiState;
use crate::event_forwarder::spawn_event_forwarder;
use agent_config::ProfileInfo;
use agent_core::facade::{
    EffectiveMcpServerView, EffectiveProfileView, EffectiveSkillView, InstallGithubSkillRequest,
    InstallRemoteSkillRequest, McpServerSettingsInput, McpServerSettingsView, ProfileSettingsInput,
    ProfileSettingsView, RemoteSkillSearchResult, SkillCatalogEntry, SkillCatalogQuery,
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
    pub permission_mode: Option<String>,
    pub project_id: Option<String>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub visibility: Option<String>,
    pub deleted_at: Option<String>,
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
    #[specta(type = i32)]
    pub sort_order: i64,
    pub expanded: bool,
    pub path_exists: bool,
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
    pub contents: Option<String>,
    pub warning: Option<String>,
}

impl From<ProjectMeta> for ProjectInfoResponse {
    fn from(project: ProjectMeta) -> Self {
        let path_exists = std::path::Path::new(&project.root_path).exists();
        Self {
            project_id: project.project_id.to_string(),
            display_name: project.display_name,
            root_path: project.root_path,
            removed_at: project.removed_at,
            sort_order: project.sort_order,
            expanded: project.expanded,
            path_exists,
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
            contents: summary.contents,
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
            permission_mode: session.permission_mode,
            project_id: session.project_id.map(|project_id| project_id.to_string()),
            worktree_path: session.worktree_path,
            branch: session.branch,
            visibility: session.visibility.map(project_visibility_to_string),
            deleted_at: session.deleted_at,
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{SessionId, WorkspaceId};

    #[test]
    fn session_info_response_exposes_deleted_at_for_archive_display() {
        let response = SessionInfoResponse::from(SessionMeta {
            project_id: None,
            worktree_path: None,
            branch: None,
            visibility: None,
            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
            session_id: SessionId::from_string("ses_archived".to_string()),
            workspace_id: WorkspaceId::from_string("wrk_default".to_string()),
            title: "Archived task".into(),
            model_profile: "default".into(),
            model_id: None,
            provider: None,
            deleted_at: Some("2026-01-02T03:04:05Z".into()),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-02T03:04:05Z".into(),
        });

        assert_eq!(response.deleted_at.as_deref(), Some("2026-01-02T03:04:05Z"));
    }
}

// ---------------------------------------------------------------------------
// MCP response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpServerStatusResponse {
    pub id: String,
    pub status: agent_mcp::McpServerStatus,
    #[specta(type = u32)]
    pub tool_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpToolDefResponse {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CheckMcpHealthResponse {
    pub tools: Vec<McpToolDefResponse>,
    pub healthy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpToolStatesResponse {
    pub disabled_tools: Vec<String>,
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
    #[specta(type = u32)]
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

mod chat;
mod draft;
mod marketplace;
mod plugins;
mod project;
mod session;
mod settings;
mod skills;

pub use chat::*;
pub use draft::*;
pub use marketplace::*;
pub use plugins::*;
pub use project::*;
pub use session::*;
pub use settings::*;
pub use skills::*;

// ---------------------------------------------------------------------------
// Shared command helpers
// ---------------------------------------------------------------------------

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

/// Inner helper: update current session.
/// Restores the session's permission mode from stored metadata.
/// No forwarder respawning needed since we use subscribe_all().
async fn switch_session_inner(
    state: &GuiState,
    session_id: agent_core::SessionId,
    _app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    // Restore permission mode from session metadata before switching
    {
        let workspace_id = {
            let ws = state.workspace_id.lock().await;
            ws.clone().ok_or("Workspace not initialized")?
        };
        let sessions = state
            .runtime
            .list_sessions(&workspace_id)
            .await
            .map_err(|e| format!("Failed to list sessions: {e}"))?;
        if let Some(session) = sessions.iter().find(|s| s.session_id == session_id) {
            if let Some(ref mode_str) = session.permission_mode {
                if let Ok(mode) = mode_str.parse::<agent_tools::PermissionMode>() {
                    state.runtime.set_permission_mode(mode).await;
                }
            }
        }
    }

    // Update current session
    {
        let mut current = state.current_session_id.lock().await;
        *current = Some(session_id);
    }

    Ok(())
}
