use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct MonitorInfoResponse {
    pub monitor_id: String,
    pub description: String,
    pub command: String,
    pub persistent: bool,
    pub timeout_ms: u32,
}

#[tauri::command]
#[specta::specta]
pub async fn list_monitors(state: State<'_, GuiState>) -> Result<Vec<MonitorInfoResponse>, String> {
    let registry = state
        .runtime
        .monitor_registry()
        .ok_or("Monitor registry not initialized")?;
    let monitors = registry.list().await;
    Ok(monitors
        .into_iter()
        .map(|m| MonitorInfoResponse {
            monitor_id: m.monitor_id,
            description: m.description,
            command: m.command,
            persistent: m.persistent,
            timeout_ms: m.timeout_ms as u32,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn stop_monitor(monitor_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let registry = state
        .runtime
        .monitor_registry()
        .ok_or("Monitor registry not initialized")?;
    registry.stop(&monitor_id).await.map_err(|e| e.to_string())
}
