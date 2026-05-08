use agent_core::ContextSource;
use agent_memory::{ContextAssembler, ContextBudget, ContextRequest};
use agent_models::ToolDefinition;

fn budget(window: u64, output: u64) -> ContextBudget {
    ContextBudget {
        context_window: window,
        output_reservation: output,
        source_caps: vec![],
    }
}

#[tokio::test]
async fn assemble_returns_usage_with_per_source_breakdown() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("You are Kairox.".into()),
                user_request: "summarise this repo".into(),
                session_history: vec!["earlier discussion".into()],
                tool_definitions: vec![ToolDefinition {
                    name: "fs.read".into(),
                    description: "Read a file".into(),
                    parameters: serde_json::json!({"type": "object"}),
                }],
                ..Default::default()
            },
            budget(8_000, 1_000),
        )
        .await;

    let usage = &bundle.usage;
    assert_eq!(usage.context_window, 8_000);
    assert_eq!(usage.output_reservation, 1_000);
    assert_eq!(usage.budget_tokens, 7_000);
    assert!(usage.total_tokens > 0);
    assert!(usage
        .by_source
        .iter()
        .any(|(s, n)| matches!(s, ContextSource::System) && *n > 0));
    assert!(usage
        .by_source
        .iter()
        .any(|(s, n)| matches!(s, ContextSource::ToolDefinitions) && *n > 0));
    assert_eq!(usage.estimator, "cl100k_base");
    assert!(!usage.corrected_by_real_usage);
}

#[tokio::test]
async fn assemble_drops_lowest_priority_sections_when_over_budget() {
    let assembler = ContextAssembler::new_standalone();
    // Give a tiny budget so almost everything except System+Request must be dropped.
    let request = ContextRequest {
        system_prompt: Some("S".into()),
        user_request: "U".into(),
        session_history: (0..50).map(|i| format!("history line {}", i)).collect(),
        selected_files: (0..10)
            .map(|i| format!("file-{}.rs contents...", i))
            .collect(),
        ..Default::default()
    };
    let bundle = assembler.assemble(request, budget(200, 50)).await;

    assert!(bundle.truncated, "bundle should be marked truncated");
    assert!(bundle.usage.total_tokens <= bundle.usage.budget_tokens);
    // System should never be dropped.
    assert!(bundle
        .usage
        .by_source
        .iter()
        .any(|(s, _)| matches!(s, ContextSource::System)));
}

#[tokio::test]
async fn per_source_cap_drops_tool_definitions_first_when_caps_exceeded() {
    let assembler = ContextAssembler::new_standalone();
    let big_schema = serde_json::json!({
        "type": "object",
        "properties": (0..200)
            .map(|i| (format!("p{}", i), serde_json::json!({"type": "string"})))
            .collect::<serde_json::Map<_, _>>(),
    });
    let request = ContextRequest {
        system_prompt: Some("S".into()),
        user_request: "U".into(),
        tool_definitions: vec![ToolDefinition {
            name: "huge".into(),
            description: "big tool".into(),
            parameters: big_schema,
        }],
        ..Default::default()
    };
    let mut bdg = budget(50_000, 1_000);
    bdg.source_caps.push((ContextSource::ToolDefinitions, 100));

    let bundle = assembler.assemble(request, bdg).await;
    let tool_tokens: u64 = bundle
        .usage
        .by_source
        .iter()
        .filter(|(s, _)| matches!(s, ContextSource::ToolDefinitions))
        .map(|(_, n)| *n)
        .sum();
    assert!(
        tool_tokens <= 100,
        "tool definitions section ({}) must respect cap",
        tool_tokens
    );
}
