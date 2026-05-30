use super::*;

#[test]
fn task_state_is_terminal() {
    assert!(!TaskState::Pending.is_terminal());
    assert!(!TaskState::Ready.is_terminal());
    assert!(!TaskState::Running.is_terminal());
    assert!(!TaskState::Blocked.is_terminal());
    assert!(TaskState::Completed.is_terminal());
    assert!(TaskState::Failed.is_terminal());
    assert!(TaskState::Skipped.is_terminal());
    assert!(TaskState::Cancelled.is_terminal());
}

#[test]
fn default_failure_policy_is_block_dependents() {
    assert_eq!(FailurePolicy::default(), FailurePolicy::BlockDependents);
}

#[test]
fn default_retry_config() {
    let config = RetryConfig::default();
    assert_eq!(config.max_model_retries, 3);
    assert_eq!(config.max_tool_retries, 2);
    assert!(matches!(
        config.backoff,
        BackoffStrategy::ExponentialJitter {
            base_ms: 1000,
            max_ms: 30_000
        }
    ));
}

#[test]
fn failure_reason_serialization_roundtrip() {
    let reason = TaskFailureReason::ToolExhausted {
        tool_id: "fs.read".into(),
        attempts: 3,
        last_error: "permission denied".into(),
    };
    let json = serde_json::to_string(&reason).unwrap();
    let deserialized: TaskFailureReason = serde_json::from_str(&json).unwrap();
    assert_eq!(reason, deserialized);
}
