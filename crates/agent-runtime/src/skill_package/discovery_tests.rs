use std::path::PathBuf;

use super::*;

// ── parse_github_skill_source ───────────────────────────────────────

#[test]
fn shorthand_owner_repo() {
    let src = parse_github_skill_source("acme/repo").expect("should parse");
    assert_eq!(src.clone_url, "https://github.com/acme/repo.git");
    assert_eq!(src.branch, None);
    assert_eq!(src.skill_subdir, PathBuf::new());
    assert_eq!(src.directory_name, "repo");
}

#[test]
fn shorthand_owner_repo_with_subdir() {
    let src = parse_github_skill_source("acme/repo/skills/review").expect("should parse");
    assert_eq!(src.clone_url, "https://github.com/acme/repo.git");
    assert_eq!(src.branch, None);
    assert_eq!(src.skill_subdir, PathBuf::from("skills/review"));
    assert_eq!(src.directory_name, "review");
}

#[test]
fn https_url_basic() {
    let src = parse_github_skill_source("https://github.com/acme/repo").expect("should parse URL");
    assert_eq!(src.clone_url, "https://github.com/acme/repo.git");
    assert_eq!(src.branch, None);
    assert_eq!(src.skill_subdir, PathBuf::new());
    assert_eq!(src.directory_name, "repo");
}

#[test]
fn https_url_strips_dot_git() {
    let src =
        parse_github_skill_source("https://github.com/acme/repo.git").expect("should strip .git");
    assert_eq!(src.clone_url, "https://github.com/acme/repo.git");
    assert_eq!(src.branch, None);
    assert_eq!(src.directory_name, "repo");
}

#[test]
fn https_url_tree_with_branch_and_subdir() {
    let src = parse_github_skill_source("https://github.com/acme/repo/tree/main/skills/foo")
        .expect("should parse tree URL");
    assert_eq!(src.clone_url, "https://github.com/acme/repo.git");
    assert_eq!(src.branch.as_deref(), Some("main"));
    assert_eq!(src.skill_subdir, PathBuf::from("skills/foo"));
    assert_eq!(src.directory_name, "foo");
}

#[test]
fn https_url_blob_with_skill_md() {
    let src =
        parse_github_skill_source("https://github.com/acme/repo/blob/main/skills/foo/SKILL.md")
            .expect("should parse blob SKILL.md URL");
    assert_eq!(src.clone_url, "https://github.com/acme/repo.git");
    assert_eq!(src.branch.as_deref(), Some("main"));
    assert_eq!(src.skill_subdir, PathBuf::from("skills/foo"));
    assert_eq!(src.directory_name, "foo");
}

#[test]
fn https_url_blob_non_skill_md_rejected() {
    let err =
        parse_github_skill_source("https://github.com/acme/repo/blob/main/skills/foo/README.md")
            .expect_err("non-SKILL.md blob should fail");
    assert!(err.to_string().contains("SKILL.md"));
}

#[test]
fn empty_input_rejected() {
    let err = parse_github_skill_source("").expect_err("empty should fail");
    assert!(err.to_string().contains("empty"));
}

#[test]
fn whitespace_only_rejected() {
    let err = parse_github_skill_source("   ").expect_err("whitespace should fail");
    assert!(err.to_string().contains("empty"));
}

#[test]
fn non_github_host_rejected() {
    let err = parse_github_skill_source("https://gitlab.com/acme/repo")
        .expect_err("non-github host should fail");
    assert!(err.to_string().contains("unsupported"));
}

#[test]
fn single_segment_no_slash_rejected() {
    let err = parse_github_skill_source("just-a-name").expect_err("single segment should fail");
    assert!(err.to_string().contains("owner/repo"));
}

#[test]
fn leading_and_trailing_slashes_trimmed() {
    let src = parse_github_skill_source("/acme/repo/").expect("should trim slashes");
    assert_eq!(src.clone_url, "https://github.com/acme/repo.git");
    assert_eq!(src.branch, None);
    assert_eq!(src.skill_subdir, PathBuf::new());
    assert_eq!(src.directory_name, "repo");
}

#[test]
fn tree_url_with_deep_subdir() {
    let src = parse_github_skill_source("https://github.com/acme/repo/tree/dev/a/b/c")
        .expect("should parse deep tree");
    assert_eq!(src.branch.as_deref(), Some("dev"));
    assert_eq!(src.skill_subdir, PathBuf::from("a/b/c"));
    assert_eq!(src.directory_name, "c");
}

#[test]
fn shorthand_with_trailing_whitespace_trimmed() {
    let src = parse_github_skill_source("  acme/repo  ").expect("should trim");
    assert_eq!(src.clone_url, "https://github.com/acme/repo.git");
    assert_eq!(src.directory_name, "repo");
}

#[test]
fn tree_url_no_subdir_uses_repo_as_directory_name() {
    let src = parse_github_skill_source("https://github.com/acme/repo/tree/main")
        .expect("tree with no subdir");
    assert_eq!(src.branch.as_deref(), Some("main"));
    assert_eq!(src.skill_subdir, PathBuf::new());
    assert_eq!(src.directory_name, "repo");
}

#[test]
fn blob_url_skill_md_at_repo_root() {
    let src = parse_github_skill_source("https://github.com/acme/repo/blob/main/SKILL.md")
        .expect("root SKILL.md blob");
    assert_eq!(src.branch.as_deref(), Some("main"));
    assert_eq!(src.skill_subdir, PathBuf::new());
    assert_eq!(src.directory_name, "repo");
}

// ── skill_directory_name ────────────────────────────────────────────

#[test]
fn directory_name_simple_unchanged() {
    assert_eq!(skill_directory_name("code-review"), "code-review");
}

#[test]
fn directory_name_url_extracts_repo() {
    assert_eq!(
        skill_directory_name("https://github.com/owner/repo.git"),
        "repo"
    );
}

#[test]
fn directory_name_special_chars_sanitized() {
    assert_eq!(skill_directory_name("my.skill@v2"), "my-skill-v2");
}

#[test]
fn directory_name_empty_after_sanitize_falls_back() {
    assert_eq!(skill_directory_name("..."), "skill");
}

#[test]
fn directory_name_trailing_slash_stripped() {
    assert_eq!(skill_directory_name("my-skill/"), "my-skill");
}

#[test]
fn directory_name_scoped_package() {
    assert_eq!(skill_directory_name("@scope/package"), "package");
}

#[test]
fn directory_name_underscores_preserved() {
    assert_eq!(skill_directory_name("my_skill"), "my_skill");
}
