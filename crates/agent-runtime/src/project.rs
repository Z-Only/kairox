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
        visibility: Some(visibility_from_storage(&row.visibility)),
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
    let mut warning = None;

    for candidate in INSTRUCTION_FILE_PRIORITY {
        let path = root_path.join(candidate);
        match tokio::fs::metadata(&path).await {
            Ok(metadata) if metadata.is_file() => source_paths.push(path.display().to_string()),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => warning = Some(error.to_string()),
        }
    }

    ProjectInstructionSummary {
        source_paths,
        warning,
    }
}

pub fn default_blank_project_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Kairox Projects")
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
        "Untitled Project".into()
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
        tokio::fs::write(temp.path().join("README.md"), "readme")
            .await
            .unwrap();
        tokio::fs::write(temp.path().join("AGENTS.md"), "agents")
            .await
            .unwrap();

        let summary = read_project_instruction_summary(temp.path()).await;

        assert_eq!(
            summary.source_paths[0],
            temp.path().join("AGENTS.md").display().to_string()
        );
        assert_eq!(
            summary.source_paths[1],
            temp.path().join("README.md").display().to_string()
        );
        assert!(summary.warning.is_none());
    }
}
