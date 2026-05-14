use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
    SkillInstallTarget, SkillUpdateState,
};
use agent_core::CoreError;
use serde::Deserialize;
use tokio::process::Command;

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

pub struct NpxSkillsPackageManager;

#[derive(Debug, Deserialize)]
struct SkillsApiResponse {
    skills: Vec<SkillsApiItem>,
}

#[derive(Debug, Deserialize)]
struct SkillsApiItem {
    id: String,
    name: String,
    #[serde(default)]
    installs: Option<u64>,
    #[serde(default)]
    source: Option<String>,
}

#[async_trait::async_trait]
impl SkillPackageManager for NpxSkillsPackageManager {
    async fn search(&self, query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        let url = format!(
            "https://skills.sh/api/search?q={}&limit=100",
            url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
        );
        let response = reqwest::get(&url).await.map_err(|e| {
            CoreError::InvalidState(format!("skills.sh search request failed: {e}"))
        })?;
        let api_response: SkillsApiResponse = response
            .json()
            .await
            .map_err(|e| CoreError::InvalidState(format!("skills.sh search parse failed: {e}")))?;
        Ok(api_response
            .skills
            .into_iter()
            .map(|r| RemoteSkillSearchResult {
                name: r.name,
                description: String::new(),
                repository: r.source,
                install_count: r.installs,
                source_url: format!("https://skills.sh/skills/{}", r.id),
                package: r.id,
            })
            .collect())
    }

    async fn install_from_registry(
        &self,
        install_root: &Path,
        request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()> {
        let mut args = vec!["skills", "add", request.package.as_str()];
        append_install_target_args(&mut args, request.target);
        run_npx_skills_command_in_directory(
            &args,
            install_working_directory(install_root, request.target),
        )
        .await
        .map(|_| ())
    }

    async fn install_from_github(
        &self,
        install_root: &Path,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        let mut args = vec!["skills", "add", request.source.as_str()];
        append_install_target_args(&mut args, request.target);
        run_npx_skills_command_in_directory(
            &args,
            install_working_directory(install_root, request.target),
        )
        .await
        .map(|_| ())
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

        download_and_extract_skill(download_url, install_root).await
    }

    async fn install_from_github(
        &self,
        install_root: &Path,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        let clone_dir = tempfile::tempdir()
            .map_err(|e| CoreError::InvalidState(format!("tempdir for clone: {e}")))?;

        let status = Command::new("git")
            .args(["clone", "--depth", "1", &request.source])
            .arg(clone_dir.path())
            .status()
            .await
            .map_err(|e| CoreError::InvalidState(format!("git clone spawn failed: {e}")))?;

        if !status.success() {
            return Err(CoreError::InvalidState(format!(
                "git clone exited with {status}"
            )));
        }

        copy_skill_files(clone_dir.path(), install_root).await?;
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

async fn copy_skill_files(src: &Path, dest: &Path) -> agent_core::Result<()> {
    tokio::fs::create_dir_all(dest)
        .await
        .map_err(|e| CoreError::InvalidState(format!("mkdir for skill: {e}")))?;

    let mut entries = tokio::fs::read_dir(src)
        .await
        .map_err(|e| CoreError::InvalidState(format!("read clone dir: {e}")))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| CoreError::InvalidState(format!("read entry: {e}")))?
    {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name.starts_with('.') && name != ".kairox" {
            continue;
        }
        let file_type = entry
            .file_type()
            .await
            .map_err(|e| CoreError::InvalidState(format!("file_type: {e}")))?;
        let dest_path = dest.join(&*file_name);

        if file_type.is_dir() {
            let mut src_sub = tokio::fs::read_dir(entry.path())
                .await
                .map_err(|e| CoreError::InvalidState(format!("read subdir: {e}")))?;
            tokio::fs::create_dir_all(&dest_path)
                .await
                .map_err(|e| CoreError::InvalidState(format!("mkdir sub: {e}")))?;
            while let Some(sub_entry) = src_sub
                .next_entry()
                .await
                .map_err(|e| CoreError::InvalidState(format!("read sub entry: {e}")))?
            {
                let sub_name = sub_entry.file_name();
                let sub_dest = dest_path.join(&*sub_name.to_string_lossy());
                tokio::fs::copy(sub_entry.path(), &sub_dest)
                    .await
                    .map_err(|e| CoreError::InvalidState(format!("copy file: {e}")))?;
            }
        } else {
            tokio::fs::copy(entry.path(), &dest_path)
                .await
                .map_err(|e| CoreError::InvalidState(format!("copy file: {e}")))?;
        }
    }
    Ok(())
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
        if columns.len() < 2 {
            eprintln!(
                "warning: skipping npx skills find output line {}: expected at least 2 columns, got {}",
                line_index + 1,
                columns.len()
            );
            continue;
        }

        let install_count = if columns.len() >= 4 {
            parse_install_count(columns[3], line_index + 1)?
        } else {
            None
        };

        let repository = if columns.len() >= 3 {
            optional_column(columns[2])
        } else {
            None
        };

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
    run_npx_skills_command_in_directory(args, None).await
}

async fn run_npx_skills_command_in_directory(
    args: &[&str],
    working_directory: Option<PathBuf>,
) -> agent_core::Result<String> {
    let mut command = Command::new("npx");
    command.args(args);
    if let Some(working_directory) = working_directory {
        command.current_dir(working_directory);
    }

    let output = command.output().await.map_err(classify_npx_spawn_error)?;

    if !output.status.success() {
        return Err(format_npx_exit_error(args, output.status, &output.stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn install_working_directory(install_root: &Path, target: SkillInstallTarget) -> Option<PathBuf> {
    match target {
        SkillInstallTarget::Project => install_root.parent().map(Path::to_path_buf),
        SkillInstallTarget::User => None,
    }
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
        // The parser now handles 2–4 columns gracefully. Lines with < 2 columns
        // are silently skipped so the result is an empty vec rather than an error.
        let results =
            parse_npx_skills_find_output("header_only\n").expect("single column should skip");
        assert!(results.is_empty(), "single-column lines should be skipped");

        // 3-column input is now valid: name, description, repository (no install_count).
        let results =
            parse_npx_skills_find_output("code-review\tReview code changes\tobra/superpowers\n")
                .expect("3-column output should parse");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "code-review");
        assert_eq!(results[0].repository.as_deref(), Some("obra/superpowers"));
        assert_eq!(results[0].install_count, None);
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
