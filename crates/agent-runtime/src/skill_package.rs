use std::io;
use std::process::ExitStatus;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
    SkillInstallTarget, SkillUpdateState,
};
use agent_core::CoreError;
use tokio::process::Command;

#[async_trait::async_trait]
pub trait SkillPackageManager: Send + Sync {
    async fn search(&self, query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>>;

    async fn install_from_registry(
        &self,
        request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()>;

    async fn install_from_github(
        &self,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()>;

    async fn check_updates(&self, skill_id: &str) -> agent_core::Result<SkillUpdateState>;

    async fn update(&self, skill_id: &str) -> agent_core::Result<()>;
}

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
    pub github_install_requests: tokio::sync::Mutex<Vec<InstallGithubSkillRequest>>,
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
            github_install_requests: tokio::sync::Mutex::new(Vec::new()),
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
        request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()> {
        self.registry_install_requests
            .lock()
            .await
            .push(request.clone());

        if let Some(message) = self.registry_install_error.lock().await.clone() {
            return Err(CoreError::InvalidState(message));
        }

        Ok(())
    }

    async fn install_from_github(
        &self,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        self.github_install_requests
            .lock()
            .await
            .push(request.clone());

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

pub struct NpxSkillsPackageManager;

#[async_trait::async_trait]
impl SkillPackageManager for NpxSkillsPackageManager {
    async fn search(&self, query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        let output = run_npx_skills_command(&["skills", "find", query]).await?;
        parse_npx_skills_find_output(&output)
    }

    async fn install_from_registry(
        &self,
        request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()> {
        let mut args = vec!["skills", "add", request.package.as_str()];
        append_install_target_args(&mut args, request.target);
        run_npx_skills_command(&args).await.map(|_| ())
    }

    async fn install_from_github(
        &self,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        let mut args = vec!["skills", "add", request.source.as_str()];
        append_install_target_args(&mut args, request.target);
        run_npx_skills_command(&args).await.map(|_| ())
    }

    async fn check_updates(&self, skill_id: &str) -> agent_core::Result<SkillUpdateState> {
        let output = run_npx_skills_command(&["skills", "check", skill_id]).await?;
        Ok(parse_npx_skills_check_output(&output))
    }

    async fn update(&self, skill_id: &str) -> agent_core::Result<()> {
        run_npx_skills_command(&["skills", "update", skill_id])
            .await
            .map(|_| ())
    }
}

pub fn parse_npx_skills_find_output(
    output: &str,
) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
    let mut results = Vec::new();

    for (line_index, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() != 4 {
            return Err(CoreError::InvalidState(format!(
                "failed to parse npx skills find output at line {}: expected 4 columns separated by tabs",
                line_index + 1
            )));
        }

        let install_count = parse_install_count(columns[3], line_index + 1)?;
        let repository = optional_column(columns[2]);
        let package = columns[0].trim().to_string();
        let source_url = repository.clone().unwrap_or_else(|| package.clone());

        results.push(RemoteSkillSearchResult {
            name: columns[0].trim().to_string(),
            description: columns[1].trim().to_string(),
            repository,
            install_count,
            source_url,
            package,
        });
    }

    Ok(results)
}

pub fn classify_npx_spawn_error(error: io::Error) -> CoreError {
    if error.kind() == io::ErrorKind::NotFound {
        return CoreError::InvalidState(
            "npx executable not found; install Node.js/npm or ensure npx is on PATH".to_string(),
        );
    }

    CoreError::InvalidState(format!("failed to run npx skills command: {error}"))
}

async fn run_npx_skills_command(args: &[&str]) -> agent_core::Result<String> {
    let output = Command::new("npx")
        .args(args)
        .output()
        .await
        .map_err(classify_npx_spawn_error)?;

    if !output.status.success() {
        return Err(format_npx_exit_error(args, output.status, &output.stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn append_install_target_args(args: &mut Vec<&str>, target: SkillInstallTarget) {
    match target {
        SkillInstallTarget::Project => args.push("--project"),
        SkillInstallTarget::User => args.push("--user"),
    }
}

fn parse_install_count(
    raw_install_count: &str,
    line_number: usize,
) -> agent_core::Result<Option<u64>> {
    let trimmed_install_count = raw_install_count.trim();
    if trimmed_install_count.is_empty() {
        return Ok(None);
    }

    trimmed_install_count
        .parse::<u64>()
        .map(Some)
        .map_err(|error| {
            CoreError::InvalidState(format!(
                "failed to parse install_count at line {line_number}: {error}"
            ))
        })
}

fn optional_column(raw_column: &str) -> Option<String> {
    let trimmed_column = raw_column.trim();
    if trimmed_column.is_empty() {
        return None;
    }

    Some(trimmed_column.to_string())
}

fn parse_npx_skills_check_output(output: &str) -> SkillUpdateState {
    let normalized_output = output.to_ascii_lowercase();
    if normalized_output.contains("update available") || normalized_output.contains("outdated") {
        return SkillUpdateState::UpdateAvailable;
    }

    if normalized_output.contains("up to date") || normalized_output.contains("up-to-date") {
        return SkillUpdateState::UpToDate;
    }

    SkillUpdateState::Unknown
}

fn format_npx_exit_error(args: &[&str], status: ExitStatus, stderr: &[u8]) -> CoreError {
    let command_summary = std::iter::once("npx")
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    let stderr_summary = summarize_stderr(stderr);

    CoreError::InvalidState(format!(
        "npx skills command failed: `{command_summary}` exited with {status}; stderr: {stderr_summary}"
    ))
}

fn summarize_stderr(stderr: &[u8]) -> String {
    let stderr_text = String::from_utf8_lossy(stderr);
    let trimmed_stderr = stderr_text.trim();

    if trimmed_stderr.is_empty() {
        return "<empty>".to_string();
    }

    let mut stderr_summary: String = trimmed_stderr.chars().take(500).collect();
    if trimmed_stderr.chars().count() > 500 {
        stderr_summary.push_str("... <truncated>");
    }

    stderr_summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skills_find_lines_into_remote_results() {
        let output = "code-review\tReview code changes\tobra/superpowers\t1200\n";
        let results = parse_npx_skills_find_output(output).expect("output should parse");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "code-review");
        assert_eq!(results[0].repository.as_deref(), Some("obra/superpowers"));
        assert_eq!(results[0].install_count, Some(1200));
    }

    #[test]
    fn missing_npx_is_classified_as_runtime_missing() {
        let error = classify_npx_spawn_error(std::io::Error::from(std::io::ErrorKind::NotFound));
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
        };
        let github_request = InstallGithubSkillRequest {
            source: "obra/superpowers".to_string(),
            target: SkillInstallTarget::User,
        };

        let search_results = manager
            .search("review")
            .await
            .expect("search should succeed");
        manager
            .install_from_registry(&registry_request)
            .await
            .expect("registry install should succeed");
        manager
            .install_from_github(&github_request)
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
        };

        let install_error = manager
            .install_from_registry(&registry_request)
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

        let results = parse_npx_skills_find_output(output).expect("output should parse");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "code-review");
        assert_eq!(results[0].repository, None);
        assert_eq!(results[0].source_url, "code-review");
        assert_eq!(results[0].package, "code-review");
        assert_eq!(results[0].install_count, Some(42));
    }

    #[test]
    fn parse_error_for_wrong_column_count_is_actionable() {
        let error = parse_npx_skills_find_output("\ncode-review\tReview code changes\t42\n")
            .expect_err("wrong column count should fail");
        let message = error.to_string();

        assert!(message.contains("line 2"), "message was: {message}");
        assert!(
            message.contains("expected 4 columns"),
            "message was: {message}"
        );
    }

    #[test]
    fn parse_error_for_invalid_install_count_is_actionable() {
        let error = parse_npx_skills_find_output(
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

        let error = format_npx_exit_error(
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
