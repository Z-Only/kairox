//! Session-level compaction orchestrator. Owns the busy-gate transitions,
//! decides which event range becomes the compaction candidate, calls the
//! `agent-memory::Compactor`, and emits the four `EventPayload` variants
//! introduced in P2 (started / completed / failed / summary).
//!
//! The orchestrator function `compact_session` is added in Task 7. This
//! file currently exposes only the boundary helper used by both the
//! orchestrator and its unit tests.

use agent_core::{DomainEvent, EventPayload};
use chrono::{DateTime, Utc};

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
        assert!(last < pair2_user_ts, "last={last:?} should be < {pair2_user_ts:?}");
    }
}
