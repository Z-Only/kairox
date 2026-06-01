use std::sync::Arc;

use agent_core::ContextSource;
use sqlx::sqlite::SqlitePoolOptions;

use crate::memory::{MemoryEntry, MemoryScope};
use crate::store::{MemoryStore, SqliteMemoryStore};

use super::assembler::{ContextAssembler, ContextRequest};
use super::budget::ContextBudget;
use super::image_pruning::{ImageEntry, ImagePruningStrategy};
use super::window::find_lowest_priority_drop;

async fn test_assembler_with_store() -> (ContextAssembler, Arc<dyn MemoryStore>) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = Arc::new(SqliteMemoryStore::new(pool).await.unwrap()) as Arc<dyn MemoryStore>;
    let assembler = ContextAssembler::new(store.clone());
    (assembler, store)
}

fn test_budget(window: u64, output: u64) -> ContextBudget {
    ContextBudget {
        context_window: window,
        output_reservation: output,
        source_caps: vec![],
    }
}

#[tokio::test]
async fn assembles_request_with_standalone_assembler() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "fix tests".into(),
                session_history: vec!["previous answer".into()],
                selected_files: vec![],
                tool_results: vec![],
                memories: vec![],
                active_task: None,
                ..Default::default()
            },
            test_budget(200, 100),
        )
        .await;

    assert!(bundle.messages.join("\n").contains("fix tests"));
    assert!(bundle.usage.total_tokens <= bundle.usage.budget_tokens);
}

#[tokio::test]
async fn includes_memories_from_store() {
    let (assembler, store) = test_assembler_with_store().await;
    store
        .store(MemoryEntry::new(
            MemoryScope::Workspace,
            "Use cargo nextest".into(),
            true,
        ))
        .await
        .unwrap();

    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "nextest config".into(),
                ..Default::default()
            },
            test_budget(600, 100),
        )
        .await;

    assert!(bundle.messages.join("\n").contains("Use cargo nextest"));
}

#[tokio::test]
async fn truncates_lowest_priority_first() {
    let assembler = ContextAssembler::new_standalone();
    let long_files: Vec<String> = (0..20)
        .map(|i| format!("file_content_{i}_with_a_long_name"))
        .collect();

    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("System".into()),
                user_request: "request".into(),
                selected_files: long_files,
                ..Default::default()
            },
            test_budget(100, 50),
        )
        .await;

    // System and request should survive
    assert!(bundle.messages[0].contains("System"));
    assert!(bundle.truncated);
}

#[tokio::test]
async fn never_drops_system_or_request() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("Important system prompt".into()),
                user_request: "User query here".into(),
                ..Default::default()
            },
            test_budget(100, 80),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("Important system prompt") || combined.contains("User query"));
}

#[test]
fn skill_drop_priority_is_below_system_and_above_tool_definitions() {
    let with_tool_definitions = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Skill, String::from("skill"), 1),
        (
            ContextSource::ToolDefinitions,
            String::from("tool definitions"),
            1,
        ),
    ];
    assert_eq!(find_lowest_priority_drop(&with_tool_definitions), Some(2));

    let without_tool_definitions = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Skill, String::from("skill"), 1),
    ];
    assert_eq!(
        find_lowest_priority_drop(&without_tool_definitions),
        Some(1)
    );

    let protected_sources = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Request, String::from("request"), 1),
    ];
    assert_eq!(find_lowest_priority_drop(&protected_sources), None);
}

#[test]
fn input_budget_subtracts_output_reservation() {
    let budget = ContextBudget {
        context_window: 200_000,
        output_reservation: 12_000,
        source_caps: vec![],
    };
    assert_eq!(budget.input_budget(), 188_000);
}

#[test]
fn input_budget_saturates_at_zero_when_reservation_exceeds_window() {
    let budget = ContextBudget {
        context_window: 8_000,
        output_reservation: 12_000,
        source_caps: vec![],
    };
    assert_eq!(budget.input_budget(), 0);
}

#[tokio::test]
async fn includes_project_instructions_section() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("System".into()),
                project_instructions: Some(
                    "### Instructions from AGENTS.md\n\nUse cargo nextest.".into(),
                ),
                user_request: "test".into(),
                ..Default::default()
            },
            test_budget(600, 100),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("<project-instructions>"));
    assert!(combined.contains("Use cargo nextest"));
    assert!(combined.contains("</project-instructions>"));
    assert!(combined.find("System").unwrap() < combined.find("<project-instructions>").unwrap());
}

#[tokio::test]
async fn project_instructions_dropped_as_last_resort() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("S".into()),
                project_instructions: Some("PI content here".into()),
                user_request: "q".into(),
                ..Default::default()
            },
            test_budget(15, 0),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("S"), "System must survive");
    assert!(combined.contains("q"), "Request must survive");
    assert!(bundle.truncated);
}

#[test]
fn project_instruction_drop_order_is_between_memory_and_tool_defs() {
    let sections = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Request, String::from("request"), 1),
        (ContextSource::Memory, String::from("memory"), 1),
        (ContextSource::ProjectInstruction, String::from("pi"), 1),
    ];
    // Memory drops first (lower priority than PI)
    assert_eq!(find_lowest_priority_drop(&sections), Some(2));

    let sections_no_mem = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Request, String::from("request"), 1),
        (ContextSource::ProjectInstruction, String::from("pi"), 1),
    ];
    assert_eq!(find_lowest_priority_drop(&sections_no_mem), Some(2));
}

#[test]
fn budget_input_equals_window_minus_output() {
    let budget = ContextBudget {
        context_window: 10000,
        output_reservation: 2000,
        source_caps: vec![],
    };
    assert_eq!(budget.input_budget(), 8000);
}

#[tokio::test]
async fn assemble_without_memory_store_produces_bundle() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                ..Default::default()
            },
            test_budget(500, 100),
        )
        .await;
    assert!(!bundle.messages.is_empty());
}

#[tokio::test]
async fn assemble_never_drops_system_or_user_request() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("System".into()),
                user_request: "request".into(),
                ..Default::default()
            },
            ContextBudget {
                context_window: 30,
                output_reservation: 0,
                source_caps: vec![],
            },
        )
        .await;
    let combined = bundle.messages.join("\n");
    assert!(combined.contains("System"), "System prompt must survive");
    assert!(combined.contains("request"), "User request must survive");
}

#[tokio::test]
async fn assemble_respects_budget_truncates() {
    let assembler = ContextAssembler::new_standalone();
    let history: Vec<String> = (0..50)
        .map(|i| {
            format!(
                "long history entry number {} with extra padding text to consume tokens",
                i
            )
        })
        .collect();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                session_history: history,
                ..Default::default()
            },
            test_budget(100, 0),
        )
        .await;
    assert!(
        bundle.truncated,
        "Expected truncated=true with small budget"
    );
}

#[test]
fn context_request_default_is_empty() {
    let req = ContextRequest::default();
    assert!(req.system_prompt.is_none());
    assert!(req.project_instructions.is_none());
    assert!(req.user_request.is_empty());
    assert!(req.session_history.is_empty());
    assert!(req.selected_files.is_empty());
    assert!(req.tool_results.is_empty());
    assert!(req.memories.is_empty());
    assert!(req.active_skills.is_empty());
    assert!(req.active_task.is_none());
    assert!(req.session_id.is_none());
    assert!(req.workspace_id.is_none());
    assert!(req.tool_definitions.is_empty());
    assert!(req.images.is_empty());
    assert_eq!(req.image_pruning, ImagePruningStrategy::None);
}

// ---------------------------------------------------------------------------
// Image drop priority
// ---------------------------------------------------------------------------

#[test]
fn image_drop_priority_is_lowest_drops_before_selected_file() {
    let sections = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Request, String::from("request"), 1),
        (ContextSource::SelectedFile, String::from("file"), 1),
        (ContextSource::Image, String::from("image"), 1),
    ];
    // Image should be dropped first (lowest priority)
    assert_eq!(find_lowest_priority_drop(&sections), Some(3));
}

#[test]
fn image_drops_before_everything_except_system_and_request() {
    let sections = vec![
        (ContextSource::System, String::from("system"), 1),
        (ContextSource::Request, String::from("request"), 1),
        (ContextSource::Image, String::from("image"), 1),
        (ContextSource::Memory, String::from("memory"), 1),
    ];
    assert_eq!(find_lowest_priority_drop(&sections), Some(2));
}

// ---------------------------------------------------------------------------
// Assembler integration: images
// ---------------------------------------------------------------------------

fn make_image_entries(count: usize, tokens_each: u64) -> Vec<ImageEntry> {
    (0..count)
        .map(|i| ImageEntry {
            position: i,
            estimated_tokens: tokens_each,
            content: format!("screenshot_{i}"),
        })
        .collect()
}

#[tokio::test]
async fn assembler_includes_images_in_bundle() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                images: make_image_entries(2, 10),
                ..Default::default()
            },
            test_budget(5000, 100),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("screenshot_0"));
    assert!(combined.contains("screenshot_1"));
    assert!(bundle
        .sources
        .iter()
        .any(|s| matches!(s, ContextSource::Image)));
}

#[tokio::test]
async fn assembler_prunes_images_with_strip_oldest() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                images: make_image_entries(5, 10),
                image_pruning: ImagePruningStrategy::StripOldestImages { keep: 2 },
                ..Default::default()
            },
            test_budget(5000, 100),
        )
        .await;

    let combined = bundle.messages.join("\n");
    // Only the last 2 (screenshot_3, screenshot_4) should survive pruning
    assert!(!combined.contains("screenshot_0"));
    assert!(!combined.contains("screenshot_1"));
    assert!(!combined.contains("screenshot_2"));
    assert!(combined.contains("screenshot_3"));
    assert!(combined.contains("screenshot_4"));
}

#[tokio::test]
async fn assembler_drops_images_before_other_sources_on_budget() {
    let assembler = ContextAssembler::new_standalone();
    // Budget that can hold system + request + file (~10 tokens) but NOT the
    // image (estimated_tokens = 500). The assembler must drop the image first
    // because Image has the lowest priority in the drop order.
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("S".into()),
                user_request: "U".into(),
                selected_files: vec!["f.rs".into()],
                images: vec![ImageEntry {
                    position: 0,
                    estimated_tokens: 500,
                    content: "big_screenshot".into(),
                }],
                ..Default::default()
            },
            test_budget(50, 0),
        )
        .await;

    assert!(bundle.truncated);
    // The image should have been dropped; the file should survive.
    let has_image = bundle
        .sources
        .iter()
        .any(|s| matches!(s, ContextSource::Image));
    let has_file = bundle
        .sources
        .iter()
        .any(|s| matches!(s, ContextSource::SelectedFile));
    assert!(!has_image, "Image should be dropped (lowest priority)");
    assert!(
        has_file,
        "SelectedFile should survive because image was dropped first"
    );
}

#[tokio::test]
async fn assembler_images_produce_correct_token_count_in_usage() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                images: make_image_entries(2, 500),
                ..Default::default()
            },
            test_budget(50000, 1000),
        )
        .await;

    let image_tokens: u64 = bundle
        .usage
        .by_source
        .iter()
        .filter(|(s, _)| matches!(s, ContextSource::Image))
        .map(|(_, n)| *n)
        .sum();
    // Each image has estimated_tokens=500, and we take max(estimated, counted).
    // The text "Image: screenshot_N" is small, so estimated (500) dominates.
    assert!(
        image_tokens >= 1000,
        "Expected >= 1000 image tokens from 2 images at 500 each, got {image_tokens}"
    );
}
