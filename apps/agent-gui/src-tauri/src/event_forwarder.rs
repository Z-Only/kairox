#![allow(dead_code)]
use agent_core::AppFacade;
use agent_models::ModelRouter;
use agent_store::SqliteEventStore;
use futures::StreamExt;
use tauri::AppHandle;
use tauri::Emitter;

/// Spawn a background task that forwards DomainEvents from the runtime
/// subscription to the Vue frontend via Tauri events.
/// Returns the JoinHandle so the caller can abort it on session switch.
pub fn spawn_event_forwarder(
    runtime: &agent_runtime::LocalRuntime<SqliteEventStore, ModelRouter>,
    session_id: agent_core::SessionId,
    app_handle: AppHandle,
) -> tokio::task::JoinHandle<()> {
    let session_id_str = session_id.to_string();
    let mut stream = runtime.subscribe_session(session_id);

    tokio::spawn(async move {
        while let Some(event) = stream.next().await {
            let event_type = event.event_type.clone();
            let sess = event.session_id.to_string();
            match serde_json::to_value(&event) {
                Ok(payload) => {
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/tmp/kairox-debug.log")
                    {
                        use std::io::Write;
                        let _ = writeln!(
                            f,
                            "[forwarder] session={} type={} payload_len={}",
                            sess,
                            event_type,
                            payload.to_string().len()
                        );
                    }
                    let _ = app_handle.emit("session-event", &payload);
                }
                Err(e) => {
                    eprintln!("Failed to serialize DomainEvent: {e}");
                }
            }
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/kairox-debug.log")
        {
            use std::io::Write;
            let _ = writeln!(f, "[forwarder] stream ended for session {}", session_id_str);
        }
    })
}
