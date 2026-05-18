pub(crate) mod catalog;
pub(crate) mod install;
pub(crate) mod skill_catalog;
pub(crate) mod sources;

use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_mcp::catalog::TrustLevel;

pub(crate) fn emit_marketplace_event(
    tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    payload: EventPayload,
) {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        payload,
    );
    let _ = tx.send(event);
}

pub(crate) fn parse_trust_str(s: &str) -> TrustLevel {
    match s {
        "verified" => TrustLevel::Verified,
        "unverified" => TrustLevel::Unverified,
        _ => TrustLevel::Community,
    }
}
