use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SaveDraftRequest {
    pub session_id: String,
    pub draft_text: String,
}

#[tauri::command]
#[specta::specta]
pub async fn save_draft(
    state: State<'_, GuiState>,
    request: SaveDraftRequest,
) -> Result<(), String> {
    state
        .runtime
        .store()
        .save_draft(&request.session_id, &request.draft_text)
        .await
        .map_err(|e| format!("Failed to save draft: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn get_draft(state: State<'_, GuiState>, session_id: String) -> Result<String, String> {
    state
        .runtime
        .store()
        .get_draft(&session_id)
        .await
        .map_err(|e| format!("Failed to get draft: {e}"))
}
