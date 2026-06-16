# Regression Eval: Risk Command Const Arrays

## Goal

Add a live vibe-coding regression scenario for the `risk.rs` command constant extraction task, using the concrete pre-PR Kairox commit `4371a71d068e94da1016b632ea2db2378a0582b2` as the programming project under test. The eval must not depend on an externally injected project or drift with future `HEAD`.

## Context

`just eval-vibe-coding` creates a detached git worktree and passes it as `--workspace` to `kairox-eval`. For this regression the worktree must be checked out at the fixed baseline commit `4371a71d068e94da1016b632ea2db2378a0582b2`, which is the `main` commit before PRs #1028/#1029 were created. That keeps the repository-real task from becoming pre-solved after the regression fixture itself is merged.

## Tasks

1. Add a fixture scenario to `crates/agent-eval/fixtures/live-vibe-coding.jsonl`.
   - Prompt the agent to extract command and subcommand constants from `crates/agent-tools/src/shell/risk.rs`.
   - Require reusable public exports from `agent_tools` crate root.
   - Require focused tests proving crate-root reuse.
   - Use workspace file expectations and post-run commands to verify behavior.
2. Update CLI/list tests to expect the new live vibe-coding scenario id.
3. If needed, adjust the `eval-vibe-coding` recipe comments or flow so it is explicit that the pinned detached worktree, not runtime `HEAD`, is the programming project.
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

2026-06-16 correction:

- The original PR used runtime `HEAD` as the programming project. That was too loose: once this regression fixture is merged, future `HEAD` can already contain the requirement or related scaffolding.
- The fixture and `just eval-vibe-coding` recipe now pin the default programming project commit to `4371a71d068e94da1016b632ea2db2378a0582b2`.
