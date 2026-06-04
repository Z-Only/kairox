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
                out.push_str(&format!("### tool_call ({tool_id}, id={tool_call_id})\n\n"));
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
            server_tools: Vec::new(),
            reasoning_effort: None,
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
                Ok(ModelEvent::Completed { usage }) => {
                    if buf.trim().is_empty() && usage.is_some() {
                        // Anthropic `message_start.usage` is surfaced as a
                        // Completed event before text deltas arrive.
                        continue;
                    }
                    return Ok(buf);
                }
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
#[path = "compactor_tests.rs"]
mod tests;
