use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::ExitStatus;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
    SkillInstallTarget, SkillUpdateState,
};
use agent_core::CoreError;
use agent_skills::parse_skill_markdown;
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

#[async_trait::async_trait]
impl SkillPackageManager for NpxSkillsPackageManager {
    async fn search(&self, _query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        Ok(Vec::new())
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

        let target_dir = install_root.join(skill_directory_name(&request.package));
        download_and_extract_skill(download_url, &target_dir).await
    }

    async fn install_from_github(
        &self,
        install_root: &Path,
        request: &InstallGithubSkillRequest,
    ) -> agent_core::Result<()> {
        let source = parse_github_skill_source(&request.source)?;
        let clone_dir = tempfile::tempdir()
            .map_err(|e| CoreError::InvalidState(format!("tempdir for clone: {e}")))?;

        let mut command = Command::new("git");
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
        validate_skill_directory(&skill_dir).await?;
        let target_dir = install_root.join(skill_directory_name(&source.directory_name));
        copy_skill_directory_atomically(&skill_dir, &target_dir).await?;
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

async fn copy_skill_files(src: &Path, dest: &Path) -> agent_core::Result<()> {
    let src = src.to_path_buf();
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || copy_skill_files_sync(&src, &dest))
        .await
        .map_err(|e| CoreError::InvalidState(format!("copy task panicked: {e}")))?
}

fn copy_skill_files_sync(src: &Path, dest: &Path) -> agent_core::Result<()> {
    std::fs::create_dir_all(dest)
        .map_err(|e| CoreError::InvalidState(format!("mkdir for skill: {e}")))?;

    for entry in std::fs::read_dir(src)
        .map_err(|e| CoreError::InvalidState(format!("read skill dir: {e}")))?
    {
        let entry = entry.map_err(|e| CoreError::InvalidState(format!("read entry: {e}")))?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name.starts_with('.') && name != ".kairox" {
            continue;
        }

        let file_type = entry
            .file_type()
            .map_err(|e| CoreError::InvalidState(format!("file_type: {e}")))?;
        let dest_path = dest.join(&file_name);
        if file_type.is_dir() {
            copy_skill_files_sync(&entry.path(), &dest_path)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &dest_path)
                .map_err(|e| CoreError::InvalidState(format!("copy file: {e}")))?;
        }
    }

    Ok(())
}

async fn copy_skill_directory_atomically(src: &Path, dest: &Path) -> agent_core::Result<()> {
    let parent = dest.parent().ok_or_else(|| {
        CoreError::InvalidState(format!("invalid skill destination: {}", dest.display()))
    })?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| CoreError::InvalidState(format!("mkdir for skill root: {e}")))?;

    if tokio::fs::try_exists(dest)
        .await
        .map_err(|e| CoreError::InvalidState(format!("check skill destination: {e}")))?
    {
        return Err(CoreError::InvalidState(format!(
            "skill destination already exists: {}",
            dest.display()
        )));
    }

    let temp_dir_name = format!(
        ".{}.tmp-{}",
        dest.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("skill"),
        std::process::id()
    );
    let temp_dest = parent.join(temp_dir_name);
    if tokio::fs::try_exists(&temp_dest)
        .await
        .map_err(|e| CoreError::InvalidState(format!("check temp destination: {e}")))?
    {
        tokio::fs::remove_dir_all(&temp_dest)
            .await
            .map_err(|e| CoreError::InvalidState(format!("remove stale temp skill dir: {e}")))?;
    }

    copy_skill_files(src, &temp_dest).await?;
    if let Err(error) = tokio::fs::rename(&temp_dest, dest).await {
        let _ = tokio::fs::remove_dir_all(&temp_dest).await;
        return Err(CoreError::InvalidState(format!(
            "move validated skill into place: {error}"
        )));
    }

    Ok(())
}

async fn validate_skill_directory(skill_dir: &Path) -> agent_core::Result<()> {
    let skill_path = skill_dir.join("SKILL.md");
    if !tokio::fs::try_exists(&skill_path)
        .await
        .map_err(|e| CoreError::InvalidState(format!("check SKILL.md: {e}")))?
    {
        return Err(CoreError::InvalidState(format!(
            "GitHub source does not contain SKILL.md at {}",
            skill_path.display()
        )));
    }

    let raw = tokio::fs::read_to_string(&skill_path)
        .await
        .map_err(|e| CoreError::InvalidState(format!("read SKILL.md: {e}")))?;
    parse_skill_markdown(&raw).map_err(|e| {
        CoreError::InvalidState(format!(
            "GitHub source contains invalid SKILL.md at {}: {e}",
            skill_path.display()
        ))
    })?;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubSkillSource {
    clone_url: String,
    branch: Option<String>,
    skill_subdir: PathBuf,
    directory_name: String,
}

fn parse_github_skill_source(raw_source: &str) -> agent_core::Result<GithubSkillSource> {
    let trimmed = raw_source.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidState(
            "GitHub skill source is empty".to_string(),
        ));
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return parse_github_url(trimmed);
    }

    parse_github_shorthand(trimmed)
}

fn parse_github_url(raw_url: &str) -> agent_core::Result<GithubSkillSource> {
    let url = url::Url::parse(raw_url)
        .map_err(|e| CoreError::InvalidState(format!("invalid GitHub URL: {e}")))?;
    let host = url.host_str().unwrap_or_default();
    if host != "github.com" {
        return Err(CoreError::InvalidState(format!(
            "unsupported GitHub host: {host}"
        )));
    }

    let segments = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();
    if segments.len() < 2 {
        return Err(CoreError::InvalidState(
            "GitHub URL must include owner and repository".to_string(),
        ));
    }

    github_source_from_parts(segments[0], segments[1], &segments[2..])
}

fn parse_github_shorthand(raw: &str) -> agent_core::Result<GithubSkillSource> {
    let trimmed = raw.trim().trim_matches('/');
    let parts = trimmed.split('/').collect::<Vec<_>>();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(CoreError::InvalidState(
            "GitHub source must be a github.com URL or owner/repo shorthand".to_string(),
        ));
    }

    github_source_from_parts(parts[0], parts[1], &parts[2..])
}

fn github_source_from_parts(
    owner: &str,
    repo: &str,
    rest: &[&str],
) -> agent_core::Result<GithubSkillSource> {
    let repo_name = repo.trim_end_matches(".git");
    if owner.is_empty() || repo_name.is_empty() {
        return Err(CoreError::InvalidState(
            "GitHub source must include owner and repository".to_string(),
        ));
    }

    let clone_url = format!("https://github.com/{owner}/{repo_name}.git");
    let mut branch = None;
    let mut subdir_segments: Vec<String> = Vec::new();

    match rest {
        [] => {}
        ["tree", branch_name, tail @ ..] => {
            branch = Some((*branch_name).to_string());
            subdir_segments.extend(tail.iter().map(|segment| (*segment).to_string()));
        }
        ["blob", branch_name, tail @ ..] => {
            branch = Some((*branch_name).to_string());
            let skill_file_path = PathBuf::from_iter(tail.iter().copied());
            if skill_file_path.file_name().and_then(|name| name.to_str()) != Some("SKILL.md") {
                return Err(CoreError::InvalidState(
                    "GitHub blob URL must point to a SKILL.md file".to_string(),
                ));
            }
            if let Some(parent) = skill_file_path.parent() {
                subdir_segments.extend(
                    parent
                        .iter()
                        .filter_map(|segment| segment.to_str())
                        .map(ToString::to_string),
                );
            }
        }
        tail => {
            subdir_segments.extend(tail.iter().map(|segment| (*segment).to_string()));
        }
    }

    if branch.as_deref() == Some("") {
        return Err(CoreError::InvalidState(
            "GitHub tree/blob URL must include a branch".to_string(),
        ));
    }

    let skill_subdir = safe_relative_path(&subdir_segments)?;
    let directory_name = skill_subdir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(repo_name)
        .to_string();

    Ok(GithubSkillSource {
        clone_url,
        branch,
        skill_subdir,
        directory_name,
    })
}

fn safe_relative_path(segments: &[String]) -> agent_core::Result<PathBuf> {
    let mut path = PathBuf::new();
    for segment in segments.iter().filter(|segment| !segment.is_empty()) {
        let segment_path = Path::new(segment);
        if segment_path.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || component.as_os_str() == "."
                || component.as_os_str() == ".."
        }) {
            return Err(CoreError::InvalidState(format!(
                "invalid GitHub skill path segment: {segment}"
            )));
        }
        path.push(segment);
    }
    Ok(path)
}

fn skill_directory_name(package: &str) -> String {
    let tail = package
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(package)
        .trim_end_matches(".git");
    let sanitized = tail
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if sanitized.is_empty() {
        "skill".to_string()
    } else {
        sanitized
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
    fn skill_directory_name_uses_slug_or_repo_name() {
        assert_eq!(
            skill_directory_name("self-improving-agent"),
            "self-improving-agent"
        );
        assert_eq!(
            skill_directory_name("https://github.com/acme/docs-helper.git"),
            "docs-helper"
        );
        assert_eq!(skill_directory_name("@scope/package"), "package");
    }

    #[test]
    fn parses_github_skill_source_variants() {
        let root = parse_github_skill_source("https://github.com/acme/skills")
            .expect("repo root should parse");
        assert_eq!(root.clone_url, "https://github.com/acme/skills.git");
        assert_eq!(root.branch, None);
        assert_eq!(root.skill_subdir, PathBuf::new());
        assert_eq!(root.directory_name, "skills");

        let tree = parse_github_skill_source(
            "https://github.com/acme/skills/tree/main/packages/code-review",
        )
        .expect("tree URL should parse");
        assert_eq!(tree.branch.as_deref(), Some("main"));
        assert_eq!(tree.skill_subdir, PathBuf::from("packages/code-review"));
        assert_eq!(tree.directory_name, "code-review");

        let blob = parse_github_skill_source(
            "https://github.com/acme/skills/blob/dev/packages/review/SKILL.md",
        )
        .expect("SKILL.md blob URL should parse");
        assert_eq!(blob.branch.as_deref(), Some("dev"));
        assert_eq!(blob.skill_subdir, PathBuf::from("packages/review"));
        assert_eq!(blob.directory_name, "review");

        let shorthand =
            parse_github_skill_source("acme/skills/packages/review").expect("shorthand parses");
        assert_eq!(shorthand.clone_url, "https://github.com/acme/skills.git");
        assert_eq!(shorthand.branch, None);
        assert_eq!(shorthand.skill_subdir, PathBuf::from("packages/review"));
    }

    #[test]
    fn rejects_github_blob_that_is_not_skill_markdown() {
        let error = parse_github_skill_source(
            "https://github.com/acme/skills/blob/main/packages/review/README.md",
        )
        .expect_err("non-SKILL.md blob should fail");

        assert!(error.to_string().contains("SKILL.md"));
    }

    #[tokio::test]
    async fn validate_skill_directory_rejects_missing_or_invalid_skill_markdown() {
        let missing = tempfile::tempdir().expect("missing skill dir");
        let missing_error = validate_skill_directory(missing.path())
            .await
            .expect_err("missing SKILL.md should fail");
        assert!(missing_error.to_string().contains("SKILL.md"));

        let invalid = tempfile::tempdir().expect("invalid skill dir");
        std::fs::write(invalid.path().join("SKILL.md"), "# No frontmatter\n")
            .expect("invalid skill should be written");
        let invalid_error = validate_skill_directory(invalid.path())
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
        validate_skill_directory(source.path())
            .await
            .expect("valid skill should pass");
        copy_skill_directory_atomically(source.path(), &target)
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
