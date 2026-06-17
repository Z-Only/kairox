use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct MonitorInfoResponse {
    pub monitor_id: String,
    pub description: String,
    pub command: String,
    pub persistent: bool,
    pub timeout_ms: u32,
}

impl From<agent_tools::MonitorInfo> for MonitorInfoResponse {
    fn from(m: agent_tools::MonitorInfo) -> Self {
        Self {
            monitor_id: m.monitor_id,
            description: m.description,
            command: m.command,
            persistent: m.persistent,
            timeout_ms: m.timeout_ms as u32,
        }
    }
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
        .map(MonitorInfoResponse::from)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_info_response_maps_registry_fields() {
        let response = MonitorInfoResponse::from(agent_tools::MonitorInfo {
            monitor_id: "mon_42".into(),
            description: "Run local server".into(),
            command: "bun dev".into(),
            persistent: true,
            timeout_ms: 120_000,
        });

        assert_eq!(response.monitor_id, "mon_42");
        assert_eq!(response.description, "Run local server");
        assert_eq!(response.command, "bun dev");
        assert!(response.persistent);
        assert_eq!(response.timeout_ms, 120_000);
    }

    #[test]
    fn monitor_info_response_serializes_frontend_shape() {
        let response = MonitorInfoResponse {
            monitor_id: "mon_7".into(),
            description: "Watch tests".into(),
            command: "cargo test".into(),
            persistent: false,
            timeout_ms: 30_000,
        };

        let json = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(
            json,
            serde_json::json!({
                "monitor_id": "mon_7",
                "description": "Watch tests",
                "command": "cargo test",
                "persistent": false,
                "timeout_ms": 30_000,
            })
        );
    }
}
