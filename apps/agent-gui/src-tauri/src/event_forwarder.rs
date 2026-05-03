#![allow(dead_code)]
use agent_core::AppFacade;
use agent_models::ModelRouter;
use agent_store::SqliteEventStore;
use futures::StreamExt;
use tauri::AppHandle;
use tauri::Emitter;

/// Spawn a background task that forwards all DomainEvents from the runtime
/// to the Vue frontend via Tauri events.
/// Uses subscribe_all() to receive events for all sessions, not just one.
/// The frontend filters events by currentSessionId.
/// Returns the JoinHandle so the caller can abort it if needed.
pub fn spawn_event_forwarder(
    runtime: &agent_runtime::LocalRuntime<SqliteEventStore, ModelRouter>,
    app_handle: &AppHandle,
) -> tokio::task::JoinHandle<()> {
    let mut stream = runtime.subscribe_all();

    let app_handle = app_handle.clone();
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
