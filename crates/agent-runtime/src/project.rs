use agent_core::{
    CoreError, ProjectGitStatus, ProjectGitStatusKind, ProjectId, ProjectInstructionSummary,
    ProjectMeta, ProjectSessionVisibility, SessionId, SessionMeta, WorkspaceId,
};
use agent_store::{event_store::ProjectSessionMetaRow, ProjectRow};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

const INSTRUCTION_FILE_PRIORITY: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    ".cursorrules",
    "GEMINI.md",
    ".windsurfrules",
    "README.md",
    "README.zh-CN.md",
];

pub fn project_row_to_meta(row: ProjectRow) -> ProjectMeta {
    ProjectMeta {
        project_id: ProjectId::from_string(row.project_id),
        display_name: row.display_name,
        root_path: row.root_path,
        created_at: row.created_at,
        updated_at: row.updated_at,
        removed_at: row.removed_at,
        sort_order: row.sort_order,
        expanded: row.expanded,
    }
}

pub fn project_session_row_to_meta(row: ProjectSessionMetaRow) -> SessionMeta {
    SessionMeta {
        project_id: Some(ProjectId::from_string(row.project_id)),
        worktree_path: Some(row.worktree_path),
        branch: row.branch,
        visibility: Some(visibility_from_storage(&row.visibility)),
        permission_mode: Some(row.permission_mode.clone()),
        session_id: SessionId::from_string(row.session_id),
        workspace_id: WorkspaceId::from_string(row.workspace_id),
        title: row.title,
        model_profile: row.model_profile,
        model_id: row.model_id,
        provider: row.provider,
        deleted_at: row.deleted_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn visibility_from_storage(value: &str) -> ProjectSessionVisibility {
    match value {
        "draft_hidden" => ProjectSessionVisibility::DraftHidden,
        "archived" => ProjectSessionVisibility::Archived,
        _ => ProjectSessionVisibility::Visible,
    }
}

pub fn visibility_to_storage(visibility: ProjectSessionVisibility) -> &'static str {
    match visibility {
        ProjectSessionVisibility::DraftHidden => "draft_hidden",
        ProjectSessionVisibility::Visible => "visible",
        ProjectSessionVisibility::Archived => "archived",
    }
}

pub fn get_git_status(path: &str) -> ProjectGitStatus {
    let root_path = Path::new(path);
    if !root_path.exists() {
        return ProjectGitStatus {
            kind: ProjectGitStatusKind::MissingPath,
            branch: None,
            worktree_path: path.to_string(),
            message: Some("path does not exist".into()),
        };
    }

    if !root_path.join(".git").exists() {
        return ProjectGitStatus {
            kind: ProjectGitStatusKind::NotInitialized,
            branch: None,
            worktree_path: path.to_string(),
            message: None,
        };
    }

    let branch_output = Command::new("git")
        .args(["-C", path, "branch", "--show-current"])
        .output();
    let branch_output = match branch_output {
        Ok(output) => output,
        Err(error) => {
            return ProjectGitStatus {
                kind: ProjectGitStatusKind::Error,
                branch: None,
                worktree_path: path.to_string(),
                message: Some(error.to_string()),
            };
        }
    };

    if !branch_output.status.success() {
        return ProjectGitStatus {
            kind: ProjectGitStatusKind::Error,
            branch: None,
            worktree_path: path.to_string(),
            message: Some(
                String::from_utf8_lossy(&branch_output.stderr)
                    .trim()
                    .to_string(),
            ),
        };
    }

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    if branch.is_empty() {
        return ProjectGitStatus {
            kind: ProjectGitStatusKind::Detached,
            branch: None,
            worktree_path: path.to_string(),
            message: None,
        };
    }

    match Command::new("git")
        .args(["-C", path, "status", "--porcelain"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let dirty = !String::from_utf8_lossy(&output.stdout).trim().is_empty();
            ProjectGitStatus {
                kind: if dirty {
                    ProjectGitStatusKind::Dirty
                } else {
                    ProjectGitStatusKind::Clean
                },
                branch: Some(branch),
                worktree_path: path.to_string(),
                message: None,
            }
        }
        Ok(output) => ProjectGitStatus {
            kind: ProjectGitStatusKind::Error,
            branch: Some(branch),
            worktree_path: path.to_string(),
            message: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(error) => ProjectGitStatus {
            kind: ProjectGitStatusKind::Error,
            branch: Some(branch),
            worktree_path: path.to_string(),
            message: Some(error.to_string()),
        },
    }
}

pub async fn read_project_instruction_summary(root_path: &Path) -> ProjectInstructionSummary {
    let mut source_paths = Vec::new();
    let mut content_parts: Vec<String> = Vec::new();
    let mut warning = None;

    for candidate in INSTRUCTION_FILE_PRIORITY {
        let path = root_path.join(candidate);
        match tokio::fs::metadata(&path).await {
            Ok(metadata) if metadata.is_file() => {
                let display_path = path.display().to_string();
                source_paths.push(display_path);
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        let header = format!("### Instructions from {candidate}\n\n");
                        let body = if content.len() > 64 * 1024 {
                            let truncated: String = content.chars().take(64 * 1024).collect();
                            format!("{truncated}\n\n[...truncated]")
                        } else {
                            content
                        };
                        content_parts.push(format!("{header}{body}"));
                    }
                    Err(error) => {
                        warning = Some(error.to_string());
                    }
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => warning = Some(error.to_string()),
        }
    }

    let contents = if content_parts.is_empty() {
        None
    } else {
        Some(content_parts.join("\n\n"))
    };

    ProjectInstructionSummary {
        source_paths,
        contents,
        warning,
    }
}

pub fn default_blank_project_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Kairox Projects")
}

pub fn worktree_dir(project_root: &str, branch: &str) -> PathBuf {
    let safe_branch = branch.replace('/', "-");
    Path::new(project_root)
        .join(".kairox")
        .join("worktrees")
        .join(safe_branch)
}

pub fn create_git_worktree(
    project_root: &str,
    branch: &str,
    worktree_path: &Path,
) -> Result<(), String> {
    let worktree_str = worktree_path.display().to_string();
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create worktree parent directory: {error}"))?;
    }
    let branch_ref = format!("refs/heads/{branch}");
    let branch_exists = Command::new("git")
        .args([
            "-C",
            project_root,
            "show-ref",
            "--verify",
            "--quiet",
            &branch_ref,
        ])
        .status()
        .map_err(|error| format!("failed to check branch existence: {error}"))?
        .success();
    let mut command = Command::new("git");
    command.args(["-C", project_root, "worktree", "add"]);
    if branch_exists {
        command.args([&worktree_str, branch]);
    } else {
        command.args(["-b", branch, &worktree_str, "HEAD"]);
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to run git worktree add: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if stderr.is_empty() { stdout } else { stderr };
        return Err(format!("git worktree add failed: {message}"));
    }
    Ok(())
}

pub fn list_git_branches(project_root: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["-C", project_root, "branch", "--format=%(refname:short)"])
        .output()
        .map_err(|error| format!("failed to run git branch: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if stderr.is_empty() { stdout } else { stderr };
        return Err(format!("git branch failed: {message}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

pub fn unique_blank_project_path(display_name: &str) -> PathBuf {
    let base_root = default_blank_project_root();
    let directory_name = sanitize_directory_name(display_name);
    let first_candidate = base_root.join(&directory_name);
    if !first_candidate.exists() {
        return first_candidate;
    }

    for suffix in 2.. {
        let candidate = base_root.join(format!("{directory_name} {suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search should always find a project path")
}

pub fn display_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Project")
        .to_string()
}

pub fn invalid_project_store_error() -> CoreError {
    CoreError::InvalidState("project metadata requires sqlite event store".into())
}

fn sanitize_directory_name(display_name: &str) -> String {
    let sanitized: String = display_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if sanitized.is_empty() {
        "New Project".into()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_project_instructions_in_priority_order() {
        let temp = tempfile::tempdir().unwrap();
        tokio::fs::write(temp.path().join("README.md"), "readme content")
            .await
            .unwrap();
        tokio::fs::write(temp.path().join("AGENTS.md"), "agents content")
            .await
            .unwrap();

        let summary = read_project_instruction_summary(temp.path()).await;

        // Priority: AGENTS.md before README.md
        assert_eq!(
            summary.source_paths[0],
            temp.path().join("AGENTS.md").display().to_string()
        );
        assert_eq!(
            summary.source_paths[1],
            temp.path().join("README.md").display().to_string()
        );
        assert!(summary.warning.is_none());

        let contents = summary.contents.expect("should have merged contents");
        assert!(contents.contains("### Instructions from AGENTS.md"));
        assert!(contents.contains("agents content"));
        assert!(contents.contains("### Instructions from README.md"));
        assert!(contents.contains("readme content"));
        let agents_pos = contents.find("AGENTS.md").unwrap();
        let readme_pos = contents.find("README.md").unwrap();
        assert!(agents_pos < readme_pos);
    }

    #[tokio::test]
    async fn returns_none_contents_when_no_files_exist() {
        let temp = tempfile::tempdir().unwrap();
        let summary = read_project_instruction_summary(temp.path()).await;
        assert!(summary.source_paths.is_empty());
        assert!(summary.contents.is_none());
        assert!(summary.warning.is_none());
    }

    #[tokio::test]
    async fn truncates_large_files() {
        let temp = tempfile::tempdir().unwrap();
        let big_content = "x".repeat(70_000);
        tokio::fs::write(temp.path().join("AGENTS.md"), &big_content)
            .await
            .unwrap();

        let summary = read_project_instruction_summary(temp.path()).await;
        let contents = summary.contents.unwrap();
        assert!(contents.contains("[...truncated]"));
        assert!(contents.len() < 70_000 + 200);
    }

    #[test]
    fn worktree_dir_uses_project_kairox_path() {
        let path = worktree_dir("/tmp/my-project", "feat/hello");
        assert_eq!(
            path,
            Path::new("/tmp/my-project/.kairox/worktrees/feat-hello")
        );
    }

    #[test]
    fn worktree_dir_uses_branch_name_as_directory() {
        let path = worktree_dir("/repo", "main");
        assert_eq!(path, Path::new("/repo/.kairox/worktrees/main"));
    }

    #[test]
    fn worktree_dir_replaces_slashes_with_dashes() {
        let path = worktree_dir("/repo", "feature/my-cool/branch");
        assert_eq!(
            path,
            Path::new("/repo/.kairox/worktrees/feature-my-cool-branch")
        );
    }
}
