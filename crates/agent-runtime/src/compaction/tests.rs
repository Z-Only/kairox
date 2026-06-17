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
            display_content: None,
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

async fn fixture_session_with_n_pairs(n: usize) -> (Arc<SqliteEventStore>, WorkspaceId, SessionId) {
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
                display_content: None,
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
async fn compact_session_emits_history_too_short_skip_when_user_requested() {
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
    let skipped = events
        .iter()
        .find_map(|e| match &e.payload {
            EventPayload::ContextCompactionSkipped { reason, ratio } => Some((*reason, *ratio)),
            _ => None,
        })
        .expect("manual short-history compaction should emit skipped event");
    assert_eq!(
        skipped,
        (agent_core::CompactionSkipReason::NotEnoughHistory, 0.0)
    );
    assert!(!events
        .iter()
        .any(|e| matches!(&e.payload, EventPayload::ContextCompactionStarted { .. })));
    assert!(!events
        .iter()
        .any(|e| matches!(&e.payload, EventPayload::ContextCompactionCompleted { .. })));
    assert!(!events
        .iter()
        .any(|e| matches!(&e.payload, EventPayload::CompactionSummary { .. })));
}

#[tokio::test]
async fn compact_session_stays_silent_for_threshold_short_history() {
    let (store, ws, ses) = fixture_session_with_n_pairs(2).await; // < 3 pairs
    let model = StubSummariser::new("ignored", 0);
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
        CompactionReason::Threshold { ratio: 0.95 },
    )
    .await
    .expect("threshold short-history compaction should remain a silent no-op");

    let events = store.load_session(&ses).await.unwrap();
    assert!(!events.iter().any(|e| {
        matches!(
            &e.payload,
            EventPayload::ContextCompactionStarted { .. }
                | EventPayload::ContextCompactionSkipped { .. }
                | EventPayload::ContextCompactionCompleted { .. }
                | EventPayload::CompactionSummary { .. }
        )
    }));
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
