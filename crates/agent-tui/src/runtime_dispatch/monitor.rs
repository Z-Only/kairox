use std::sync::Arc;

use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app::App;
use crate::components::monitor_overlay::types::MonitorEntry;
use crate::components::{Command, CrossPanelEffect, MonitorOverlaySnapshot};

use super::{push_status_error, push_status_message};

pub(super) async fn dispatch(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    command: Command,
) {
    match command {
        Command::OpenMonitorOverlay => {
            let Some(registry) = runtime.monitor_registry() else {
                push_status_error(app, "[monitor] registry not initialized".into());
                return;
            };
            let monitors = registry.list().await;
            let entries: Vec<MonitorEntry> = monitors
                .into_iter()
                .map(|m| MonitorEntry {
                    monitor_id: m.monitor_id,
                    description: m.description,
                    command: m.command,
                    persistent: m.persistent,
                    timeout_ms: m.timeout_ms,
                })
                .collect();
            let snapshot = MonitorOverlaySnapshot { monitors: entries };
            app.dispatch_effects(vec![CrossPanelEffect::ShowMonitorOverlay(snapshot)]);
        }
        Command::MonitorList => {
            let Some(registry) = runtime.monitor_registry() else {
                push_status_error(app, "[monitor] registry not initialized".into());
                return;
            };
            let monitors = registry.list().await;
            if monitors.is_empty() {
                push_status_message(app, "[monitor] no active monitors".into());
            } else {
                let summary: Vec<String> = monitors
                    .iter()
                    .map(|m| format!("{}: {}", m.monitor_id, m.description))
                    .collect();
                push_status_message(
                    app,
                    format!(
                        "[monitor] {} active: {}",
                        monitors.len(),
                        summary.join(", ")
                    ),
                );
            }
        }
        Command::MonitorStop { monitor_id } => {
            let Some(registry) = runtime.monitor_registry() else {
                push_status_error(app, "[monitor] registry not initialized".into());
                return;
            };
            match registry.stop(&monitor_id).await {
                Ok(()) => {
                    push_status_message(app, format!("[monitor] stopped {monitor_id}"));
                }
                Err(err) => {
                    push_status_error(app, format!("[monitor] stop failed: {err}"));
                }
            }
            // Refresh the overlay if it's still visible after stop.
            if app.monitor_overlay.is_visible() {
                let monitors = registry.list().await;
                let entries: Vec<MonitorEntry> = monitors
                    .into_iter()
                    .map(|m| MonitorEntry {
                        monitor_id: m.monitor_id,
                        description: m.description,
                        command: m.command,
                        persistent: m.persistent,
                        timeout_ms: m.timeout_ms,
                    })
                    .collect();
                let snapshot = MonitorOverlaySnapshot { monitors: entries };
                app.dispatch_effects(vec![CrossPanelEffect::ShowMonitorOverlay(snapshot)]);
            }
        }
        _ => {}
    }
    app.state.render_scheduler.mark_dirty();
}
