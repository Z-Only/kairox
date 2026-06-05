use std::path::{Component, Path, PathBuf};

use agent_core::CoreError;
use agent_skills::parse_skill_markdown;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubSkillSource {
    pub(crate) clone_url: String,
    pub(crate) branch: Option<String>,
    pub(crate) skill_subdir: PathBuf,
    pub(crate) directory_name: String,
}

pub(crate) fn parse_github_skill_source(raw_source: &str) -> agent_core::Result<GithubSkillSource> {
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

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod discovery_tests;

pub(crate) fn skill_directory_name(package: &str) -> String {
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

pub(crate) async fn copy_skill_files(src: &Path, dest: &Path) -> agent_core::Result<()> {
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

pub(crate) async fn copy_skill_directory_atomically(
    src: &Path,
    dest: &Path,
) -> agent_core::Result<()> {
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

pub(crate) async fn validate_skill_directory(skill_dir: &Path) -> agent_core::Result<()> {
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
