//! End-to-end tests for the `kairox-eval` binary against the smoke
//! fixtures.
//!
//! Each test drives the CLI over a deterministic fake-profile fixture
//! and asserts that the binary exits 0 and produces a summary in which
//! all scenarios pass.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    let (output, _outputs, _results_path, summary_path) = run_cli_output(fixture, extra_args);

    assert!(
        output.status.success(),
        "kairox-eval exited non-zero: status={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let summary_raw =
        std::fs::read_to_string(&summary_path).expect("summary.json should be readable");
    serde_json::from_str(&summary_raw).expect("summary.json should parse as JSON")
}

fn run_cli_output<I, S>(
    fixture: &Path,
    extra_args: I,
) -> (Output, tempfile::TempDir, PathBuf, PathBuf)
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
        results_path.is_file(),
        "results jsonl must be written at {}",
        results_path.display()
    );
    (output, outputs, results_path, summary_path)
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
fn combined_report_json_contains_summary_and_results() {
    let fixture = fixture_path("smoke.jsonl");
    let bin = env!("CARGO_BIN_EXE_kairox-eval");
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let outputs = tempfile::tempdir().expect("outputs tempdir");
    let home_dir = tempfile::tempdir().expect("home tempdir");
    let results_path = outputs.path().join("results.jsonl");
    let report_path = outputs.path().join("report.json");

    let output = Command::new(bin)
        .env("HOME", home_dir.path())
        .env("USERPROFILE", home_dir.path())
        .current_dir(workspace.path())
        .arg("run")
        .arg("--scenarios")
        .arg(&fixture)
        .arg("--output")
        .arg(&results_path)
        .arg("--report")
        .arg(&report_path)
        .arg("--workspace")
        .arg(workspace.path())
        .arg("--profile")
        .arg("fake")
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
        report_path.is_file(),
        "combined report must be written at {}",
        report_path.display()
    );

    let report_raw = std::fs::read_to_string(&report_path).expect("report.json should be readable");
    let report: serde_json::Value =
        serde_json::from_str(&report_raw).expect("report.json should parse as JSON");

    assert_eq!(report["summary"]["total"].as_u64(), Some(3));
    assert_eq!(report["summary"]["failed"].as_u64(), Some(0));

    let results = report["results"]
        .as_array()
        .expect("report results should be an array");
    assert_eq!(results.len(), 3);
    let scenario_ids = results
        .iter()
        .map(|result| result["scenario_id"].as_str().expect("scenario_id"))
        .collect::<Vec<_>>();
    assert_eq!(
        scenario_ids,
        vec!["smoke-hello", "smoke-event-trace", "smoke-no-tool-failures"]
    );
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
fn fail_fast_stops_cli_after_first_failed_scenario() {
    let fixture = fixture_path("fail-fast.jsonl");
    let (output, _outputs, results_path, summary_path) = run_cli_output(&fixture, ["--fail-fast"]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "failing eval should exit 2\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let summary_raw =
        std::fs::read_to_string(&summary_path).expect("summary.json should be readable");
    let summary: serde_json::Value =
        serde_json::from_str(&summary_raw).expect("summary.json should parse as JSON");
    assert_eq!(summary["total"].as_u64(), Some(2), "{summary}");
    assert_eq!(summary["passed"].as_u64(), Some(1), "{summary}");
    assert_eq!(summary["failed"].as_u64(), Some(1), "{summary}");

    let results_raw =
        std::fs::read_to_string(&results_path).expect("results jsonl should be readable");
    let result_ids = results_raw
        .lines()
        .map(|line| {
            let result: serde_json::Value =
                serde_json::from_str(line).expect("result line should parse as JSON");
            result["scenario_id"]
                .as_str()
                .expect("scenario_id should be a string")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(result_ids, vec!["passes-first", "fails-second"]);
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

#[test]
fn list_command_prints_live_vibe_coding_scenario_ids() {
    let fixture = fixture_path("live-vibe-coding.jsonl");
    let ids = run_list_cli(&fixture, ["--tag", "vibe-coding", "--format", "json"]);
    assert_eq!(
        ids,
        vec![
            "vibe-coding-rust-kata",
            "vibe-coding-risk-command-const-arrays"
        ]
    );
}

#[test]
fn live_vibe_coding_risk_command_has_fmt_post_run_gate() {
    let fixture = fixture_path("live-vibe-coding.jsonl");
    let scenarios = agent_eval::load_scenarios(&fixture).expect("live fixture should parse");
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.id == "vibe-coding-risk-command-const-arrays")
        .expect("risk command scenario should exist");
    let first_command = scenario
        .expected
        .post_run_commands
        .first()
        .expect("risk command scenario should have post-run commands");

    assert_eq!(first_command.program, "cargo");
    assert_eq!(first_command.args, ["fmt", "--all", "--check"]);
}
