//! Internal event dispatch helper for [`McpServerManager`].

use super::McpServerManager;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};

impl McpServerManager {
    /// Emit a domain event via the broadcast channel (best-effort).
    pub(super) fn emit_event(&self, payload: EventPayload) {
        if let Some(tx) = &self.event_tx {
            let event = DomainEvent::new(
                WorkspaceId::new(),
                SessionId::new(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                payload,
            );
            let _ = tx.send(event);
        }
    }
}
