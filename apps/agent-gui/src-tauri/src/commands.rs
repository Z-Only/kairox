#![allow(dead_code)]
use crate::app_state::{GuiState, WorkspaceSession};
use crate::event_forwarder::spawn_event_forwarder;
use agent_config::ProfileInfo;
use agent_core::projection::SessionProjection;
use agent_core::AppFacade;
use serde::{Deserialize, Serialize};
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
pub async fn send_message(content: String, state: State<'_, GuiState>) -> Result<(), String> {
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
