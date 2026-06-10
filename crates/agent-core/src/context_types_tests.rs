use super::*;

#[test]
fn ratio_returns_fraction_of_budget_consumed() {
    let usage = ContextUsage {
        total_tokens: 60_000,
        budget_tokens: 200_000,
        context_window: 200_000,
        output_reservation: 0,
        by_source: vec![(ContextSource::System, 60_000)],
        estimator: "cl100k_base".into(),
        corrected_by_real_usage: false,
    };
    assert!((usage.ratio() - 0.30).abs() < 1e-4);
}

#[test]
fn context_source_serializes_snake_case_with_new_variants() {
    assert_eq!(
        serde_json::to_value(ContextSource::ToolDefinitions).unwrap(),
        "tool_definitions"
    );
    assert_eq!(
        serde_json::to_value(ContextSource::CompactionSummary).unwrap(),
        "compaction_summary"
    );
    assert_eq!(serde_json::to_value(ContextSource::Skill).unwrap(), "skill");
    assert_eq!(
        serde_json::to_value(ContextSource::WorkspaceRetrieval).unwrap(),
        "workspace_retrieval"
    );
    assert_eq!(serde_json::to_value(ContextSource::Git).unwrap(), "git");
}

#[test]
fn project_instruction_serializes_snake_case() {
    assert_eq!(
        serde_json::to_value(ContextSource::ProjectInstruction).unwrap(),
        "project_instruction"
    );
}

#[test]
fn context_usage_round_trips_through_json() {
    let usage = ContextUsage {
        total_tokens: 1_234,
        budget_tokens: 200_000,
        context_window: 200_000,
        output_reservation: 9_000,
        by_source: vec![
            (ContextSource::System, 800),
            (ContextSource::ToolDefinitions, 434),
        ],
        estimator: "cl100k_base".into(),
        corrected_by_real_usage: true,
    };
    let json = serde_json::to_value(&usage).unwrap();
    let back: ContextUsage = serde_json::from_value(json).unwrap();
    assert_eq!(back, usage);
}
