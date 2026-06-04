use super::*;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_models::{ModelClient, ModelEvent, ModelRequest, ModelUsage};
use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};
use std::sync::{Arc, Mutex};

fn make_event(payload: EventPayload) -> DomainEvent {
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        payload,
    )
}

/// Stub `ModelClient` that fails the first `fail_count` calls then
/// streams a single `TokenDelta` → `Completed` sequence.
struct StubModel {
    fail_count: Arc<Mutex<u32>>,
    success_text: String,
}

struct EventModel {
    events: Vec<ModelEvent>,
}

#[async_trait]
impl ModelClient for EventModel {
    async fn stream(
        &self,
        _req: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let events = self.events.clone().into_iter().map(Ok);
        Ok(stream::iter(events).boxed())
    }
}

impl StubModel {
    fn new(fails: u32, text: &str) -> Self {
        Self {
            fail_count: Arc::new(Mutex::new(fails)),
            success_text: text.to_string(),
        }
    }
}

#[async_trait]
impl ModelClient for StubModel {
    async fn stream(
        &self,
        _req: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let mut left = self.fail_count.lock().unwrap();
        if *left > 0 {
            *left -= 1;
            return Err(agent_models::ModelError::Request("stub-failure".into()));
        }
        let text = self.success_text.clone();
        let events: Vec<agent_models::Result<ModelEvent>> = vec![
            Ok(ModelEvent::TokenDelta(text)),
            Ok(ModelEvent::Completed { usage: None }),
        ];
        Ok(stream::iter(events).boxed())
    }
}

#[test]
fn prompt_template_starts_with_known_preamble() {
    // Guard: the prompt is part of the model's contract; if someone edits
    // it, this test forces them to also update the test (and presumably
    // the integration tests that depend on output format).
    assert!(
        COMPACTOR_PROMPT.starts_with("You are summarising a developer-AI conversation"),
        "compactor prompt drifted: {}",
        COMPACTOR_PROMPT.lines().next().unwrap_or("")
    );
    assert!(COMPACTOR_PROMPT.contains("## User goal"));
    assert!(COMPACTOR_PROMPT.contains("## Key decisions & constraints"));
    assert!(COMPACTOR_PROMPT.contains("## Tool calls executed and their outcomes"));
    assert!(COMPACTOR_PROMPT.contains("## Open questions / pending work"));
}

#[test]
fn render_transcript_includes_user_assistant_and_tool_events() {
    let events = vec![
        make_event(EventPayload::UserMessageAdded {
            message_id: "u1".into(),
            content: "list rust files".into(),
            display_content: None,
        }),
        make_event(EventPayload::AssistantMessageCompleted {
            message_id: "a1".into(),
            content: "let me search".into(),
        }),
        make_event(EventPayload::ModelToolCallRequested {
            tool_call_id: "tc1".into(),
            tool_id: "search.ripgrep".into(),
        }),
        make_event(EventPayload::ToolInvocationCompleted {
            invocation_id: "tc1".into(),
            tool_id: "search.ripgrep".into(),
            output_preview: "found 5 matches".into(),
            exit_code: Some(0),
            duration_ms: 50,
            truncated: false,
        }),
    ];
    let out = render_transcript(&events);
    assert!(
        out.contains("### user\nlist rust files"),
        "missing user: {out}"
    );
    assert!(
        out.contains("### assistant\nlet me search"),
        "missing assistant: {out}"
    );
    assert!(
        out.contains("tool_call (search.ripgrep, id=tc1)"),
        "missing tool_call: {out}"
    );
    assert!(
        out.contains("tool_result (search.ripgrep)\nfound 5 matches"),
        "missing tool_result: {out}"
    );
}

#[test]
fn render_transcript_skips_meta_events() {
    let events = vec![
        make_event(EventPayload::UserMessageAdded {
            message_id: "u1".into(),
            content: "do thing".into(),
            display_content: None,
        }),
        make_event(EventPayload::PermissionGranted {
            request_id: "perm1".into(),
        }),
        make_event(EventPayload::AssistantMessageCompleted {
            message_id: "a1".into(),
            content: "done".into(),
        }),
    ];
    let out = render_transcript(&events);
    assert!(
        !out.contains("perm1"),
        "permission events leaked into transcript: {out}"
    );
    assert!(out.contains("### user"));
    assert!(out.contains("### assistant"));
}

#[tokio::test]
async fn compact_with_llm_returns_first_successful_summary() {
    let model = StubModel::new(0, "## User goal\nfix tests\n");
    let summary = Compactor::compact_with_llm(&model, "fast", "transcript")
        .await
        .expect("should succeed");
    assert!(summary.contains("## User goal"));
}

#[tokio::test]
async fn compact_with_llm_retries_then_succeeds() {
    let model = StubModel::new(2, "## User goal\nok\n");
    let summary = Compactor::compact_with_llm(&model, "fast", "transcript")
        .await
        .expect("should succeed after retries");
    assert!(summary.contains("ok"));
}

#[tokio::test]
async fn compact_with_llm_fails_after_max_retries() {
    let model = StubModel::new(99, "");
    let err = Compactor::compact_with_llm(&model, "fast", "transcript")
        .await
        .expect_err("should fail after 3 retries");
    match err {
        CompactorError::LlmFailed(_) => {}
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[tokio::test]
async fn compact_with_llm_rejects_empty_response() {
    let model = StubModel::new(0, "");
    let err = Compactor::compact_with_llm(&model, "fast", "transcript")
        .await
        .expect_err("empty summary should fail");
    assert!(matches!(err, CompactorError::Empty));
}

#[tokio::test]
async fn compact_with_llm_ignores_initial_usage_completion_before_tokens() {
    let model = EventModel {
        events: vec![
            ModelEvent::Completed {
                usage: Some(ModelUsage {
                    input_tokens: 512,
                    output_tokens: 9,
                    cache_creation_input_tokens: Some(256),
                    cache_read_input_tokens: None,
                }),
            },
            ModelEvent::TokenDelta("## User goal\nkeep the real summary\n".into()),
            ModelEvent::Completed {
                usage: Some(ModelUsage {
                    input_tokens: 512,
                    output_tokens: 16,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                }),
            },
        ],
    };

    let summary = Compactor::compact_with_llm(&model, "fast", "transcript")
        .await
        .expect("initial usage event should not end summary collection");

    assert!(summary.contains("keep the real summary"));
}

#[test]
fn sliding_window_fallback_includes_count_and_marker() {
    let s = Compactor::sliding_window_fallback(42);
    assert!(s.contains("42"), "expected count, got: {s}");
    assert!(s.contains("sliding window"), "expected marker, got: {s}");
}
