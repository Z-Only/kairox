use agent_config::Config;
use agent_eval::{
    load_scenarios_from_str, EvalExpectation, EvalHarness, EvalResult, EvalRunOptions,
    EvalScenario, EvalSummary,
};

#[test]
fn loads_jsonl_scenarios_and_skips_comments() {
    let input = r#"
# smoke cases
{"id":"hello","prompt":"Say hello","profile":"fake","expected":{"assistant_contains":["hello"]}}

{"id":"trace","prompt":"Emit trace","expected":{"event_types":["UserMessageAdded"]}}
"#;

    let scenarios = load_scenarios_from_str(input).expect("scenarios should parse");

    assert_eq!(scenarios.len(), 2);
    assert_eq!(scenarios[0].id, "hello");
    assert_eq!(scenarios[0].profile.as_deref(), Some("fake"));
    assert_eq!(scenarios[0].expected.assistant_contains, vec!["hello"]);
    assert_eq!(scenarios[1].expected.event_types, vec!["UserMessageAdded"]);
}

#[tokio::test]
async fn runs_fake_scenario_and_records_trace_metrics() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        include_trace: true,
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "fake-smoke".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            assistant_contains: vec!["hello from Kairox".into()],
            event_types: vec![
                "UserMessageAdded".into(),
                "AssistantMessageCompleted".into(),
            ],
            max_tool_failures: Some(0),
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(result.passed, "{:?}", result.failures);
    assert_eq!(result.scenario_id, "fake-smoke");
    assert_eq!(result.profile, "fake");
    assert_eq!(
        result.assistant_response.as_deref(),
        Some("hello from Kairox")
    );
    assert!(result.elapsed_ms > 0);
    assert!(result.event_types.contains(&"UserMessageAdded".into()));
    assert!(result
        .event_types
        .contains(&"AssistantMessageCompleted".into()));
    assert_eq!(result.tool_invocations, 0);
    assert_eq!(result.tool_failures, 0);
    assert!(result.trace.is_some());
}

#[test]
fn summary_counts_passes_failures_and_cost_drivers() {
    let passed = EvalResult {
        scenario_id: "passed".into(),
        passed: true,
        elapsed_ms: 100,
        tool_invocations: 2,
        context_input_tokens: Some(50),
        ..EvalResult::default()
    };
    let failed = EvalResult {
        scenario_id: "failed".into(),
        passed: false,
        elapsed_ms: 300,
        tool_failures: 1,
        context_input_tokens: Some(70),
        failures: vec!["missing expected text".into()],
        ..EvalResult::default()
    };

    let summary = EvalSummary::from_results(&[passed, failed]);

    assert_eq!(summary.total, 2);
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.success_rate, 0.5);
    assert_eq!(summary.total_elapsed_ms, 400);
    assert_eq!(summary.avg_elapsed_ms, 200.0);
    assert_eq!(summary.total_tool_invocations, 2);
    assert_eq!(summary.total_tool_failures, 1);
    assert_eq!(summary.total_context_input_tokens, Some(120));
}
