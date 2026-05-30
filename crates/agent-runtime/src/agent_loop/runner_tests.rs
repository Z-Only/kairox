use super::latest_model_profile_for;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};

fn init_event(profile: &str) -> DomainEvent {
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::SessionInitialized {
            model_profile: profile.into(),
        },
    )
}

fn switch_event(from: &str, to: &str) -> DomainEvent {
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ModelProfileSwitched {
            from_profile: from.into(),
            to_profile: to.into(),
            reasoning_effort: None,
            effective_at: chrono::Utc::now(),
            context_window: 0,
            output_limit: 0,
            limit_source: "fallback".into(),
        },
    )
}

#[test]
fn returns_session_initialized_profile_when_no_switch() {
    let events = vec![init_event("fast")];
    assert_eq!(latest_model_profile_for(&events), "fast");
}

#[test]
fn returns_latest_switch_when_one_exists() {
    let events = vec![init_event("fast"), switch_event("fast", "claude-opus")];
    assert_eq!(latest_model_profile_for(&events), "claude-opus");
}

#[test]
fn returns_most_recent_switch_when_multiple_exist() {
    let events = vec![
        init_event("fast"),
        switch_event("fast", "gpt-4o"),
        switch_event("gpt-4o", "claude-opus"),
    ];
    assert_eq!(latest_model_profile_for(&events), "claude-opus");
}

#[test]
fn falls_back_to_fake_when_no_initialization_event() {
    let events: Vec<DomainEvent> = vec![];
    assert_eq!(latest_model_profile_for(&events), "fake");
}
