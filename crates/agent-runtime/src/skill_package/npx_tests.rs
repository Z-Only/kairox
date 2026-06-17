use std::path::PathBuf;

use agent_core::facade::{SkillInstallTarget, SkillUpdateState};

use super::*;

// ── parse_npx_skills_find_output ───────────────────────────────────

#[test]
fn find_output_parses_full_four_column_row() {
    let output = "my-skill\tA cool skill\thttps://github.com/acme/skills\t500\n";
    let results = parse_npx_skills_find_output(output).expect("should parse");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "my-skill");
    assert_eq!(results[0].description, "A cool skill");
    assert_eq!(
        results[0].repository.as_deref(),
        Some("https://github.com/acme/skills")
    );
    assert_eq!(results[0].install_count, Some(500));
    assert_eq!(results[0].source_url, "https://github.com/acme/skills");
    assert_eq!(results[0].package, "my-skill");
}

#[test]
fn find_output_parses_two_column_row() {
    let output = "my-skill\tA cool skill\n";
    let results = parse_npx_skills_find_output(output).expect("should parse");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "my-skill");
    assert_eq!(results[0].description, "A cool skill");
    assert_eq!(results[0].repository, None);
    assert_eq!(results[0].install_count, None);
    assert_eq!(results[0].source_url, "my-skill");
    assert_eq!(results[0].package, "my-skill");
}

#[test]
fn find_output_parses_multiple_rows() {
    let output = "skill-a\tFirst skill\trepo-a\t10\nskill-b\tSecond skill\trepo-b\t20\n";
    let results = parse_npx_skills_find_output(output).expect("should parse");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "skill-a");
    assert_eq!(results[1].name, "skill-b");
    assert_eq!(results[0].install_count, Some(10));
    assert_eq!(results[1].install_count, Some(20));
}

#[test]
fn find_output_returns_empty_for_empty_input() {
    let results = parse_npx_skills_find_output("").expect("should parse");
    assert!(results.is_empty());
}

#[test]
fn find_output_returns_empty_for_whitespace_only_lines() {
    let results = parse_npx_skills_find_output("  \n\n  \n").expect("should parse");
    assert!(results.is_empty());
}

#[test]
fn find_output_skips_single_column_lines() {
    let output = "header_only\nmy-skill\tA cool skill\n";
    let results = parse_npx_skills_find_output(output).expect("should parse");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "my-skill");
}

#[test]
fn find_output_empty_repository_column_produces_none() {
    let output = "my-skill\tDescription\t\t42\n";
    let results = parse_npx_skills_find_output(output).expect("should parse");
    assert_eq!(results[0].repository, None);
    assert_eq!(results[0].install_count, Some(42));
    assert_eq!(results[0].source_url, "my-skill");
}

#[test]
fn find_output_empty_install_count_column_produces_none() {
    let output = "my-skill\tDescription\trepo\t\n";
    let results = parse_npx_skills_find_output(output).expect("should parse");
    assert_eq!(results[0].install_count, None);
    assert_eq!(results[0].repository.as_deref(), Some("repo"));
}

#[test]
fn find_output_rejects_non_numeric_install_count() {
    let error = parse_npx_skills_find_output("my-skill\tDescription\trepo\tabc\n")
        .expect_err("should reject non-numeric install_count");
    let message = error.to_string();
    assert!(message.contains("install_count"), "message was: {message}");
    assert!(message.contains("line 1"), "message was: {message}");
}

#[test]
fn find_output_trims_whitespace_from_columns() {
    let output = "  my-skill  \t  A cool skill  \t  repo  \t  42  \n";
    let results = parse_npx_skills_find_output(output).expect("should parse");
    assert_eq!(results[0].name, "my-skill");
    assert_eq!(results[0].description, "A cool skill");
    assert_eq!(results[0].repository.as_deref(), Some("repo"));
    assert_eq!(results[0].install_count, Some(42));
}

#[test]
fn find_output_large_install_count_parses() {
    let output = "my-skill\tDescription\trepo\t9999999999\n";
    let results = parse_npx_skills_find_output(output).expect("should parse");
    assert_eq!(results[0].install_count, Some(9_999_999_999));
}

// ── classify_npx_spawn_error ───────────────────────────────────────

#[test]
fn classify_not_found_error_mentions_npx_and_not_found() {
    let error = classify_npx_spawn_error(std::io::Error::from(std::io::ErrorKind::NotFound));
    let message = error.to_string();
    assert!(message.contains("npx"), "message was: {message}");
    assert!(message.contains("not found"), "message was: {message}");
}

#[test]
fn classify_permission_denied_error_wraps_original() {
    let error = classify_npx_spawn_error(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "permission denied",
    ));
    let message = error.to_string();
    assert!(
        message.contains("failed to run npx"),
        "message was: {message}"
    );
    assert!(
        message.contains("permission denied"),
        "message was: {message}"
    );
}

#[test]
fn classify_other_io_error_wraps_original() {
    let error = classify_npx_spawn_error(std::io::Error::other("something weird happened"));
    let message = error.to_string();
    assert!(
        message.contains("failed to run npx"),
        "message was: {message}"
    );
    assert!(
        message.contains("something weird happened"),
        "message was: {message}"
    );
}

// ── parse_npx_skills_check_output ──────────────────────────────────

#[test]
fn check_output_detects_update_available() {
    assert_eq!(
        parse_npx_skills_check_output("Update available for skill-x v1.2.3"),
        SkillUpdateState::UpdateAvailable
    );
}

#[test]
fn check_output_detects_outdated() {
    assert_eq!(
        parse_npx_skills_check_output("Package is outdated"),
        SkillUpdateState::UpdateAvailable
    );
}

#[test]
fn check_output_detects_up_to_date_with_spaces() {
    assert_eq!(
        parse_npx_skills_check_output("Skill is up to date"),
        SkillUpdateState::UpToDate
    );
}

#[test]
fn check_output_detects_up_to_date_hyphenated() {
    assert_eq!(
        parse_npx_skills_check_output("Status: up-to-date"),
        SkillUpdateState::UpToDate
    );
}

#[test]
fn check_output_is_case_insensitive() {
    assert_eq!(
        parse_npx_skills_check_output("UPDATE AVAILABLE"),
        SkillUpdateState::UpdateAvailable
    );
    assert_eq!(
        parse_npx_skills_check_output("UP TO DATE"),
        SkillUpdateState::UpToDate
    );
}

#[test]
fn check_output_returns_unknown_for_unrecognized_text() {
    assert_eq!(
        parse_npx_skills_check_output("some unrelated output"),
        SkillUpdateState::Unknown
    );
}

#[test]
fn check_output_returns_unknown_for_empty_string() {
    assert_eq!(parse_npx_skills_check_output(""), SkillUpdateState::Unknown);
}

#[test]
fn check_output_update_available_takes_priority_when_both_match() {
    // If output somehow contains both phrases, "update available" appears first
    // in the check order so it wins.
    assert_eq!(
        parse_npx_skills_check_output("update available but also up to date"),
        SkillUpdateState::UpdateAvailable
    );
}

// ── format_npx_exit_error ──────────────────────────────────────────

#[test]
fn exit_error_includes_full_command_and_status() {
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(256);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(1);

    let error = format_npx_exit_error(&["skills", "add", "my-skill"], status, b"some error");
    let message = error.to_string();
    assert!(
        message.contains("npx skills add my-skill"),
        "message was: {message}"
    );
    assert!(message.contains("exit"), "message was: {message}");
    assert!(message.contains("some error"), "message was: {message}");
}

#[test]
fn exit_error_shows_empty_marker_for_no_stderr() {
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(256);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(1);

    let error = format_npx_exit_error(&["skills", "find", "x"], status, b"");
    let message = error.to_string();
    assert!(message.contains("<empty>"), "message was: {message}");
}

#[test]
fn exit_error_shows_empty_marker_for_whitespace_only_stderr() {
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(256);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(1);

    let error = format_npx_exit_error(&["skills", "find", "x"], status, b"   \n  ");
    let message = error.to_string();
    assert!(message.contains("<empty>"), "message was: {message}");
}

#[test]
fn exit_error_truncates_long_stderr() {
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(256);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(1);

    let long_stderr = "e".repeat(600);
    let error = format_npx_exit_error(&["skills", "find", "x"], status, long_stderr.as_bytes());
    let message = error.to_string();
    assert!(message.contains("<truncated>"), "message was: {message}");
}

#[test]
fn exit_error_does_not_truncate_short_stderr() {
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(256);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(1);

    let short_stderr = "short error";
    let error = format_npx_exit_error(&["skills", "update", "x"], status, short_stderr.as_bytes());
    let message = error.to_string();
    assert!(message.contains("short error"), "message was: {message}");
    assert!(
        !message.contains("<truncated>"),
        "short stderr should not be truncated, message was: {message}"
    );
}

// ── install_working_directory ──────────────────────────────────────

#[test]
fn working_directory_returns_parent_for_project_target() {
    let install_root = PathBuf::from("/home/user/project/.kairox/skills");
    let result = install_working_directory(&install_root, SkillInstallTarget::Project);
    assert_eq!(result, Some(PathBuf::from("/home/user/project/.kairox")));
}

#[test]
fn working_directory_returns_none_for_user_target() {
    let install_root = PathBuf::from("/home/user/.kairox/skills");
    let result = install_working_directory(&install_root, SkillInstallTarget::User);
    assert_eq!(result, None);
}

// ── append_install_target_args ─────────────────────────────────────

#[test]
fn append_target_args_adds_project_flag() {
    let mut args = vec!["skills", "add", "my-skill"];
    append_install_target_args(&mut args, SkillInstallTarget::Project);
    assert_eq!(args, vec!["skills", "add", "my-skill", "--project"]);
}

#[test]
fn append_target_args_adds_user_flag() {
    let mut args = vec!["skills", "add", "my-skill"];
    append_install_target_args(&mut args, SkillInstallTarget::User);
    assert_eq!(args, vec!["skills", "add", "my-skill", "--user"]);
}

// ── parse_install_count ────────────────────────────────────────────

#[test]
fn parse_install_count_valid_number() {
    assert_eq!(
        parse_install_count("42", 1).expect("should parse"),
        Some(42)
    );
}

#[test]
fn parse_install_count_zero() {
    assert_eq!(parse_install_count("0", 1).expect("should parse"), Some(0));
}

#[test]
fn parse_install_count_empty_returns_none() {
    assert_eq!(parse_install_count("", 1).expect("should parse"), None);
}

#[test]
fn parse_install_count_whitespace_only_returns_none() {
    assert_eq!(parse_install_count("   ", 1).expect("should parse"), None);
}

#[test]
fn parse_install_count_invalid_includes_line_number() {
    let error = parse_install_count("abc", 7).expect_err("should fail");
    let message = error.to_string();
    assert!(message.contains("line 7"), "message was: {message}");
    assert!(message.contains("install_count"), "message was: {message}");
}

#[test]
fn parse_install_count_negative_fails() {
    let error = parse_install_count("-1", 1).expect_err("should fail");
    assert!(error.to_string().contains("install_count"));
}

// ── optional_column ────────────────────────────────────────────────

#[test]
fn optional_column_non_empty_returns_some() {
    assert_eq!(optional_column("hello"), Some("hello".to_string()));
}

#[test]
fn optional_column_empty_returns_none() {
    assert_eq!(optional_column(""), None);
}

#[test]
fn optional_column_whitespace_only_returns_none() {
    assert_eq!(optional_column("   "), None);
}

#[test]
fn optional_column_trims_surrounding_whitespace() {
    assert_eq!(optional_column("  value  "), Some("value".to_string()));
}
