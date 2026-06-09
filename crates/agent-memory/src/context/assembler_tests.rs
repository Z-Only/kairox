use super::*;
use crate::memory::MemoryScope;
use agent_core::ContextSource;

fn standalone() -> ContextAssembler {
    ContextAssembler::new_standalone()
}

fn small_budget() -> ContextBudget {
    ContextBudget {
        context_window: 100,
        output_reservation: 20,
        source_caps: Vec::new(),
    }
}

fn large_budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        output_reservation: 16_384,
        source_caps: Vec::new(),
    }
}

#[tokio::test]
async fn basic_user_request_produces_single_message() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Hello world".into(),
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    assert!(!bundle.messages.is_empty());
    assert!(bundle.messages.iter().any(|m| m.contains("Hello world")));
    assert!(!bundle.truncated);
    assert!(bundle.usage.total_tokens > 0);
}

#[tokio::test]
async fn system_prompt_included_first() {
    let asm = standalone();
    let req = ContextRequest {
        system_prompt: Some("You are helpful.".into()),
        user_request: "Hi".into(),
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    assert_eq!(bundle.sources[0], ContextSource::System);
    assert!(bundle.messages[0].contains("You are helpful."));
}

#[tokio::test]
async fn project_instructions_placed_after_system() {
    let asm = standalone();
    let req = ContextRequest {
        system_prompt: Some("System".into()),
        project_instructions: Some("Project rule".into()),
        user_request: "Hi".into(),
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    let system_idx = bundle
        .sources
        .iter()
        .position(|s| *s == ContextSource::System)
        .unwrap();
    let project_idx = bundle
        .sources
        .iter()
        .position(|s| *s == ContextSource::ProjectInstruction)
        .unwrap();
    assert!(project_idx > system_idx);
    assert!(bundle.messages[project_idx].contains("Project rule"));
}

#[tokio::test]
async fn active_skills_included() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Hi".into(),
        active_skills: vec!["skill-one".into(), "skill-two".into()],
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    assert!(bundle.sources.contains(&ContextSource::Skill));
    let skill_msg = bundle
        .messages
        .iter()
        .find(|m| m.contains("skill-one"))
        .unwrap();
    assert!(skill_msg.contains("skill-two"));
}

#[tokio::test]
async fn session_history_included() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Hi".into(),
        session_history: vec!["previous turn".into()],
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    assert!(bundle.sources.contains(&ContextSource::History));
    assert!(bundle.messages.iter().any(|m| m.contains("previous turn")));
}

#[tokio::test]
async fn tool_results_included() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Hi".into(),
        tool_results: vec!["tool output data".into()],
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    assert!(bundle.sources.contains(&ContextSource::ToolResult));
    assert!(bundle
        .messages
        .iter()
        .any(|m| m.contains("tool output data")));
}

#[tokio::test]
async fn selected_files_included() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Hi".into(),
        selected_files: vec!["fn main() {}".into()],
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    assert!(bundle.sources.contains(&ContextSource::SelectedFile));
}

#[tokio::test]
async fn truncation_when_budget_exceeded() {
    let asm = standalone();
    let long_history: Vec<String> = (0..50)
        .map(|i| format!("History entry {i} with some padding text to consume tokens"))
        .collect();
    let req = ContextRequest {
        user_request: "Hi".into(),
        session_history: long_history,
        ..Default::default()
    };
    let bundle = asm.assemble(req, small_budget()).await;

    assert!(bundle.truncated);
    assert!(bundle.usage.total_tokens <= small_budget().input_budget());
}

#[tokio::test]
async fn source_caps_limit_specific_source() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Hi".into(),
        session_history: vec![
            "h1 padding text".into(),
            "h2 padding text".into(),
            "h3 padding text".into(),
        ],
        ..Default::default()
    };
    let budget = ContextBudget {
        context_window: 200_000,
        output_reservation: 16_384,
        source_caps: vec![(ContextSource::History, 1)], // very tight cap
    };
    let bundle = asm.assemble(req, budget).await;

    // Should have dropped some history entries
    let history_count = bundle
        .sources
        .iter()
        .filter(|s| **s == ContextSource::History)
        .count();
    assert!(history_count < 4); // 3 history + 1 would be uncapped
}

#[tokio::test]
async fn usage_reports_context_window_and_reservation() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Test".into(),
        ..Default::default()
    };
    let budget = large_budget();
    let bundle = asm.assemble(req, budget.clone()).await;

    assert_eq!(bundle.usage.context_window, budget.context_window);
    assert_eq!(bundle.usage.output_reservation, budget.output_reservation);
    assert_eq!(bundle.usage.budget_tokens, budget.input_budget());
    assert_eq!(bundle.usage.estimator, "cl100k_base");
}

#[tokio::test]
async fn active_task_included_in_history() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Hi".into(),
        active_task: Some("implement feature X".into()),
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    assert!(bundle
        .messages
        .iter()
        .any(|m| m.contains("implement feature X")));
}

#[tokio::test]
async fn count_tokens_returns_nonzero_for_text() {
    let asm = standalone();
    let count = asm.count_tokens("Hello, world!");
    assert!(count > 0);
    assert!(count < 100);
}

#[tokio::test]
async fn empty_request_still_produces_bundle() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: String::new(),
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;
    // Should at least have the request section
    assert!(!bundle.messages.is_empty());
}

#[tokio::test]
async fn accepted_memories_included_rejected_excluded() {
    let asm = standalone();
    let req = ContextRequest {
        user_request: "Hi".into(),
        memories: vec![
            MemoryEntry::new(MemoryScope::Session, "accepted memory".into(), true),
            MemoryEntry::new(MemoryScope::Session, "rejected memory".into(), false),
        ],
        ..Default::default()
    };
    let bundle = asm.assemble(req, large_budget()).await;

    assert!(bundle
        .messages
        .iter()
        .any(|m| m.contains("accepted memory")));
    assert!(!bundle
        .messages
        .iter()
        .any(|m| m.contains("rejected memory")));
}
