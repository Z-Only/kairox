use super::*;
use agent_runtime::ui_bootstrap::ensure_workspace_session;

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

    let profile = state.config.read().unwrap().default_profile();
    let bootstrap = ensure_workspace_session(state.runtime.as_ref(), workspace_path, profile)
        .await
        .map_err(|e| format!("Failed to initialize workspace: {e}"))?;

    let workspace_id = bootstrap.workspace.workspace_id.clone();
    let session_id = bootstrap.session_id.clone();

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
        path: bootstrap.workspace.path,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn start_session(
    profile: String,
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
            approval_policy: None,
            sandbox_policy: None,
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
        approval_policy: None,
        sandbox_policy: None,
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

    let runtime = state.runtime.clone();
    spawn_send_message_task(
        runtime,
        workspace_id,
        session_id,
        content,
        attachments,
        app_handle,
        false,
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn send_message_to_session(
    session_id: String,
    content: String,
    attachments: Vec<agent_core::AttachmentInfo>,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = SessionId::from_string(session_id);
    state
        .runtime
        .ensure_session_accepts_turn(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    let runtime = state.runtime.clone();
    spawn_send_message_task(
        runtime,
        workspace_id,
        session_id,
        content,
        attachments,
        app_handle,
        true,
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn send_message_to_session_and_wait(
    session_id: String,
    content: String,
    attachments: Vec<agent_core::AttachmentInfo>,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = SessionId::from_string(session_id);
    let runtime = state.runtime.clone();
    let request = build_send_message_request(
        runtime.as_ref(),
        workspace_id,
        session_id,
        content,
        attachments,
    )
    .await?;

    runtime
        .send_message_queued(request)
        .await
        .map_err(|e| e.to_string())
}

fn spawn_send_message_task(
    runtime: std::sync::Arc<
        agent_runtime::LocalRuntime<agent_store::SqliteEventStore, agent_models::ModelRouter>,
    >,
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
    content: String,
    attachments: Vec<agent_core::AttachmentInfo>,
    app_handle: tauri::AppHandle,
    strict: bool,
) {
    let session_id_str = session_id.to_string();
    tokio::spawn(async move {
        let request = match build_send_message_request(
            runtime.as_ref(),
            workspace_id,
            session_id,
            content,
            attachments,
        )
        .await
        {
            Ok(request) => request,
            Err(e) => {
                eprintln!("[commands] send_message preparation failed: {e}");
                let payload = serde_json::json!({
                    "type": "SendMessageError",
                    "error": e,
                    "session_id": session_id_str
                });
                let _ = app_handle.emit("session-error", &payload);
                return;
            }
        };
        let result = if strict {
            runtime.send_message_queued(request).await
        } else {
            runtime.send_message(request).await
        };

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
}

async fn build_send_message_request<S, M>(
    runtime: &agent_runtime::LocalRuntime<S, M>,
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
    content: String,
    attachments: Vec<agent_core::AttachmentInfo>,
) -> Result<agent_core::SendMessageRequest, String>
where
    S: agent_store::EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    let prepared = prepare_outbound_message(runtime, &workspace_id, &session_id, content).await?;
    let enriched = match tokio::task::spawn_blocking({
        let content = prepared.model_content.clone();
        let attachments = attachments.clone();
        move || enrich_content_with_attachments(&content, &attachments)
    })
    .await
    {
        Ok(enriched) => enriched,
        Err(e) => {
            eprintln!("[commands] attachment enrichment failed: {e}");
            prepared.model_content.clone()
        }
    };

    Ok(agent_core::SendMessageRequest {
        workspace_id,
        session_id,
        content: enriched,
        display_content: prepared.display_content,
        attachments,
    })
}

struct PreparedOutboundMessage {
    model_content: String,
    display_content: Option<String>,
}

async fn prepare_outbound_message<S, M>(
    runtime: &agent_runtime::LocalRuntime<S, M>,
    workspace_id: &agent_core::WorkspaceId,
    session_id: &agent_core::SessionId,
    content: String,
) -> Result<PreparedOutboundMessage, String>
where
    S: agent_store::EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    let (model_content, display_content) = prepare_goal_message(content.clone());
    if display_content.is_some() {
        return Ok(PreparedOutboundMessage {
            model_content,
            display_content,
        });
    }

    let Some(slash_skill) = parse_slash_skill_message(&content) else {
        return Ok(PreparedOutboundMessage {
            model_content,
            display_content: Some(content),
        });
    };

    let roots = runtime.skill_settings_roots_for_session(session_id).await;
    let skill = runtime
        .get_skill_with_roots(roots.clone(), slash_skill.skill_id.clone())
        .await
        .map_err(|error| error.to_string())?;
    if skill.is_none() {
        return Ok(PreparedOutboundMessage {
            model_content,
            display_content: Some(content),
        });
    }

    runtime
        .activate_skill_with_roots(
            roots,
            agent_core::ActivateSkillRequest {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                skill_id: slash_skill.skill_id,
            },
        )
        .await
        .map_err(|error| error.to_string())?;

    Ok(PreparedOutboundMessage {
        model_content: slash_skill.task,
        display_content: Some(content),
    })
}

struct SlashSkillMessage {
    skill_id: String,
    task: String,
}

fn parse_slash_skill_message(content: &str) -> Option<SlashSkillMessage> {
    let command = content.strip_prefix('/')?;
    let (skill_id, task) = command.split_once(char::is_whitespace)?;
    if skill_id.is_empty()
        || !skill_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        return None;
    }

    let task = task.trim();
    if task.is_empty() {
        return None;
    }

    Some(SlashSkillMessage {
        skill_id: skill_id.to_string(),
        task: task.to_string(),
    })
}

fn prepare_goal_message(content: String) -> (String, Option<String>) {
    let Some(goal) = content
        .strip_prefix("/goal ")
        .map(str::trim)
        .filter(|goal| !goal.is_empty())
    else {
        return (content, None);
    };

    let model_content = format!(
        "# Goal\n\n{goal}\n\nWork toward this goal until it is complete. Track progress, verify concrete changes, and report blockers explicitly."
    );
    (model_content, Some(content))
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

    fn write_test_skill(root: &std::path::Path, id: &str) {
        let skill_dir = root.join(id);
        std::fs::create_dir_all(&skill_dir).expect("skill directory should be created");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {id}\ndescription: Test skill\n---\nSkill body.\n"),
        )
        .expect("skill file should be written");
    }

    #[test]
    fn send_message_to_session_and_wait_command_is_compiled() {
        let _command = send_message_to_session_and_wait;
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

    #[test]
    fn marks_missing_text_attachment_as_read_error() {
        let path = temp_path("missing.md");
        let _ = std::fs::remove_file(&path);

        let enriched = enrich_content_with_attachments(
            "please inspect",
            &[attachment(&path, "missing.md", "text/markdown")],
        );

        assert_eq!(
            enriched,
            "[file: missing.md (read error)]\n\nplease inspect"
        );
    }

    #[test]
    fn marks_oversized_text_attachment_without_reading_content() {
        let path = temp_path("large.md");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_TEXT_BYTES + 1).unwrap();

        let enriched =
            enrich_content_with_attachments("", &[attachment(&path, "large.md", "text/markdown")]);

        let _ = std::fs::remove_file(&path);
        assert_eq!(enriched, "[file: large.md (file too large, >10MB)]");
    }

    #[test]
    fn marks_oversized_image_attachment_without_reading_content() {
        let path = temp_path("large.png");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();

        let enriched =
            enrich_content_with_attachments("", &[attachment(&path, "large.png", "image/png")]);

        let _ = std::fs::remove_file(&path);
        assert_eq!(enriched, "[image: large.png (file too large, >50MB)]");
    }

    #[test]
    fn goal_command_prepares_model_content_and_display_content() {
        let (model_content, display_content) =
            prepare_goal_message("/goal ship the release".to_string());

        assert!(model_content.contains("# Goal"));
        assert!(model_content.contains("ship the release"));
        assert!(model_content.contains("Track progress"));
        assert_eq!(display_content.as_deref(), Some("/goal ship the release"));
    }

    #[test]
    fn malformed_goal_command_is_left_unchanged() {
        let (model_content, display_content) = prepare_goal_message("/goal   ".to_string());

        assert_eq!(model_content, "/goal   ");
        assert_eq!(display_content, None);
    }

    #[test]
    fn slash_skill_message_parses_skill_id_and_task() {
        let parsed = parse_slash_skill_message("/kairox-dev-workflow   GUI project task").unwrap();

        assert_eq!(parsed.skill_id, "kairox-dev-workflow");
        assert_eq!(parsed.task, "GUI project task");
    }

    #[test]
    fn slash_skill_message_rejects_paths_and_missing_task() {
        assert!(parse_slash_skill_message("/Users/chanyu/project").is_none());
        assert!(parse_slash_skill_message("/kairox-dev-workflow   ").is_none());
    }

    #[tokio::test]
    async fn slash_skill_message_activates_skill_and_sends_task_body() {
        let workspace_root = tempfile::tempdir().expect("workspace root should be created");
        write_test_skill(
            &workspace_root.path().join(".agents/skills"),
            "kairox-dev-workflow",
        );
        let store = agent_store::SqliteEventStore::in_memory()
            .await
            .expect("store should be created");
        let runtime = agent_runtime::LocalRuntime::new(
            store,
            agent_models::FakeModelClient::new(vec!["ok".into()]),
        )
        .with_skill_settings_roots(agent_runtime::skill_settings::SkillSettingsRoots {
            workspace_root: Some(workspace_root.path().join(".kairox/skills")),
            user_root: None,
            builtin_root: None,
            plugin_roots: Vec::new(),
        });
        let workspace = runtime
            .open_workspace(workspace_root.path().display().to_string())
            .await
            .expect("workspace should open");
        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .expect("session should start");

        let prepared = prepare_outbound_message(
            &runtime,
            &workspace.workspace_id,
            &session_id,
            "/kairox-dev-workflow GUI项目对话创建后自动切换".to_string(),
        )
        .await
        .expect("message should prepare");

        assert_eq!(prepared.model_content, "GUI项目对话创建后自动切换");
        assert_eq!(
            prepared.display_content.as_deref(),
            Some("/kairox-dev-workflow GUI项目对话创建后自动切换")
        );

        let trace = runtime
            .get_trace(session_id)
            .await
            .expect("trace should load");
        assert!(trace.iter().any(|entry| matches!(
            &entry.event.payload,
            agent_core::EventPayload::SkillActivated { skill_id, .. }
                if skill_id == "kairox-dev-workflow"
        )));
    }

    #[tokio::test]
    async fn unknown_slash_message_is_sent_unchanged() {
        let workspace_root = tempfile::tempdir().expect("workspace root should be created");
        let store = agent_store::SqliteEventStore::in_memory()
            .await
            .expect("store should be created");
        let runtime = agent_runtime::LocalRuntime::new(
            store,
            agent_models::FakeModelClient::new(vec!["ok".into()]),
        )
        .with_skill_settings_roots(agent_runtime::skill_settings::SkillSettingsRoots {
            workspace_root: Some(workspace_root.path().join(".kairox/skills")),
            user_root: None,
            builtin_root: None,
            plugin_roots: Vec::new(),
        });
        let workspace = runtime
            .open_workspace(workspace_root.path().display().to_string())
            .await
            .expect("workspace should open");
        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .expect("session should start");

        let prepared = prepare_outbound_message(
            &runtime,
            &workspace.workspace_id,
            &session_id,
            "/not-a-skill keep this exact text".to_string(),
        )
        .await
        .expect("message should prepare");

        assert_eq!(prepared.model_content, "/not-a-skill keep this exact text");
        assert_eq!(
            prepared.display_content.as_deref(),
            Some("/not-a-skill keep this exact text")
        );
    }
}
