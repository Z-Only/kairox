use crate::facade_runtime::LocalRuntime;
use agent_core::{
    ProjectFacade, ProjectGitStatus, ProjectGitStatusKind, ProjectId, ProjectInstructionSummary,
    ProjectMeta, SessionId, SessionMeta, WorkspaceId,
};
use agent_store::EventStore;
use async_trait::async_trait;
use std::process::Command;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_projects(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<ProjectMeta>> {
        let repository = self.project_repository()?;
        let rows = repository
            .list_active_projects(workspace_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(crate::project::project_row_to_meta)
            .collect())
    }

    pub(crate) async fn create_blank_project(
        &self,
        workspace_id: WorkspaceId,
        display_name: Option<String>,
    ) -> agent_core::Result<ProjectMeta> {
        let repository = self.project_repository()?;
        let display_name = display_name.unwrap_or_else(|| "New Project".into());
        let root_path = crate::project::unique_blank_project_path(&display_name);
        tokio::fs::create_dir_all(&root_path)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let root_path_string = root_path.display().to_string();
        let git_init_output = Command::new("git")
            .args(["-C", &root_path_string, "init"])
            .output()
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        if !git_init_output.status.success() {
            let stderr = String::from_utf8_lossy(&git_init_output.stderr)
                .trim()
                .to_string();
            let stdout = String::from_utf8_lossy(&git_init_output.stdout)
                .trim()
                .to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(agent_core::CoreError::InvalidState(format!(
                "git init failed: {message}"
            )));
        }

        let project = repository
            .create_project(workspace_id.as_str(), &display_name, &root_path_string, 0)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(crate::project::project_row_to_meta(project))
    }

    pub(crate) async fn add_existing_project(
        &self,
        workspace_id: WorkspaceId,
        path: String,
    ) -> agent_core::Result<ProjectMeta> {
        let repository = self.project_repository()?;
        let display_name = crate::project::display_name_from_path(&path);
        let project = repository
            .create_project(workspace_id.as_str(), &display_name, &path, 0)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(crate::project::project_row_to_meta(project))
    }

    pub(crate) async fn rename_project(
        &self,
        project_id: ProjectId,
        display_name: String,
    ) -> agent_core::Result<()> {
        self.project_repository()?
            .rename_project(project_id.as_str(), &display_name)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    pub(crate) async fn remove_project(&self, project_id: ProjectId) -> agent_core::Result<()> {
        let archived_session_ids: Vec<SessionId> = self
            .store
            .list_visible_project_sessions(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
            .into_iter()
            .map(|session| SessionId::from_string(session.session_id))
            .collect();

        for session_id in &archived_session_ids {
            self.session_execution.shutdown_session(session_id).await?;
        }

        self.project_repository()?
            .remove_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    pub(crate) async fn restore_project_session(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<ProjectMeta> {
        let repository = self.project_repository()?;
        let binding = repository
            .get_session_binding(session_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState("session is not bound to a project".into())
            })?;
        let project = repository
            .restore_project(&binding.project_id)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        repository
            .set_session_visibility(session_id.as_str(), "visible")
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        self.session_execution.ensure_session(&session_id).await;
        Ok(crate::project::project_row_to_meta(project))
    }

    pub(crate) async fn update_project_order(
        &self,
        project_ids: Vec<ProjectId>,
    ) -> agent_core::Result<()> {
        let project_id_strings: Vec<String> = project_ids
            .into_iter()
            .map(|project_id| project_id.to_string())
            .collect();
        self.project_repository()?
            .update_project_order(&project_id_strings)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    pub(crate) async fn update_project_expanded(
        &self,
        project_id: ProjectId,
        expanded: bool,
    ) -> agent_core::Result<()> {
        self.project_repository()?
            .update_project_expanded(project_id.as_str(), expanded)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    pub(crate) async fn create_project_draft_session(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<SessionId> {
        let repository = self.project_repository()?;
        let project = repository
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        self.start_lsp_servers(&crate::lsp_manager::file_uri_from_path(&project.root_path))
            .await;
        let model_profile = self.config.default_profile();
        let session_id = crate::session::start_session(
            &*self.store,
            &self.event_tx,
            WorkspaceId::from_string(project.workspace_id.clone()),
            model_profile.clone(),
            None,
            None,
        )
        .await?;
        self.initialize_session_limits(&session_id, &model_profile)
            .await;
        let git_status = crate::project::get_git_status(&project.root_path);
        let branch = git_status.branch.as_deref();
        repository
            .bind_session(
                session_id.as_str(),
                project_id.as_str(),
                &project.root_path,
                branch,
            )
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        repository
            .set_session_visibility(session_id.as_str(), "draft_hidden")
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        self.session_execution.ensure_session(&session_id).await;
        Ok(session_id)
    }

    pub(crate) async fn create_project_worktree_session(
        &self,
        project_id: ProjectId,
        branch_name: String,
    ) -> agent_core::Result<SessionId> {
        let repository = self.project_repository()?;
        let project = repository
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let worktree_path = crate::project::worktree_dir(&project.root_path, &branch_name);
        crate::project::create_git_worktree(&project.root_path, &branch_name, &worktree_path)
            .map_err(agent_core::CoreError::InvalidState)?;
        let worktree_path_string = worktree_path.display().to_string();
        self.start_lsp_servers(&crate::lsp_manager::file_uri_from_path(
            &worktree_path_string,
        ))
        .await;
        let model_profile = self.config.default_profile();
        let session_id = crate::session::start_session(
            &*self.store,
            &self.event_tx,
            WorkspaceId::from_string(project.workspace_id.clone()),
            model_profile.clone(),
            None,
            None,
        )
        .await?;
        self.initialize_session_limits(&session_id, &model_profile)
            .await;
        repository
            .bind_session(
                session_id.as_str(),
                project_id.as_str(),
                &worktree_path_string,
                Some(&branch_name),
            )
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        self.session_execution.ensure_session(&session_id).await;
        Ok(session_id)
    }

    pub(crate) async fn list_project_branches(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<Vec<String>> {
        let project = self
            .project_repository()?
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let git_status = crate::project::get_git_status(&project.root_path);
        if matches!(git_status.kind, ProjectGitStatusKind::NotInitialized) {
            return Ok(Vec::new());
        }
        crate::project::list_git_branches(&project.root_path)
            .map_err(agent_core::CoreError::InvalidState)
    }

    pub(crate) async fn list_project_sessions(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<Vec<SessionMeta>> {
        let _repository = self.project_repository()?;
        let rows = self
            .store
            .list_visible_project_sessions(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(crate::project::project_session_row_to_meta)
            .collect())
    }

    pub(crate) async fn list_archived_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<SessionMeta>> {
        let _repository = self.project_repository()?;
        let rows = self
            .store
            .list_archived_project_session_metas(workspace_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(crate::project::project_session_row_to_meta)
            .collect())
    }

    pub(crate) async fn get_project_git_status(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectGitStatus> {
        let project = self
            .project_repository()?
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(crate::project::get_git_status(&project.root_path))
    }

    pub(crate) async fn get_session_git_status(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<ProjectGitStatus> {
        let binding = self
            .project_repository()?
            .get_session_binding(session_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState("session is not bound to a project".into())
            })?;
        Ok(crate::project::get_git_status(&binding.worktree_path))
    }

    pub(crate) async fn init_project_git(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectGitStatus> {
        let project = self
            .project_repository()?
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let output = Command::new("git")
            .args(["-C", &project.root_path, "init"])
            .output()
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        if !output.status.success() {
            return Ok(ProjectGitStatus {
                kind: agent_core::ProjectGitStatusKind::Error,
                branch: None,
                worktree_path: project.root_path,
                message: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
            });
        }
        Ok(crate::project::get_git_status(&project.root_path))
    }

    pub(crate) async fn get_project_instruction_summary(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectInstructionSummary> {
        let project = self
            .project_repository()?
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(
            crate::project::read_project_instruction_summary(std::path::Path::new(
                &project.root_path,
            ))
            .await,
        )
    }
}

#[async_trait]
impl<S, M> ProjectFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn list_projects(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<ProjectMeta>> {
        LocalRuntime::list_projects(self, workspace_id).await
    }

    async fn create_blank_project(
        &self,
        workspace_id: WorkspaceId,
        display_name: Option<String>,
    ) -> agent_core::Result<ProjectMeta> {
        LocalRuntime::create_blank_project(self, workspace_id, display_name).await
    }

    async fn add_existing_project(
        &self,
        workspace_id: WorkspaceId,
        path: String,
    ) -> agent_core::Result<ProjectMeta> {
        LocalRuntime::add_existing_project(self, workspace_id, path).await
    }

    async fn rename_project(
        &self,
        project_id: ProjectId,
        display_name: String,
    ) -> agent_core::Result<()> {
        LocalRuntime::rename_project(self, project_id, display_name).await
    }

    async fn remove_project(&self, project_id: ProjectId) -> agent_core::Result<()> {
        LocalRuntime::remove_project(self, project_id).await
    }

    async fn restore_project_session(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<ProjectMeta> {
        LocalRuntime::restore_project_session(self, session_id).await
    }

    async fn update_project_order(&self, project_ids: Vec<ProjectId>) -> agent_core::Result<()> {
        LocalRuntime::update_project_order(self, project_ids).await
    }

    async fn update_project_expanded(
        &self,
        project_id: ProjectId,
        expanded: bool,
    ) -> agent_core::Result<()> {
        LocalRuntime::update_project_expanded(self, project_id, expanded).await
    }

    async fn create_project_draft_session(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<SessionId> {
        LocalRuntime::create_project_draft_session(self, project_id).await
    }

    async fn create_project_worktree_session(
        &self,
        project_id: ProjectId,
        branch_name: String,
    ) -> agent_core::Result<SessionId> {
        LocalRuntime::create_project_worktree_session(self, project_id, branch_name).await
    }

    async fn list_project_branches(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<Vec<String>> {
        LocalRuntime::list_project_branches(self, project_id).await
    }

    async fn list_project_sessions(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<Vec<SessionMeta>> {
        LocalRuntime::list_project_sessions(self, project_id).await
    }

    async fn list_archived_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<SessionMeta>> {
        LocalRuntime::list_archived_sessions(self, workspace_id).await
    }

    async fn get_project_git_status(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectGitStatus> {
        LocalRuntime::get_project_git_status(self, project_id).await
    }

    async fn get_session_git_status(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<ProjectGitStatus> {
        LocalRuntime::get_session_git_status(self, session_id).await
    }

    async fn init_project_git(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectGitStatus> {
        LocalRuntime::init_project_git(self, project_id).await
    }

    async fn get_project_instruction_summary(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectInstructionSummary> {
        LocalRuntime::get_project_instruction_summary(self, project_id).await
    }
}

#[cfg(test)]
#[path = "facade_projects_tests.rs"]
mod tests;
