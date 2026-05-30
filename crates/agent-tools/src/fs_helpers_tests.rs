use super::*;

fn temp_workspace() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn canon_root(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().canonicalize().unwrap()
}

// -----------------------------------------------------------------------
// resolve_workspace_read_path
// -----------------------------------------------------------------------

#[test]
fn read_path_resolves_existing_file() {
    let dir = temp_workspace();
    let file = dir.path().join("test.txt");
    std::fs::write(&file, "hi").unwrap();
    let resolved = resolve_workspace_read_path(&canon_root(&dir), "test.txt").unwrap();
    assert!(resolved.starts_with(canon_root(&dir)));
}

#[test]
fn read_path_rejects_escape() {
    let dir = temp_workspace();
    let outside = dir.path().join("outside.txt");
    std::fs::write(&outside, "secret").unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();
    let result = resolve_workspace_read_path(&workspace, "../outside.txt");
    assert!(result.is_err());
}

#[test]
fn read_path_rejects_nonexistent() {
    let dir = temp_workspace();
    let result = resolve_workspace_read_path(&canon_root(&dir), "nope.txt");
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// resolve_workspace_write_path
// -----------------------------------------------------------------------

#[test]
fn write_path_allows_new_file_in_subdir() {
    let dir = temp_workspace();
    let root = canon_root(&dir);
    std::fs::create_dir(root.join("sub")).unwrap();
    let resolved = resolve_workspace_write_path(&root, "sub/new.txt").unwrap();
    assert_eq!(resolved, root.join("sub/new.txt"));
}

#[test]
fn write_path_allows_new_file_in_root() {
    let dir = temp_workspace();
    let root = canon_root(&dir);
    let resolved = resolve_workspace_write_path(&root, "new.txt").unwrap();
    assert_eq!(resolved, root.join("new.txt"));
}

#[test]
fn write_path_allows_overwrite_existing() {
    let dir = temp_workspace();
    let root = canon_root(&dir);
    std::fs::write(root.join("existing.txt"), "old").unwrap();
    let resolved = resolve_workspace_write_path(&root, "existing.txt").unwrap();
    assert!(resolved.starts_with(&root));
}

#[test]
fn write_path_rejects_dot_dot() {
    let dir = temp_workspace();
    let root = canon_root(&dir);
    let result = resolve_workspace_write_path(&root, "../escape.txt");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("escape") || msg.contains("WorkspaceEscape"),
        "Expected escape error, got: {msg}"
    );
}

#[test]
fn write_path_rejects_embedded_dot_dot() {
    let dir = temp_workspace();
    let root = canon_root(&dir);
    let result = resolve_workspace_write_path(&root, "sub/../../escape.txt");
    assert!(result.is_err());
}

#[test]
fn write_path_allows_deep_new_file() {
    let dir = temp_workspace();
    let root = canon_root(&dir);
    // Neither sub/nor/deep exist yet; this should be OK since we rejected ".."
    let resolved = resolve_workspace_write_path(&root, "sub/nor/deep/file.txt").unwrap();
    assert_eq!(resolved, root.join("sub/nor/deep/file.txt"));
}

// -----------------------------------------------------------------------
// Additional path validation tests
// -----------------------------------------------------------------------

#[test]
fn resolve_read_path_rejects_parent_traversal() {
    let dir = temp_workspace();
    // Create a file outside the workspace subdirectory
    let outside_file = dir.path().join("sensitive.txt");
    std::fs::write(&outside_file, "secret").unwrap();
    // Create a workspace subdirectory
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();

    let result = resolve_workspace_read_path(&workspace, "../sensitive.txt");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::ToolError::WorkspaceEscape(_)
    ));
}

#[test]
fn resolve_read_path_rejects_absolute_in_relative() {
    let dir = temp_workspace();
    let workspace = canon_root(&dir);
    // Passing an absolute path as the relative_path argument: Path::join
    // replaces the entire path with the absolute one, which then
    // resolves outside the workspace root.
    let result = resolve_workspace_read_path(&workspace, "/etc/passwd");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::ToolError::WorkspaceEscape(_)
    ));
}

#[test]
fn resolve_read_path_allows_normal_file() {
    let dir = temp_workspace();
    let workspace = canon_root(&dir);
    // Create src/main.rs inside the workspace
    let src_dir = workspace.join("src");
    std::fs::create_dir(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

    let result = resolve_workspace_read_path(&workspace, "src/main.rs").unwrap();
    assert!(result.ends_with("src/main.rs"));
    assert!(result.starts_with(&workspace));
}

#[test]
fn resolve_write_path_same_rules() {
    let dir = temp_workspace();
    let workspace = canon_root(&dir);
    // write path rejects ".." components before touching the filesystem
    let result = resolve_workspace_write_path(&workspace, "../escape");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::ToolError::WorkspaceEscape(_)
    ));
}
