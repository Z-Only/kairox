//! Bridge from `agent_mcp::DomainEventSink` to the runtime's broadcast
//! channel. Lives here (not in `agent-mcp`) because `agent-mcp` cannot
//! depend on `agent-core::DomainEvent` plumbing.

use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_mcp::DomainEventSink;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

/// Forwards Phase 2 catalog source observability events into the runtime's
/// global `DomainEvent` broadcast.
#[allow(dead_code)] // Wired up in T11.S3 (build_catalog_provider).
pub(crate) struct CatalogEventSink {
    tx: Sender<DomainEvent>,
}

#[allow(dead_code)] // Wired up in T11.S3 (build_catalog_provider).
impl CatalogEventSink {
    pub fn new(tx: Sender<DomainEvent>) -> Arc<Self> {
        Arc::new(Self { tx })
    }

    fn build(payload: EventPayload) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            payload,
        )
    }
}

#[async_trait]
impl DomainEventSink for CatalogEventSink {
    async fn emit_source_failed(&self, source_id: &str, error: &str) {
        let _ = self.tx.send(Self::build(EventPayload::CatalogSourceFailed {
            source: source_id.to_string(),
            error: error.to_string(),
        }));
    }

    async fn emit_source_added(&self, source_id: &str) {
        let _ = self.tx.send(Self::build(EventPayload::CatalogSourceAdded {
            source: source_id.to_string(),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn sink_forwards_failed_event_to_broadcast() {
        let (tx, mut rx) = broadcast::channel(8);
        let sink = CatalogEventSink::new(tx);
        sink.emit_source_failed("mcp-registry", "timeout").await;
        let ev = rx.recv().await.unwrap();
        match ev.payload {
            EventPayload::CatalogSourceFailed { source, error } => {
                assert_eq!(source, "mcp-registry");
                assert_eq!(error, "timeout");
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[tokio::test]
    async fn sink_forwards_added_event_to_broadcast() {
        let (tx, mut rx) = broadcast::channel(8);
        let sink = CatalogEventSink::new(tx);
        sink.emit_source_added("internal").await;
        let ev = rx.recv().await.unwrap();
        assert!(matches!(
            ev.payload,
            EventPayload::CatalogSourceAdded { ref source } if source == "internal"
        ));
    }

    #[tokio::test]
    async fn sink_does_not_panic_when_no_subscribers() {
        let (tx, _) = broadcast::channel::<DomainEvent>(8);
        // Drop receiver immediately by dropping the channel's only rx.
        let sink = CatalogEventSink::new(tx);
        // Should be a silent no-op.
        sink.emit_source_failed("x", "y").await;
        sink.emit_source_added("x").await;
    }
}
