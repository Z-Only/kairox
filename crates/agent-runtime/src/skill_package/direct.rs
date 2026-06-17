use std::path::Path;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult, SkillUpdateState,
};
use agent_core::CoreError;

use super::discovery;
use super::SkillPackageManager;

#[cfg(test)]
#[path = "direct_tests.rs"]
mod direct_tests;

pub struct DirectDownloadPackageManager;

#[async_trait::async_trait]
impl SkillPackageManager for DirectDownloadPackageManager {
    async fn search(&self, _query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        Ok(Vec::new())
    }

    async fn install_from_registry(
        &self,
        install_root: &Path,
        request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()> {
        let download_url = request
            .package_url
            .as_deref()
            .or_else(|| {
                if request.package.starts_with("http://") || request.package.starts_with("https://")
                {
                    Some(request.package.as_str())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                CoreError::InvalidState(format!(
                    "no package_url for skill install; package={}",
                    request.package
                ))
            })?;

        let target_dir = install_root.join(discovery::skill_directory_name(&request.package));
        download_and_extract_skill(download_url, &target_dir).await
    }

    async fn install_from_github(
        &self,
        install_root: &Path,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        let source = discovery::parse_github_skill_source(&request.source)?;
        let clone_dir = tempfile::tempdir()
            .map_err(|e| CoreError::InvalidState(format!("tempdir for clone: {e}")))?;

        let mut command = tokio::process::Command::new("git");
        command.args(["clone", "--depth", "1"]);
        if let Some(branch) = source.branch.as_deref() {
            command.args(["--branch", branch]);
        }
        let status = command
            .arg(&source.clone_url)
            .arg(clone_dir.path())
            .status()
            .await
            .map_err(|e| CoreError::InvalidState(format!("git clone spawn failed: {e}")))?;

        if !status.success() {
            return Err(CoreError::InvalidState(format!(
                "git clone exited with {status}"
            )));
        }

        let skill_dir = clone_dir.path().join(&source.skill_subdir);
        discovery::validate_skill_directory(&skill_dir).await?;
        let target_dir = install_root.join(discovery::skill_directory_name(&source.directory_name));
        discovery::copy_skill_directory_atomically(&skill_dir, &target_dir).await?;
        Ok(())
    }

    async fn check_updates(&self, _skill_id: &str) -> agent_core::Result<SkillUpdateState> {
        Ok(SkillUpdateState::Unknown)
    }

    async fn update(&self, _skill_id: &str) -> agent_core::Result<()> {
        Err(CoreError::InvalidState(
            "skill update not yet supported".into(),
        ))
    }
}

async fn download_and_extract_skill(url: &str, install_root: &Path) -> agent_core::Result<()> {
    tokio::fs::create_dir_all(install_root)
        .await
        .map_err(|e| CoreError::InvalidState(format!("mkdir for skill: {e}")))?;

    let response = reqwest::get(url)
        .await
        .map_err(|e| CoreError::InvalidState(format!("skill download failed: {e}")))?;

    if !response.status().is_success() {
        return Err(CoreError::InvalidState(format!(
            "skill download returned HTTP {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| CoreError::InvalidState(format!("skill download read failed: {e}")))?;

    let temp_dir = tempfile::tempdir()
        .map_err(|e| CoreError::InvalidState(format!("tempdir for zip: {e}")))?;

    let zip_path = temp_dir.path().join("skill.zip");
    tokio::fs::write(&zip_path, &bytes)
        .await
        .map_err(|e| CoreError::InvalidState(format!("write zip: {e}")))?;

    let dest = install_root.to_path_buf();
    let zip_path_owned = zip_path.clone();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path_owned)
            .map_err(|e| CoreError::InvalidState(format!("open zip: {e}")))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| CoreError::InvalidState(format!("read zip: {e}")))?;
        archive
            .extract(&dest)
            .map_err(|e| CoreError::InvalidState(format!("extract zip: {e}")))?;
        Ok::<_, CoreError>(())
    })
    .await
    .map_err(|e| CoreError::InvalidState(format!("extract task panicked: {e}")))??;

    Ok(())
}
