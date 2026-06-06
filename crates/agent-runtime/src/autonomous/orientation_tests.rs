use agent_core::autonomous::{AutonomousTaskGoal, Checkpoint, VerificationResult};
use agent_core::SessionId;

use super::*;

fn sample_goal() -> AutonomousTaskGoal {
    AutonomousTaskGoal {
        description: "Build feature X with tests".into(),
        acceptance_criteria: vec!["tests pass".into(), "lint clean".into()],
        verification_commands: vec!["cargo test".into(), "cargo clippy".into()],
    }
}

fn sample_checkpoint() -> Checkpoint {
    Checkpoint {
        checkpoint_id: "ckpt_test".into(),
        session_id: SessionId::from_string("ses_prev".into()),
        session_index: 0,
        git_sha: Some("abc123".into()),
        completed_items: vec!["implemented core logic".into()],
        remaining_items: vec!["add tests".into(), "fix lint".into()],
        verification_results: vec![VerificationResult {
            criterion: "cargo test".into(),
            passed: true,
            output_preview: "42 passed".into(),
        }],
        notes: "Session ended: context_limit_reached. Files patched: 3.".into(),
        created_at: chrono::Utc::now(),
    }
}

#[test]
fn orientation_contains_goal() {
    let prompt = OrientationPromptBuilder::build(&sample_goal(), &sample_checkpoint(), 1, 10);
    assert!(prompt.contains("Build feature X with tests"));
    assert!(prompt.contains("session 2 of up to 10"));
}

#[test]
fn orientation_contains_completed_items() {
    let prompt = OrientationPromptBuilder::build(&sample_goal(), &sample_checkpoint(), 1, 10);
    assert!(prompt.contains("implemented core logic"));
}

#[test]
fn orientation_contains_remaining_items() {
    let prompt = OrientationPromptBuilder::build(&sample_goal(), &sample_checkpoint(), 1, 10);
    assert!(prompt.contains("add tests"));
    assert!(prompt.contains("fix lint"));
}

#[test]
fn orientation_contains_git_sha() {
    let prompt = OrientationPromptBuilder::build(&sample_goal(), &sample_checkpoint(), 1, 10);
    assert!(prompt.contains("abc123"));
}

#[test]
fn orientation_contains_verification_commands() {
    let prompt = OrientationPromptBuilder::build(&sample_goal(), &sample_checkpoint(), 1, 10);
    assert!(prompt.contains("cargo test"));
    assert!(prompt.contains("cargo clippy"));
}

#[test]
fn orientation_contains_verification_results() {
    let prompt = OrientationPromptBuilder::build(&sample_goal(), &sample_checkpoint(), 1, 10);
    assert!(prompt.contains("42 passed"));
}

#[test]
fn initial_prompt_contains_goal_and_criteria() {
    let prompt = OrientationPromptBuilder::build_initial_prompt(&sample_goal());
    assert!(prompt.contains("Build feature X with tests"));
    assert!(prompt.contains("tests pass"));
    assert!(prompt.contains("lint clean"));
    assert!(prompt.contains("cargo test"));
}
