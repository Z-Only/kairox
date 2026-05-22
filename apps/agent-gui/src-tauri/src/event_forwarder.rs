use agent_runtime::ui_bootstrap::{spawn_runtime_event_forwarder, UiRuntime};
use tauri::AppHandle;
use tauri::Emitter;

/// Spawn a background task that forwards all DomainEvents from the runtime
/// to the Vue frontend via Tauri events.
/// Uses subscribe_all() to receive events for all sessions, not just one.
/// The frontend filters events by currentSessionId.
/// Returns the JoinHandle so the caller can abort it if needed.
pub fn spawn_event_forwarder(
    runtime: &UiRuntime,
    app_handle: &AppHandle,
) -> tokio::task::JoinHandle<()> {
    let app_handle = app_handle.clone();
    spawn_runtime_event_forwarder(runtime, move |event| {
        let app_handle = app_handle.clone();
        async move {
            match serde_json::to_value(&event) {
                Ok(payload) => {
                    let _ = app_handle.emit("session-event", &payload);
                }
                Err(e) => {
                    eprintln!("Failed to serialize DomainEvent: {e}");
                }
            }
            true
        }
    })
}
