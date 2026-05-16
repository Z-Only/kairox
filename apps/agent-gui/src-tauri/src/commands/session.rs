use super::*;

#[tauri::command]
pub async fn switch_session(
    session_id: String,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let sid: agent_core::SessionId = session_id.into();
    switch_session_inner(&state, sid.clone(), &app_handle).await?;

    let projection = state
        .runtime
        .get_session_projection(sid)
        .await
        .map_err(|e| format!("Failed to get session projection: {e}"))?;

    Ok(serde_json::to_value(&projection).unwrap_or_default())
}

/// Returns historical trace events for a session as a JSON array.
/// Used by the frontend to repopulate the trace panel when switching sessions.
#[tauri::command]
pub async fn get_trace(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<String>, String> {
    let sid: agent_core::SessionId = session_id.into();
    let trace = state
        .runtime
        .get_trace(sid)
        .await
        .map_err(|e| format!("Failed to get trace: {e}"))?;
    Ok(trace
        .into_iter()
        .filter_map(|entry| serde_json::to_string(&entry.event).ok())
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn list_sessions(state: State<'_, GuiState>) -> Result<Vec<SessionInfoResponse>, String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };

    let sessions = state
        .runtime
        .list_sessions(&workspace_id)
        .await
        .map_err(|e| format!("Failed to list sessions: {e}"))?;

    let current_session_id = state.current_session_id.lock().await;

    let mut result: Vec<SessionInfoResponse> = sessions
        .into_iter()
        .map(SessionInfoResponse::from)
        .collect();

    // Sort: current session first
    if let Some(ref current_id) = *current_session_id {
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

#[tauri::command]
#[specta::specta]
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
#[specta::specta]
pub async fn query_memories(
    state: State<'_, GuiState>,
    scope: Option<String>,
    keywords: Option<Vec<String>>,
    limit: Option<u32>,
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
            limit: limit.unwrap_or(50) as usize,
            session_id: None,
            workspace_id: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(MemoryEntryResponse::from).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_memory(state: State<'_, GuiState>, id: String) -> Result<(), String> {
    state
        .memory_store
        .delete(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_workspaces(
    state: State<'_, GuiState>,
) -> Result<Vec<WorkspaceInfoResponse>, String> {
    let workspaces = state
        .runtime
        .list_workspaces()
        .await
        .map_err(|e| format!("Failed to list workspaces: {e}"))?;
    Ok(workspaces
        .into_iter()
        .map(|w| WorkspaceInfoResponse {
            workspace_id: w.workspace_id.to_string(),
            path: w.path,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn rename_session(
    session_id: String,
    title: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .rename_session(&sid, title)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_session(session_id: String, state: State<'_, GuiState>) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .soft_delete_session(&sid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn permanently_delete_session(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .permanently_delete_session(&sid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn restore_archived_session(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .restore_archived_session(&sid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_session(state: State<'_, GuiState>) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current = state.current_session_id.lock().await;
        current.clone().ok_or("No active session")?
    };
    state
        .runtime
        .cancel_session(workspace_id, session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_permission_mode(
    mode: String,
    state: State<'_, GuiState>,
) -> Result<String, String> {
    let session_id = {
        let current = state.current_session_id.lock().await;
        current
            .clone()
            .ok_or_else(|| "No active session".to_string())?
    };
    let permission_mode: agent_tools::PermissionMode = mode.parse().map_err(|e: String| e)?;
    state
        .runtime
        .set_session_permission_mode(&session_id, permission_mode)
        .await
        .map_err(|e| e.to_string())?;
    Ok(permission_mode.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_permission_mode(state: State<'_, GuiState>) -> Result<String, String> {
    // Try to read from current session's metadata first
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current = state.current_session_id.lock().await;
        current.clone().ok_or("No active session")?
    };
    let sessions = state
        .runtime
        .list_sessions(&workspace_id)
        .await
        .map_err(|e| format!("Failed to list sessions: {e}"))?;
    if let Some(session) = sessions.iter().find(|s| s.session_id == session_id) {
        if let Some(ref mode_str) = session.permission_mode {
            return Ok(mode_str.clone());
        }
    }
    // Fallback to engine's current mode
    Ok(state.runtime.permission_mode().await.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_profile_detail(
    profile: String,
    state: State<'_, GuiState>,
) -> Result<ProfileDetailResponse, String> {
    let info = state
        .config
        .read()
        .unwrap()
        .profile_info()
        .into_iter()
        .find(|p| p.alias == profile)
        .ok_or_else(|| format!("Profile '{}' not found", profile))?;
    Ok(ProfileDetailResponse {
        alias: info.alias,
        provider: info.provider,
        model_id: info.model_id,
        local: info.local,
        has_api_key: info.has_api_key,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn get_task_graph(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<TaskSnapshotResponse>, String> {
    let sid: agent_core::SessionId = session_id.into();
    let snapshot = state
        .runtime
        .get_task_graph(sid)
        .await
        .map_err(|e| format!("Failed to get task graph: {e}"))?;
    Ok(snapshot
        .tasks
        .into_iter()
        .map(|t| TaskSnapshotResponse {
            id: t.id.to_string(),
            title: t.title,
            role: format!("{:?}", t.role),
            state: format!("{:?}", t.state),
            dependencies: t.dependencies.iter().map(|d| d.to_string()).collect(),
            error: t.error,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn restore_workspace(
    workspace_id: String,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let wid: agent_core::WorkspaceId = workspace_id.into();
    {
        let mut ws = state.workspace_id.lock().await;
        *ws = Some(wid);
    }

    // Spawn event forwarder if not already running.
    // This is needed because restore_workspace is called on app restart
    // (via recoverSessions), which bypasses initialize_workspace where
    // the forwarder is normally started.
    {
        let handle = state.forwarder_handle.lock().await;
        if handle.is_none() {
            drop(handle); // Release lock before spawning
            let mut handle = state.forwarder_handle.lock().await;
            if handle.is_none() {
                *handle = Some(spawn_event_forwarder(&state.runtime, &app_handle));
            }
        }
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_build_info() -> BuildInfoResponse {
    let info = agent_core::build_info::BuildInfo::from_env();
    BuildInfoResponse {
        version: info.version.to_string(),
        git_hash: info.git_hash.to_string(),
        build_time: info.build_time.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Skill commands
// ---------------------------------------------------------------------------
#[tauri::command]
#[specta::specta]
pub async fn compact_session(state: State<'_, GuiState>) -> Result<(), String> {
    let session_id = {
        let current = state.current_session_id.lock().await;
        current
            .clone()
            .ok_or_else(|| "No active session to compact".to_string())?
    };

    state
        .runtime
        .compact_session(session_id, agent_core::CompactionReason::UserRequested)
        .await
        .map_err(|e| e.to_string())
}

/// P4: swap the active model profile for the current session.
///
/// The switch takes effect at the next `send_message` — in-flight
/// streams keep using the old profile end-to-end. Returns an error
/// when the alias is unknown or the session is currently compacting.
#[tauri::command]
#[specta::specta]
pub async fn switch_model(
    state: State<'_, GuiState>,
    session_id: String,
    profile_alias: String,
) -> Result<(), String> {
    let session_id = agent_core::SessionId::from_string(session_id);
    state
        .runtime
        .switch_model(session_id, profile_alias)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod compact_session_command_tests {
    use super::compact_session;

    #[test]
    fn compact_session_command_function_exists() {
        // Compile-time presence check — if `compact_session` is renamed or
        // removed this fails to compile, which is exactly the signal we want
        // before `collect_commands![]` / `generate_handler![]` blow up.
        let _ = compact_session;
    }
}
#[cfg(test)]
mod switch_model_command_tests {
    use super::switch_model;

    #[test]
    fn switch_model_command_function_exists() {
        // Compile-time presence check — if `switch_model` is renamed or
        // removed this fails to compile before `collect_commands!` /
        // `generate_handler!` get a chance to blow up at runtime.
        let _ = switch_model;
    }
}
