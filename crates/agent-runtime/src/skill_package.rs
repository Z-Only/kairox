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

#[derive(Default)]
pub struct FakeSkillPackageManager {
    pub search_results: tokio::sync::Mutex<Vec<RemoteSkillSearchResult>>,
}

#[async_trait::async_trait]
impl SkillPackageManager for FakeSkillPackageManager {
    async fn search(&self, _query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        Ok(self.search_results.lock().await.clone())
    }

    async fn install_from_registry(
        &self,
        _request: &InstallRemoteSkillRequest,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn install_from_github(
        &self,
        _request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn check_updates(&self, _skill_id: &str) -> agent_core::Result<SkillUpdateState> {
        Ok(SkillUpdateState::Unknown)
    }

    async fn update(&self, _skill_id: &str) -> agent_core::Result<()> {
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
                "failed to parse npx skills find output at line {}: expected 4 tab-separated columns",
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

    trimmed_stderr.chars().take(500).collect()
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
}
