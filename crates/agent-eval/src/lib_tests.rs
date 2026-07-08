use super::*;

fn event(payload: EventPayload) -> DomainEvent {
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        payload,
    )
}

#[test]
fn model_usage_from_events_aggregates_recorded_usage() {
    let usage = model_usage_from_events(&[
        event(EventPayload::ModelUsageRecorded {
            model_profile: "fast".into(),
            input_tokens: 100,
            output_tokens: 25,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(40),
        }),
        event(EventPayload::ModelUsageRecorded {
            model_profile: "fast".into(),
            input_tokens: 50,
            output_tokens: 5,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: Some(20),
        }),
    ]);

    assert_eq!(
        usage,
        Some(EvalModelUsage {
            request_count: 2,
            input_tokens: 150,
            output_tokens: 30,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 60,
        })
    );
}
