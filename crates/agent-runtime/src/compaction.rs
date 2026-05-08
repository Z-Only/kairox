//! Session-level compaction orchestrator. Owns the busy-gate transitions,
//! decides which event range becomes the compaction candidate, calls the
//! `agent-memory::Compactor`, and emits the four `EventPayload` variants
//! introduced in P2 (started / completed / failed / summary).

use crate::event_emitter::append_and_broadcast;
use crate::session::SessionState;
use agent_core::{
    AgentId, CompactionReason, CoreError, DomainEvent, EventPayload, PrivacyClassification,
    SessionId, WorkspaceId,
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
/// Returns `Ok(())` (no events emitted) when there's not enough history
/// to compact yet (`< KEEP_LAST_PAIRS + 1` complete pairs).
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
        // Not enough history yet; nothing to compact.
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
mod tests {
    use super::*;
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };
    use chrono::Duration;

    fn make_event_at(payload: EventPayload, ts_offset_secs: i64) -> DomainEvent {
        let ts = chrono::Utc::now() + Duration::seconds(ts_offset_secs);
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
        .with_timestamp(ts)
    }

    fn user(i: usize, t: i64) -> DomainEvent {
        make_event_at(
            EventPayload::UserMessageAdded {
                message_id: format!("u{i}"),
                content: format!("u{i}"),
            },
            t,
        )
    }
    fn assistant(i: usize, t: i64) -> DomainEvent {
        make_event_at(
            EventPayload::AssistantMessageCompleted {
                message_id: format!("a{i}"),
                content: format!("a{i}"),
            },
            t,
        )
    }

    #[test]
    fn returns_none_when_not_enough_history() {
        // 2 pairs, asked to keep 3 pairs → nothing to compact.
        let events = vec![user(0, 0), assistant(0, 1), user(1, 2), assistant(1, 3)];
        assert!(pick_compaction_boundary(&events, 3).is_none());
    }

    #[test]
    fn boundary_excludes_kept_recent_pairs() {
        // 5 pairs, keep last 3 → first 2 pairs are candidates.
        let events: Vec<DomainEvent> = (0..5)
            .flat_map(|i| {
                let t = (i as i64) * 10;
                vec![user(i, t), assistant(i, t + 1)]
            })
            .collect();
        let (first, last) = pick_compaction_boundary(&events, 3).expect("boundary");
        // Candidates are pairs 0 and 1 (indices 0..=3).
        assert_eq!(first, events[0].timestamp); // user 0
        assert_eq!(last, events[3].timestamp); // assistant 1
    }

    use agent_models::{ModelClient, ModelEvent, ModelRequest};
    use agent_store::{EventStore, SqliteEventStore};
    use async_trait::async_trait;
    use futures::stream::{self, BoxStream, StreamExt};
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    /// Stub `ModelClient` that fails the first `fail_count` calls then
    /// streams a single `TokenDelta` → `Completed` sequence with `summary`.
    struct StubSummariser {
        summary: String,
        fail_count: Arc<StdMutex<u32>>,
    }

    impl StubSummariser {
        fn new(summary: &str, fails: u32) -> Self {
            Self {
                summary: summary.to_string(),
                fail_count: Arc::new(StdMutex::new(fails)),
            }
        }
    }

    #[async_trait]
    impl ModelClient for StubSummariser {
        async fn stream(
            &self,
            _req: ModelRequest,
        ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
            let mut left = self.fail_count.lock().unwrap();
            if *left > 0 {
                *left -= 1;
                return Err(agent_models::ModelError::Request("transient".into()));
            }
            let text = self.summary.clone();
            let events: Vec<agent_models::Result<ModelEvent>> = vec![
                Ok(ModelEvent::TokenDelta(text)),
                Ok(ModelEvent::Completed { usage: None }),
            ];
            Ok(stream::iter(events).boxed())
        }
    }

    async fn fixture_session_with_n_pairs(
        n: usize,
    ) -> (Arc<SqliteEventStore>, WorkspaceId, SessionId) {
        let store = Arc::new(SqliteEventStore::in_memory().await.unwrap());
        let ws = WorkspaceId::new();
        let ses = SessionId::new();
        let base = chrono::Utc::now();
        for i in 0..n {
            let u = DomainEvent::new(
                ws.clone(),
                ses.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::UserMessageAdded {
                    message_id: format!("u{i}"),
                    content: format!("user {i}"),
                },
            )
            .with_timestamp(base + Duration::seconds(i as i64 * 2));
            store.append(&u).await.unwrap();
            let a = DomainEvent::new(
                ws.clone(),
                ses.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::AssistantMessageCompleted {
                    message_id: format!("a{i}"),
                    content: format!("assistant {i}"),
                },
            )
            .with_timestamp(base + Duration::seconds(i as i64 * 2 + 1));
            store.append(&a).await.unwrap();
        }
        (store, ws, ses)
    }

    #[tokio::test]
    async fn compact_session_emits_started_summary_completed_in_order() {
        let (store, ws, ses) = fixture_session_with_n_pairs(8).await;
        let model = StubSummariser::new("## User goal\nfix tests\n", 0);
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        let states: Arc<tokio::sync::Mutex<HashMap<String, crate::session::SessionState>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        compact_session(
            &*store,
            &tx,
            &model,
            "fast",
            &states,
            ws,
            ses.clone(),
            CompactionReason::UserRequested,
        )
        .await
        .expect("compaction should succeed");

        let events = store.load_session(&ses).await.unwrap();
        let types: Vec<&str> = events.iter().map(|e| e.payload.event_type()).collect();
        let started = types
            .iter()
            .position(|t| *t == "ContextCompactionStarted")
            .expect("started");
        let summary = types
            .iter()
            .position(|t| *t == "CompactionSummary")
            .expect("summary");
        let completed = types
            .iter()
            .position(|t| *t == "ContextCompactionCompleted")
            .expect("completed");
        assert!(
            started < summary && summary < completed,
            "events out of order: {types:?}"
        );

        // After completion, compacting must be false.
        let states = states.lock().await;
        assert!(
            !states
                .get(&ses.to_string())
                .map(|s| s.compacting)
                .unwrap_or(true),
            "busy gate must be cleared after compaction"
        );
    }

    #[tokio::test]
    async fn compact_session_uses_sliding_window_fallback_after_llm_failures() {
        let (store, ws, ses) = fixture_session_with_n_pairs(8).await;
        let model = StubSummariser::new("ignored", 99); // always fails
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        let states: Arc<tokio::sync::Mutex<HashMap<String, crate::session::SessionState>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        compact_session(
            &*store,
            &tx,
            &model,
            "fast",
            &states,
            ws,
            ses.clone(),
            CompactionReason::UserRequested,
        )
        .await
        .expect("fallback should still complete the chain");

        let events = store.load_session(&ses).await.unwrap();
        let summary_evt = events
            .iter()
            .find_map(|e| match &e.payload {
                EventPayload::CompactionSummary { content, .. } => Some(content.clone()),
                _ => None,
            })
            .expect("must have summary even on LLM failure");
        assert!(
            summary_evt.contains("sliding window"),
            "expected fallback marker, got: {summary_evt}"
        );

        let failed = events.iter().any(|e| {
            matches!(
                &e.payload,
                EventPayload::ContextCompactionFailed {
                    fallback_used: true,
                    ..
                }
            )
        });
        assert!(
            failed,
            "expected ContextCompactionFailed (fallback_used=true)"
        );
    }

    #[tokio::test]
    async fn compact_session_returns_ok_when_history_too_short() {
        let (store, ws, ses) = fixture_session_with_n_pairs(2).await; // < 3 pairs
        let model = StubSummariser::new("ignored", 0);
        let (tx, _rx) = tokio::sync::broadcast::channel(64);
        let states: Arc<tokio::sync::Mutex<HashMap<String, crate::session::SessionState>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let result = compact_session(
            &*store,
            &tx,
            &model,
            "fast",
            &states,
            ws,
            ses.clone(),
            CompactionReason::UserRequested,
        )
        .await;
        assert!(result.is_ok());

        // No CompactionSummary should be appended.
        let events = store.load_session(&ses).await.unwrap();
        assert!(!events
            .iter()
            .any(|e| matches!(&e.payload, EventPayload::CompactionSummary { .. })));
    }

    #[test]
    fn meta_events_ride_along_with_pairs() {
        // Insert a permission event at t=5 (between pair 0 and pair 1).
        // It must be part of the candidate range because t=5 < split_ts (=20).
        let mut events: Vec<DomainEvent> = (0..5)
            .flat_map(|i| {
                let t = (i as i64) * 10;
                vec![user(i, t), assistant(i, t + 1)]
            })
            .collect();
        events.push(make_event_at(
            EventPayload::PermissionGranted {
                request_id: "p1".into(),
            },
            5,
        ));
        // Re-sort by timestamp to mimic event-store order.
        events.sort_by_key(|e| e.timestamp);

        let (_, last) = pick_compaction_boundary(&events, 3).expect("boundary");
        // The candidate range's max timestamp must be strictly less than
        // pair-2's user message timestamp (t=20).
        let pair2_user_ts = events
            .iter()
            .find(|e| {
                matches!(&e.payload, EventPayload::UserMessageAdded { content, .. } if content == "u2")
            })
            .unwrap()
            .timestamp;
        assert!(
            last < pair2_user_ts,
            "last={last:?} should be < {pair2_user_ts:?}"
        );
    }
}
