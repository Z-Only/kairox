//! Project DTOs and management sub-trait.

use crate::{ProjectId, SessionId, WorkspaceId};
use serde::{Deserialize, Serialize};

use super::SessionMeta;
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ProjectSessionVisibility {
    DraftHidden,
    Visible,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ProjectGitStatusKind {
    NotInitialized,
    Clean,
    Dirty,
    Detached,
    MissingPath,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectMeta {
    pub project_id: ProjectId,
    pub display_name: String,
    pub root_path: String,
    pub created_at: String,
    pub updated_at: String,
    pub removed_at: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub sort_order: i64,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectSessionBinding {
    pub session_id: SessionId,
    pub project_id: ProjectId,
    pub worktree_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectGitStatus {
    pub kind: ProjectGitStatusKind,
    pub branch: Option<String>,
    pub worktree_path: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectGitDiffSection {
    pub label: String,
    pub stat: String,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectGitReview {
    pub kind: ProjectGitStatusKind,
    pub branch: Option<String>,
    pub worktree_path: String,
    pub message: Option<String>,
    pub changed_files: Vec<String>,
    pub staged: Option<ProjectGitDiffSection>,
    pub unstaged: Option<ProjectGitDiffSection>,
    pub untracked: Option<ProjectGitDiffSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectInstructionSummary {
    pub source_paths: Vec<String>,
    pub contents: Option<String>,
    pub warning: Option<String>,
}

#[async_trait]
pub trait ProjectFacade: Send + Sync {
    async fn list_projects(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<ProjectMeta>> {
        let _ = workspace_id;
        Ok(Vec::new())
    }

    async fn create_blank_project(
        &self,
        workspace_id: WorkspaceId,
        display_name: Option<String>,
    ) -> crate::Result<ProjectMeta> {
        let _ = workspace_id;
        let _ = display_name;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn add_existing_project(
        &self,
        workspace_id: WorkspaceId,
        path: String,
    ) -> crate::Result<ProjectMeta> {
        let _ = workspace_id;
        let _ = path;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn rename_project(
        &self,
        project_id: ProjectId,
        display_name: String,
    ) -> crate::Result<()> {
        let _ = project_id;
        let _ = display_name;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn remove_project(&self, project_id: ProjectId) -> crate::Result<()> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn restore_project_session(&self, session_id: SessionId) -> crate::Result<ProjectMeta> {
        let _ = session_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn update_project_order(&self, project_ids: Vec<ProjectId>) -> crate::Result<()> {
        let _ = project_ids;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn update_project_expanded(
        &self,
        project_id: ProjectId,
        expanded: bool,
    ) -> crate::Result<()> {
        let _ = project_id;
        let _ = expanded;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn create_project_draft_session(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<SessionId> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn create_project_worktree_session(
        &self,
        project_id: ProjectId,
        branch_name: String,
    ) -> crate::Result<SessionId> {
        let _ = project_id;
        let _ = branch_name;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn list_project_branches(&self, project_id: ProjectId) -> crate::Result<Vec<String>> {
        let _ = project_id;
        Ok(Vec::new())
    }

    async fn list_project_sessions(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<Vec<SessionMeta>> {
        let _ = project_id;
        Ok(Vec::new())
    }

    async fn list_archived_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> crate::Result<Vec<SessionMeta>> {
        let _ = workspace_id;
        Ok(Vec::new())
    }

    async fn get_project_git_status(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectGitStatus> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn get_session_git_status(
        &self,
        session_id: SessionId,
    ) -> crate::Result<ProjectGitStatus> {
        let _ = session_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn get_project_git_review(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectGitReview> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn get_session_git_review(
        &self,
        session_id: SessionId,
    ) -> crate::Result<ProjectGitReview> {
        let _ = session_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn init_project_git(&self, project_id: ProjectId) -> crate::Result<ProjectGitStatus> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }

    async fn get_project_instruction_summary(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectInstructionSummary> {
        let _ = project_id;
        Err(crate::CoreError::InvalidState(
            "project support is not implemented".into(),
        ))
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
