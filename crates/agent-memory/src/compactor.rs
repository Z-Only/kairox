//! Session compaction: render an event range into a transcript and
//! call an LLM to summarise it (with a sliding-window fallback used
//! when the LLM call repeatedly fails).
//!
//! This module owns the prompt template + transcript renderer + LLM
//! retry loop. The runtime layer (`agent-runtime::compaction`) is
//! responsible for picking the event range, emitting the four
//! `EventPayload` variants, and applying the fallback when this
//! module returns [`CompactorError::LlmFailed`].

use agent_core::{DomainEvent, EventPayload};
use agent_models::{ModelClient, ModelEvent, ModelMessage, ModelRequest};
use futures::StreamExt;
use std::time::Duration;
use thiserror::Error;

/// Embedded summarisation prompt. Stable; do NOT inline the string —
/// `include_str!` keeps it editable as a separate file (and keeps the
/// `compactor_prompt.txt` content out of the Rust source diff noise).
pub const COMPACTOR_PROMPT: &str = include_str!("compactor_prompt.txt");

/// Number of LLM retry attempts (per spec §4.4 — the third failure
/// triggers the sliding-window fallback at the runtime layer).
pub const LLM_RETRY_ATTEMPTS: u32 = 3;

/// Initial backoff between LLM retries; doubles each attempt.
pub const LLM_RETRY_INITIAL_BACKOFF: Duration = Duration::from_millis(200);

/// Errors returned by [`Compactor::compact_with_llm`].
#[derive(Debug, Error)]
pub enum CompactorError {
    /// LLM call failed every retry attempt.
    #[error("compactor LLM failed: {0}")]
    LlmFailed(String),
    /// LLM returned an empty (whitespace-only) summary.
    #[error("compactor returned empty summary")]
    Empty,
}

/// Render a slice of events into a markdown transcript suitable for the
/// summariser LLM. Tool-call events are condensed into one-line summaries
/// (the full output preview lives separately and would blow the budget).
pub fn render_transcript(events: &[DomainEvent]) -> String {
    let mut out = String::with_capacity(events.len() * 64);
    for event in events {
        match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => {
                out.push_str("### user\n");
                out.push_str(content);
                out.push_str("\n\n");
            }
            EventPayload::AssistantMessageCompleted { content, .. } => {
                out.push_str("### assistant\n");
                out.push_str(content);
                out.push_str("\n\n");
            }
            EventPayload::ModelToolCallRequested {
                tool_id,
                tool_call_id,
                ..
            } => {
                out.push_str(&format!(
                    "### tool_call ({tool_id}, id={tool_call_id})\n\n"
                ));
            }
            EventPayload::ToolInvocationCompleted {
                tool_id,
                output_preview,
                ..
            } => {
                out.push_str(&format!(
                    "### tool_result ({tool_id})\n{output_preview}\n\n"
                ));
            }
            EventPayload::ToolInvocationFailed { tool_id, error, .. } => {
                out.push_str(&format!("### tool_failed ({tool_id})\n{error}\n\n"));
            }
            // Ignore meta events (permissions, task graph, etc.) — they
            // don't carry semantic conversation content.
            _ => {}
        }
    }
    out
}

pub struct Compactor;

impl Compactor {
    /// Call the configured model with [`COMPACTOR_PROMPT`] + `transcript`,
    /// retrying up to [`LLM_RETRY_ATTEMPTS`] times with exponential backoff.
    /// Returns the assembled summary text on success.
    pub async fn compact_with_llm(
        model: &dyn ModelClient,
        profile_alias: &str,
        transcript: &str,
    ) -> Result<String, CompactorError> {
        let messages = vec![ModelMessage {
            role: "user".into(),
            content: format!(
                "{COMPACTOR_PROMPT}\n\n--- BEGIN TRANSCRIPT ---\n{transcript}\n--- END TRANSCRIPT ---"
            ),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }];
        let request = ModelRequest {
            model_profile: profile_alias.to_string(),
            messages,
            system_prompt: None,
            tools: Vec::new(),
        };

        let mut backoff = LLM_RETRY_INITIAL_BACKOFF;
        let mut last_err: Option<String> = None;
        for attempt in 0..LLM_RETRY_ATTEMPTS {
            match Self::collect_summary(model, request.clone()).await {
                Ok(text) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        return Err(CompactorError::Empty);
                    }
                    return Ok(trimmed.to_string());
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt + 1 < LLM_RETRY_ATTEMPTS {
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                    }
                }
            }
        }
        Err(CompactorError::LlmFailed(
            last_err.unwrap_or_else(|| "unknown".into()),
        ))
    }

    async fn collect_summary(
        model: &dyn ModelClient,
        request: ModelRequest,
    ) -> Result<String, String> {
        let mut stream = model.stream(request).await.map_err(|e| e.to_string())?;
        let mut buf = String::new();
        while let Some(event) = stream.next().await {
            match event {
                Ok(ModelEvent::TokenDelta(delta)) => buf.push_str(&delta),
                Ok(ModelEvent::Completed { .. }) => return Ok(buf),
                Ok(ModelEvent::Failed { message }) => return Err(message),
                Ok(ModelEvent::ToolCallRequested { .. }) => {
                    // Summarisation prompts do not advertise tools; if a model
                    // ever returns one anyway, ignore it (don't pollute the
                    // summary buffer).
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(buf)
    }

    /// Fallback used when the LLM call exhausted its retries: produce a
    /// synthetic placeholder so the compaction event chain still completes
    /// (and the next agent loop iteration sees a smaller history).
    pub fn sliding_window_fallback(candidate_event_count: usize) -> String {
        format!("[Dropped {candidate_event_count} earlier turns by sliding window]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };
    use agent_models::{ModelClient, ModelEvent, ModelRequest};
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

    #[test]
    fn sliding_window_fallback_includes_count_and_marker() {
        let s = Compactor::sliding_window_fallback(42);
        assert!(s.contains("42"), "expected count, got: {s}");
        assert!(
            s.contains("sliding window"),
            "expected marker, got: {s}"
        );
    }
}
