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
