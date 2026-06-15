use super::*;
use std::time::Duration;

// ── Constants ─────────────────────────────────────────────────────────

#[test]
fn default_timeout_secs_is_30() {
    assert_eq!(DEFAULT_TIMEOUT_SECS, 30);
}

#[test]
fn default_max_output_bytes_is_100kb() {
    assert_eq!(DEFAULT_MAX_OUTPUT_BYTES, 102_400);
}

#[test]
fn allowed_env_vars_contains_path() {
    assert!(ALLOWED_ENV_VARS.contains(&"PATH"));
}

#[test]
fn allowed_env_vars_contains_home() {
    assert!(ALLOWED_ENV_VARS.contains(&"HOME"));
}

#[test]
fn allowed_env_vars_contains_all_expected() {
    let expected = ["PATH", "HOME", "LANG", "TERM", "USER", "TMPDIR", "SHELL"];
    for var in &expected {
        assert!(
            ALLOWED_ENV_VARS.contains(var),
            "ALLOWED_ENV_VARS should contain {var}"
        );
    }
    assert_eq!(ALLOWED_ENV_VARS.len(), expected.len());
}

// ── default_timeout ───────────────────────────────────────────────────

#[test]
fn default_timeout_returns_30_seconds() {
    assert_eq!(default_timeout(), Duration::from_secs(30));
}

// ── default_max_output_bytes ──────────────────────────────────────────

#[test]
fn default_max_output_bytes_returns_100kb() {
    assert_eq!(default_max_output_bytes(), 102_400);
}

// ── truncate_bytes ────────────────────────────────────────────────────

#[test]
fn truncate_bytes_no_truncation_when_under_limit() {
    let data = b"hello world";
    let (result, truncated) = truncate_bytes(data, 100);
    assert_eq!(result, b"hello world");
    assert!(!truncated);
}

#[test]
fn truncate_bytes_no_truncation_when_exactly_at_limit() {
    let data = b"12345";
    let (result, truncated) = truncate_bytes(data, 5);
    assert_eq!(result, b"12345");
    assert!(!truncated);
}

#[test]
fn truncate_bytes_truncates_when_over_limit() {
    let data = b"hello world";
    let (result, truncated) = truncate_bytes(data, 5);
    assert_eq!(result, b"hello");
    assert!(truncated);
}

#[test]
fn truncate_bytes_empty_input_returns_empty() {
    let data: &[u8] = b"";
    let (result, truncated) = truncate_bytes(data, 100);
    assert!(result.is_empty());
    assert!(!truncated);
}

#[test]
fn truncate_bytes_zero_limit_returns_empty_and_truncated() {
    let data = b"hello";
    let (result, truncated) = truncate_bytes(data, 0);
    assert!(result.is_empty());
    assert!(truncated);
}

#[test]
fn truncate_bytes_empty_input_with_zero_limit() {
    let data: &[u8] = b"";
    let (result, truncated) = truncate_bytes(data, 0);
    assert!(result.is_empty());
    assert!(!truncated);
}

#[test]
fn truncate_bytes_limit_of_one() {
    let data = b"abc";
    let (result, truncated) = truncate_bytes(data, 1);
    assert_eq!(result, b"a");
    assert!(truncated);
}

#[test]
fn truncate_bytes_preserves_binary_data() {
    let data: &[u8] = &[0x00, 0xFF, 0x80, 0x7F, 0xFE];
    let (result, truncated) = truncate_bytes(data, 3);
    assert_eq!(result, &[0x00, 0xFF, 0x80]);
    assert!(truncated);
}

#[test]
fn truncate_bytes_large_data_truncated_to_default_limit() {
    let data = vec![b'x'; 200_000];
    let (result, truncated) = truncate_bytes(&data, DEFAULT_MAX_OUTPUT_BYTES);
    assert_eq!(result.len(), DEFAULT_MAX_OUTPUT_BYTES);
    assert!(truncated);
}

#[test]
fn truncate_bytes_data_exactly_at_default_limit_not_truncated() {
    let data = vec![b'y'; DEFAULT_MAX_OUTPUT_BYTES];
    let (result, truncated) = truncate_bytes(&data, DEFAULT_MAX_OUTPUT_BYTES);
    assert_eq!(result.len(), DEFAULT_MAX_OUTPUT_BYTES);
    assert!(!truncated);
}

// ── apply_sandbox_env ─────────────────────────────────────────────────

#[tokio::test]
async fn apply_sandbox_env_clears_custom_env_vars() {
    // Set a custom env var that should NOT survive the sandbox
    std::env::set_var("KAIROX_TEST_SANDBOX_VAR", "should_not_appear");

    let mut cmd = tokio::process::Command::new("env");
    apply_sandbox_env(&mut cmd);

    let output = cmd.output().await.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("KAIROX_TEST_SANDBOX_VAR"),
        "custom env var should be cleared by sandbox"
    );

    // Cleanup
    std::env::remove_var("KAIROX_TEST_SANDBOX_VAR");
}

#[tokio::test]
async fn apply_sandbox_env_forwards_path() {
    let mut cmd = tokio::process::Command::new("env");
    apply_sandbox_env(&mut cmd);

    let output = cmd.output().await.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // PATH should always be set on any Unix system
    if std::env::var("PATH").is_ok() {
        assert!(
            stdout.contains("PATH="),
            "PATH should be forwarded through sandbox"
        );
    }
}

#[tokio::test]
async fn apply_sandbox_env_forwards_only_allowed_vars() {
    let mut cmd = tokio::process::Command::new("env");
    apply_sandbox_env(&mut cmd);

    let output = cmd.output().await.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Every line in the output should be one of the allowed vars
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let var_name = line.split('=').next().unwrap_or("");
        assert!(
            ALLOWED_ENV_VARS.contains(&var_name),
            "unexpected env var in sandbox output: {var_name}"
        );
    }
}

#[tokio::test]
async fn apply_sandbox_env_skips_vars_not_in_parent() {
    // Remove TMPDIR if it exists, to verify it's skipped when absent
    let original = std::env::var("TMPDIR").ok();
    std::env::remove_var("TMPDIR");

    let mut cmd = tokio::process::Command::new("env");
    apply_sandbox_env(&mut cmd);

    let output = cmd.output().await.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("TMPDIR="),
        "TMPDIR should not appear when not set in parent"
    );

    // Restore
    if let Some(val) = original {
        std::env::set_var("TMPDIR", val);
    }
}
