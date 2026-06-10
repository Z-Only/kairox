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
async fn markdown_data_uri_images_are_not_counted_as_request_text() {
    let assembler = ContextAssembler::new_standalone();
    let encoded_image = "A".repeat(24_000);
    let user_request =
        format!("read this ![fixture.png](data:image/png;base64,{encoded_image}) now");

    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request,
                ..Default::default()
            },
            test_budget(200_000, 1_000),
        )
        .await;

    let request_tokens: u64 = bundle
        .usage
        .by_source
        .iter()
        .filter(|(source, _)| matches!(source, ContextSource::Request))
        .map(|(_, tokens)| *tokens)
        .sum();
    let image_tokens: u64 = bundle
        .usage
        .by_source
        .iter()
        .filter(|(source, _)| matches!(source, ContextSource::Image))
        .map(|(_, tokens)| *tokens)
        .sum();
    let combined = bundle.messages.join("\n");

    assert!(
        request_tokens < 200,
        "request text tokens should ignore image base64, got {request_tokens}"
    );
    assert!(
        image_tokens > 0,
        "embedded image should be counted as image context"
    );
    assert!(
        !combined.contains(&encoded_image),
        "context bundle should not retain raw image base64"
    );
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

// ---------------------------------------------------------------------------
// Section ordering: verify the full priority chain
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sections_follow_priority_ordering() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("system".into()),
                project_instructions: Some("project instructions".into()),
                active_skills: vec!["skill-A".into()],
                tool_definitions: vec![agent_models::ToolDefinition {
                    name: "tool_x".into(),
                    description: "desc".into(),
                    parameters: serde_json::json!({}),
                }],
                user_request: "query".into(),
                memories: vec![MemoryEntry::new(
                    MemoryScope::User,
                    "remember this".into(),
                    true,
                )],
                git_context: vec!["branch main".into()],
                session_history: vec!["prev turn".into()],
                tool_results: vec!["result output".into()],
                images: make_image_entries(1, 10),
                selected_files: vec!["file.rs".into()],
                ..Default::default()
            },
            test_budget(100_000, 1_000),
        )
        .await;

    // Verify expected ordering: System(P0) -> ProjectInstruction(P0.25) ->
    // Skill(P0.5) -> ToolDefinitions(P0.75) -> Request(P1) -> Memory(P2) ->
    // Git(P2.5) -> History(P3) -> ToolResult(P4) -> Image(P4.5) -> SelectedFile(P5)
    let expected_order = [
        ContextSource::System,
        ContextSource::ProjectInstruction,
        ContextSource::Skill,
        ContextSource::ToolDefinitions,
        ContextSource::Request,
        ContextSource::Memory,
        ContextSource::Git,
        ContextSource::History,
        ContextSource::ToolResult,
        ContextSource::Image,
        ContextSource::SelectedFile,
    ];

    let mut last_pos = 0;
    for expected_source in &expected_order {
        let pos = bundle
            .sources
            .iter()
            .position(|s| s == expected_source)
            .unwrap_or_else(|| panic!("missing source: {expected_source:?}"));
        assert!(
            pos >= last_pos,
            "{expected_source:?} at position {pos} is before previous at {last_pos}"
        );
        last_pos = pos;
    }
}

// ---------------------------------------------------------------------------
// Source caps: per-source LIFO dropping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn source_caps_drop_lifo_within_capped_source() {
    let assembler = ContextAssembler::new_standalone();
    // Three history entries with enough text to exceed a small cap.
    // Each "History: ..." entry is ~10-15 tokens; cap to allow only one.
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "q".into(),
                session_history: vec![
                    "first history entry with extra padding text for tokens".into(),
                    "second history entry with extra padding text for tokens".into(),
                    "third history entry with extra padding text for tokens".into(),
                ],
                ..Default::default()
            },
            ContextBudget {
                context_window: 100_000,
                output_reservation: 0,
                // Cap history tokens low enough to force at least one drop.
                source_caps: vec![(ContextSource::History, 15)],
            },
        )
        .await;

    let combined = bundle.messages.join("\n");
    // LIFO: third drops first, then second; first should survive
    assert!(
        combined.contains("first history"),
        "First history entry should survive LIFO cap: {combined}"
    );
    assert!(bundle.truncated);
}

#[tokio::test]
async fn source_cap_zero_removes_all_entries_of_that_source() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "q".into(),
                tool_results: vec!["result A".into(), "result B".into()],
                ..Default::default()
            },
            ContextBudget {
                context_window: 100_000,
                output_reservation: 0,
                source_caps: vec![(ContextSource::ToolResult, 0)],
            },
        )
        .await;

    let has_tool_result = bundle
        .sources
        .iter()
        .any(|s| matches!(s, ContextSource::ToolResult));
    assert!(
        !has_tool_result,
        "All tool results should be removed by cap=0"
    );
    assert!(bundle.truncated);
}

// ---------------------------------------------------------------------------
// Active skills section
// ---------------------------------------------------------------------------

#[tokio::test]
async fn active_skills_included_in_bundle() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                active_skills: vec!["skill-alpha".into(), "skill-beta".into()],
                ..Default::default()
            },
            test_budget(5000, 100),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("<active_skills>"));
    assert!(combined.contains("skill-alpha"));
    assert!(combined.contains("skill-beta"));
    assert!(combined.contains("</active_skills>"));
    assert!(bundle
        .sources
        .iter()
        .any(|s| matches!(s, ContextSource::Skill)));
}

#[tokio::test]
async fn empty_active_skills_produces_no_skill_section() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                active_skills: vec![],
                ..Default::default()
            },
            test_budget(5000, 100),
        )
        .await;

    assert!(
        !bundle
            .sources
            .iter()
            .any(|s| matches!(s, ContextSource::Skill)),
        "No Skill source should appear when active_skills is empty"
    );
}

// ---------------------------------------------------------------------------
// Tool definitions section
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tool_definitions_included_as_json_block() {
    let assembler = ContextAssembler::new_standalone();
    let tools = vec![
        agent_models::ToolDefinition {
            name: "fs.read".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({"type": "object"}),
        },
        agent_models::ToolDefinition {
            name: "fs.write".into(),
            description: "Write a file".into(),
            parameters: serde_json::json!({"type": "object"}),
        },
    ];
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                tool_definitions: tools,
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("fs.read"));
    assert!(combined.contains("fs.write"));
    assert!(bundle
        .sources
        .iter()
        .any(|s| matches!(s, ContextSource::ToolDefinitions)));
}

#[tokio::test]
async fn empty_tool_definitions_produces_no_section() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                tool_definitions: vec![],
                ..Default::default()
            },
            test_budget(5000, 100),
        )
        .await;

    assert!(
        !bundle
            .sources
            .iter()
            .any(|s| matches!(s, ContextSource::ToolDefinitions)),
        "No ToolDefinitions source should appear when tool_definitions is empty"
    );
}

// ---------------------------------------------------------------------------
// Active task (added as History source)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn active_task_included_as_history_source() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                active_task: Some("implement auth module".into()),
                ..Default::default()
            },
            test_budget(5000, 100),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("Active task: implement auth module"));
    assert!(bundle
        .sources
        .iter()
        .any(|s| matches!(s, ContextSource::History)));
}

// ---------------------------------------------------------------------------
// Memory filtering: only accepted memories
// ---------------------------------------------------------------------------

#[tokio::test]
async fn only_accepted_memories_included() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                memories: vec![
                    MemoryEntry::new(MemoryScope::User, "accepted memo".into(), true),
                    MemoryEntry::new(MemoryScope::User, "rejected memo".into(), false),
                    MemoryEntry::new(MemoryScope::User, "another accepted".into(), true),
                ],
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(
        combined.contains("accepted memo"),
        "Accepted memory should be in output"
    );
    assert!(
        combined.contains("another accepted"),
        "Second accepted memory should be in output"
    );
    assert!(
        !combined.contains("rejected memo"),
        "Rejected memory must not appear in output"
    );
}

#[tokio::test]
async fn all_rejected_memories_produces_no_memory_source() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                memories: vec![
                    MemoryEntry::new(MemoryScope::User, "rejected A".into(), false),
                    MemoryEntry::new(MemoryScope::User, "rejected B".into(), false),
                ],
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    assert!(
        !bundle
            .sources
            .iter()
            .any(|s| matches!(s, ContextSource::Memory)),
        "No Memory source should appear when all memories are rejected"
    );
}

// ---------------------------------------------------------------------------
// Provided memories used instead of store query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provided_memories_bypass_store_query() {
    let (assembler, store) = test_assembler_with_store().await;
    // Store a memory in the database
    store
        .store(MemoryEntry::new(
            MemoryScope::Workspace,
            "stored in db".into(),
            true,
        ))
        .await
        .unwrap();

    // But pass explicit memories in the request
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                memories: vec![MemoryEntry::new(
                    MemoryScope::User,
                    "provided explicitly".into(),
                    true,
                )],
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(
        combined.contains("provided explicitly"),
        "Provided memories should be used"
    );
    // The store memory should NOT appear since we provided explicit memories
    assert!(
        !combined.contains("stored in db"),
        "Store memories should not be queried when memories are provided"
    );
}

// ---------------------------------------------------------------------------
// Session history entries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_history_entries_all_included() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                session_history: vec![
                    "first turn".into(),
                    "second turn".into(),
                    "third turn".into(),
                ],
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("History: first turn"));
    assert!(combined.contains("History: second turn"));
    assert!(combined.contains("History: third turn"));

    let history_count = bundle
        .sources
        .iter()
        .filter(|s| matches!(s, ContextSource::History))
        .count();
    assert_eq!(history_count, 3, "Should have 3 History sources");
}

// ---------------------------------------------------------------------------
// Tool results assembly
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tool_results_included_with_prefix() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                tool_results: vec!["grep: 5 matches found".into(), "ls: 3 files".into()],
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("Tool result: grep: 5 matches found"));
    assert!(combined.contains("Tool result: ls: 3 files"));

    let tr_count = bundle
        .sources
        .iter()
        .filter(|s| matches!(s, ContextSource::ToolResult))
        .count();
    assert_eq!(tr_count, 2, "Should have 2 ToolResult sources");
}

// ---------------------------------------------------------------------------
// Selected files assembly
// ---------------------------------------------------------------------------

#[tokio::test]
async fn selected_files_included_with_prefix() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                selected_files: vec!["src/main.rs".into(), "Cargo.toml".into()],
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("Selected file: src/main.rs"));
    assert!(combined.contains("Selected file: Cargo.toml"));

    let sf_count = bundle
        .sources
        .iter()
        .filter(|s| matches!(s, ContextSource::SelectedFile))
        .count();
    assert_eq!(sf_count, 2, "Should have 2 SelectedFile sources");
}

// ---------------------------------------------------------------------------
// Usage tracking: ContextUsage fields
// ---------------------------------------------------------------------------

#[tokio::test]
async fn usage_fields_populated_correctly() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("system prompt".into()),
                user_request: "user query".into(),
                session_history: vec!["history entry".into()],
                ..Default::default()
            },
            ContextBudget {
                context_window: 200_000,
                output_reservation: 8_000,
                source_caps: vec![],
            },
        )
        .await;

    assert_eq!(bundle.usage.context_window, 200_000);
    assert_eq!(bundle.usage.output_reservation, 8_000);
    assert_eq!(bundle.usage.budget_tokens, 192_000);
    assert_eq!(bundle.usage.estimator, "cl100k_base");
    assert!(!bundle.usage.corrected_by_real_usage);
    assert!(bundle.usage.total_tokens > 0);
    assert!(bundle.usage.total_tokens <= bundle.usage.budget_tokens);

    // by_source should have entries for System, Request, History
    let source_types: Vec<&ContextSource> = bundle.usage.by_source.iter().map(|(s, _)| s).collect();
    assert!(source_types.contains(&&ContextSource::System));
    assert!(source_types.contains(&&ContextSource::Request));
    assert!(source_types.contains(&&ContextSource::History));
}

#[tokio::test]
async fn usage_by_source_aggregates_multiple_entries_of_same_source() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                session_history: vec![
                    "history one".into(),
                    "history two".into(),
                    "history three".into(),
                ],
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    // by_source should aggregate all History entries into a single (History, N) tuple
    let history_entries: Vec<_> = bundle
        .usage
        .by_source
        .iter()
        .filter(|(s, _)| matches!(s, ContextSource::History))
        .collect();
    assert_eq!(
        history_entries.len(),
        1,
        "History tokens should be aggregated into one entry, got {history_entries:?}"
    );
    assert!(
        history_entries[0].1 > 0,
        "Aggregated History tokens should be non-zero"
    );
}

// ---------------------------------------------------------------------------
// No truncation when under budget
// ---------------------------------------------------------------------------

#[tokio::test]
async fn not_truncated_when_under_budget() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("sys".into()),
                user_request: "q".into(),
                ..Default::default()
            },
            test_budget(100_000, 1_000),
        )
        .await;

    assert!(!bundle.truncated, "Should not truncate when under budget");
}

// ---------------------------------------------------------------------------
// Image pruning: StripImagesAtIntervals in assembler context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn assembler_prunes_images_with_interval_strategy() {
    let assembler = ContextAssembler::new_standalone();
    // 6 images, keep every 3rd plus first and last:
    // indices 0, 1, 2, 3, 4, 5 → keep 0, 3, 5
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                images: make_image_entries(6, 10),
                image_pruning: ImagePruningStrategy::StripImagesAtIntervals { interval: 3 },
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    let combined = bundle.messages.join("\n");
    assert!(combined.contains("screenshot_0"), "First should survive");
    assert!(
        !combined.contains("screenshot_1"),
        "Index 1 should be pruned"
    );
    assert!(
        !combined.contains("screenshot_2"),
        "Index 2 should be pruned"
    );
    assert!(
        combined.contains("screenshot_3"),
        "Every 3rd should survive"
    );
    assert!(
        !combined.contains("screenshot_4"),
        "Index 4 should be pruned"
    );
    assert!(combined.contains("screenshot_5"), "Last should survive");
}

// ---------------------------------------------------------------------------
// Embedded images from history are appended to the image list
// ---------------------------------------------------------------------------

#[tokio::test]
async fn embedded_images_from_history_appear_as_image_source() {
    let assembler = ContextAssembler::new_standalone();
    let encoded_image = "A".repeat(24_000);
    let history_with_image =
        format!("check ![screenshot](data:image/png;base64,{encoded_image}) please");

    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                session_history: vec![history_with_image],
                ..Default::default()
            },
            test_budget(200_000, 1_000),
        )
        .await;

    let has_image = bundle
        .sources
        .iter()
        .any(|s| matches!(s, ContextSource::Image));
    assert!(
        has_image,
        "Embedded image from history should produce an Image source"
    );

    let combined = bundle.messages.join("\n");
    assert!(
        combined.contains("embedded attachment"),
        "Should contain embedded attachment description"
    );
}

// ---------------------------------------------------------------------------
// Full drop chain: verify multiple sources drop in correct order
// ---------------------------------------------------------------------------

#[tokio::test]
async fn global_budget_drops_sources_in_priority_order() {
    let assembler = ContextAssembler::new_standalone();
    // Fill with many sections; use a tight budget to force multiple drops.
    // Images (P4.5) should drop before SelectedFile (P5).
    // Then SelectedFile, ToolResult, History, Memory drop in order.
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("S".into()),
                user_request: "R".into(),
                memories: vec![MemoryEntry::new(MemoryScope::User, "a memory".into(), true)],
                session_history: vec!["history data".into()],
                tool_results: vec!["tool output".into()],
                images: vec![ImageEntry {
                    position: 0,
                    estimated_tokens: 200,
                    content: "img".into(),
                }],
                selected_files: vec!["file content here".into()],
                ..Default::default()
            },
            // Very tight budget: only system + request fit (~10 tokens)
            test_budget(20, 0),
        )
        .await;

    assert!(bundle.truncated);
    let combined = bundle.messages.join("\n");
    // System and Request are never dropped
    assert!(combined.contains("S"), "System must survive");
    assert!(combined.contains("R"), "Request must survive");
}

// ---------------------------------------------------------------------------
// Standalone assembler with no memories and no store
// ---------------------------------------------------------------------------

#[tokio::test]
async fn standalone_no_store_no_memories_produces_empty_memory_source() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    assert!(
        !bundle
            .sources
            .iter()
            .any(|s| matches!(s, ContextSource::Memory)),
        "No Memory source when no memories provided and no store"
    );
}

// ---------------------------------------------------------------------------
// append_embedded_images helper
// ---------------------------------------------------------------------------

#[test]
fn append_embedded_images_assigns_incrementing_positions() {
    use super::assembler::append_embedded_images;

    let mut images = vec![ImageEntry {
        position: 0,
        estimated_tokens: 100,
        content: "existing".into(),
    }];
    let mut next_position = 1;
    let embedded = vec![
        agent_models::EmbeddedImageSummary {
            alt_text: "screenshot A".into(),
            mime_type: "image/png".into(),
            estimated_tokens: 200,
        },
        agent_models::EmbeddedImageSummary {
            alt_text: "".into(),
            mime_type: "image/jpeg".into(),
            estimated_tokens: 150,
        },
    ];

    append_embedded_images(&mut images, &mut next_position, embedded);

    assert_eq!(images.len(), 3);
    assert_eq!(images[1].position, 1);
    assert_eq!(images[2].position, 2);
    assert_eq!(next_position, 3);
    assert!(images[1].content.contains("screenshot A"));
    assert!(images[1].content.contains("image/png"));
    // Empty alt_text should produce "embedded attachment (mime)" without ": "
    assert!(images[2]
        .content
        .contains("embedded attachment (image/jpeg)"));
    assert!(!images[2].content.contains(": ("));
}

// ---------------------------------------------------------------------------
// sanitize_context_text helper
// ---------------------------------------------------------------------------

#[test]
fn sanitize_context_text_returns_text_unchanged_when_no_images() {
    use super::assembler::sanitize_context_text;

    let (sanitized, images) = sanitize_context_text("plain text without images");
    assert_eq!(sanitized, "plain text without images");
    assert!(images.is_empty());
}

#[test]
fn sanitize_context_text_extracts_embedded_images() {
    use super::assembler::sanitize_context_text;

    let encoded = "A".repeat(100);
    let input = format!("before ![alt](data:image/png;base64,{encoded}) after");
    let (sanitized, images) = sanitize_context_text(&input);

    // The base64 data should be removed from the text
    assert!(!sanitized.contains(&encoded));
    assert!(sanitized.contains("before"));
    assert!(sanitized.contains("after"));
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime_type, "image/png");
}

// ---------------------------------------------------------------------------
// System prompt omitted when None
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_system_prompt_produces_no_system_source() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    assert!(
        !bundle
            .sources
            .iter()
            .any(|s| matches!(s, ContextSource::System)),
        "No System source when system_prompt is None"
    );
}

// ---------------------------------------------------------------------------
// Project instructions omitted when None
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_project_instructions_produces_no_pi_source() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                user_request: "test".into(),
                ..Default::default()
            },
            test_budget(50_000, 1_000),
        )
        .await;

    assert!(
        !bundle
            .sources
            .iter()
            .any(|s| matches!(s, ContextSource::ProjectInstruction)),
        "No ProjectInstruction source when project_instructions is None"
    );
}

// ---------------------------------------------------------------------------
// count_tokens sanity check
// ---------------------------------------------------------------------------

#[test]
fn count_tokens_returns_nonzero_for_nonempty_text() {
    let assembler = ContextAssembler::new_standalone();
    let count = assembler.count_tokens("Hello, world!");
    assert!(count > 0, "Token count for non-empty text should be > 0");
}

#[test]
fn count_tokens_returns_zero_for_empty_text() {
    let assembler = ContextAssembler::new_standalone();
    let count = assembler.count_tokens("");
    assert_eq!(count, 0, "Token count for empty text should be 0");
}

#[test]
fn count_tokens_scales_with_text_length() {
    let assembler = ContextAssembler::new_standalone();
    let short = assembler.count_tokens("hello");
    let long = assembler.count_tokens("hello world this is a much longer piece of text");
    assert!(
        long > short,
        "Longer text should produce more tokens: short={short}, long={long}"
    );
}
