use agent_core::AppFacade;
use futures::StreamExt;
use tauri::AppHandle;
use tauri::Emitter;

/// Spawn a background task that forwards DomainEvents from the runtime
/// subscription to the Vue frontend via Tauri events.
/// Returns the JoinHandle so the caller can abort it on session switch.
pub fn spawn_event_forwarder(
    runtime: &agent_runtime::LocalRuntime<
        agent_store::SqliteEventStore,
        agent_models::FakeModelClient,
    >,
    session_id: agent_core::SessionId,
    app_handle: AppHandle,
) -> tokio::task::JoinHandle<()> {
    let mut stream = runtime.subscribe_session(session_id);

    tokio::spawn(async move {
        while let Some(event) = stream.next().await {
            match serde_json::to_value(&event) {
                Ok(payload) => {
                    let _ = app_handle.emit("session-event", &payload);
                }
                Err(e) => {
                    eprintln!("Failed to serialize DomainEvent: {e}");
                }
            }
        }
    })
}
