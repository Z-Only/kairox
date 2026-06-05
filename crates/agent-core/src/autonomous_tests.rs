use super::*;
use crate::SessionId;

#[test]
fn autonomous_task_state_roundtrip() {
    let states = [
        AutonomousTaskState::Active,
        AutonomousTaskState::Paused,
        AutonomousTaskState::Completed,
        AutonomousTaskState::Failed,
        AutonomousTaskState::Cancelled,
    ];
    for state in &states {
        let s = state.to_string();
        let parsed: AutonomousTaskState = s.parse().unwrap();
        assert_eq!(*state, parsed);
    }
}

#[test]
fn autonomous_task_state_parse_rejects_unknown() {
    let result: Result<AutonomousTaskState, _> = "bogus".parse();
    assert!(result.is_err());
}

#[test]
fn session_end_reason_roundtrip() {
    let reasons = [
        SessionEndReason::ContextLimitReached,
        SessionEndReason::MaxIterationsReached,
        SessionEndReason::UserPaused,
        SessionEndReason::TaskCompleted,
        SessionEndReason::TaskFailed,
    ];
    for reason in &reasons {
        let s = reason.to_string();
        let parsed: SessionEndReason = s.parse().unwrap();
        assert_eq!(*reason, parsed);
    }
}

#[test]
fn session_end_reason_parse_rejects_unknown() {
    let result: Result<SessionEndReason, _> = "nope".parse();
    assert!(result.is_err());
}

#[test]
fn autonomous_config_default_values() {
    let config = AutonomousConfig::default();
    assert_eq!(config.max_sessions, 10);
    assert!(config.auto_continue);
    assert!(config.verification_required);
    assert!(config.git_checkpoint);
}

#[test]
fn autonomous_task_goal_serde_roundtrip() {
    let goal = AutonomousTaskGoal {
        description: "Implement feature X".into(),
        acceptance_criteria: vec!["tests pass".into(), "no lint errors".into()],
        verification_commands: vec!["cargo test".into()],
    };
    let json = serde_json::to_string(&goal).unwrap();
    let parsed: AutonomousTaskGoal = serde_json::from_str(&json).unwrap();
    assert_eq!(goal, parsed);
}

#[test]
fn checkpoint_serde_roundtrip() {
    let checkpoint = Checkpoint {
        checkpoint_id: "ckpt_001".into(),
        session_id: SessionId::from_string("ses_test".into()),
        session_index: 2,
        git_sha: Some("abc123".into()),
        completed_items: vec!["step 1 done".into()],
        remaining_items: vec!["step 2 pending".into()],
        verification_results: vec![VerificationResult {
            criterion: "cargo test".into(),
            passed: true,
            output_preview: "ok".into(),
        }],
        notes: "all good so far".into(),
        created_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&checkpoint).unwrap();
    let parsed: Checkpoint = serde_json::from_str(&json).unwrap();
    assert_eq!(checkpoint, parsed);
}

#[test]
fn verification_result_serde_roundtrip() {
    let vr = VerificationResult {
        criterion: "lint check".into(),
        passed: false,
        output_preview: "error on line 42".into(),
    };
    let json = serde_json::to_string(&vr).unwrap();
    let parsed: VerificationResult = serde_json::from_str(&json).unwrap();
    assert_eq!(vr, parsed);
}
