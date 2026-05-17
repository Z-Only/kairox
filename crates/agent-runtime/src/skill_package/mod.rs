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
mod tests {
    use std::path::PathBuf;

    use agent_core::facade::{
        InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
        SkillInstallTarget, SkillUpdateState,
    };

    use super::discovery;
    use super::fake::FakeSkillPackageManager;
    use super::npx;
    use super::SkillPackageManager;

    #[test]
    fn parses_skills_find_lines_into_remote_results() {
        let output = "code-review\tReview code changes\tobra/superpowers\t1200\n";
        let results = npx::parse_npx_skills_find_output(output).expect("output should parse");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "code-review");
        assert_eq!(results[0].repository.as_deref(), Some("obra/superpowers"));
        assert_eq!(results[0].install_count, Some(1200));
    }

    #[test]
    fn missing_npx_is_classified_as_runtime_missing() {
        let error =
            npx::classify_npx_spawn_error(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(error.to_string().contains("npx"));
        assert!(error.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn fake_manager_records_calls_and_returns_configured_results() {
        let manager = FakeSkillPackageManager::default();
        let expected_result = remote_skill_result("code-review");
        manager
            .search_results
            .lock()
            .await
            .push(expected_result.clone());
        *manager.check_updates_result.lock().await = SkillUpdateState::UpToDate;

        let registry_request = InstallRemoteSkillRequest {
            package: "@skills/code-review".to_string(),
            source: "registry".to_string(),
            target: SkillInstallTarget::Project,
            package_url: None,
        };
        let github_request = InstallGithubSkillRequest {
            source: "obra/superpowers".to_string(),
            target: SkillInstallTarget::User,
        };

        let search_results = manager
            .search("review")
            .await
            .expect("search should succeed");
        let project_install_root = tempfile::tempdir().expect("project install root");
        let user_install_root = tempfile::tempdir().expect("user install root");
        manager
            .install_from_registry(project_install_root.path(), &registry_request)
            .await
            .expect("registry install should succeed");
        manager
            .install_from_github(user_install_root.path(), &github_request)
            .await
            .expect("github install should succeed");
        let update_state = manager
            .check_updates("code-review")
            .await
            .expect("check updates should succeed");
        manager
            .update("code-review")
            .await
            .expect("update should succeed");

        assert_eq!(search_results, vec![expected_result]);
        assert_eq!(update_state, SkillUpdateState::UpToDate);
        assert_eq!(manager.search_queries.lock().await.as_slice(), ["review"]);
        assert_eq!(
            manager.registry_install_requests.lock().await.as_slice(),
            [registry_request]
        );
        assert_eq!(
            manager.github_install_requests.lock().await.as_slice(),
            [github_request]
        );
        assert_eq!(
            manager.registry_install_roots.lock().await.as_slice(),
            [project_install_root.path().to_path_buf()]
        );
        assert_eq!(
            manager.github_install_roots.lock().await.as_slice(),
            [user_install_root.path().to_path_buf()]
        );
        assert_eq!(
            manager.check_update_skill_ids.lock().await.as_slice(),
            ["code-review"]
        );
        assert_eq!(
            manager.update_skill_ids.lock().await.as_slice(),
            ["code-review"]
        );
    }

    #[tokio::test]
    async fn fake_manager_can_simulate_failures() {
        let manager = FakeSkillPackageManager::default();
        *manager.registry_install_error.lock().await = Some("registry refused install".to_string());
        *manager.update_error.lock().await = Some("update failed offline".to_string());

        let registry_request = InstallRemoteSkillRequest {
            package: "@skills/code-review".to_string(),
            source: "registry".to_string(),
            target: SkillInstallTarget::Project,
            package_url: None,
        };

        let project_install_root = tempfile::tempdir().expect("project install root");
        let install_error = manager
            .install_from_registry(project_install_root.path(), &registry_request)
            .await
            .expect_err("registry install should fail");
        let update_error = manager
            .update("code-review")
            .await
            .expect_err("update should fail");

        assert!(install_error
            .to_string()
            .contains("registry refused install"));
        assert!(update_error.to_string().contains("update failed offline"));
        assert_eq!(
            manager.registry_install_requests.lock().await.as_slice(),
            [registry_request]
        );
        assert_eq!(
            manager.update_skill_ids.lock().await.as_slice(),
            ["code-review"]
        );
    }

    #[test]
    fn parses_skills_find_output_skips_empty_lines_and_falls_back_for_empty_repository() {
        let output = "\ncode-review\tReview code changes\t\t42\n\n";

        let results = npx::parse_npx_skills_find_output(output).expect("output should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "code-review");
        assert_eq!(results[0].repository, None);
        assert_eq!(results[0].source_url, "code-review");
        assert_eq!(results[0].package, "code-review");
        assert_eq!(results[0].install_count, Some(42));
    }

    #[test]
    fn skill_directory_name_uses_slug_or_repo_name() {
        assert_eq!(
            discovery::skill_directory_name("self-improving-agent"),
            "self-improving-agent"
        );
        assert_eq!(
            discovery::skill_directory_name("https://github.com/acme/docs-helper.git"),
            "docs-helper"
        );
        assert_eq!(discovery::skill_directory_name("@scope/package"), "package");
    }

    #[test]
    fn parses_github_skill_source_variants() {
        let root = discovery::parse_github_skill_source("https://github.com/acme/skills")
            .expect("repo root should parse");
        assert_eq!(root.clone_url, "https://github.com/acme/skills.git");
        assert_eq!(root.branch, None);
        assert_eq!(root.skill_subdir, PathBuf::new());
        assert_eq!(root.directory_name, "skills");

        let tree = discovery::parse_github_skill_source(
            "https://github.com/acme/skills/tree/main/packages/code-review",
        )
        .expect("tree URL should parse");
        assert_eq!(tree.branch.as_deref(), Some("main"));
        assert_eq!(tree.skill_subdir, PathBuf::from("packages/code-review"));
        assert_eq!(tree.directory_name, "code-review");

        let blob = discovery::parse_github_skill_source(
            "https://github.com/acme/skills/blob/dev/packages/review/SKILL.md",
        )
        .expect("SKILL.md blob URL should parse");
        assert_eq!(blob.branch.as_deref(), Some("dev"));
        assert_eq!(blob.skill_subdir, PathBuf::from("packages/review"));
        assert_eq!(blob.directory_name, "review");

        let shorthand = discovery::parse_github_skill_source("acme/skills/packages/review")
            .expect("shorthand parses");
        assert_eq!(shorthand.clone_url, "https://github.com/acme/skills.git");
        assert_eq!(shorthand.branch, None);
        assert_eq!(shorthand.skill_subdir, PathBuf::from("packages/review"));
    }

    #[test]
    fn rejects_github_blob_that_is_not_skill_markdown() {
        let error = discovery::parse_github_skill_source(
            "https://github.com/acme/skills/blob/main/packages/review/README.md",
        )
        .expect_err("non-SKILL.md blob should fail");

        assert!(error.to_string().contains("SKILL.md"));
    }

    #[tokio::test]
    async fn validate_skill_directory_rejects_missing_or_invalid_skill_markdown() {
        let missing = tempfile::tempdir().expect("missing skill dir");
        let missing_error = discovery::validate_skill_directory(missing.path())
            .await
            .expect_err("missing SKILL.md should fail");
        assert!(missing_error.to_string().contains("SKILL.md"));

        let invalid = tempfile::tempdir().expect("invalid skill dir");
        std::fs::write(invalid.path().join("SKILL.md"), "# No frontmatter\n")
            .expect("invalid skill should be written");
        let invalid_error = discovery::validate_skill_directory(invalid.path())
            .await
            .expect_err("invalid SKILL.md should fail");
        assert!(invalid_error.to_string().contains("invalid SKILL.md"));
    }

    #[tokio::test]
    async fn copy_skill_directory_atomically_copies_nested_valid_skill() {
        let source = tempfile::tempdir().expect("source skill dir");
        std::fs::create_dir_all(source.path().join("assets/icons")).expect("asset dir");
        std::fs::write(
            source.path().join("SKILL.md"),
            "---\nname: code-review\ndescription: Review code\n---\nBody\n",
        )
        .expect("skill markdown");
        std::fs::write(source.path().join("assets/icons/icon.txt"), "icon").expect("asset");

        let install_root = tempfile::tempdir().expect("install root");
        let target = install_root.path().join("code-review");
        discovery::validate_skill_directory(source.path())
            .await
            .expect("valid skill should pass");
        discovery::copy_skill_directory_atomically(source.path(), &target)
            .await
            .expect("copy should succeed");

        assert!(target.join("SKILL.md").exists());
        assert_eq!(
            std::fs::read_to_string(target.join("assets/icons/icon.txt")).expect("asset copied"),
            "icon"
        );
    }

    #[test]
    fn parse_error_for_wrong_column_count_is_actionable() {
        let results =
            npx::parse_npx_skills_find_output("header_only\n").expect("single column should skip");
        assert!(results.is_empty(), "single-column lines should be skipped");

        let results = npx::parse_npx_skills_find_output(
            "code-review\tReview code changes\tobra/superpowers\n",
        )
        .expect("3-column output should parse");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "code-review");
        assert_eq!(results[0].repository.as_deref(), Some("obra/superpowers"));
        assert_eq!(results[0].install_count, None);
    }

    #[test]
    fn parse_error_for_invalid_install_count_is_actionable() {
        let error = npx::parse_npx_skills_find_output(
            "code-review\tReview code changes\tobra/superpowers\tmany\n",
        )
        .expect_err("invalid install count should fail");
        let message = error.to_string();

        assert!(message.contains("line 1"), "message was: {message}");
        assert!(message.contains("install_count"), "message was: {message}");
    }

    #[test]
    fn npx_exit_error_includes_command_status_stderr_and_truncation_marker() {
        #[cfg(unix)]
        let status = std::os::unix::process::ExitStatusExt::from_raw(256);
        #[cfg(windows)]
        let status = std::os::windows::process::ExitStatusExt::from_raw(1);
        let long_stderr = "x".repeat(600);

        let error = npx::format_npx_exit_error(
            &["skills", "find", "review"],
            status,
            long_stderr.as_bytes(),
        );
        let message = error.to_string();

        assert!(
            message.contains("npx skills find review"),
            "message was: {message}"
        );
        assert!(message.contains("exit status"), "message was: {message}");
        assert!(message.contains("stderr"), "message was: {message}");
        assert!(message.contains("<truncated>"), "message was: {message}");
    }

    fn remote_skill_result(name: &str) -> RemoteSkillSearchResult {
        RemoteSkillSearchResult {
            name: name.to_string(),
            description: "Review code changes".to_string(),
            repository: Some("obra/superpowers".to_string()),
            install_count: Some(1200),
            source_url: "obra/superpowers".to_string(),
            package: format!("@skills/{name}"),
        }
    }
}
