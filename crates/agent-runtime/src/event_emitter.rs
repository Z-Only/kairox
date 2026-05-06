//! Event emission helpers for the runtime.

use agent_core::DomainEvent;
use agent_store::EventStore;

/// Append an event to the store and broadcast it to all subscribers.
pub async fn append_and_broadcast<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    event: &DomainEvent,
) -> agent_core::Result<()> {
    store
        .append(event)
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
    let _ = event_tx.send(event.clone());
    Ok(())
}
