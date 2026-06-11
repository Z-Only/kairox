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

#[test]
fn build_git_context_includes_branch_diff_drafts_and_blame() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    run_git(root, &["init"]);
    run_git(root, &["config", "user.email", "tester@example.com"]);
    run_git(root, &["config", "user.name", "Tester"]);
    std::fs::write(root.join("src.txt"), "original\n").unwrap();
    run_git(root, &["add", "src.txt"]);
    run_git(root, &["commit", "-m", "initial commit"]);
    run_git(root, &["checkout", "-b", "feat/git-context"]);
    std::fs::write(root.join("src.txt"), "original\nchanged\n").unwrap();
    run_git(root, &["add", "src.txt"]);
    std::fs::write(root.join("src.txt"), "original\nchanged\nunstaged\n").unwrap();
    std::fs::write(root.join("notes.txt"), "draft\n").unwrap();

    let context = build_git_context(root, &["user: finish git context".to_string()])
        .expect("git context should be available");

    assert!(context.contains("Branch: feat/git-context"));
    assert!(context.contains("Staged changes"));
    assert!(context.contains("Unstaged changes"));
    assert!(context.contains("Commit message draft"));
    assert!(context.contains("PR description draft"));
    assert!(context.contains("Blame context"));
    assert!(context.contains("src.txt"));
    assert!(context.contains("finish git context"));
}

#[test]
fn get_git_review_includes_staged_unstaged_and_untracked_diffs() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    run_git(root, &["init"]);
    run_git(root, &["config", "user.email", "tester@example.com"]);
    run_git(root, &["config", "user.name", "Tester"]);
    std::fs::write(root.join("src.txt"), "original\n").unwrap();
    run_git(root, &["add", "src.txt"]);
    run_git(root, &["commit", "-m", "initial commit"]);
    run_git(root, &["checkout", "-b", "feat/git-review"]);

    std::fs::write(root.join("src.txt"), "original\nstaged\n").unwrap();
    run_git(root, &["add", "src.txt"]);
    std::fs::write(root.join("src.txt"), "original\nstaged\nunstaged\n").unwrap();
    std::fs::write(root.join("notes.txt"), "draft\n").unwrap();

    let review = get_git_review(root.to_string_lossy().as_ref());

    assert_eq!(review.kind, agent_core::ProjectGitStatusKind::Dirty);
    assert_eq!(review.branch.as_deref(), Some("feat/git-review"));
    assert!(review.changed_files.iter().any(|file| file == "src.txt"));
    assert!(review.changed_files.iter().any(|file| file == "notes.txt"));

    let staged = review.staged.expect("staged diff should be present");
    assert_eq!(staged.label, "Staged changes");
    assert!(staged.stat.contains("src.txt"));
    assert!(staged.diff.contains("+staged"));

    let unstaged = review.unstaged.expect("unstaged diff should be present");
    assert_eq!(unstaged.label, "Unstaged changes");
    assert!(unstaged.diff.contains("+unstaged"));

    let untracked = review.untracked.expect("untracked diff should be present");
    assert_eq!(untracked.label, "Untracked files");
    assert!(untracked.diff.contains("+++ b/notes.txt"));
    assert!(untracked.diff.contains("+draft"));
}

#[test]
fn get_git_review_truncates_large_untracked_file_preview() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    run_git(root, &["init"]);
    run_git(root, &["config", "user.email", "tester@example.com"]);
    run_git(root, &["config", "user.name", "Tester"]);
    std::fs::write(root.join("src.txt"), "original\n").unwrap();
    run_git(root, &["add", "src.txt"]);
    run_git(root, &["commit", "-m", "initial commit"]);
    std::fs::write(root.join("large.txt"), "x\n".repeat(40_000)).unwrap();

    let review = get_git_review(root.to_string_lossy().as_ref());
    let untracked = review.untracked.expect("untracked diff should be present");

    assert!(untracked.stat.contains("preview truncated"));
    assert!(untracked.diff.contains("[...truncated]"));
    assert!(untracked.diff.len() < 12_000);
}

#[test]
fn changed_files_preserves_unstaged_porcelain_leading_space() {
    let status = " M README.md\n?? VIBE_REVIEW_NOTES.md\n";
    let changed_files = changed_files_from_status(status);

    assert_eq!(
        changed_files,
        vec!["README.md".to_string(), "VIBE_REVIEW_NOTES.md".to_string()]
    );
}

fn run_git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
