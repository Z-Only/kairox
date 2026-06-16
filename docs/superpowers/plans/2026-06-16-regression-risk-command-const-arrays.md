# Regression Eval: Risk Command Const Arrays

## Goal

Add a live vibe-coding regression scenario for the `risk.rs` command constant extraction task, using the current Kairox commit as the programming project under test. The eval must not depend on an externally injected project.

## Context

`just eval-vibe-coding` already creates a detached git worktree at `HEAD` and passes it as `--workspace` to `kairox-eval`. That makes the current commit itself the programming project. The existing fixture only covers a synthetic Rust kata under `target/`; this task adds a repository-real scenario that edits `crates/agent-tools`.

## Tasks

1. Add a fixture scenario to `crates/agent-eval/fixtures/live-vibe-coding.jsonl`.
   - Prompt the agent to extract command and subcommand constants from `crates/agent-tools/src/shell/risk.rs`.
   - Require reusable public exports from `agent_tools` crate root.
   - Require focused tests proving crate-root reuse.
   - Use workspace file expectations and post-run commands to verify behavior.
2. Update CLI/list tests to expect the new live vibe-coding scenario id.
3. If needed, adjust the `eval-vibe-coding` recipe comments or flow so it is explicit that the detached `HEAD` worktree is the programming project.
4. Verify focused eval behavior.
   - Run `cargo test -p agent-eval`.
   - Run `cargo test -p agent-eval --test cli list_command_prints_live_vibe_coding_scenario_ids`.
   - Run `cargo fmt --all --check`.
   - Run `cargo clippy -p agent-eval --all-targets -- -D warnings`.

## Live Model Execution

Do not run the full live model eval unless explicitly requested or a profile is confirmed available. The regression suite change is validated structurally by scenario parsing/listing and post-run command definitions.

## Verification Record

2026-06-16 worker verification:

- `target/debug/kairox-eval list --scenarios crates/agent-eval/fixtures/live-vibe-coding.jsonl --tag vibe-coding --format json` passed and listed `vibe-coding-risk-command-const-arrays`.
- `cargo test -p agent-eval --test cli list_command_prints_live_vibe_coding_scenario_ids -- --nocapture` passed.
- `cargo test -p agent-eval` passed.
- `cargo fmt --all --check` passed.
- `cargo clippy -p agent-eval --all-targets -- -D warnings` passed.

The full live model eval was not run, per plan guidance.
