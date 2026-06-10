use agent_config::Config;
use agent_eval::{
    compare_reports, filter_scenarios_by_tags, load_scenarios_from_str, EvalCommandExpectation,
    EvalExpectation, EvalFileExpectation, EvalHarness, EvalReport, EvalResult, EvalRunOptions,
    EvalScenario, EvalSummary,
};
use std::collections::HashMap;

#[test]
fn loads_jsonl_scenarios_and_skips_comments() {
    let input = r#"
# smoke cases
{"id":"hello","prompt":"Say hello","profile":"fake","expected":{"assistant_contains":["hello"]}}

{"id":"trace","prompt":"Emit trace","expected":{"event_types":["UserMessageAdded"],"max_elapsed_ms":250,"max_context_input_tokens":2048}}
"#;

    let scenarios = load_scenarios_from_str(input).expect("scenarios should parse");

    assert_eq!(scenarios.len(), 2);
    assert_eq!(scenarios[0].id, "hello");
    assert_eq!(scenarios[0].profile.as_deref(), Some("fake"));
    assert_eq!(scenarios[0].expected.assistant_contains, vec!["hello"]);
    assert_eq!(scenarios[1].expected.event_types, vec!["UserMessageAdded"]);
    assert_eq!(scenarios[1].expected.max_elapsed_ms, Some(250));
    assert_eq!(scenarios[1].expected.max_context_input_tokens, Some(2048));
}

#[test]
fn loads_extended_expectation_fields_from_jsonl() {
    let input = r#"{"id":"ext","prompt":"test","expected":{"assistant_not_contains":["error"],"assistant_matches_regex":["\\d+"],"min_events_of_type":{"ToolInvocationStarted":2},"max_events_of_type":{"ToolInvocationFailed":0},"max_turns":3,"trajectory_actions":["fs.read","fs.write"],"max_trajectory_steps":5,"workspace_files":[{"path":"src/lib.rs","contains":["pub fn add"],"not_contains":["TODO"]}],"post_run_commands":[{"program":"sh","args":["-c","test -f src/lib.rs"],"timeout_ms":1000}]}}"#;

    let scenarios = load_scenarios_from_str(input).expect("should parse");
    let expected = &scenarios[0].expected;

    assert_eq!(expected.assistant_not_contains, vec!["error"]);
    assert_eq!(expected.assistant_matches_regex, vec!["\\d+"]);
    assert_eq!(
        expected.min_events_of_type.get("ToolInvocationStarted"),
        Some(&2)
    );
    assert_eq!(
        expected.max_events_of_type.get("ToolInvocationFailed"),
        Some(&0)
    );
    assert_eq!(expected.max_turns, Some(3));
    assert_eq!(expected.trajectory_actions, vec!["fs.read", "fs.write"]);
    assert_eq!(expected.max_trajectory_steps, Some(5));
    assert_eq!(
        expected.workspace_files,
        vec![EvalFileExpectation {
            path: "src/lib.rs".into(),
            contains: vec!["pub fn add".into()],
            not_contains: vec!["TODO".into()],
        }]
    );
    assert_eq!(
        expected.post_run_commands,
        vec![EvalCommandExpectation {
            program: "sh".into(),
            args: vec!["-c".into(), "test -f src/lib.rs".into()],
            cwd: None,
            timeout_ms: Some(1000),
            exit_code: Some(0),
            stdout_contains: Vec::new(),
            stderr_contains: Vec::new(),
        }]
    );
}

#[test]
fn loads_multi_turn_scenario_from_jsonl() {
    let input = r#"{"id":"multi","prompt":"hello","turns":["follow up 1","follow up 2"],"system_instructions":"Be concise","expected":{"assistant_contains":["hello"]}}"#;

    let scenarios = load_scenarios_from_str(input).expect("should parse");
    let scenario = &scenarios[0];

    assert_eq!(scenario.turns, vec!["follow up 1", "follow up 2"]);
    assert_eq!(scenario.system_instructions.as_deref(), Some("Be concise"));
}

#[test]
fn filters_scenarios_by_include_and_exclude_tags() {
    let scenarios = vec![
        EvalScenario {
            id: "fast".into(),
            tags: vec!["smoke".into(), "fast".into()],
            ..EvalScenario::default()
        },
        EvalScenario {
            id: "slow".into(),
            tags: vec!["smoke".into(), "slow".into()],
            ..EvalScenario::default()
        },
        EvalScenario {
            id: "untagged".into(),
            ..EvalScenario::default()
        },
    ];
    let include = vec!["smoke".to_string()];
    let exclude = vec!["slow".to_string()];

    let filtered = filter_scenarios_by_tags(&scenarios, &include, &exclude);

    assert_eq!(
        filtered
            .iter()
            .map(|scenario| scenario.id.as_str())
            .collect::<Vec<_>>(),
        vec!["fast"]
    );
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
    assert_eq!(result.turns_count, 1);
}

#[tokio::test]
async fn scenario_fails_when_forbidden_event_type_is_seen() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "forbidden-event".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            assistant_contains: vec!["hello from Kairox".into()],
            forbidden_event_types: vec!["AssistantMessageCompleted".into()],
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(!result.passed);
    assert!(result
        .failures
        .contains(&"forbidden event type present: AssistantMessageCompleted".into()));
}

#[tokio::test]
async fn scenario_fails_when_budget_expectations_are_exceeded() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "budget-guard".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            assistant_contains: vec!["hello from Kairox".into()],
            max_elapsed_ms: Some(0),
            max_context_input_tokens: Some(0),
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(!result.passed);
    assert!(result
        .failures
        .iter()
        .any(|failure| failure.starts_with("elapsed time above maximum: expected at most 0 ms")));
    assert!(result.failures.iter().any(|failure| failure
        .starts_with("context input tokens above maximum: expected at most 0, got ")));
}

#[tokio::test]
async fn scenario_fails_with_not_contains_expectation() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "not-contains".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            assistant_not_contains: vec!["Kairox".into()],
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(!result.passed);
    assert!(result
        .failures
        .iter()
        .any(|f| f.contains("assistant response contains forbidden substring: Kairox")));
}

#[tokio::test]
async fn scenario_passes_with_regex_match() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "regex-pass".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            assistant_matches_regex: vec!["hello.*Kairox".into()],
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(result.passed, "{:?}", result.failures);
}

#[tokio::test]
async fn scenario_fails_with_regex_mismatch() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "regex-fail".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            assistant_matches_regex: vec!["^goodbye".into()],
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(!result.passed);
    assert!(result
        .failures
        .iter()
        .any(|f| f.contains("assistant response does not match regex: ^goodbye")));
}

#[tokio::test]
async fn scenario_checks_min_events_of_type() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let mut min_events = HashMap::new();
    min_events.insert("AssistantMessageCompleted".into(), 2);

    let scenario = EvalScenario {
        id: "min-events".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            min_events_of_type: min_events,
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(!result.passed);
    assert!(result.failures.iter().any(|f| f.contains(
        "event type `AssistantMessageCompleted` count below minimum: expected at least 2, got 1"
    )));
}

#[tokio::test]
async fn scenario_checks_max_turns() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "max-turns-pass".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            max_turns: Some(1),
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(result.passed, "{:?}", result.failures);
}

#[tokio::test]
async fn fake_tool_call_scenario_emits_tool_lifecycle_events() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        include_trace: true,
        fake_emit_tool_call: true,
        wait_timeout_ms: Some(5_000),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "fake-tool-call".into(),
        prompt: "List the workspace root".into(),
        expected: EvalExpectation {
            event_types: vec![
                "UserMessageAdded".into(),
                "ModelToolCallRequested".into(),
                "ToolInvocationStarted".into(),
                "ToolInvocationCompleted".into(),
                "AssistantMessageCompleted".into(),
            ],
            min_tool_invocations: Some(1),
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
    assert!(result
        .event_types
        .contains(&"ModelToolCallRequested".into()));
    assert!(result.event_types.contains(&"ToolInvocationStarted".into()));
    assert!(result
        .event_types
        .contains(&"ToolInvocationCompleted".into()));
    assert_eq!(result.tool_invocations, 1);
    assert_eq!(result.tool_failures, 0);
}

#[tokio::test]
async fn fake_tool_call_checks_trajectory_actions() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        fake_emit_tool_call: true,
        wait_timeout_ms: Some(5_000),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "trajectory-check".into(),
        prompt: "List the workspace root".into(),
        expected: EvalExpectation {
            trajectory_actions: vec!["fs.list".into()],
            max_trajectory_steps: Some(5),
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(result.passed, "{:?}", result.failures);
    assert_eq!(result.trajectory_actions, vec!["fs.list"]);
}

#[tokio::test]
async fn fake_compaction_scenario_triggers_auto_compaction_events() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        include_trace: true,
        auto_compact_threshold: Some(0.001),
        seed_synthetic_pairs: Some(4),
        wait_timeout_ms: Some(5_000),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "fake-compaction".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            assistant_contains: vec!["hello from Kairox".into()],
            event_types: vec![
                "UserMessageAdded".into(),
                "AssistantMessageCompleted".into(),
                "ContextCompactionStarted".into(),
                "ContextCompactionCompleted".into(),
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
    assert!(result
        .event_types
        .contains(&"ContextCompactionStarted".into()));
    assert!(result
        .event_types
        .contains(&"ContextCompactionCompleted".into()));
}

#[tokio::test]
async fn scenario_checks_workspace_files_and_post_run_commands() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let src_dir = workspace.path().join("src");
    std::fs::create_dir_all(&src_dir).expect("src dir");
    std::fs::write(
        src_dir.join("lib.rs"),
        "pub fn add(left: i32, right: i32) -> i32 { left + right }\n",
    )
    .expect("workspace file");

    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        allow_post_run_commands: true,
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "workspace-assertions".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            workspace_files: vec![EvalFileExpectation {
                path: "src/lib.rs".into(),
                contains: vec!["pub fn add".into()],
                not_contains: vec!["TODO".into()],
            }],
            post_run_commands: vec![EvalCommandExpectation {
                program: "sh".into(),
                args: vec![
                    "-c".into(),
                    "test -f src/lib.rs && grep -q 'pub fn add' src/lib.rs".into(),
                ],
                timeout_ms: Some(1_000),
                ..EvalCommandExpectation::default()
            }],
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(result.passed, "{:?}", result.failures);
}

#[tokio::test]
async fn post_run_commands_require_explicit_opt_in() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "command-opt-in".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            post_run_commands: vec![EvalCommandExpectation {
                program: "sh".into(),
                args: vec!["-c".into(), "true".into()],
                ..EvalCommandExpectation::default()
            }],
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let result = harness
        .run_scenario(&scenario)
        .await
        .expect("scenario should run");

    assert!(!result.passed);
    assert!(result.failures.iter().any(|failure| failure
        .contains("post-run command expectations require --allow-post-run-commands")));
}

#[tokio::test]
async fn scenario_timeout_stops_long_running_scenario() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        allow_post_run_commands: true,
        scenario_timeout_ms: Some(3_000),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenario = EvalScenario {
        id: "scenario-timeout".into(),
        prompt: "Say hello from the configured fake model".into(),
        expected: EvalExpectation {
            post_run_commands: vec![EvalCommandExpectation {
                program: "sh".into(),
                args: vec!["-c".into(), "sleep 30".into()],
                exit_code: None,
                ..EvalCommandExpectation::default()
            }],
            ..EvalExpectation::default()
        },
        ..EvalScenario::default()
    };

    let results = harness.run_scenarios_until_failure(&[scenario]).await;

    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);
    assert!(results[0]
        .failures
        .contains(&"scenario timed out after 3000 ms".to_string()));
    assert!(results[0]
        .event_types
        .contains(&"AssistantMessageCompleted".to_string()));
}

#[tokio::test]
async fn fail_fast_scenario_run_stops_after_first_failure() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut harness = EvalHarness::new(EvalRunOptions {
        workspace_root: workspace.path().to_path_buf(),
        default_profile: Some("fake".into()),
        config: Some(Config::defaults()),
        ..EvalRunOptions::default()
    })
    .await
    .expect("harness should initialize");

    let scenarios = vec![
        EvalScenario {
            id: "passes-first".into(),
            prompt: "Say hello from the configured fake model".into(),
            expected: EvalExpectation {
                assistant_contains: vec!["hello from Kairox".into()],
                ..EvalExpectation::default()
            },
            ..EvalScenario::default()
        },
        EvalScenario {
            id: "fails-second".into(),
            prompt: "Say hello from the configured fake model".into(),
            expected: EvalExpectation {
                assistant_contains: vec!["text that never appears".into()],
                ..EvalExpectation::default()
            },
            ..EvalScenario::default()
        },
        EvalScenario {
            id: "should-not-run".into(),
            prompt: "Say hello from the configured fake model".into(),
            ..EvalScenario::default()
        },
    ];

    let results = harness.run_scenarios_until_failure(&scenarios).await;

    assert_eq!(
        results
            .iter()
            .map(|result| result.scenario_id.as_str())
            .collect::<Vec<_>>(),
        vec!["passes-first", "fails-second"]
    );
    assert!(results[0].passed);
    assert!(!results[1].passed);
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

#[test]
fn compare_reports_detects_regression_and_improvement() {
    let baseline = EvalReport {
        summary: EvalSummary::from_results(&[
            EvalResult {
                scenario_id: "a".into(),
                passed: true,
                elapsed_ms: 100,
                context_input_tokens: Some(50),
                ..EvalResult::default()
            },
            EvalResult {
                scenario_id: "b".into(),
                passed: false,
                elapsed_ms: 200,
                context_input_tokens: Some(80),
                ..EvalResult::default()
            },
        ]),
        results: vec![
            EvalResult {
                scenario_id: "a".into(),
                passed: true,
                elapsed_ms: 100,
                context_input_tokens: Some(50),
                ..EvalResult::default()
            },
            EvalResult {
                scenario_id: "b".into(),
                passed: false,
                elapsed_ms: 200,
                context_input_tokens: Some(80),
                ..EvalResult::default()
            },
        ],
    };

    let candidate = EvalReport {
        summary: EvalSummary::from_results(&[
            EvalResult {
                scenario_id: "a".into(),
                passed: false,
                elapsed_ms: 100,
                context_input_tokens: Some(50),
                ..EvalResult::default()
            },
            EvalResult {
                scenario_id: "b".into(),
                passed: true,
                elapsed_ms: 200,
                context_input_tokens: Some(80),
                ..EvalResult::default()
            },
        ]),
        results: vec![
            EvalResult {
                scenario_id: "a".into(),
                passed: false,
                elapsed_ms: 100,
                context_input_tokens: Some(50),
                ..EvalResult::default()
            },
            EvalResult {
                scenario_id: "b".into(),
                passed: true,
                elapsed_ms: 200,
                context_input_tokens: Some(80),
                ..EvalResult::default()
            },
        ],
    };

    let comparison = compare_reports(&baseline, &candidate);

    assert_eq!(comparison.pass_rate_delta, 0.0);
    assert!(comparison
        .regressions
        .iter()
        .any(|r| r.scenario_id == "a" && r.kind == "passed_to_failed"));
    assert!(comparison
        .improvements
        .iter()
        .any(|i| i.scenario_id == "b" && i.kind == "failed_to_passed"));
}

#[test]
fn compare_reports_detects_speed_regression() {
    let baseline = EvalReport::from_results(vec![EvalResult {
        scenario_id: "slow".into(),
        passed: true,
        elapsed_ms: 100,
        ..EvalResult::default()
    }]);

    let candidate = EvalReport::from_results(vec![EvalResult {
        scenario_id: "slow".into(),
        passed: true,
        elapsed_ms: 200,
        ..EvalResult::default()
    }]);

    let comparison = compare_reports(&baseline, &candidate);

    assert!(comparison
        .regressions
        .iter()
        .any(|r| r.scenario_id == "slow" && r.kind.starts_with("slower_by_")));
}

// ──────────────────────────────────────────────────────────────────────────────
// Unit tests for EvalSummary::from_results and compare_reports
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn summary_from_empty_results() {
    let summary = EvalSummary::from_results(&[]);

    assert_eq!(summary.total, 0);
    assert_eq!(summary.passed, 0);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.success_rate, 0.0);
    assert_eq!(summary.total_elapsed_ms, 0);
    assert_eq!(summary.avg_elapsed_ms, 0.0);
    assert_eq!(summary.total_tool_invocations, 0);
    assert_eq!(summary.total_tool_failures, 0);
    assert_eq!(summary.total_context_input_tokens, None);
}

#[test]
fn summary_from_mixed_results() {
    let results = vec![
        EvalResult {
            scenario_id: "a".into(),
            passed: true,
            elapsed_ms: 120,
            tool_invocations: 3,
            tool_failures: 0,
            context_input_tokens: Some(100),
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "b".into(),
            passed: true,
            elapsed_ms: 80,
            tool_invocations: 1,
            tool_failures: 0,
            context_input_tokens: Some(200),
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "c".into(),
            passed: false,
            elapsed_ms: 300,
            tool_invocations: 2,
            tool_failures: 1,
            context_input_tokens: Some(150),
            ..EvalResult::default()
        },
    ];

    let summary = EvalSummary::from_results(&results);

    assert_eq!(summary.total, 3);
    assert_eq!(summary.passed, 2);
    assert_eq!(summary.failed, 1);
    assert!((summary.success_rate - 2.0 / 3.0).abs() < f64::EPSILON);
    assert_eq!(summary.total_elapsed_ms, 500);
    assert!((summary.avg_elapsed_ms - 500.0 / 3.0).abs() < f64::EPSILON);
    assert_eq!(summary.total_tool_invocations, 6);
    assert_eq!(summary.total_tool_failures, 1);
    assert_eq!(summary.total_context_input_tokens, Some(450));
}

#[test]
fn summary_aggregates_context_tokens() {
    let results = vec![
        EvalResult {
            scenario_id: "with-tokens".into(),
            passed: true,
            elapsed_ms: 50,
            context_input_tokens: Some(1000),
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "no-tokens".into(),
            passed: true,
            elapsed_ms: 50,
            context_input_tokens: None,
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "more-tokens".into(),
            passed: true,
            elapsed_ms: 50,
            context_input_tokens: Some(2000),
            ..EvalResult::default()
        },
    ];

    let summary = EvalSummary::from_results(&results);

    // Only Some values are summed; None is skipped but result is still Some
    assert_eq!(summary.total_context_input_tokens, Some(3000));
}

#[test]
fn compare_reports_detects_pass_to_fail_regression() {
    let baseline = EvalReport::from_results(vec![EvalResult {
        scenario_id: "regressed".into(),
        passed: true,
        elapsed_ms: 100,
        ..EvalResult::default()
    }]);

    let candidate = EvalReport::from_results(vec![EvalResult {
        scenario_id: "regressed".into(),
        passed: false,
        elapsed_ms: 100,
        ..EvalResult::default()
    }]);

    let comparison = compare_reports(&baseline, &candidate);

    assert!(comparison
        .regressions
        .iter()
        .any(|r| r.scenario_id == "regressed" && r.kind == "passed_to_failed"));
    assert!(comparison.improvements.is_empty());
    assert!((comparison.pass_rate_delta - (-1.0)).abs() < f64::EPSILON);
}

#[test]
fn compare_reports_detects_fail_to_pass_improvement() {
    let baseline = EvalReport::from_results(vec![EvalResult {
        scenario_id: "improved".into(),
        passed: false,
        elapsed_ms: 100,
        ..EvalResult::default()
    }]);

    let candidate = EvalReport::from_results(vec![EvalResult {
        scenario_id: "improved".into(),
        passed: true,
        elapsed_ms: 100,
        ..EvalResult::default()
    }]);

    let comparison = compare_reports(&baseline, &candidate);

    assert!(comparison
        .improvements
        .iter()
        .any(|i| i.scenario_id == "improved" && i.kind == "failed_to_passed"));
    assert!(comparison.regressions.is_empty());
    assert!((comparison.pass_rate_delta - 1.0).abs() < f64::EPSILON);
}

#[test]
fn compare_reports_detects_speed_regression_above_50_percent() {
    let baseline = EvalReport::from_results(vec![EvalResult {
        scenario_id: "slow-scenario".into(),
        passed: true,
        elapsed_ms: 100,
        ..EvalResult::default()
    }]);

    // 160ms is 60% slower than 100ms → triggers regression (>50%)
    let candidate = EvalReport::from_results(vec![EvalResult {
        scenario_id: "slow-scenario".into(),
        passed: true,
        elapsed_ms: 160,
        ..EvalResult::default()
    }]);

    let comparison = compare_reports(&baseline, &candidate);

    assert!(comparison
        .regressions
        .iter()
        .any(|r| r.scenario_id == "slow-scenario" && r.kind.starts_with("slower_by_")));
}

#[test]
fn compare_reports_detects_speed_improvement_above_50_percent() {
    let baseline = EvalReport::from_results(vec![EvalResult {
        scenario_id: "fast-scenario".into(),
        passed: true,
        elapsed_ms: 200,
        ..EvalResult::default()
    }]);

    // 80ms is 60% faster than 200ms → triggers improvement (>50%)
    let candidate = EvalReport::from_results(vec![EvalResult {
        scenario_id: "fast-scenario".into(),
        passed: true,
        elapsed_ms: 80,
        ..EvalResult::default()
    }]);

    let comparison = compare_reports(&baseline, &candidate);

    assert!(comparison
        .improvements
        .iter()
        .any(|i| i.scenario_id == "fast-scenario" && i.kind.starts_with("faster_by_")));
}

#[test]
fn compare_reports_token_delta() {
    let baseline = EvalReport::from_results(vec![
        EvalResult {
            scenario_id: "a".into(),
            passed: true,
            elapsed_ms: 100,
            context_input_tokens: Some(500),
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "b".into(),
            passed: true,
            elapsed_ms: 100,
            context_input_tokens: Some(300),
            ..EvalResult::default()
        },
    ]);

    let candidate = EvalReport::from_results(vec![
        EvalResult {
            scenario_id: "a".into(),
            passed: true,
            elapsed_ms: 100,
            context_input_tokens: Some(600),
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "b".into(),
            passed: true,
            elapsed_ms: 100,
            context_input_tokens: Some(200),
            ..EvalResult::default()
        },
    ]);

    let comparison = compare_reports(&baseline, &candidate);

    // Baseline total tokens: 500 + 300 = 800
    // Candidate total tokens: 600 + 200 = 800
    // Delta: 800 - 800 = 0
    assert_eq!(comparison.total_token_delta, Some(0));

    // Now test with an actual difference
    let candidate2 = EvalReport::from_results(vec![
        EvalResult {
            scenario_id: "a".into(),
            passed: true,
            elapsed_ms: 100,
            context_input_tokens: Some(700),
            ..EvalResult::default()
        },
        EvalResult {
            scenario_id: "b".into(),
            passed: true,
            elapsed_ms: 100,
            context_input_tokens: Some(400),
            ..EvalResult::default()
        },
    ]);

    let comparison2 = compare_reports(&baseline, &candidate2);

    // Baseline total: 800, Candidate total: 1100, delta: +300
    assert_eq!(comparison2.total_token_delta, Some(300));
}
