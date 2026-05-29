pub mod direct;
pub(crate) mod discovery;
pub mod fake;
pub mod npx;

use std::path::Path;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult, SkillUpdateState,
};

#[async_trait::async_trait]
pub trait SkillPackageManager: Send + Sync {
    async fn search(&self, query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>>;

    async fn install_from_registry(
        &self,
        install_root: &Path,
        request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()>;

    async fn install_from_github(
        &self,
        install_root: &Path,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()>;

    async fn check_updates(&self, skill_id: &str) -> agent_core::Result<SkillUpdateState>;

    async fn update(&self, skill_id: &str) -> agent_core::Result<()>;
}

pub use direct::DirectDownloadPackageManager;
pub use fake::FakeSkillPackageManager;
pub use npx::NpxSkillsPackageManager;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
