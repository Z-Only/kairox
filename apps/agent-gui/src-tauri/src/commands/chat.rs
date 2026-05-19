use super::*;

#[tauri::command]
#[specta::specta]
pub async fn initialize_workspace(
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<WorkspaceInfoResponse, String> {
    // Prevent double initialization
    {
        let ws = state.workspace_id.lock().await;
        if ws.is_some() {
            return Err("Workspace already initialized".into());
        }
    }

    let workspace_path = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {e}"))?
        .display()
        .to_string();

    // Try to reuse an existing workspace for this path
    let workspace = {
        let workspaces = state
            .runtime
            .list_workspaces()
            .await
            .map_err(|e| format!("Failed to list workspaces: {e}"))?;
        if let Some(existing) = workspaces.iter().find(|w| w.path == workspace_path) {
            existing.clone()
        } else {
            state
                .runtime
                .open_workspace(workspace_path)
                .await
                .map_err(|e| format!("Failed to open workspace: {e}"))?
        }
    };

    let workspace_id = workspace.workspace_id.clone();
    let profile = state.config.read().unwrap().default_profile();

    // Try to restore an existing session, or create a new one
    let session_id = {
        let sessions = state
            .runtime
            .list_sessions(&workspace_id)
            .await
            .map_err(|e| format!("Failed to list sessions: {e}"))?;
        if let Some(last) = sessions.last() {
            last.session_id.clone()
        } else {
            state
                .runtime
                .start_session(agent_core::StartSessionRequest {
                    workspace_id: workspace_id.clone(),
                    model_profile: profile.clone(),
                    permission_mode: None,
                })
                .await
                .map_err(|e| format!("Failed to start session: {e}"))?
        }
    };

    // Spawn event forwarder for all sessions
    {
        let mut handle = state.forwarder_handle.lock().await;
        *handle = Some(spawn_event_forwarder(&state.runtime, &app_handle));
    }

    // Store workspace and session info
    {
        let mut ws = state.workspace_id.lock().await;
        *ws = Some(workspace_id.clone());
    }
    {
        let mut current = state.current_session_id.lock().await;
        *current = Some(session_id.clone());
    }

    Ok(WorkspaceInfoResponse {
        workspace_id: workspace_id.to_string(),
        path: workspace.path,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn start_session(
    profile: String,
    permission_mode: Option<String>,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<SessionInfoResponse, String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };

    let session_id = state
        .runtime
        .start_session(agent_core::StartSessionRequest {
            workspace_id: workspace_id.clone(),
            model_profile: profile.clone(),
            permission_mode: permission_mode.clone(),
        })
        .await
        .map_err(|e| format!("Failed to start session: {e}"))?;

    let title = "New Session".to_string();

    // Switch to the new session (no forwarder respawn needed with subscribe_all)
    switch_session_inner(&state, session_id.clone(), &app_handle).await?;

    Ok(SessionInfoResponse {
        id: session_id.to_string(),
        title,
        profile,
        permission_mode: permission_mode.or(Some("suggest".into())),
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: None,
        deleted_at: None,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn send_message(
    content: String,
    attachments: Vec<agent_core::AttachmentInfo>,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current = state.current_session_id.lock().await;
        current.clone().ok_or("No active session")?
    };

    let enriched = enrich_content_with_attachments(&content, &attachments);

    let session_id_str = session_id.to_string();
    let runtime = state.runtime.clone();
    tokio::spawn(async move {
        let result = runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id,
                session_id,
                content: enriched,
                attachments,
            })
            .await;

        if let Err(e) = result {
            eprintln!("[commands] send_message failed: {e}");
            let payload = serde_json::json!({
                "type": "SendMessageError",
                "error": e.to_string(),
                "session_id": session_id_str
            });
            let _ = app_handle.emit("session-error", &payload);
        }
    });

    Ok(())
}

const MAX_TEXT_BYTES: u64 = 10 * 1024 * 1024; // 10 MB
const MAX_IMAGE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB

/// Read attachment files and format their content into the message.
///
/// - Images: base64-encoded data URIs appended to the content.
/// - Text files: content wrapped in markdown code blocks with filename headers.
/// - Other binaries: filename reference only.
fn enrich_content_with_attachments(
    content: &str,
    attachments: &[agent_core::AttachmentInfo],
) -> String {
    let mut parts: Vec<String> = Vec::new();

    for att in attachments {
        let mime = att.mime_type.as_str();
        if mime.starts_with("image/") {
            match std::fs::metadata(&att.path) {
                Ok(meta) if meta.len() > MAX_IMAGE_BYTES => {
                    parts.push(format!("[image: {} (file too large, >50MB)]", att.name));
                    continue;
                }
                Err(e) => {
                    eprintln!("[commands] failed to stat image {}: {e}", att.path);
                    parts.push(format!("[image: {} (read error)]", att.name));
                    continue;
                }
                _ => {}
            }
            match std::fs::read(&att.path) {
                Ok(bytes) => {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    parts.push(format!("![{}](data:{};base64,{})", att.name, mime, b64));
                }
                Err(e) => {
                    eprintln!("[commands] failed to read image {}: {e}", att.path);
                    parts.push(format!("[image: {} (read error)]", att.name));
                }
            }
        } else if is_text_mime(mime) {
            match std::fs::metadata(&att.path) {
                Ok(meta) if meta.len() > MAX_TEXT_BYTES => {
                    parts.push(format!("[file: {} (file too large, >10MB)]", att.name));
                    continue;
                }
                Err(e) => {
                    eprintln!("[commands] failed to stat file {}: {e}", att.path);
                    parts.push(format!("[file: {} (read error)]", att.name));
                    continue;
                }
                _ => {}
            }
            match std::fs::read_to_string(&att.path) {
                Ok(text) => {
                    let ext = std::path::Path::new(&att.name)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    parts.push(format!("```{}\n// file: {}\n{}\n```", ext, att.name, text));
                }
                Err(e) => {
                    eprintln!("[commands] failed to read file {}: {e}", att.path);
                    parts.push(format!("[file: {} (read error)]", att.name));
                }
            }
        } else {
            parts.push(format!("[attached file: {}]", att.name));
        }
    }

    if parts.is_empty() {
        content.to_string()
    } else if content.trim().is_empty() {
        parts.join("\n\n")
    } else {
        format!("{}\n\n{}", parts.join("\n\n"), content)
    }
}

fn is_text_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/xhtml+xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/toml"
                | "application/x-sh"
                | "application/x-shellscript"
        )
}

#[cfg(test)]
mod chat_attachment_tests {
    use super::*;

    fn attachment(
        path: &std::path::Path,
        name: &str,
        mime_type: &str,
    ) -> agent_core::AttachmentInfo {
        agent_core::AttachmentInfo {
            path: path.display().to_string(),
            name: name.to_string(),
            mime_type: mime_type.to_string(),
        }
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let unique = format!(
            "kairox-chat-attachment-{}-{nanos}-{name}",
            std::process::id(),
        );
        std::env::temp_dir().join(unique)
    }

    #[test]
    fn enriches_text_attachment_using_display_name_extension() {
        let path = temp_path("text-no-ext");
        std::fs::write(&path, "alpha\nbeta\n").unwrap();

        let enriched = enrich_content_with_attachments(
            "summarize this",
            &[attachment(&path, "report.md", "text/markdown")],
        );

        let _ = std::fs::remove_file(&path);
        assert_eq!(
            enriched,
            "```md\n// file: report.md\nalpha\nbeta\n\n```\n\nsummarize this"
        );
    }

    #[test]
    fn enriches_image_attachment_as_data_uri() {
        let path = temp_path("image.bin");
        std::fs::write(&path, [1_u8, 2, 3]).unwrap();

        let enriched =
            enrich_content_with_attachments("", &[attachment(&path, "pixel.png", "image/png")]);

        let _ = std::fs::remove_file(&path);
        assert_eq!(enriched, "![pixel.png](data:image/png;base64,AQID)");
    }

    #[test]
    fn enriches_binary_attachment_as_filename_reference() {
        let path = temp_path("archive.zip");

        let enriched = enrich_content_with_attachments(
            "inspect metadata",
            &[attachment(&path, "archive.zip", "application/zip")],
        );

        assert_eq!(enriched, "[attached file: archive.zip]\n\ninspect metadata");
    }
}
