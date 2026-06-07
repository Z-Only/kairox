use super::*;

#[test]
fn advisor_mode_display() {
    assert_eq!(AdvisorMode::Off.to_string(), "off");
    assert_eq!(AdvisorMode::Lightweight.to_string(), "lightweight");
    assert_eq!(AdvisorMode::Full.to_string(), "full");
}

#[test]
fn advisor_mode_default_is_off() {
    assert_eq!(AdvisorMode::default(), AdvisorMode::Off);
}

#[test]
fn advisor_verdict_display() {
    assert_eq!(AdvisorVerdict::Approve.to_string(), "approve");
    assert_eq!(
        AdvisorVerdict::ApproveWithWarnings.to_string(),
        "approve_with_warnings"
    );
    assert_eq!(AdvisorVerdict::Reject.to_string(), "reject");
}

#[test]
fn advisor_review_serde_roundtrip() {
    let review = AdvisorReview {
        verdict: AdvisorVerdict::ApproveWithWarnings,
        concerns: vec![AdvisorConcern {
            tool_name: "shell.exec".into(),
            severity: "high".into(),
            message: "rm -rf is destructive".into(),
        }],
        summary: "Proceed with caution".into(),
        advisor_profile: "haiku".into(),
    };
    let json = serde_json::to_string(&review).unwrap();
    let deserialized: AdvisorReview = serde_json::from_str(&json).unwrap();
    assert_eq!(review, deserialized);
}

#[test]
fn advisor_mode_serde_roundtrip() {
    for mode in [
        AdvisorMode::Off,
        AdvisorMode::Lightweight,
        AdvisorMode::Full,
    ] {
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: AdvisorMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }
}
