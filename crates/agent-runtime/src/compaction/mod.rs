//! Session-level compaction orchestrator. Owns the busy-gate transitions,
//! decides which event range becomes the compaction candidate, calls the
//! `agent-memory::Compactor`, and emits the four `EventPayload` variants
//! introduced in P2 (started / completed / failed / summary).

use crate::event_emitter::append_and_broadcast;
use crate::session::SessionState;
use agent_core::{
    AgentId, CompactionReason, CompactionSkipReason, CoreError, DomainEvent, EventPayload,
    PrivacyClassification, SessionId, WorkspaceId,
};
use agent_memory::{render_transcript, Compactor};
use agent_models::ModelClient;
use agent_store::EventStore;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

/// We always keep this many of the most recent user/assistant message
/// PAIRS in the live history (the rest become a compaction candidate).
/// Per spec §4.4 → K = 6 messages = 3 pairs.
pub const KEEP_LAST_PAIRS: usize = 3;

/// Pick the timestamp range `[first, last]` of events that should be
/// compacted, given that we want to keep the last `keep_pairs` pairs of
/// `UserMessageAdded` + `AssistantMessageCompleted` intact in the live
/// history.
///
/// Returns `None` when there are not strictly more than `keep_pairs`
/// completed pairs — there is nothing meaningful to compact yet.
///
/// Meta events (permissions, tool calls, etc.) ride along with the
/// message pairs based on timestamp: any meta event whose timestamp is
/// `< split_ts` is part of the compaction candidate.
pub fn pick_compaction_boundary(
    events: &[DomainEvent],
    keep_pairs: usize,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    // Collect indices of completed user/assistant pairs (user first, then
    // assistant). We do this by walking forward and matching consecutive
    // user → assistant on a single-state machine.
    let mut pair_end_indices: Vec<usize> = Vec::new();
    let mut last_user_idx: Option<usize> = None;
    for (i, e) in events.iter().enumerate() {
        match &e.payload {
            EventPayload::UserMessageAdded { .. } => {
                last_user_idx = Some(i);
            }
            EventPayload::AssistantMessageCompleted { .. } if last_user_idx.is_some() => {
                last_user_idx = None;
                // Index of the assistant message that closes a pair.
                pair_end_indices.push(i);
            }
            _ => {}
        }
    }

    if pair_end_indices.len() <= keep_pairs {
        return None;
    }

    // The split point is the FIRST user message of the last `keep_pairs`
    // pairs. Everything strictly before that timestamp becomes the
    // compaction candidate.
    let kept_first_pair_assistant_idx = pair_end_indices[pair_end_indices.len() - keep_pairs];

    // Find the matching user message (most recent UserMessageAdded
    // before the assistant index `kept_first_pair_assistant_idx`).
    let mut split_user_idx = None;
    for j in (0..kept_first_pair_assistant_idx).rev() {
        if matches!(events[j].payload, EventPayload::UserMessageAdded { .. }) {
            split_user_idx = Some(j);
            break;
        }
    }
    let split_user_idx = split_user_idx?;
    let split_ts = events[split_user_idx].timestamp;

    // Candidate range: every event with timestamp strictly less than
    // `split_ts`. Events are typically in chronological order in the
    // store, but we don't assume that — scan the whole slice.
    let candidates: Vec<&DomainEvent> = events.iter().filter(|e| e.timestamp < split_ts).collect();
    if candidates.is_empty() {
        return None;
    }
    let first_ts = candidates.iter().map(|e| e.timestamp).min()?;
    let last_ts = candidates.iter().map(|e| e.timestamp).max()?;
    if last_ts < first_ts {
        return None;
    }
    Some((first_ts, last_ts))
}

/// Drive a single compaction pass for `session_id`. The caller `.await`s
/// until the chain completes; the busy-gate is set on entry and cleared
/// on exit (both success and fallback paths).
///
/// Event sequence on success:
///   1. `ContextCompactionStarted`
///   2. `CompactionSummary { content: <llm summary> }`
///   3. `ContextCompactionCompleted { fallback_used: false }`
///
/// Event sequence on LLM failure:
///   1. `ContextCompactionStarted`
///   2. `ContextCompactionFailed { fallback_used: true, error }`
///   3. `CompactionSummary { content: "[Dropped N earlier turns by sliding window]" }`
///   4. `ContextCompactionCompleted { fallback_used: true }`
///
/// Returns `Ok(())` even when the LLM failed (the fallback ensures the
/// runtime always exits the busy state with a usable summary).
/// Returns `Ok(())` when there's not enough history to compact yet
/// (`< KEEP_LAST_PAIRS + 1` complete pairs). Manual requests emit
/// `ContextCompactionSkipped { NotEnoughHistory }`; automatic threshold
/// compaction remains silent for this steady-state path.
#[allow(clippy::too_many_arguments)]
pub async fn compact_session<S: EventStore>(
    store: &S,
    event_tx: &broadcast::Sender<DomainEvent>,
    model: &dyn ModelClient,
    profile_alias: &str,
    session_states: &Arc<Mutex<HashMap<String, SessionState>>>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    reason: CompactionReason,
) -> agent_core::Result<()> {
    // Acquire busy gate. If already compacting, treat as no-op (the caller
    // is responsible for not stacking compactions; `send_message`'s gate
    // returns SessionBusy upstream).
    {
        let mut states = session_states.lock().await;
        let entry = states
            .entry(session_id.to_string())
            .or_insert_with(SessionState::default);
        if entry.compacting {
            return Ok(());
        }
        entry.compacting = true;
    }

    let outcome = compact_inner(
        store,
        event_tx,
        model,
        profile_alias,
        workspace_id,
        session_id.clone(),
        reason,
    )
    .await;

    // Always clear the busy flag.
    {
        let mut states = session_states.lock().await;
        if let Some(entry) = states.get_mut(&session_id.to_string()) {
            entry.compacting = false;
        }
    }

    outcome
}

async fn compact_inner<S: EventStore>(
    store: &S,
    event_tx: &broadcast::Sender<DomainEvent>,
    model: &dyn ModelClient,
    profile_alias: &str,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    reason: CompactionReason,
) -> agent_core::Result<()> {
    let events = store
        .load_session(&session_id)
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))?;

    let Some((first_ts, last_ts)) = pick_compaction_boundary(&events, KEEP_LAST_PAIRS) else {
        if reason == CompactionReason::UserRequested {
            let skipped = DomainEvent::new(
                workspace_id,
                session_id,
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::ContextCompactionSkipped {
                    reason: CompactionSkipReason::NotEnoughHistory,
                    ratio: 0.0,
                },
            );
            append_and_broadcast(store, event_tx, &skipped).await?;
        }
        return Ok(());
    };

    let candidates: Vec<DomainEvent> = events
        .iter()
        .filter(|e| e.timestamp >= first_ts && e.timestamp <= last_ts)
        .cloned()
        .collect();
    let candidate_count = candidates.len();

    // Crude before-tokens estimate: char count / 4. The runtime layer
    // uses tiktoken in `ContextAssembler`, but here we only need a
    // diagnostic number for the event payload.
    let before_tokens = candidates
        .iter()
        .map(|e| match &e.payload {
            EventPayload::UserMessageAdded { content, .. }
            | EventPayload::AssistantMessageCompleted { content, .. } => (content.len() / 4) as u64,
            _ => 0,
        })
        .sum::<u64>();

    // 1) Emit ContextCompactionStarted.
    let started = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionStarted {
            reason,
            before_tokens,
            candidate_event_count: candidate_count,
        },
    );
    append_and_broadcast(store, event_tx, &started).await?;

    // 2) Render transcript and call the summariser.
    let transcript = render_transcript(&candidates);
    let llm_outcome = Compactor::compact_with_llm(model, profile_alias, &transcript).await;

    let (summary_text, fallback_used) = match llm_outcome {
        Ok(text) => (text, false),
        Err(err) => {
            // Emit ContextCompactionFailed before falling back.
            let failed = DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::ContextCompactionFailed {
                    error: err.to_string(),
                    fallback_used: true,
                },
            );
            append_and_broadcast(store, event_tx, &failed).await?;
            (Compactor::sliding_window_fallback(candidate_count), true)
        }
    };

    // 3) Emit CompactionSummary.
    let summary_id = format!("sum_{}", Uuid::new_v4().simple());
    let after_tokens = (summary_text.len() / 4) as u64;
    let summary = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::CompactionSummary {
            summary_id: summary_id.clone(),
            content: summary_text,
            replaces_event_range: (first_ts, last_ts),
            reason,
            before_tokens,
            after_tokens,
            summarised_by_profile: profile_alias.to_string(),
        },
    );
    append_and_broadcast(store, event_tx, &summary).await?;

    // 4) Emit ContextCompactionCompleted.
    let completed = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionCompleted {
            summary_id,
            after_tokens,
            fallback_used,
        },
    );
    append_and_broadcast(store, event_tx, &completed).await?;

    Ok(())
}

#[cfg(test)]
mod tests;
