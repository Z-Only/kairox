use super::*;

#[test]
fn trajectory_id_generates_unique_values() {
    let a = TrajectoryId::new();
    let b = TrajectoryId::new();
    assert_ne!(a, b);
}

#[test]
fn trajectory_id_display_matches_inner() {
    let id = TrajectoryId("test-id-123".into());
    assert_eq!(id.to_string(), "test-id-123");
}

#[test]
fn trajectory_outcome_serializes_as_snake_case() {
    let json = serde_json::to_string(&TrajectoryOutcome::InProgress).unwrap();
    assert_eq!(json, "\"in_progress\"");
}

#[test]
fn trajectory_step_roundtrips_json() {
    let step = TrajectoryStep {
        step_index: 0,
        action: "shell.exec".into(),
        action_input: serde_json::json!({"command": "ls"}),
        observation: "file1.rs\nfile2.rs".into(),
        screenshot_id: None,
        timestamp: chrono::Utc::now(),
        duration_ms: 120,
    };
    let json = serde_json::to_string(&step).unwrap();
    let back: TrajectoryStep = serde_json::from_str(&json).unwrap();
    assert_eq!(back.action, "shell.exec");
    assert_eq!(back.duration_ms, 120);
}
