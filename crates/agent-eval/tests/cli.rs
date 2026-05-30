//! End-to-end tests for the `kairox-eval` binary against the smoke
//! fixtures.
//!
//! Each test drives the CLI over a deterministic fake-profile fixture
//! and asserts that the binary exits 0 and produces a summary in which
//! all scenarios pass.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_path(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("fixtures").join(name);
    assert!(path.is_file(), "fixture must exist at {}", path.display());
    path
}

fn run_cli<I, S>(fixture: &Path, extra_args: I) -> serde_json::Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let bin = env!("CARGO_BIN_EXE_kairox-eval");
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let outputs = tempfile::tempdir().expect("outputs tempdir");
    let home_dir = tempfile::tempdir().expect("home tempdir");
    let results_path = outputs.path().join("results.jsonl");
    let summary_path = outputs.path().join("summary.json");

    // Isolate config discovery from any user-level `~/.kairox/config.toml`
    // by pointing HOME at an empty temp dir; this keeps the fake profile
    // bound to its built-in defaults regardless of the developer's setup.
    let output = Command::new(bin)
        .env("HOME", home_dir.path())
        .env("USERPROFILE", home_dir.path())
        .current_dir(workspace.path())
        .arg("run")
        .arg("--scenarios")
        .arg(fixture)
        .arg("--output")
        .arg(&results_path)
        .arg("--summary")
        .arg(&summary_path)
        .arg("--workspace")
        .arg(workspace.path())
        .arg("--profile")
        .arg("fake")
        .args(extra_args)
        .output()
        .expect("kairox-eval binary should execute");

    assert!(
        output.status.success(),
        "kairox-eval exited non-zero: status={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    assert!(
        results_path.is_file(),
        "results jsonl must be written at {}",
        results_path.display()
    );
    let summary_raw =
        std::fs::read_to_string(&summary_path).expect("summary.json should be readable");
    serde_json::from_str(&summary_raw).expect("summary.json should parse as JSON")
}

fn assert_all_passed(summary: &serde_json::Value, expected_total: u64) {
    assert_eq!(
        summary["total"].as_u64(),
        Some(expected_total),
        "summary should report {expected_total} total scenarios, got: {summary}"
    );
    assert_eq!(
        summary["failed"].as_u64(),
        Some(0),
        "summary should report 0 failed scenarios, got: {summary}"
    );
    assert_eq!(
        summary["passed"].as_u64(),
        Some(expected_total),
        "summary should report {expected_total} passed scenarios, got: {summary}"
    );
}

fn run_list_cli<I, S>(fixture: &Path, extra_args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let bin = env!("CARGO_BIN_EXE_kairox-eval");
    let output = Command::new(bin)
        .arg("list")
        .arg("--scenarios")
        .arg(fixture)
        .args(extra_args)
        .output()
        .expect("kairox-eval binary should execute");

    assert!(
        output.status.success(),
        "kairox-eval list exited non-zero: status={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    serde_json::from_slice(&output.stdout).expect("list JSON output should parse")
}

#[test]
fn smoke_fixture_runs_clean_through_cli() {
    let fixture = fixture_path("smoke.jsonl");
    let summary = run_cli(&fixture, std::iter::empty::<&str>());
    assert_all_passed(&summary, 3);
}

#[test]
fn smoke_tool_call_fixture_runs_clean_through_cli() {
    let fixture = fixture_path("smoke-tool-call.jsonl");
    let summary = run_cli(
        &fixture,
        ["--fake-emit-tool-call", "--wait-timeout-ms", "5000"],
    );
    assert_all_passed(&summary, 1);
}

#[test]
fn smoke_compaction_fixture_runs_clean_through_cli() {
    let fixture = fixture_path("smoke-compaction.jsonl");
    let summary = run_cli(
        &fixture,
        [
            "--auto-compact-threshold",
            "0.001",
            "--seed-synthetic-pairs",
            "4",
            "--wait-timeout-ms",
            "5000",
        ],
    );
    assert_all_passed(&summary, 1);
}

#[test]
fn tag_filters_limit_cli_scenarios() {
    let fixture = fixture_path("smoke-tags.jsonl");
    let summary = run_cli(&fixture, ["--tag", "smoke", "--exclude-tag", "slow"]);
    assert_all_passed(&summary, 1);
}

#[test]
fn list_command_prints_filtered_scenario_ids() {
    let fixture = fixture_path("smoke-tags.jsonl");
    let ids = run_list_cli(
        &fixture,
        [
            "--tag",
            "smoke",
            "--exclude-tag",
            "slow",
            "--format",
            "json",
        ],
    );
    assert_eq!(ids, vec!["tag-fast"]);
}
