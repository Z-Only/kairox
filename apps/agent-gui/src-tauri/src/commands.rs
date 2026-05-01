#![allow(dead_code)]
#![allow(clippy::new_without_default)]
use crate::app_state::{GuiState, WorkspaceSession};
use crate::event_forwarder::spawn_event_forwarder;
use agent_config::ProfileInfo;
use agent_core::projection::SessionProjection;
use agent_core::AppFacade;
use agent_core::PermissionDecision;
use agent_memory::{MemoryEntry, MemoryQuery, MemoryScope};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfoResponse {
    pub workspace_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfoResponse {
    pub id: String,
    pub title: String,
    pub profile: String,
}

#[tauri::command]
pub async fn list_profiles(state: State<'_, GuiState>) -> Result<Vec<String>, String> {
    Ok(state.config.profile_names())
}

#[tauri::command]
pub async fn get_profile_info(state: State<'_, GuiState>) -> Result<Vec<ProfileInfo>, String> {
    Ok(state.config.profile_info())
}

#[tauri::command]
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

    let workspace = state
        .runtime
        .open_workspace(workspace_path)
        .await
        .map_err(|e| format!("Failed to open workspace: {e}"))?;

    let workspace_id = workspace.workspace_id.clone();
    let profile = state.config.default_profile();

    let session_id = state
        .runtime
        .start_session(agent_core::StartSessionRequest {
            workspace_id: workspace_id.clone(),
            model_profile: profile.clone(),
        })
        .await
        .map_err(|e| format!("Failed to start session: {e}"))?;

    // Store workspace and session info
    {
        let mut ws = state.workspace_id.lock().await;
        *ws = Some(workspace_id.clone());
    }
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(
            session_id.to_string(),
            WorkspaceSession {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                profile: profile.clone(),
            },
        );
    }
    {
        let mut current = state.current_session_id.lock().await;
        *current = Some(session_id.clone());
    }

    // Spawn event forwarder for the initial session
    {
        let mut handle = state.forwarder_handle.lock().await;
        *handle = Some(spawn_event_forwarder(
            &state.runtime,
            session_id.clone(),
            app_handle,
        ));
    }

    Ok(WorkspaceInfoResponse {
        workspace_id: workspace_id.to_string(),
        path: workspace.path,
    })
}

#[tauri::command]
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
        })
        .await
        .map_err(|e| format!("Failed to start session: {e}"))?;

    let title = format!("Session using {profile}");

    // Register session
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(
            session_id.to_string(),
            WorkspaceSession {
                workspace_id,
                session_id: session_id.clone(),
                profile: profile.clone(),
            },
        );
    }

    // Switch to the new session
    switch_session_inner(&state, session_id.clone(), &app_handle).await?;

    Ok(SessionInfoResponse {
        id: session_id.to_string(),
        title,
        profile,
    })
}

#[tauri::command]
pub async fn send_message(
    content: String,
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

    // Spawn the message processing on a background task so that
    // the Tauri command returns immediately and streaming events
    // can flow to the frontend in real-time through the event forwarder.
    let session_id_str = session_id.to_string();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/kairox-debug.log")
    {
        use std::io::Write;
        let _ = writeln!(
            f,
            "[commands] send_message: session={} content_len={}",
            session_id_str,
            content.len()
        );
    }
    let runtime = state.runtime.clone();
    tokio::spawn(async move {
        let result = runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id,
                session_id,
                content,
            })
            .await;

        if let Err(e) = result {
            eprintln!("[commands] send_message background task failed: {e}");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/kairox-debug.log")
            {
                use std::io::Write;
                let _ = writeln!(f, "[commands] send_message FAILED: {e}");
            }
            // Emit an error event to the frontend so the UI can display
            // the error and reset the streaming state.
            let error_payload = serde_json::json!({
                "type": "AgentTaskFailed",
                "task_id": "",
                "error": e.to_string()
            });
            let event = serde_json::json!({
                "schema_version": 1,
                "session_id": session_id_str,
                "payload": error_payload
            });
            let _ = app_handle.emit("session-event", &event);
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn switch_session(
    session_id: String,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<SessionProjection, String> {
    let sid: agent_core::SessionId = session_id.into();
    switch_session_inner(&state, sid.clone(), &app_handle).await?;

    let projection = state
        .runtime
        .get_session_projection(sid)
        .await
        .map_err(|e| format!("Failed to get session projection: {e}"))?;

    Ok(projection)
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, GuiState>) -> Result<Vec<SessionInfoResponse>, String> {
    let sessions = state.sessions.lock().await;
    let current_session_id = state.current_session_id.lock().await;

    let mut result: Vec<SessionInfoResponse> = sessions
        .values()
        .map(|s| SessionInfoResponse {
            id: s.session_id.to_string(),
            title: format!("Session using {}", s.profile),
            profile: s.profile.clone(),
        })
        .collect();

    // Sort: current session first
    if let Some(current_id) = current_session_id.as_ref() {
        let current_str = current_id.to_string();
        result.sort_by(|a, b| {
            if a.id == current_str {
                std::cmp::Ordering::Less
            } else if b.id == current_str {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
    }

    Ok(result)
}

/// Inner helper: abort old forwarder, spawn new one, update current session.
async fn switch_session_inner(
    state: &GuiState,
    session_id: agent_core::SessionId,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    // Abort existing forwarder
    {
        let mut handle = state.forwarder_handle.lock().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    // Update current session
    {
        let mut current = state.current_session_id.lock().await;
        *current = Some(session_id.clone());
    }

    // Spawn new forwarder for the target session
    {
        let mut handle = state.forwarder_handle.lock().await;
        *handle = Some(spawn_event_forwarder(
            &state.runtime,
            session_id,
            app_handle.clone(),
        ));
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntryResponse {
    pub id: String,
    pub scope: String,
    pub key: Option<String>,
    pub content: String,
    pub accepted: bool,
}

impl From<MemoryEntry> for MemoryEntryResponse {
    fn from(e: MemoryEntry) -> Self {
        Self {
            id: e.id,
            scope: match e.scope {
                MemoryScope::User => "user".into(),
                MemoryScope::Workspace => "workspace".into(),
                MemoryScope::Session => "session".into(),
            },
            key: e.key,
            content: e.content,
            accepted: e.accepted,
        }
    }
}

#[tauri::command]
pub async fn resolve_permission(
    state: State<'_, GuiState>,
    request_id: String,
    decision: String,
    reason: Option<String>,
) -> Result<(), String> {
    let perm_decision = match decision.as_str() {
        "grant" => PermissionDecision {
            request_id: request_id.clone(),
            approve: true,
            reason: None,
        },
        "deny" => PermissionDecision {
            request_id: request_id.clone(),
            approve: false,
            reason: reason.or_else(|| Some("User denied".into())),
        },
        _ => return Err("Invalid decision: must be 'grant' or 'deny'".into()),
    };
    state
        .runtime
        .resolve_permission(&request_id, perm_decision)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_memories(
    state: State<'_, GuiState>,
    scope: Option<String>,
    keywords: Option<Vec<String>>,
    limit: Option<usize>,
) -> Result<Vec<MemoryEntryResponse>, String> {
    let scope = scope.map(|s| match s.as_str() {
        "user" => MemoryScope::User,
        "workspace" => MemoryScope::Workspace,
        _ => MemoryScope::Session,
    });
    let entries = state
        .memory_store
        .query(MemoryQuery {
            scope,
            keywords: keywords.unwrap_or_default(),
            limit: limit.unwrap_or(50),
            session_id: None,
            workspace_id: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(MemoryEntryResponse::from).collect())
}

#[tauri::command]
pub async fn delete_memory(state: State<'_, GuiState>, id: String) -> Result<(), String> {
    state
        .memory_store
        .delete(&id)
        .await
        .map_err(|e| e.to_string())
}
