//! End-to-end test for the `kairox-eval` binary against the smoke fixture.
//!
//! Verifies that invoking the CLI over `fixtures/smoke.jsonl` with the
//! deterministic `fake` profile exits 0 and produces a summary in which
//! all scenarios pass.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn smoke_fixture_runs_clean_through_cli() {
    let bin = env!("CARGO_BIN_EXE_kairox-eval");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("fixtures").join("smoke.jsonl");
    assert!(
        fixture.is_file(),
        "smoke fixture must exist at {}",
        fixture.display()
    );

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let outputs = tempfile::tempdir().expect("outputs tempdir");
    let results_path = outputs.path().join("results.jsonl");
    let summary_path = outputs.path().join("summary.json");

    let output = Command::new(bin)
        .arg("run")
        .arg("--scenarios")
        .arg(&fixture)
        .arg("--output")
        .arg(&results_path)
        .arg("--summary")
        .arg(&summary_path)
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
        results_path.is_file(),
        "results jsonl must be written at {}",
        results_path.display()
    );
    let summary_raw =
        std::fs::read_to_string(&summary_path).expect("summary.json should be readable");
    let summary: serde_json::Value =
        serde_json::from_str(&summary_raw).expect("summary.json should parse as JSON");

    assert_eq!(
        summary["total"].as_u64(),
        Some(3),
        "summary should report 3 total scenarios, got: {summary_raw}"
    );
    assert_eq!(
        summary["failed"].as_u64(),
        Some(0),
        "summary should report 0 failed scenarios, got: {summary_raw}"
    );
    assert_eq!(
        summary["passed"].as_u64(),
        Some(3),
        "summary should report 3 passed scenarios, got: {summary_raw}"
    );
}
