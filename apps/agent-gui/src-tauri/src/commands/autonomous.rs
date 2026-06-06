use super::*;
use agent_core::facade::{AutonomousTaskView, CheckpointView};
use agent_core::AutonomousTaskId;

#[tauri::command]
#[specta::specta]
pub async fn list_autonomous_tasks(
    state: State<'_, GuiState>,
) -> Result<Vec<AutonomousTaskView>, String> {
    let workspace_id = current_workspace_id(&state).await?;
    state
        .runtime
        .list_autonomous_tasks(workspace_id)
        .await
        .map_err(|e| format!("Failed to list autonomous tasks: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn get_autonomous_task(
    task_id: String,
    state: State<'_, GuiState>,
) -> Result<Option<AutonomousTaskView>, String> {
    let task_id: AutonomousTaskId = task_id.into();
    state
        .runtime
        .get_autonomous_task(task_id)
        .await
        .map_err(|e| format!("Failed to get autonomous task: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn get_autonomous_checkpoints(
    task_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<CheckpointView>, String> {
    let task_id: AutonomousTaskId = task_id.into();
    state
        .runtime
        .get_autonomous_checkpoints(task_id)
        .await
        .map_err(|e| format!("Failed to get autonomous checkpoints: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn pause_autonomous_task(
    task_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let task_id: AutonomousTaskId = task_id.into();
    state
        .runtime
        .pause_autonomous_task(task_id)
        .await
        .map_err(|e| format!("Failed to pause autonomous task: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn resume_autonomous_task(
    task_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let task_id: AutonomousTaskId = task_id.into();
    state
        .runtime
        .resume_autonomous_task(task_id)
        .await
        .map_err(|e| format!("Failed to resume autonomous task: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_autonomous_task(
    task_id: String,
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let workspace_id = current_workspace_id(&state).await?;
    let task_id: AutonomousTaskId = task_id.into();
    let session_id: agent_core::SessionId = session_id.into();
    state
        .runtime
        .cancel_autonomous_task(workspace_id, session_id, task_id)
        .await
        .map_err(|e| format!("Failed to cancel autonomous task: {e}"))
}
