use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
    SkillInstallTarget, SkillUpdateState,
};
use agent_core::CoreError;

use super::SkillPackageManager;

pub struct NpxSkillsPackageManager;

#[async_trait::async_trait]
impl SkillPackageManager for NpxSkillsPackageManager {
    async fn search(&self, query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        let output = run_npx_skills_command(&["skills", "find", query]).await?;
        parse_npx_skills_find_output(&output)
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

pub(crate) fn parse_npx_skills_find_output(
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

pub(crate) fn classify_npx_spawn_error(error: io::Error) -> CoreError {
    if error.kind() == io::ErrorKind::NotFound {
        return CoreError::InvalidState(
            "npx executable not found; install Node.js/npm or ensure npx is on PATH".to_string(),
        );
    }

    CoreError::InvalidState(format!("failed to run npx skills command: {error}"))
}

pub(crate) async fn run_npx_skills_command(args: &[&str]) -> agent_core::Result<String> {
    run_npx_skills_command_in_directory(args, None).await
}

async fn run_npx_skills_command_in_directory(
    args: &[&str],
    working_directory: Option<PathBuf>,
) -> agent_core::Result<String> {
    let mut command = tokio::process::Command::new("npx");
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

pub(crate) fn format_npx_exit_error(args: &[&str], status: ExitStatus, stderr: &[u8]) -> CoreError {
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
