use super::types::*;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::collections::HashMap;

// ── EvalError Display ────────────────────────────────────────────────────────

#[test]
fn error_display_scenario_parse() {
    let source = serde_json::from_str::<EvalScenario>("not json").unwrap_err();
    let err = EvalError::ScenarioParse { line: 3, source };
    let msg = err.to_string();
    assert!(msg.starts_with("scenario parse error on line 3:"), "{msg}");
}

#[test]
fn error_display_io() {
    let err = EvalError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
    assert_eq!(err.to_string(), "io error: gone");
}

#[test]
fn error_display_json() {
    let source = serde_json::from_str::<i32>("\"x\"").unwrap_err();
    let err = EvalError::Json(source);
    assert!(err.to_string().starts_with("json error:"), "{}", err);
}

#[test]
fn error_display_policy() {
    let err = EvalError::Policy("bad policy value".into());
    assert_eq!(err.to_string(), "invalid policy: bad policy value");
}

#[test]
fn error_display_regex() {
    let bad_pattern = String::from("[invalid");
    let source = regex::Regex::new(&bad_pattern).unwrap_err();
    let err = EvalError::Regex {
        pattern: bad_pattern,
        source,
    };
    let msg = err.to_string();
    assert!(msg.starts_with("invalid regex `[invalid`:"), "{msg}");
}

#[test]
fn error_display_cli() {
    let err = EvalError::Cli("missing --scenarios".into());
    assert_eq!(err.to_string(), "missing --scenarios");
}

// ── EvalScenario serde ───────────────────────────────────────────────────────

#[test]
fn scenario_serde_roundtrip_minimal() {
    let scenario = EvalScenario {
        id: "s1".into(),
        prompt: "hello".into(),
        ..EvalScenario::default()
    };
    let json = serde_json::to_string(&scenario).unwrap();
    let back: EvalScenario = serde_json::from_str(&json).unwrap();
    assert_eq!(scenario, back);
}

#[test]
fn scenario_serde_roundtrip_full() {
    let scenario = EvalScenario {
        id: "full".into(),
        prompt: "test all fields".into(),
        profile: Some("gpt4".into()),
        approval_policy: Some(ApprovalPolicy::Always),
        sandbox_policy: Some(SandboxPolicy::ReadOnly),
        tags: vec!["smoke".into(), "fast".into()],
        expected: EvalExpectation {
            assistant_contains: vec!["answer".into()],
            assistant_not_contains: vec!["error".into()],
            assistant_matches_regex: vec!["\\d+".into()],
            event_types: vec!["UserMessageAdded".into()],
            forbidden_event_types: vec!["ToolInvocationFailed".into()],
            min_events_of_type: HashMap::from([("ToolInvocationStarted".into(), 1)]),
            max_events_of_type: HashMap::from([("ToolInvocationFailed".into(), 0)]),
            min_tool_invocations: Some(2),
            max_tool_failures: Some(0),
            max_elapsed_ms: Some(5000),
            max_context_input_tokens: Some(10_000),
            max_turns: Some(3),
            trajectory_actions: vec!["fs.read".into()],
            max_trajectory_steps: Some(10),
            workspace_files: vec![EvalFileExpectation {
                path: "src/lib.rs".into(),
                contains: vec!["pub fn add".into()],
                not_contains: vec!["TODO".into()],
            }],
            post_run_commands: vec![EvalCommandExpectation {
                program: "cargo".into(),
                args: vec!["test".into(), "--quiet".into()],
                timeout_ms: Some(30_000),
                ..EvalCommandExpectation::default()
            }],
        },
        turns: vec!["follow up".into()],
        system_instructions: Some("Be concise".into()),
    };
    let json = serde_json::to_string(&scenario).unwrap();
    let back: EvalScenario = serde_json::from_str(&json).unwrap();
    assert_eq!(scenario, back);
}

#[test]
fn scenario_skip_serializing_if_omits_empty_fields() {
    let scenario = EvalScenario {
        id: "minimal".into(),
        prompt: "hi".into(),
        ..EvalScenario::default()
    };
    let json = serde_json::to_string(&scenario).unwrap();
    // Optional/empty fields should be omitted
    assert!(!json.contains("\"profile\""), "profile should be omitted");
    assert!(!json.contains("\"tags\""), "tags should be omitted");
    assert!(!json.contains("\"turns\""), "turns should be omitted");
    assert!(
        !json.contains("\"system_instructions\""),
        "system_instructions should be omitted"
    );
    assert!(
        !json.contains("\"approval_policy\""),
        "approval_policy should be omitted"
    );
    assert!(
        !json.contains("\"sandbox_policy\""),
        "sandbox_policy should be omitted"
    );
}

#[test]
fn scenario_deserialize_from_jsonl_with_defaults() {
    let input = r#"{"id":"basic","prompt":"say hello"}"#;
    let scenario: EvalScenario = serde_json::from_str(input).unwrap();
    assert_eq!(scenario.id, "basic");
    assert_eq!(scenario.prompt, "say hello");
    assert_eq!(scenario.profile, None);
    assert!(scenario.tags.is_empty());
    assert!(scenario.turns.is_empty());
    assert_eq!(scenario.system_instructions, None);
    assert_eq!(scenario.expected, EvalExpectation::default());
}

// ── EvalExpectation serde ────────────────────────────────────────────────────

#[test]
fn expectation_default_is_empty() {
    let exp = EvalExpectation::default();
    assert!(exp.assistant_contains.is_empty());
    assert!(exp.assistant_not_contains.is_empty());
    assert!(exp.assistant_matches_regex.is_empty());
    assert!(exp.event_types.is_empty());
    assert!(exp.forbidden_event_types.is_empty());
    assert!(exp.min_events_of_type.is_empty());
    assert!(exp.max_events_of_type.is_empty());
    assert_eq!(exp.min_tool_invocations, None);
    assert_eq!(exp.max_tool_failures, None);
    assert_eq!(exp.max_elapsed_ms, None);
    assert_eq!(exp.max_context_input_tokens, None);
    assert_eq!(exp.max_turns, None);
    assert!(exp.trajectory_actions.is_empty());
    assert_eq!(exp.max_trajectory_steps, None);
    assert!(exp.workspace_files.is_empty());
    assert!(exp.post_run_commands.is_empty());
}

#[test]
fn expectation_serde_roundtrip() {
    let exp = EvalExpectation {
        assistant_contains: vec!["hello".into()],
        assistant_matches_regex: vec!["^\\d{3}$".into()],
        min_events_of_type: HashMap::from([("A".into(), 1), ("B".into(), 2)]),
        max_events_of_type: HashMap::from([("C".into(), 5)]),
        min_tool_invocations: Some(3),
        max_tool_failures: Some(1),
        max_elapsed_ms: Some(999),
        max_context_input_tokens: Some(4096),
        max_turns: Some(5),
        trajectory_actions: vec!["shell.exec".into(), "fs.write".into()],
        max_trajectory_steps: Some(20),
        workspace_files: vec![EvalFileExpectation {
            path: "src/lib.rs".into(),
            contains: vec!["pub fn add".into()],
            not_contains: vec!["TODO".into()],
        }],
        post_run_commands: vec![EvalCommandExpectation {
            program: "cargo".into(),
            args: vec!["test".into()],
            cwd: Some("target/vibe-coding-kata".into()),
            exit_code: Some(0),
            timeout_ms: Some(60_000),
            stdout_contains: vec!["test result".into()],
            stderr_contains: Vec::new(),
        }],
        ..EvalExpectation::default()
    };
    let json = serde_json::to_string(&exp).unwrap();
    let back: EvalExpectation = serde_json::from_str(&json).unwrap();
    assert_eq!(exp, back);
}

#[test]
fn expectation_skip_serializing_if_omits_empty() {
    let exp = EvalExpectation::default();
    let json = serde_json::to_string(&exp).unwrap();
    // All vec and map fields are empty, all Options are None => should all be omitted
    assert!(!json.contains("assistant_contains"));
    assert!(!json.contains("event_types"));
    assert!(!json.contains("min_tool_invocations"));
    assert!(!json.contains("max_elapsed_ms"));
    assert!(!json.contains("trajectory_actions"));
    assert!(!json.contains("workspace_files"));
    assert!(!json.contains("post_run_commands"));
}

// ── EvalRunOptions Default ───────────────────────────────────────────────────

#[test]
fn run_options_default_values() {
    let opts = EvalRunOptions::default();
    assert_eq!(opts.default_profile, None);
    assert!(opts.config.is_none());
    assert_eq!(opts.approval_policy, ApprovalPolicy::OnRequest);
    assert!(matches!(
        opts.sandbox_policy,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            ..
        }
    ));
    assert!(!opts.include_trace);
    assert!(!opts.enable_mcp);
    assert!(!opts.enable_hooks);
    assert_eq!(opts.auto_compact_threshold, None);
    assert!(!opts.fake_emit_tool_call);
    assert_eq!(opts.fake_tool_id, None);
    assert_eq!(opts.fake_tool_arguments, None);
    assert_eq!(opts.wait_timeout_ms, None);
    assert_eq!(opts.seed_synthetic_pairs, None);
    assert!(!opts.allow_post_run_commands);
    assert_eq!(opts.scenario_timeout_ms, None);
}

// ── EvalResult serde ─────────────────────────────────────────────────────────

#[test]
fn result_serde_roundtrip_with_all_fields() {
    let result = EvalResult {
        scenario_id: "test-1".into(),
        profile: "default".into(),
        passed: true,
        failures: vec!["oops".into()],
        error: Some("runtime error: oops".into()),
        elapsed_ms: 250,
        assistant_response: Some("I helped!".into()),
        event_types: vec![
            "UserMessageAdded".into(),
            "AssistantMessageCompleted".into(),
        ],
        tool_invocations: 3,
        tool_failures: 1,
        context_input_tokens: Some(5000),
        context_window: Some(128_000),
        model_usage: Some(EvalModelUsage {
            request_count: 2,
            input_tokens: 1000,
            output_tokens: 250,
            cache_creation_input_tokens: 100,
            cache_read_input_tokens: 400,
        }),
        trace: None,
        turns_count: 2,
        trajectory_actions: vec!["fs.read".into()],
        trajectory_step_count: Some(1),
    };
    let json = serde_json::to_string(&result).unwrap();
    let back: EvalResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result, back);
}

#[test]
fn result_serde_roundtrip_minimal() {
    let result = EvalResult {
        scenario_id: "min".into(),
        profile: "fake".into(),
        passed: true,
        elapsed_ms: 10,
        ..EvalResult::default()
    };
    let json = serde_json::to_string(&result).unwrap();
    let back: EvalResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result, back);
}

#[test]
fn result_skip_serializing_if_omits_optional_fields() {
    let result = EvalResult {
        scenario_id: "clean".into(),
        profile: "default".into(),
        passed: true,
        elapsed_ms: 100,
        ..EvalResult::default()
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("\"failures\""), "empty failures omitted");
    assert!(!json.contains("\"error\""), "None error omitted");
    assert!(
        !json.contains("\"assistant_response\""),
        "None assistant_response omitted"
    );
    assert!(
        !json.contains("\"context_input_tokens\""),
        "None context_input_tokens omitted"
    );
    assert!(
        !json.contains("\"context_window\""),
        "None context_window omitted"
    );
    assert!(
        !json.contains("\"model_usage\""),
        "None model_usage omitted"
    );
    assert!(!json.contains("\"trace\""), "None trace omitted");
    assert!(
        !json.contains("\"trajectory_actions\""),
        "empty trajectory_actions omitted"
    );
    assert!(
        !json.contains("\"trajectory_step_count\""),
        "None trajectory_step_count omitted"
    );
}

#[test]
fn result_context_input_tokens_none_vs_some() {
    let without = EvalResult {
        scenario_id: "no-tokens".into(),
        ..EvalResult::default()
    };
    assert_eq!(without.context_input_tokens, None);

    let with = EvalResult {
        scenario_id: "with-tokens".into(),
        context_input_tokens: Some(42),
        ..EvalResult::default()
    };
    assert_eq!(with.context_input_tokens, Some(42));

    // Serde roundtrip preserves distinction
    let json_without = serde_json::to_string(&without).unwrap();
    let json_with = serde_json::to_string(&with).unwrap();
    let back_without: EvalResult = serde_json::from_str(&json_without).unwrap();
    let back_with: EvalResult = serde_json::from_str(&json_with).unwrap();
    assert_eq!(back_without.context_input_tokens, None);
    assert_eq!(back_with.context_input_tokens, Some(42));
}

// ── EvalSummary serde and edge cases ─────────────────────────────────────────

#[test]
fn summary_serde_roundtrip() {
    let summary = EvalSummary {
        total: 5,
        passed: 3,
        failed: 2,
        success_rate: 0.6,
        total_elapsed_ms: 1000,
        avg_elapsed_ms: 200.0,
        total_tool_invocations: 10,
        total_tool_failures: 1,
        total_context_input_tokens: Some(50_000),
        total_model_usage: None,
    };
    let json = serde_json::to_string(&summary).unwrap();
    let back: EvalSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(summary, back);
}

#[test]
fn summary_serde_without_tokens() {
    let summary = EvalSummary {
        total: 1,
        passed: 1,
        failed: 0,
        success_rate: 1.0,
        total_elapsed_ms: 50,
        avg_elapsed_ms: 50.0,
        total_tool_invocations: 0,
        total_tool_failures: 0,
        total_context_input_tokens: None,
        total_model_usage: None,
    };
    let json = serde_json::to_string(&summary).unwrap();
    assert!(!json.contains("total_context_input_tokens"));
    let back: EvalSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(back.total_context_input_tokens, None);
}

#[test]
fn summary_from_all_passed() {
    let results = vec![
        EvalResult {
            scenario_id: "a".into(),
            passed: true,
            elapsed_ms: 100,
            tool_invocations: 1,
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "b".into(),
            passed: true,
            elapsed_ms: 200,
            tool_invocations: 2,
            ..EvalResult::default()
        },
    ];
    let summary = EvalSummary::from_results(&results);
    assert_eq!(summary.total, 2);
    assert_eq!(summary.passed, 2);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.success_rate, 1.0);
    assert_eq!(summary.total_elapsed_ms, 300);
    assert_eq!(summary.avg_elapsed_ms, 150.0);
    assert_eq!(summary.total_tool_invocations, 3);
}

#[test]
fn summary_from_all_failed() {
    let results = vec![
        EvalResult {
            scenario_id: "x".into(),
            passed: false,
            elapsed_ms: 50,
            tool_failures: 2,
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "y".into(),
            passed: false,
            elapsed_ms: 150,
            tool_failures: 3,
            ..EvalResult::default()
        },
    ];
    let summary = EvalSummary::from_results(&results);
    assert_eq!(summary.total, 2);
    assert_eq!(summary.passed, 0);
    assert_eq!(summary.failed, 2);
    assert_eq!(summary.success_rate, 0.0);
    assert_eq!(summary.total_tool_failures, 5);
    assert_eq!(summary.total_context_input_tokens, None);
}

#[test]
fn summary_from_results_all_none_tokens_yields_none() {
    let results = vec![
        EvalResult {
            scenario_id: "a".into(),
            passed: true,
            elapsed_ms: 10,
            context_input_tokens: None,
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "b".into(),
            passed: true,
            elapsed_ms: 20,
            context_input_tokens: None,
            ..EvalResult::default()
        },
    ];
    let summary = EvalSummary::from_results(&results);
    assert_eq!(summary.total_context_input_tokens, None);
}

#[test]
fn summary_from_results_aggregates_model_usage() {
    let results = vec![
        EvalResult {
            scenario_id: "a".into(),
            model_usage: Some(EvalModelUsage {
                request_count: 1,
                input_tokens: 100,
                output_tokens: 20,
                cache_creation_input_tokens: 10,
                cache_read_input_tokens: 30,
            }),
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "b".into(),
            model_usage: Some(EvalModelUsage {
                request_count: 2,
                input_tokens: 300,
                output_tokens: 50,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 25,
            }),
            ..EvalResult::default()
        },
    ];

    let summary = EvalSummary::from_results(&results);

    assert_eq!(
        summary.total_model_usage,
        Some(EvalModelUsage {
            request_count: 3,
            input_tokens: 400,
            output_tokens: 70,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 55,
        })
    );
}

// ── EvalReport serde ─────────────────────────────────────────────────────────

#[test]
fn report_from_results_builds_summary() {
    let results = vec![
        EvalResult {
            scenario_id: "r1".into(),
            passed: true,
            elapsed_ms: 100,
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "r2".into(),
            passed: false,
            elapsed_ms: 200,
            ..EvalResult::default()
        },
    ];
    let report = EvalReport::from_results(results.clone());
    assert_eq!(report.summary.total, 2);
    assert_eq!(report.summary.passed, 1);
    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.results.len(), 2);
}

#[test]
fn report_serde_roundtrip() {
    let report = EvalReport::from_results(vec![EvalResult {
        scenario_id: "rt".into(),
        profile: "fake".into(),
        passed: true,
        elapsed_ms: 42,
        context_input_tokens: Some(999),
        ..EvalResult::default()
    }]);
    let json = serde_json::to_string(&report).unwrap();
    let back: EvalReport = serde_json::from_str(&json).unwrap();
    assert_eq!(report, back);
}

#[test]
fn report_from_empty_results() {
    let report = EvalReport::from_results(vec![]);
    assert_eq!(report.summary.total, 0);
    assert_eq!(report.summary.success_rate, 0.0);
    assert_eq!(report.summary.avg_elapsed_ms, 0.0);
    assert!(report.results.is_empty());
}

// ── EvalComparison serde ─────────────────────────────────────────────────────

#[test]
fn comparison_serde_roundtrip_with_regressions_and_improvements() {
    let comparison = EvalComparison {
        pass_rate_delta: -0.25,
        avg_elapsed_delta_ms: 50.0,
        total_token_delta: Some(1000),
        regressions: vec![
            ScenarioRegression {
                scenario_id: "s1".into(),
                kind: "passed_to_failed".into(),
            },
            ScenarioRegression {
                scenario_id: "s2".into(),
                kind: "slower_by_60%".into(),
            },
        ],
        improvements: vec![ScenarioImprovement {
            scenario_id: "s3".into(),
            kind: "failed_to_passed".into(),
        }],
    };
    let json = serde_json::to_string(&comparison).unwrap();
    let back: EvalComparison = serde_json::from_str(&json).unwrap();
    assert_eq!(comparison, back);
}

#[test]
fn comparison_serde_roundtrip_empty() {
    let comparison = EvalComparison {
        pass_rate_delta: 0.0,
        avg_elapsed_delta_ms: 0.0,
        total_token_delta: None,
        regressions: vec![],
        improvements: vec![],
    };
    let json = serde_json::to_string(&comparison).unwrap();
    let back: EvalComparison = serde_json::from_str(&json).unwrap();
    assert_eq!(comparison, back);
}

#[test]
fn comparison_total_token_delta_none_roundtrip() {
    let comparison = EvalComparison {
        pass_rate_delta: 0.5,
        avg_elapsed_delta_ms: -10.0,
        total_token_delta: None,
        regressions: vec![],
        improvements: vec![],
    };
    let json = serde_json::to_string(&comparison).unwrap();
    let back: EvalComparison = serde_json::from_str(&json).unwrap();
    assert_eq!(back.total_token_delta, None);
}

// ── ScenarioRegression / ScenarioImprovement serde ───────────────────────────

#[test]
fn regression_serde_roundtrip() {
    let reg = ScenarioRegression {
        scenario_id: "regressed-scenario".into(),
        kind: "more_tokens_75%".into(),
    };
    let json = serde_json::to_string(&reg).unwrap();
    let back: ScenarioRegression = serde_json::from_str(&json).unwrap();
    assert_eq!(reg, back);
}

#[test]
fn improvement_serde_roundtrip() {
    let imp = ScenarioImprovement {
        scenario_id: "improved-scenario".into(),
        kind: "faster_by_80%".into(),
    };
    let json = serde_json::to_string(&imp).unwrap();
    let back: ScenarioImprovement = serde_json::from_str(&json).unwrap();
    assert_eq!(imp, back);
}

// ── Cross-struct JSON interop ────────────────────────────────────────────────

#[test]
fn scenario_with_approval_and_sandbox_policy_roundtrip() {
    let scenario = EvalScenario {
        id: "policy-test".into(),
        prompt: "test".into(),
        approval_policy: Some(ApprovalPolicy::Always),
        sandbox_policy: Some(SandboxPolicy::WorkspaceWrite {
            network_access: true,
            writable_roots: vec!["/tmp".into()],
        }),
        ..EvalScenario::default()
    };
    let json = serde_json::to_string(&scenario).unwrap();
    let back: EvalScenario = serde_json::from_str(&json).unwrap();
    assert_eq!(scenario, back);
}

#[test]
fn result_default_has_expected_zero_values() {
    let result = EvalResult::default();
    assert_eq!(result.scenario_id, "");
    assert_eq!(result.profile, "");
    assert!(!result.passed);
    assert!(result.failures.is_empty());
    assert_eq!(result.error, None);
    assert_eq!(result.elapsed_ms, 0);
    assert_eq!(result.assistant_response, None);
    assert!(result.event_types.is_empty());
    assert_eq!(result.tool_invocations, 0);
    assert_eq!(result.tool_failures, 0);
    assert_eq!(result.context_input_tokens, None);
    assert_eq!(result.context_window, None);
    assert_eq!(result.trace, None);
    assert_eq!(result.turns_count, 0);
    assert!(result.trajectory_actions.is_empty());
    assert_eq!(result.trajectory_step_count, None);
}
