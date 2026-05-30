use super::*;

#[test]
fn agent_id_exposes_string_value_consistently() {
    let agent_id = AgentId::planner();

    assert_eq!(agent_id.as_str(), "agent_planner");
    assert_eq!(agent_id.to_string(), "agent_planner");
}

#[test]
fn session_id_creation_and_display() {
    let id = SessionId::new();
    let displayed = id.to_string();

    // Display produces a non-empty string with the expected prefix.
    assert!(!displayed.is_empty(), "SessionId Display must be non-empty");
    assert!(
        displayed.starts_with("ses_"),
        "SessionId must start with 'ses_', got: {displayed}"
    );

    // Roundtrip: Display → from_string → Display again.
    let roundtripped = SessionId::from_string(displayed.clone());
    assert_eq!(
        roundtripped.to_string(),
        displayed,
        "SessionId Display → from_string roundtrip mismatch"
    );

    // as_str matches Display.
    assert_eq!(id.as_str(), displayed);
}

#[test]
fn workspace_id_creation_and_display() {
    let id = WorkspaceId::new();
    let displayed = id.to_string();

    // Display produces a non-empty string with the expected prefix.
    assert!(
        !displayed.is_empty(),
        "WorkspaceId Display must be non-empty"
    );
    assert!(
        displayed.starts_with("wrk_"),
        "WorkspaceId must start with 'wrk_', got: {displayed}"
    );

    // Roundtrip: Display → from_string → Display again.
    let roundtripped = WorkspaceId::from_string(displayed.clone());
    assert_eq!(
        roundtripped.to_string(),
        displayed,
        "WorkspaceId Display → from_string roundtrip mismatch"
    );

    // as_str matches Display.
    assert_eq!(id.as_str(), displayed);
}

#[test]
fn session_id_default_creates_fresh_id() {
    let id = SessionId::default();
    assert!(!id.to_string().is_empty());
    assert!(id.to_string().starts_with("ses_"));
}

#[test]
fn workspace_id_default_creates_fresh_id() {
    let id = WorkspaceId::default();
    assert!(!id.to_string().is_empty());
    assert!(id.to_string().starts_with("wrk_"));
}

#[test]
fn session_id_from_string_preserves_exact_value() {
    let original = "ses_custom_abc123".to_string();
    let id = SessionId::from_string(original.clone());
    assert_eq!(id.to_string(), original);
    assert_eq!(id.as_str(), "ses_custom_abc123");
}

#[test]
fn workspace_id_from_string_preserves_exact_value() {
    let original = "wrk_custom_xyz789".to_string();
    let id = WorkspaceId::from_string(original.clone());
    assert_eq!(id.to_string(), original);
    assert_eq!(id.as_str(), "wrk_custom_xyz789");
}

#[test]
fn session_id_from_impl_preserves_value() {
    let s = "ses_from_impl".to_string();
    let id: SessionId = s.clone().into();
    assert_eq!(id.to_string(), s);
}

#[test]
fn workspace_id_from_impl_preserves_value() {
    let s = "wrk_from_impl".to_string();
    let id: WorkspaceId = s.clone().into();
    assert_eq!(id.to_string(), s);
}
