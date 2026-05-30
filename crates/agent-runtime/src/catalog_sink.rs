//! Bridge from `agent_mcp::DomainEventSink` to the runtime's broadcast
//! channel. Lives here (not in `agent-mcp`) because `agent-mcp` cannot
//! depend on `agent-core::DomainEvent` plumbing.

use agent_core::facade::ServerEntry as CoreServerEntry;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_mcp::catalog::ServerEntry;
use agent_mcp::DomainEventSink;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

/// Forwards Phase 2 catalog source observability events into the runtime's
/// global `DomainEvent` broadcast.
pub(crate) struct CatalogEventSink {
    tx: Sender<DomainEvent>,
}

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

    async fn emit_source_results_arrived(&self, source_id: &str, entries: &[ServerEntry]) {
        let core_entries: Vec<CoreServerEntry> = entries.iter().map(map_entry_to_core).collect();
        let _ = self
            .tx
            .send(Self::build(EventPayload::CatalogSourceResultsArrived {
                source: source_id.to_string(),
                entries: core_entries,
            }));
    }
}

fn map_entry_to_core(e: &ServerEntry) -> CoreServerEntry {
    let install_spec_json = serde_json::to_string(&e.install).unwrap_or_else(|_| "{}".into());
    let requirements_json = serde_json::to_string(&e.requirements).unwrap_or_else(|_| "[]".into());
    let default_env_json = serde_json::to_string(&e.default_env).unwrap_or_else(|_| "[]".into());
    CoreServerEntry {
        id: e.id.clone(),
        source: e.source.clone(),
        display_name: e.display_name.clone(),
        summary: e.summary.clone(),
        description: e.description.clone(),
        categories: e.categories.clone(),
        tags: e.tags.clone(),
        author: e.author.clone(),
        homepage: e.homepage.clone(),
        version: e.version.clone(),
        trust: trust_to_str(e.trust).into(),
        verified: e.verified,
        icon: e.icon.clone(),
        install_spec_json,
        requirements_json,
        default_env_json,
    }
}

fn trust_to_str(t: agent_mcp::catalog::TrustLevel) -> &'static str {
    use agent_mcp::catalog::TrustLevel;
    match t {
        TrustLevel::Unverified => "unverified",
        TrustLevel::Community => "community",
        TrustLevel::Verified => "verified",
    }
}

#[cfg(test)]
#[path = "catalog_sink_tests.rs"]
mod tests;
