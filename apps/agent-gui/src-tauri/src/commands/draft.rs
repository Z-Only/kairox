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
    let (session_id, draft_text) = draft_save_args(&request);
    state
        .runtime
        .store()
        .save_draft(session_id, draft_text)
        .await
        .map_err(format_save_draft_error)
}

#[tauri::command]
#[specta::specta]
pub async fn get_draft(state: State<'_, GuiState>, session_id: String) -> Result<String, String> {
    state
        .runtime
        .store()
        .get_draft(&session_id)
        .await
        .map_err(format_get_draft_error)
}

fn draft_save_args(request: &SaveDraftRequest) -> (&str, &str) {
    (&request.session_id, &request.draft_text)
}

fn format_save_draft_error(error: impl std::fmt::Display) -> String {
    format!("Failed to save draft: {error}")
}

fn format_get_draft_error(error: impl std::fmt::Display) -> String {
    format!("Failed to get draft: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_draft_request_serializes_frontend_shape() {
        let request = SaveDraftRequest {
            session_id: "ses_123".into(),
            draft_text: "Continue from here".into(),
        };

        let json = serde_json::to_value(request).expect("request should serialize");

        assert_eq!(
            json,
            serde_json::json!({
                "session_id": "ses_123",
                "draft_text": "Continue from here",
            })
        );
    }

    #[test]
    fn draft_args_preserve_raw_session_id_and_text() {
        let request: SaveDraftRequest = serde_json::from_value(serde_json::json!({
            "session_id": "ses raw/../value",
            "draft_text": "",
        }))
        .expect("request should deserialize");

        let (session_id, draft_text) = draft_save_args(&request);

        assert_eq!(session_id, "ses raw/../value");
        assert_eq!(draft_text, "");
    }

    #[test]
    fn draft_errors_include_command_path_context() {
        assert_eq!(
            format_save_draft_error("store unavailable"),
            "Failed to save draft: store unavailable"
        );
        assert_eq!(
            format_get_draft_error("store unavailable"),
            "Failed to get draft: store unavailable"
        );
    }
}
