use std::path::{Path, PathBuf};

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult, SkillUpdateState,
};
use agent_core::CoreError;

use super::SkillPackageManager;

pub struct FakeSkillPackageManager {
    pub search_results: tokio::sync::Mutex<Vec<RemoteSkillSearchResult>>,
    pub search_error: tokio::sync::Mutex<Option<String>>,
    pub registry_install_error: tokio::sync::Mutex<Option<String>>,
    pub github_install_error: tokio::sync::Mutex<Option<String>>,
    pub check_updates_result: tokio::sync::Mutex<SkillUpdateState>,
    pub check_updates_error: tokio::sync::Mutex<Option<String>>,
    pub update_error: tokio::sync::Mutex<Option<String>>,
    pub search_queries: tokio::sync::Mutex<Vec<String>>,
    pub registry_install_requests: tokio::sync::Mutex<Vec<InstallRemoteSkillRequest>>,
    pub registry_install_roots: tokio::sync::Mutex<Vec<PathBuf>>,
    pub github_install_requests: tokio::sync::Mutex<Vec<InstallGithubSkillRequest>>,
    pub github_install_roots: tokio::sync::Mutex<Vec<PathBuf>>,
    pub check_update_skill_ids: tokio::sync::Mutex<Vec<String>>,
    pub update_skill_ids: tokio::sync::Mutex<Vec<String>>,
}

impl Default for FakeSkillPackageManager {
    fn default() -> Self {
        Self {
            search_results: tokio::sync::Mutex::new(Vec::new()),
            search_error: tokio::sync::Mutex::new(None),
            registry_install_error: tokio::sync::Mutex::new(None),
            github_install_error: tokio::sync::Mutex::new(None),
            check_updates_result: tokio::sync::Mutex::new(SkillUpdateState::Unknown),
            check_updates_error: tokio::sync::Mutex::new(None),
            update_error: tokio::sync::Mutex::new(None),
            search_queries: tokio::sync::Mutex::new(Vec::new()),
            registry_install_requests: tokio::sync::Mutex::new(Vec::new()),
            registry_install_roots: tokio::sync::Mutex::new(Vec::new()),
            github_install_requests: tokio::sync::Mutex::new(Vec::new()),
            github_install_roots: tokio::sync::Mutex::new(Vec::new()),
            check_update_skill_ids: tokio::sync::Mutex::new(Vec::new()),
            update_skill_ids: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl SkillPackageManager for FakeSkillPackageManager {
    async fn search(&self, query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        self.search_queries.lock().await.push(query.to_string());

        if let Some(message) = self.search_error.lock().await.clone() {
            return Err(CoreError::InvalidState(message));
        }

        Ok(self.search_results.lock().await.clone())
    }

    async fn install_from_registry(
        &self,
        install_root: &Path,
        request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()> {
        self.registry_install_requests
            .lock()
            .await
            .push(request.clone());
        self.registry_install_roots
            .lock()
            .await
            .push(install_root.to_path_buf());

        if let Some(message) = self.registry_install_error.lock().await.clone() {
            return Err(CoreError::InvalidState(message));
        }

        Ok(())
    }

    async fn install_from_github(
        &self,
        install_root: &Path,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        self.github_install_requests
            .lock()
            .await
            .push(request.clone());
        self.github_install_roots
            .lock()
            .await
            .push(install_root.to_path_buf());

        if let Some(message) = self.github_install_error.lock().await.clone() {
            return Err(CoreError::InvalidState(message));
        }

        Ok(())
    }

    async fn check_updates(&self, skill_id: &str) -> agent_core::Result<SkillUpdateState> {
        self.check_update_skill_ids
            .lock()
            .await
            .push(skill_id.to_string());

        if let Some(message) = self.check_updates_error.lock().await.clone() {
            return Err(CoreError::InvalidState(message));
        }

        Ok(*self.check_updates_result.lock().await)
    }

    async fn update(&self, skill_id: &str) -> agent_core::Result<()> {
        self.update_skill_ids
            .lock()
            .await
            .push(skill_id.to_string());

        if let Some(message) = self.update_error.lock().await.clone() {
            return Err(CoreError::InvalidState(message));
        }

        Ok(())
    }
}
