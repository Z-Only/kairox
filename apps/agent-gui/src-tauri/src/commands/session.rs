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

/// Returns a structured trace export envelope for diagnostics and replay tools.
#[tauri::command]
#[specta::specta]
pub async fn export_trace(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<TraceExport, String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .export_trace(sid)
        .await
        .map_err(|e| format!("Failed to export trace: {e}"))
}

/// Returns compact trace diagnostics for eval and pilot assertions.
#[tauri::command]
#[specta::specta]
pub async fn export_session_diagnostics(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<SessionDiagnosticsResponse, String> {
    let sid: agent_core::SessionId = session_id.into();
    let trace = state
        .runtime
        .export_trace(sid)
        .await
        .map_err(|e| format!("Failed to export session diagnostics: {e}"))?;
    let mut summary = summarize_trace_export(&trace);
    attach_event_db_metadata(&mut summary, &state.home_dir);
    Ok(summary)
}

fn summarize_trace_export(trace: &TraceExport) -> SessionDiagnosticsResponse {
    let mut event_type_counts = std::collections::BTreeMap::<String, u32>::new();
    let mut user_messages = Vec::new();
    let mut assistant_messages = Vec::new();
    let mut model_stream_statuses =
        std::collections::VecDeque::<ModelStreamStatusDiagnosticsResponse>::new();
    let mut model_tool_calls = Vec::new();
    let mut mcp_tool_calls = Vec::new();
    let mut trajectory_started_count = 0_u32;
    let mut trajectory_completed_count = 0_u32;
    let mut trajectory_completed_outcomes = Vec::new();
    let mut running_model_requests = 0_u32;
    let mut running_tool_invocations = 0_u32;
    let mut trajectory_failed_count = 0_u32;
    let mut has_terminal_assistant_message = false;
    let mut permission_request_tool_ids = std::collections::BTreeMap::<String, String>::new();
    let mut permission_denied_tool_counts = std::collections::BTreeMap::<String, u32>::new();
    let mut model_usage = ModelUsageDiagnosticsResponse::default();
    let mut model_usage_by_profile =
        std::collections::BTreeMap::<String, ModelUsageByProfileDiagnosticsResponse>::new();

    for event in &trace.events {
        let count = event_type_counts
            .entry(event.event_type.clone())
            .or_insert(0);
        *count = count.saturating_add(1);

        match &event.payload {
            agent_core::EventPayload::UserMessageAdded {
                message_id,
                content,
                ..
            } => user_messages.push(SessionDiagnosticsMessageResponse {
                message_id: message_id.clone(),
                content: content.clone(),
            }),
            agent_core::EventPayload::ModelRequestStarted { .. } => {
                running_model_requests = running_model_requests.saturating_add(1);
            }
            agent_core::EventPayload::ModelUsageRecorded {
                model_profile,
                input_tokens,
                output_tokens,
                cache_creation_input_tokens,
                cache_read_input_tokens,
            } => {
                let cache_creation = cache_creation_input_tokens.unwrap_or(0);
                let cache_read = cache_read_input_tokens.unwrap_or(0);
                model_usage.request_count = model_usage.request_count.saturating_add(1);
                saturating_add_tokens(&mut model_usage.total_input_tokens, *input_tokens);
                saturating_add_tokens(&mut model_usage.total_output_tokens, *output_tokens);
                saturating_add_tokens(
                    &mut model_usage.total_cache_creation_input_tokens,
                    cache_creation,
                );
                saturating_add_tokens(&mut model_usage.total_cache_read_input_tokens, cache_read);

                let profile_usage = model_usage_by_profile
                    .entry(model_profile.clone())
                    .or_insert_with(|| ModelUsageByProfileDiagnosticsResponse {
                        model_profile: model_profile.clone(),
                        request_count: 0,
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    });
                profile_usage.request_count = profile_usage.request_count.saturating_add(1);
                saturating_add_tokens(&mut profile_usage.input_tokens, *input_tokens);
                saturating_add_tokens(&mut profile_usage.output_tokens, *output_tokens);
                saturating_add_tokens(
                    &mut profile_usage.cache_creation_input_tokens,
                    cache_creation,
                );
                saturating_add_tokens(&mut profile_usage.cache_read_input_tokens, cache_read);
            }
            agent_core::EventPayload::ModelStreamStatus {
                phase,
                retrying,
                retry_attempt,
                max_retries,
                message,
            } => {
                const RECENT_MODEL_STREAM_STATUS_LIMIT: usize = 5;
                model_stream_statuses.push_back(ModelStreamStatusDiagnosticsResponse {
                    phase: phase.clone(),
                    retrying: *retrying,
                    retry_attempt: *retry_attempt,
                    max_retries: *max_retries,
                    message: message.clone(),
                });
                while model_stream_statuses.len() > RECENT_MODEL_STREAM_STATUS_LIMIT {
                    model_stream_statuses.pop_front();
                }
            }
            agent_core::EventPayload::AssistantMessageCompleted {
                message_id,
                content,
            } => {
                running_model_requests = running_model_requests.saturating_sub(1);
                has_terminal_assistant_message = true;
                assistant_messages.push(SessionDiagnosticsMessageResponse {
                    message_id: message_id.clone(),
                    content: content.clone(),
                });
            }
            agent_core::EventPayload::ModelToolCallRequested {
                tool_call_id,
                tool_id,
            } => {
                permission_request_tool_ids.insert(tool_call_id.clone(), tool_id.clone());
                model_tool_calls.push(ModelToolCallDiagnosticsResponse {
                    tool_call_id: tool_call_id.clone(),
                    tool_id: tool_id.clone(),
                });
            }
            agent_core::EventPayload::PermissionRequested {
                request_id,
                tool_id,
                ..
            } => {
                permission_request_tool_ids.insert(request_id.clone(), tool_id.clone());
            }
            agent_core::EventPayload::PermissionDenied { request_id, .. } => {
                if let Some(tool_id) = permission_request_tool_ids.get(request_id) {
                    let count = permission_denied_tool_counts
                        .entry(tool_id.clone())
                        .or_insert(0);
                    *count = count.saturating_add(1);
                }
            }
            agent_core::EventPayload::ToolInvocationStarted { .. } => {
                running_tool_invocations = running_tool_invocations.saturating_add(1);
            }
            agent_core::EventPayload::ToolInvocationCompleted { .. }
            | agent_core::EventPayload::ToolInvocationFailed { .. } => {
                running_tool_invocations = running_tool_invocations.saturating_sub(1);
            }
            agent_core::EventPayload::McpToolCallStarted {
                server_id,
                tool_name,
            } => mcp_tool_calls.push(McpToolCallDiagnosticsResponse {
                server_id: server_id.clone(),
                tool_name: tool_name.clone(),
                status: "started".into(),
            }),
            agent_core::EventPayload::McpToolCallCompleted {
                server_id,
                tool_name,
                ..
            } => mcp_tool_calls.push(McpToolCallDiagnosticsResponse {
                server_id: server_id.clone(),
                tool_name: tool_name.clone(),
                status: "completed".into(),
            }),
            agent_core::EventPayload::TrajectoryStarted { .. } => {
                trajectory_started_count = trajectory_started_count.saturating_add(1);
            }
            agent_core::EventPayload::TrajectoryCompleted {
                trajectory_id,
                step_count,
                outcome,
            } => {
                trajectory_completed_count = trajectory_completed_count.saturating_add(1);
                if matches!(outcome, agent_core::TrajectoryOutcome::Failed) {
                    trajectory_failed_count = trajectory_failed_count.saturating_add(1);
                }
                trajectory_completed_outcomes.push(TrajectoryCompletedDiagnosticsResponse {
                    trajectory_id: trajectory_id.clone(),
                    step_count: *step_count,
                    outcome: trajectory_outcome_to_string(outcome).into(),
                });
            }
            _ => {}
        }
    }

    model_usage.by_profile = model_usage_by_profile.into_values().collect();

    SessionDiagnosticsResponse {
        session_id: trace.session_id.to_string(),
        event_count: trace.event_count as u32,
        event_type_counts: event_type_counts
            .into_iter()
            .map(|(event_type, count)| EventTypeCountResponse { event_type, count })
            .collect(),
        last_event_type: trace.events.last().map(|event| event.event_type.clone()),
        event_db_path: None,
        event_db_path_source: None,
        user_messages,
        assistant_messages,
        model_tool_calls,
        mcp_tool_calls,
        permission_denied_tools: permission_denied_tool_counts
            .into_iter()
            .map(|(tool_id, count)| PermissionDeniedToolDiagnosticsResponse { tool_id, count })
            .collect(),
        trajectory_started_count,
        trajectory_completed_count,
        trajectory_completed_outcomes,
        running_model_requests,
        running_tool_invocations,
        trajectory_failed_count,
        has_terminal_assistant_message,
        recent_model_stream_statuses: model_stream_statuses.into_iter().collect(),
        model_usage,
    }
}

fn saturating_add_tokens(total: &mut u32, amount: u64) {
    *total = (*total).saturating_add(u32::try_from(amount).unwrap_or(u32::MAX));
}

fn attach_event_db_metadata(summary: &mut SessionDiagnosticsResponse, data_dir: &std::path::Path) {
    summary.event_db_path = Some(
        data_dir
            .join("kairox-gui.sqlite")
            .to_string_lossy()
            .into_owned(),
    );
    summary.event_db_path_source = Some("tauri_state".to_string());
}

fn trajectory_outcome_to_string(outcome: &agent_core::TrajectoryOutcome) -> &'static str {
    match outcome {
        agent_core::TrajectoryOutcome::Success => "success",
        agent_core::TrajectoryOutcome::Failed => "failed",
        agent_core::TrajectoryOutcome::Cancelled => "cancelled",
        agent_core::TrajectoryOutcome::InProgress => "in_progress",
    }
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
pub async fn resolve_task_confirmation(
    state: State<'_, GuiState>,
    decision: TaskConfirmationDecision,
) -> Result<(), String> {
    state
        .runtime
        .resolve_task_confirmation(decision)
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
        .query_including_pending(MemoryQuery {
            scope,
            keywords: keywords.unwrap_or_default(),
            limit: limit.unwrap_or(50) as usize,
            session_id: None,
            workspace_id: None,
            branch: None,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(MemoryEntryResponse::from).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn accept_memory(state: State<'_, GuiState>, id: String) -> Result<(), String> {
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
        .accept_memory(&id, workspace_id, session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn reject_memory(state: State<'_, GuiState>, id: String) -> Result<(), String> {
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
        .reject_memory(&id, workspace_id, session_id, "Rejected by user".into())
        .await
        .map_err(|e| e.to_string())
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
pub async fn set_session_approval_policy(
    approval: String,
    state: State<'_, GuiState>,
) -> Result<String, String> {
    let session_id = {
        let current = state.current_session_id.lock().await;
        current
            .clone()
            .ok_or_else(|| "No active session".to_string())?
    };
    let approval_policy: agent_tools::ApprovalPolicy = approval.parse().map_err(|e: String| e)?;
    state
        .runtime
        .set_session_approval_policy(&session_id, approval_policy)
        .await
        .map_err(|e| e.to_string())?;
    Ok(approval_policy.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_session_sandbox_policy(
    sandbox_json: String,
    state: State<'_, GuiState>,
) -> Result<String, String> {
    let session_id = {
        let current = state.current_session_id.lock().await;
        current
            .clone()
            .ok_or_else(|| "No active session".to_string())?
    };
    let sandbox: agent_tools::SandboxPolicy = serde_json::from_str(&sandbox_json)
        .map_err(|e| format!("invalid sandbox policy JSON: {e}"))?;
    state
        .runtime
        .set_session_sandbox_policy(&session_id, &sandbox)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&sandbox).map_err(|e| e.to_string())
}

fn session_policy_value<'a>(
    session_id: &SessionId,
    ordinary_sessions: &'a [SessionMeta],
    project_sessions: &'a [SessionMeta],
    select_policy: impl Fn(&'a SessionMeta) -> Option<&'a str>,
) -> Option<String> {
    ordinary_sessions
        .iter()
        .chain(project_sessions.iter())
        .find(|session| session.session_id.as_str() == session_id.as_str())
        .and_then(select_policy)
        .map(str::to_string)
}

#[tauri::command]
#[specta::specta]
pub async fn get_session_approval_policy(state: State<'_, GuiState>) -> Result<String, String> {
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
    if let Some(approval) = session_policy_value(&session_id, &sessions, &[], |session| {
        session.approval_policy.as_deref()
    }) {
        return Ok(approval);
    }
    let projects = state
        .runtime
        .list_projects(&workspace_id)
        .await
        .map_err(|e| format!("Failed to list projects: {e}"))?;
    for project in projects {
        let project_sessions = state
            .runtime
            .list_project_sessions(project.project_id)
            .await
            .map_err(|e| format!("Failed to list project sessions: {e}"))?;
        if let Some(approval) =
            session_policy_value(&session_id, &[], &project_sessions, |session| {
                session.approval_policy.as_deref()
            })
        {
            return Ok(approval);
        }
    }
    Ok(agent_tools::ApprovalPolicy::default().to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_session_sandbox_policy(state: State<'_, GuiState>) -> Result<String, String> {
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
    if let Some(sandbox) = session_policy_value(&session_id, &sessions, &[], |session| {
        session.sandbox_policy.as_deref()
    }) {
        return Ok(sandbox);
    }
    let projects = state
        .runtime
        .list_projects(&workspace_id)
        .await
        .map_err(|e| format!("Failed to list projects: {e}"))?;
    for project in projects {
        let project_sessions = state
            .runtime
            .list_project_sessions(project.project_id)
            .await
            .map_err(|e| format!("Failed to list project sessions: {e}"))?;
        if let Some(sandbox) =
            session_policy_value(&session_id, &[], &project_sessions, |session| {
                session.sandbox_policy.as_deref()
            })
        {
            return Ok(sandbox);
        }
    }
    serde_json::to_string(&agent_tools::SandboxPolicy::default()).map_err(|e| e.to_string())
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
pub async fn retry_task(
    session_id: String,
    task_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let sid: agent_core::SessionId = session_id.into();
    let tid: agent_core::TaskId = task_id.into();
    state
        .runtime
        .retry_task(workspace_id, sid, tid)
        .await
        .map_err(|e| format!("Failed to retry task: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_task(
    session_id: String,
    task_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let sid: agent_core::SessionId = session_id.into();
    let tid: agent_core::TaskId = task_id.into();
    state
        .runtime
        .cancel_task(workspace_id, sid, tid)
        .await
        .map_err(|e| format!("Failed to cancel task: {e}"))
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
    reasoning_effort: Option<String>,
) -> Result<(), String> {
    let session_id = agent_core::SessionId::from_string(session_id);
    state
        .runtime
        .switch_model(session_id, profile_alias, reasoning_effort)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Trajectory commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn list_trajectories(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<TrajectoryMetaResponse>, String> {
    let sid: agent_core::SessionId = session_id.into();
    let metas = state
        .runtime
        .list_trajectories(sid)
        .await
        .map_err(|e| format!("Failed to list trajectories: {e}"))?;
    Ok(metas
        .into_iter()
        .map(TrajectoryMetaResponse::from)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn get_trajectory_steps(
    trajectory_id: String,
    state: State<'_, GuiState>,
) -> Result<Vec<TrajectoryStepResponse>, String> {
    let tid = agent_core::TrajectoryId(trajectory_id);
    let steps = state
        .runtime
        .get_trajectory_steps(tid)
        .await
        .map_err(|e| format!("Failed to get trajectory steps: {e}"))?;
    Ok(steps
        .into_iter()
        .map(TrajectoryStepResponse::from)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn export_trajectory(
    trajectory_id: String,
    state: State<'_, GuiState>,
) -> Result<String, String> {
    let tid = agent_core::TrajectoryId(trajectory_id);
    let json = state
        .runtime
        .export_trajectory(tid)
        .await
        .map_err(|e| format!("Failed to export trajectory: {e}"))?;
    serde_json::to_string_pretty(&json).map_err(|e| format!("Failed to serialize: {e}"))
}

#[cfg(test)]
mod session_diagnostics_tests {
    use super::{attach_event_db_metadata, summarize_trace_export};
    use agent_core::{
        DomainEvent, EventPayload, PrivacyClassification, SessionId, TraceExport, WorkspaceId,
    };
    use std::path::Path;

    fn event(payload: EventPayload) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::from_string("wrk_diag".to_string()),
            SessionId::from_string("ses_diag".to_string()),
            agent_core::AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
    }

    #[test]
    fn summarize_trace_export_counts_messages_and_tool_calls() {
        let trace = TraceExport::new(
            SessionId::from_string("ses_diag".to_string()),
            vec![
                event(EventPayload::SessionInitialized {
                    model_profile: "default".into(),
                }),
                event(EventPayload::UserMessageAdded {
                    message_id: "u1".into(),
                    content: "hello".into(),
                    display_content: None,
                }),
                event(EventPayload::ModelToolCallRequested {
                    tool_call_id: "call_1".into(),
                    tool_id: "shell.exec".into(),
                }),
                event(EventPayload::McpToolCallStarted {
                    server_id: "srv".into(),
                    tool_name: "lookup".into(),
                }),
                event(EventPayload::McpToolCallCompleted {
                    server_id: "srv".into(),
                    tool_name: "lookup".into(),
                    duration_ms: 42,
                }),
                event(EventPayload::AssistantMessageCompleted {
                    message_id: "a1".into(),
                    content: "done".into(),
                }),
                event(EventPayload::TrajectoryStarted {
                    trajectory_id: "traj_1".into(),
                    task_id: "task_1".into(),
                }),
                event(EventPayload::TrajectoryCompleted {
                    trajectory_id: "traj_1".into(),
                    step_count: 3,
                    outcome: agent_core::TrajectoryOutcome::Success,
                }),
            ],
        );

        let summary = summarize_trace_export(&trace);

        assert_eq!(summary.session_id, "ses_diag");
        assert_eq!(summary.event_count, 8);
        assert_eq!(
            summary.last_event_type.as_deref(),
            Some("TrajectoryCompleted")
        );
        assert_eq!(summary.user_messages[0].message_id, "u1");
        assert_eq!(summary.user_messages[0].content, "hello");
        assert_eq!(summary.assistant_messages[0].message_id, "a1");
        assert_eq!(summary.assistant_messages[0].content, "done");
        assert_eq!(summary.model_tool_calls[0].tool_call_id, "call_1");
        assert_eq!(summary.model_tool_calls[0].tool_id, "shell.exec");
        assert_eq!(summary.mcp_tool_calls[0].status, "started");
        assert_eq!(summary.mcp_tool_calls[1].status, "completed");
        assert_eq!(summary.trajectory_started_count, 1);
        assert_eq!(summary.trajectory_completed_count, 1);
        assert_eq!(summary.trajectory_completed_outcomes[0].outcome, "success");

        let counts: std::collections::BTreeMap<_, _> = summary
            .event_type_counts
            .into_iter()
            .map(|entry| (entry.event_type, entry.count))
            .collect();
        assert_eq!(counts.get("UserMessageAdded"), Some(&1));
        assert_eq!(counts.get("AssistantMessageCompleted"), Some(&1));
        assert_eq!(counts.get("ModelToolCallRequested"), Some(&1));
        assert_eq!(counts.get("McpToolCallStarted"), Some(&1));
        assert_eq!(counts.get("McpToolCallCompleted"), Some(&1));
    }

    #[test]
    fn summarize_trace_export_counts_denied_permission_tools() {
        let trace = TraceExport::new(
            SessionId::from_string("ses_diag".to_string()),
            vec![
                event(EventPayload::ModelToolCallRequested {
                    tool_call_id: "call_browser".into(),
                    tool_id: "browser.action".into(),
                }),
                event(EventPayload::PermissionRequested {
                    request_id: "call_browser".into(),
                    tool_id: "browser.action".into(),
                    preview: "browser.action({})".into(),
                }),
                event(EventPayload::PermissionDenied {
                    request_id: "call_browser".into(),
                    reason: "forbidden by task".into(),
                }),
                event(EventPayload::ModelToolCallRequested {
                    tool_call_id: "call_write".into(),
                    tool_id: "fs.write".into(),
                }),
                event(EventPayload::PermissionDenied {
                    request_id: "call_write".into(),
                    reason: "sandbox denied".into(),
                }),
            ],
        );

        let summary = summarize_trace_export(&trace);
        let denied: std::collections::BTreeMap<_, _> = summary
            .permission_denied_tools
            .into_iter()
            .map(|entry| (entry.tool_id, entry.count))
            .collect();

        assert_eq!(denied.get("browser.action"), Some(&1));
        assert_eq!(denied.get("fs.write"), Some(&1));
    }

    #[test]
    fn summarize_trace_export_totals_model_usage() {
        let trace = TraceExport::new(
            SessionId::from_string("ses_diag".to_string()),
            vec![
                event(EventPayload::ModelUsageRecorded {
                    model_profile: "fast".into(),
                    input_tokens: 100,
                    output_tokens: 40,
                    cache_creation_input_tokens: Some(7),
                    cache_read_input_tokens: Some(11),
                }),
                event(EventPayload::ModelUsageRecorded {
                    model_profile: "fast".into(),
                    input_tokens: 20,
                    output_tokens: 5,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: Some(3),
                }),
            ],
        );

        let summary = summarize_trace_export(&trace);

        assert_eq!(summary.model_usage.total_input_tokens, 120);
        assert_eq!(summary.model_usage.total_output_tokens, 45);
        assert_eq!(summary.model_usage.total_cache_creation_input_tokens, 7);
        assert_eq!(summary.model_usage.total_cache_read_input_tokens, 14);
        assert_eq!(summary.model_usage.request_count, 2);
        assert_eq!(summary.model_usage.by_profile.len(), 1);
        assert_eq!(summary.model_usage.by_profile[0].model_profile, "fast");
        assert_eq!(summary.model_usage.by_profile[0].input_tokens, 120);
        assert_eq!(summary.model_usage.by_profile[0].output_tokens, 45);
    }

    #[test]
    fn summarize_trace_export_reports_stuck_signals_without_terminal_message() {
        let trace = TraceExport::new(
            SessionId::from_string("ses_diag".to_string()),
            vec![
                event(EventPayload::ModelRequestStarted {
                    model_profile: "default".into(),
                    model_id: "model-a".into(),
                }),
                event(EventPayload::ModelRequestStarted {
                    model_profile: "default".into(),
                    model_id: "model-b".into(),
                }),
                event(EventPayload::ToolInvocationCompleted {
                    invocation_id: "unmatched".into(),
                    tool_id: "shell.exec".into(),
                    output_preview: String::new(),
                    exit_code: Some(0),
                    duration_ms: 1,
                    truncated: false,
                    images: Vec::new(),
                }),
                event(EventPayload::ToolInvocationStarted {
                    invocation_id: "tool_1".into(),
                    tool_id: "shell.exec".into(),
                    input_preview: String::new(),
                }),
                event(EventPayload::ToolInvocationStarted {
                    invocation_id: "tool_2".into(),
                    tool_id: "fs.read".into(),
                    input_preview: String::new(),
                }),
                event(EventPayload::ToolInvocationFailed {
                    invocation_id: "tool_1".into(),
                    tool_id: "shell.exec".into(),
                    error: "denied".into(),
                }),
                event(EventPayload::TrajectoryCompleted {
                    trajectory_id: "traj_failed".into(),
                    step_count: 2,
                    outcome: agent_core::TrajectoryOutcome::Failed,
                }),
                event(EventPayload::TrajectoryCompleted {
                    trajectory_id: "traj_success".into(),
                    step_count: 1,
                    outcome: agent_core::TrajectoryOutcome::Success,
                }),
            ],
        );

        let summary = summarize_trace_export(&trace);

        assert_eq!(summary.running_model_requests, 2);
        assert_eq!(summary.running_tool_invocations, 1);
        assert_eq!(summary.trajectory_failed_count, 1);
        assert!(!summary.has_terminal_assistant_message);
    }

    #[test]
    fn summarize_trace_export_reports_terminal_message_and_closes_model_request() {
        let trace = TraceExport::new(
            SessionId::from_string("ses_diag".to_string()),
            vec![
                event(EventPayload::ModelRequestStarted {
                    model_profile: "default".into(),
                    model_id: "model-a".into(),
                }),
                event(EventPayload::AssistantMessageCompleted {
                    message_id: "a1".into(),
                    content: "done".into(),
                }),
            ],
        );

        let summary = summarize_trace_export(&trace);

        assert_eq!(summary.running_model_requests, 0);
        assert!(summary.has_terminal_assistant_message);
    }

    #[test]
    fn summarize_trace_export_keeps_recent_model_stream_statuses() {
        let mut events = Vec::new();
        for index in 0..7 {
            events.push(event(EventPayload::ModelStreamStatus {
                phase: format!("phase_{index}"),
                retrying: index < 6,
                retry_attempt: index,
                max_retries: 6,
                message: format!("status {index}"),
            }));
        }
        let trace = TraceExport::new(SessionId::from_string("ses_diag".to_string()), events);

        let summary = summarize_trace_export(&trace);

        assert_eq!(summary.recent_model_stream_statuses.len(), 5);
        assert_eq!(summary.recent_model_stream_statuses[0].phase, "phase_2");
        assert_eq!(summary.recent_model_stream_statuses[4].phase, "phase_6");
        assert!(!summary.recent_model_stream_statuses[4].retrying);
        assert_eq!(summary.recent_model_stream_statuses[4].retry_attempt, 6);
        assert_eq!(summary.recent_model_stream_statuses[4].max_retries, 6);
        assert_eq!(summary.recent_model_stream_statuses[4].message, "status 6");
    }

    #[test]
    fn session_diagnostics_event_db_metadata() {
        let trace = TraceExport::new(SessionId::from_string("ses_diag".to_string()), vec![]);
        let mut summary = summarize_trace_export(&trace);
        let data_dir = Path::new("/tmp/kairox-home/.kairox");
        let expected_path = data_dir
            .join("kairox-gui.sqlite")
            .to_string_lossy()
            .into_owned();

        attach_event_db_metadata(&mut summary, data_dir);

        assert_eq!(
            summary.event_db_path.as_deref(),
            Some(expected_path.as_str())
        );
        assert_eq!(summary.event_db_path_source.as_deref(), Some("tauri_state"));
    }
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
mod session_policy_lookup_tests {
    use super::session_policy_value;
    use agent_core::{SessionId, SessionMeta, WorkspaceId};

    fn meta(id: &str, approval_policy: Option<&str>, sandbox_policy: Option<&str>) -> SessionMeta {
        SessionMeta {
            session_id: SessionId::from_string(id.to_string()),
            workspace_id: WorkspaceId::from_string("wrk_test".to_string()),
            title: "Test".into(),
            model_profile: "fake".into(),
            model_id: None,
            provider: None,
            approval_policy: approval_policy.map(str::to_string),
            sandbox_policy: sandbox_policy.map(str::to_string),
            project_id: None,
            worktree_path: None,
            branch: None,
            visibility: None,
            deleted_at: None,
            created_at: "2026-06-02T00:00:00Z".into(),
            updated_at: "2026-06-02T00:00:00Z".into(),
        }
    }

    #[test]
    fn policy_lookup_includes_project_sessions() {
        let session_id = SessionId::from_string("ses_project".to_string());
        let ordinary_sessions = vec![meta("ses_other", Some("never"), None)];
        let project_sessions = vec![meta("ses_project", Some("always"), None)];

        let policy = session_policy_value(
            &session_id,
            &ordinary_sessions,
            &project_sessions,
            |session| session.approval_policy.as_deref(),
        );

        assert_eq!(policy.as_deref(), Some("always"));
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
