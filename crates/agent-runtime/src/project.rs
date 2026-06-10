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
        approval_policy: row.approval_policy.clone(),
        sandbox_policy: row.sandbox_policy.clone(),
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

pub fn current_git_branch(root_path: &Path) -> Option<String> {
    if !is_git_worktree(root_path) {
        return None;
    }

    let branch = run_git_text(root_path, &["branch", "--show-current"]).unwrap_or_default();
    if !branch.is_empty() {
        return Some(branch);
    }

    run_git_text(root_path, &["rev-parse", "--short", "HEAD"])
        .filter(|sha| !sha.is_empty())
        .map(|sha| format!("detached@{sha}"))
}

pub fn build_git_context(root_path: &Path, conversation_context: &[String]) -> Option<String> {
    if !is_git_worktree(root_path) {
        return None;
    }

    let branch = current_git_branch(root_path).unwrap_or_else(|| "unknown".into());
    let status = run_git_text(root_path, &["status", "--porcelain=v1"]).unwrap_or_default();
    let changed_files = changed_files_from_status(&status);
    let staged_diff = diff_section(
        root_path,
        "Staged changes",
        &["diff", "--cached", "--stat"],
        &["diff", "--cached", "--no-ext-diff", "--unified=3", "--"],
    );
    let unstaged_diff = diff_section(
        root_path,
        "Unstaged changes",
        &["diff", "--stat"],
        &["diff", "--no-ext-diff", "--unified=3", "--"],
    );
    let recent_commits =
        run_git_text(root_path, &["log", "--oneline", "--decorate", "-5"]).unwrap_or_default();
    let commit_draft = draft_commit_message(&branch, &changed_files, conversation_context);
    let pr_draft = draft_pr_description(&branch, &changed_files, conversation_context);
    let blame_context = blame_context(root_path, &changed_files);

    let mut sections = vec![
        "Repository git context".to_string(),
        format!("Branch: {branch}"),
    ];

    if status.trim().is_empty() {
        sections.push("Working tree status: clean".into());
    } else {
        sections.push(format!(
            "Working tree status:\n{}",
            truncate_chars(&status, 2_000)
        ));
    }

    if !recent_commits.is_empty() {
        sections.push(format!("Recent commits:\n{recent_commits}"));
    }
    if !staged_diff.is_empty() {
        sections.push(staged_diff);
    }
    if !unstaged_diff.is_empty() {
        sections.push(unstaged_diff);
    }
    sections.push(format!("Commit message draft:\n{commit_draft}"));
    sections.push(format!("PR description draft:\n{pr_draft}"));
    if !blame_context.is_empty() {
        sections.push(format!("Blame context:\n{blame_context}"));
    }

    Some(truncate_chars(&sections.join("\n\n"), 16_000))
}

fn is_git_worktree(root_path: &Path) -> bool {
    matches!(
        run_git_text(root_path, &["rev-parse", "--is-inside-work-tree"]).as_deref(),
        Some("true")
    )
}

fn run_git_text(root_path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root_path)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn changed_files_from_status(status: &str) -> Vec<String> {
    let mut files = Vec::new();
    for line in status.lines() {
        let path = line.get(3..).unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }
        let path = path
            .rsplit_once(" -> ")
            .map(|(_, after)| after)
            .unwrap_or(path);
        if !files.iter().any(|file| file == path) {
            files.push(path.to_string());
        }
        if files.len() >= 24 {
            break;
        }
    }
    files
}

fn diff_section(root_path: &Path, label: &str, stat_args: &[&str], diff_args: &[&str]) -> String {
    let stat = run_git_text(root_path, stat_args).unwrap_or_default();
    let diff = run_git_text(root_path, diff_args).unwrap_or_default();
    if stat.is_empty() && diff.is_empty() {
        return String::new();
    }

    let mut parts = vec![format!("{label}:")];
    if !stat.is_empty() {
        parts.push(format!("Stat:\n{}", truncate_chars(&stat, 2_000)));
    }
    if !diff.is_empty() {
        parts.push(format!("Diff:\n{}", truncate_chars(&diff, 6_000)));
    }
    parts.join("\n")
}

fn draft_commit_message(
    branch: &str,
    changed_files: &[String],
    conversation_context: &[String],
) -> String {
    let scope = infer_commit_scope(changed_files);
    let file_summary = summarize_changed_files(changed_files);
    let cue = latest_conversation_cue(conversation_context);
    let subject = if file_summary == "working tree" {
        format!("feat({scope}): update git-aware context")
    } else {
        format!("feat({scope}): update {file_summary}")
    };

    let mut lines = vec![subject, String::new(), format!("- Branch: {branch}")];
    if !changed_files.is_empty() {
        lines.push(format!("- Changed files: {}", changed_files.join(", ")));
    }
    if let Some(cue) = cue {
        lines.push(format!("- Conversation cue: {cue}"));
    }
    lines.join("\n")
}

fn draft_pr_description(
    branch: &str,
    changed_files: &[String],
    conversation_context: &[String],
) -> String {
    let file_summary = summarize_changed_files(changed_files);
    let cue = latest_conversation_cue(conversation_context)
        .unwrap_or_else(|| "No recent conversation cue available".into());
    format!(
        "## Summary\n- Update {file_summary} on `{branch}`\n- Conversation context: {cue}\n\n## Testing\n- Not run; draft generated from local git context"
    )
}

fn infer_commit_scope(changed_files: &[String]) -> &'static str {
    if changed_files
        .iter()
        .any(|file| file.contains("agent-memory"))
    {
        "memory"
    } else if changed_files
        .iter()
        .any(|file| file.contains("agent-runtime"))
    {
        "runtime"
    } else if changed_files
        .iter()
        .any(|file| file.starts_with("apps/agent-gui"))
    {
        "gui"
    } else if changed_files.iter().any(|file| file.starts_with("docs/")) {
        "docs"
    } else {
        "git"
    }
}

fn summarize_changed_files(changed_files: &[String]) -> String {
    match changed_files {
        [] => "working tree".into(),
        [one] => one.clone(),
        files if files.len() <= 3 => files.join(", "),
        files => format!("{}, and {} more files", files[0], files.len() - 1),
    }
}

fn latest_conversation_cue(conversation_context: &[String]) -> Option<String> {
    conversation_context
        .iter()
        .rev()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(|line| truncate_chars(line, 240))
}

fn blame_context(root_path: &Path, changed_files: &[String]) -> String {
    let mut lines = Vec::new();
    for file in changed_files.iter().take(8) {
        let last_commit = run_git_text(
            root_path,
            &["log", "-1", "--format=%h %an %ar %s", "--", file],
        )
        .unwrap_or_else(|| "untracked or not committed yet".into());
        lines.push(format!("{file}: {last_commit}"));
    }
    lines.join("\n")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut truncated: String = text.chars().take(max_chars).collect();
    truncated.push_str("\n[...truncated]");
    truncated
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
#[path = "project_tests.rs"]
mod tests;
