mod app;
mod app_state;
mod components;
mod keybindings;
mod view;

use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;

use agent_config::Config;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_memory::{MemoryQuery, SqliteMemoryStore};
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use anyhow::Result;
use crossterm::event::{Event, EventStream};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use app::App;
use components::trace::MemoryRow;
use components::{
    Command, CrossPanelEffect, McpConnectivityEntry, McpPromptEntry, McpResourceEntry,
    McpServerEntry, McpServerStatusView, McpToolEntry, ModelOverlaySnapshot, ModelProfileEntry,
    SessionInfo, SessionState,
};

// ---------------------------------------------------------------------------
// AppEvent — unified event type for the main loop
// ---------------------------------------------------------------------------

enum AppEvent {
    Key(crossterm::event::KeyEvent),
    DomainEvent(Box<agent_core::DomainEvent>),
    Tick,
}

// ---------------------------------------------------------------------------
// Command dispatch — executes runtime commands and updates app state
// ---------------------------------------------------------------------------

async fn dispatch_commands(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    commands: Vec<Command>,
) {
    for command in commands {
        match command {
            Command::SendMessage {
                workspace_id,
                session_id,
                content,
                attachments,
            } => {
                if let Err(e) = runtime
                    .send_message(SendMessageRequest {
                        workspace_id,
                        session_id,
                        content,
                        attachments,
                    })
                    .await
                {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::DecidePermission {
                request_id,
                approved,
            } => {
                if let Err(e) = runtime
                    .resolve_permission(
                        &request_id,
                        agent_core::PermissionDecision {
                            request_id: request_id.clone(),
                            approve: approved,
                            reason: None,
                        },
                    )
                    .await
                {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[permission error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::OpenMcpOverlay => {
                refresh_mcp_overlay(runtime, app).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }

            Command::TrustMcpServer { server_id } => {
                // Trust the MCP server via the runtime's MCP manager
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let manager = mcp_manager.lock().await;
                    let result = manager.trust_server(&server_id).await;
                    drop(manager);
                    if let Err(e) = result {
                        push_status_message(app, format!("[MCP trust error: {e}]"));
                    } else {
                        push_status_message(
                            app,
                            format!("MCP server '{}' is now trusted", server_id),
                        );
                        refresh_mcp_overlay(runtime, app).await;
                    }
                }
            }

            Command::RevokeMcpTrust { server_id } => {
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let manager = mcp_manager.lock().await;
                    let result = manager.revoke_trust(&server_id).await;
                    drop(manager);
                    if let Err(e) = result {
                        push_status_message(app, format!("[MCP revoke trust error: {e}]"));
                    } else {
                        push_status_message(
                            app,
                            format!("MCP server '{}' trust revoked", server_id),
                        );
                        refresh_mcp_overlay(runtime, app).await;
                    }
                }
            }

            Command::StartMcpServer { server_id } => {
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let mut manager = mcp_manager.lock().await;
                    match manager.ensure_server(&server_id).await {
                        Ok(_) => {
                            app.state.current_session.messages.push(
                                agent_core::projection::ProjectedMessage {
                                    role: agent_core::projection::ProjectedRole::Assistant,
                                    content: format!("MCP server '{}' started", server_id),
                                },
                            );
                        }
                        Err(e) => {
                            app.state.current_session.messages.push(
                                agent_core::projection::ProjectedMessage {
                                    role: agent_core::projection::ProjectedRole::Assistant,
                                    content: format!("[MCP start error: {e}]"),
                                },
                            );
                        }
                    }
                    drop(manager);
                    refresh_mcp_overlay(runtime, app).await;
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::StopMcpServer { server_id } => {
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let mut manager = mcp_manager.lock().await;
                    match manager.shutdown_server(&server_id).await {
                        Ok(()) => {
                            app.state.current_session.messages.push(
                                agent_core::projection::ProjectedMessage {
                                    role: agent_core::projection::ProjectedRole::Assistant,
                                    content: format!("MCP server '{}' stopped", server_id),
                                },
                            );
                        }
                        Err(e) => {
                            app.state.current_session.messages.push(
                                agent_core::projection::ProjectedMessage {
                                    role: agent_core::projection::ProjectedRole::Assistant,
                                    content: format!("[MCP stop error: {e}]"),
                                },
                            );
                        }
                    }
                    drop(manager);
                    refresh_mcp_overlay(runtime, app).await;
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::RefreshMcpTools { server_id } => {
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let mut manager = mcp_manager.lock().await;
                    match manager.refresh_tools(&server_id).await {
                        Ok(tools) => {
                            let disabled = manager.get_disabled_tools(&server_id);
                            let entries = mcp_tool_entries(&server_id, tools, &disabled);
                            drop(manager);
                            app.dispatch_effects(vec![CrossPanelEffect::McpToolsLoaded {
                                server_id: server_id.clone(),
                                healthy: true,
                                error: None,
                                tools: entries,
                            }]);
                            push_status_message(
                                app,
                                format!("MCP server '{}' refreshed", server_id),
                            );
                            refresh_mcp_overlay(runtime, app).await;
                        }
                        Err(e) => {
                            drop(manager);
                            push_status_message(app, format!("[MCP refresh error: {e}]"));
                        }
                    }
                }
            }

            Command::CheckMcpHealth { server_id } => {
                match runtime.check_mcp_health(&server_id).await {
                    Ok(result) => {
                        let disabled = runtime
                            .get_mcp_disabled_tools(&server_id)
                            .await
                            .unwrap_or_default();
                        let healthy = result.healthy;
                        let error = result.error.clone();
                        let tool_count = result.tools.len();
                        let entries = mcp_tool_entries(&server_id, result.tools, &disabled);
                        refresh_mcp_overlay(runtime, app).await;
                        app.dispatch_effects(vec![CrossPanelEffect::McpToolsLoaded {
                            server_id: server_id.clone(),
                            tools: entries,
                            healthy,
                            error: error.clone(),
                        }]);
                        if healthy {
                            push_status_message(
                                app,
                                format!(
                                    "MCP server '{}' healthy ({} tools)",
                                    server_id, tool_count
                                ),
                            );
                        } else {
                            let reason = error.unwrap_or_else(|| "unknown error".to_string());
                            push_status_message(
                                app,
                                format!("[MCP health error: {server_id}: {reason}]"),
                            );
                        }
                    }
                    Err(e) => {
                        push_status_message(app, format!("[MCP health error: {e}]"));
                    }
                }
            }

            Command::TestMcpConnectivity { server_id } => {
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let mut manager = mcp_manager.lock().await;
                    let result = manager
                        .test_connectivity(&server_id, Some(Duration::from_secs(15)))
                        .await;
                    drop(manager);
                    match result {
                        Ok(agent_mcp::ConnectivityResult::Connected { tool_count }) => {
                            app.dispatch_effects(vec![CrossPanelEffect::McpConnectivityChecked(
                                McpConnectivityEntry {
                                    server_id: server_id.clone(),
                                    connected: true,
                                    tool_count: Some(tool_count),
                                    reason: None,
                                },
                            )]);
                            push_status_message(
                                app,
                                format!(
                                    "MCP server '{}' connected ({} tools)",
                                    server_id, tool_count
                                ),
                            );
                            refresh_mcp_overlay(runtime, app).await;
                        }
                        Ok(agent_mcp::ConnectivityResult::Failed { reason }) => {
                            app.dispatch_effects(vec![CrossPanelEffect::McpConnectivityChecked(
                                McpConnectivityEntry {
                                    server_id: server_id.clone(),
                                    connected: false,
                                    tool_count: None,
                                    reason: Some(reason.clone()),
                                },
                            )]);
                            push_status_message(
                                app,
                                format!("[MCP connectivity error: {server_id}: {reason}]"),
                            );
                        }
                        Err(e) => {
                            push_status_message(app, format!("[MCP connectivity error: {e}]"));
                        }
                    }
                }
            }

            Command::SetMcpToolDisabled {
                server_id,
                tool_name,
                disabled,
            } => {
                match runtime
                    .set_mcp_tool_disabled(&server_id, &tool_name, disabled)
                    .await
                {
                    Ok(()) => {
                        let state = if disabled { "disabled" } else { "enabled" };
                        push_status_message(
                            app,
                            format!("MCP tool '{}.{}' {}", server_id, tool_name, state),
                        );
                        match runtime.check_mcp_health(&server_id).await {
                            Ok(result) => {
                                let disabled_tools = runtime
                                    .get_mcp_disabled_tools(&server_id)
                                    .await
                                    .unwrap_or_default();
                                app.dispatch_effects(vec![CrossPanelEffect::McpToolsLoaded {
                                    server_id: server_id.clone(),
                                    tools: mcp_tool_entries(
                                        &server_id,
                                        result.tools,
                                        &disabled_tools,
                                    ),
                                    healthy: result.healthy,
                                    error: result.error,
                                }]);
                                refresh_mcp_overlay(runtime, app).await;
                            }
                            Err(e) => {
                                push_status_message(app, format!("[MCP health error: {e}]"));
                            }
                        }
                    }
                    Err(e) => {
                        push_status_message(app, format!("[MCP tool state error: {e}]"));
                    }
                }
            }

            Command::ListMcpResources { server_id } => {
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let manager = mcp_manager.lock().await;
                    let result = manager.list_resources(&server_id).await;
                    drop(manager);
                    match result {
                        Ok(resources) => {
                            let entries = resources
                                .into_iter()
                                .map(|resource| McpResourceEntry {
                                    server_id: server_id.clone(),
                                    uri: resource.uri,
                                    name: resource.name,
                                    description: resource.description,
                                    mime_type: resource.mime_type,
                                })
                                .collect::<Vec<_>>();
                            let count = entries.len();
                            app.dispatch_effects(vec![CrossPanelEffect::McpResourcesLoaded {
                                server_id: server_id.clone(),
                                resources: entries,
                            }]);
                            push_status_message(
                                app,
                                format!("MCP server '{}' resources: {}", server_id, count),
                            );
                        }
                        Err(e) => {
                            push_status_message(app, format!("[MCP resources error: {e}]"));
                        }
                    }
                }
            }

            Command::ListMcpPrompts { server_id } => {
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let manager = mcp_manager.lock().await;
                    let result = manager.list_prompts(&server_id).await;
                    drop(manager);
                    match result {
                        Ok(prompts) => {
                            let entries = prompts
                                .into_iter()
                                .map(|prompt| McpPromptEntry {
                                    server_id: server_id.clone(),
                                    name: prompt.name,
                                    description: prompt.description,
                                    argument_count: prompt.arguments.len(),
                                })
                                .collect::<Vec<_>>();
                            let count = entries.len();
                            app.dispatch_effects(vec![CrossPanelEffect::McpPromptsLoaded {
                                server_id: server_id.clone(),
                                prompts: entries,
                            }]);
                            push_status_message(
                                app,
                                format!("MCP server '{}' prompts: {}", server_id, count),
                            );
                        }
                        Err(e) => {
                            push_status_message(app, format!("[MCP prompts error: {e}]"));
                        }
                    }
                }
            }

            Command::ReadMcpResource { server_id, uri } => {
                if let Some(mcp_manager) = runtime.mcp_manager() {
                    let manager = mcp_manager.lock().await;
                    let result = manager.read_resource(&server_id, &uri).await;
                    drop(manager);
                    match result {
                        Ok(blocks) => {
                            let preview = mcp_content_preview(&blocks);
                            app.dispatch_effects(vec![CrossPanelEffect::McpResourceRead {
                                server_id: server_id.clone(),
                                uri: uri.clone(),
                                preview: preview.clone(),
                            }]);
                            push_status_message(
                                app,
                                format!("MCP resource '{}'\n{}", uri, preview),
                            );
                        }
                        Err(e) => {
                            push_status_message(app, format!("[MCP resource read error: {e}]"));
                        }
                    }
                }
            }

            Command::CancelSession {
                workspace_id,
                session_id,
            } => {
                if let Err(e) = runtime.cancel_session(workspace_id, session_id).await {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[cancel error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::RetryTask {
                workspace_id,
                session_id,
                task_id,
            } => {
                if let Err(e) = runtime.retry_task(workspace_id, session_id, task_id).await {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[task retry error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::CancelTask {
                workspace_id,
                session_id,
                task_id,
            } => {
                if let Err(e) = runtime.cancel_task(workspace_id, session_id, task_id).await {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[task cancel error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::LoadMemories {
                scope,
                keywords,
                limit,
            } => {
                match runtime.memory_store() {
                    Some(memory_store) => {
                        match memory_store
                            .query(MemoryQuery {
                                scope,
                                keywords,
                                limit,
                                session_id: None,
                                workspace_id: None,
                            })
                            .await
                        {
                            Ok(entries) => {
                                app.trace.set_memory_rows(
                                    entries.into_iter().map(MemoryRow::from).collect(),
                                );
                            }
                            Err(e) => {
                                app.state.current_session.messages.push(
                                    agent_core::projection::ProjectedMessage {
                                        role: agent_core::projection::ProjectedRole::Assistant,
                                        content: format!("[memory query error: {e}]"),
                                    },
                                );
                            }
                        }
                    }
                    None => {
                        app.trace.set_memory_rows(Vec::new());
                    }
                }
                app.state.render_scheduler.mark_dirty();
            }

            Command::DeleteMemory { memory_id } => {
                match runtime.memory_store() {
                    Some(memory_store) => {
                        if let Err(e) = memory_store.delete(&memory_id).await {
                            app.state.current_session.messages.push(
                                agent_core::projection::ProjectedMessage {
                                    role: agent_core::projection::ProjectedRole::Assistant,
                                    content: format!("[memory delete error: {e}]"),
                                },
                            );
                        } else {
                            app.trace.remove_memory_row(&memory_id);
                        }
                    }
                    None => {
                        app.trace.remove_memory_row(&memory_id);
                    }
                }
                app.state.render_scheduler.mark_dirty();
            }

            Command::CompactSession {
                workspace_id: _,
                session_id,
            } => {
                if let Err(e) = runtime
                    .compact_session(session_id, agent_core::CompactionReason::UserRequested)
                    .await
                {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[compact error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::SwitchModel {
                workspace_id: _,
                session_id,
                alias,
                reasoning_effort,
            } => {
                if let Err(e) = runtime
                    .switch_model(session_id, alias, reasoning_effort)
                    .await
                {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[switch_model error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::OpenModelOverlay => {
                refresh_model_overlay(runtime, app);
            }

            Command::OpenSkillsOverlay
            | Command::OpenPluginsOverlay
            | Command::ListSkills
            | Command::ShowSkill { .. }
            | Command::ActivateSkill { .. }
            | Command::DeactivateSkill { .. }
            | Command::ListSkillCatalog { .. }
            | Command::InstallRemoteSkill { .. }
            | Command::InstallGithubSkill { .. }
            | Command::UpdateSkillSettings { .. }
            | Command::DeleteSkillSettings { .. }
            | Command::SetSkillEnabled { .. }
            | Command::SetSkillSourceEnabled { .. }
            | Command::RefreshSkillCatalog
            | Command::SetPluginEnabled { .. }
            | Command::DeletePluginSettings { .. }
            | Command::SetPluginMarketplaceSourceEnabled { .. }
            | Command::InstallPlugin { .. }
            | Command::SetMcpServerEnabled { .. }
            | Command::DeleteMcpServerSettings { .. }
            | Command::InstallMcpServer { .. }
            | Command::UninstallMcpServer { .. }
            | Command::SetMcpCatalogSourceEnabled { .. } => {
                let refresh_mcp_after = matches!(
                    command,
                    Command::SetMcpServerEnabled { .. }
                        | Command::DeleteMcpServerSettings { .. }
                        | Command::InstallMcpServer { .. }
                        | Command::UninstallMcpServer { .. }
                        | Command::SetMcpCatalogSourceEnabled { .. }
                );
                app::dispatch_commands(runtime, app, vec![command]).await;
                if refresh_mcp_after && app.mcp_overlay.is_visible() {
                    refresh_mcp_overlay(runtime, app).await;
                }
            }

            Command::SetPermissionMode { mode } => {
                runtime.set_permission_mode(mode).await;
                app.sync_status_bar();
                app.state.render_scheduler.mark_dirty();
            }

            Command::StartSession {
                workspace_id: ws_id,
                model_profile: mp,
            } => {
                match runtime
                    .start_session(StartSessionRequest {
                        workspace_id: ws_id,
                        model_profile: mp.clone(),
                        permission_mode: None,
                    })
                    .await
                {
                    Ok(session_id) => {
                        app.current_session_id = Some(session_id.clone());
                        app.state.sessions.push(SessionInfo {
                            id: session_id,
                            title: format!("Session using {mp}"),
                            model_profile: mp,
                            state: SessionState::Idle,
                            pinned: false,
                            archived: false,
                        });
                        app.state.current_session =
                            agent_core::projection::SessionProjection::default();
                        app.domain_events.clear();
                        app.state.render_scheduler.reset();
                        // Select the new session in the sessions panel
                        app.sessions
                            .state
                            .select(Some(app.state.sessions.len() - 1));
                    }
                    Err(e) => {
                        app.state.current_session.messages.push(
                            agent_core::projection::ProjectedMessage {
                                role: agent_core::projection::ProjectedRole::Assistant,
                                content: format!("[start session error: {e}]"),
                            },
                        );
                        app.state.render_scheduler.mark_dirty();
                    }
                }
            }

            Command::SwitchSession { session_id } => {
                switch_app_to_session(runtime, app, session_id).await;
            }

            Command::RenameSession { session_id, title } => {
                match runtime.rename_session(&session_id, title.clone()).await {
                    Ok(()) => {
                        if let Some(session) =
                            app.state.sessions.iter_mut().find(|s| s.id == session_id)
                        {
                            session.title = title;
                        }
                        app.state.render_scheduler.mark_dirty();
                    }
                    Err(e) => push_status_error(app, format!("[rename session error: {e}]")),
                }
            }

            Command::ArchiveSession { session_id } => {
                match runtime.soft_delete_session(&session_id).await {
                    Ok(()) => {
                        if let Some(session) =
                            app.state.sessions.iter_mut().find(|s| s.id == session_id)
                        {
                            session.archived = true;
                            session.state = SessionState::Idle;
                        }
                        if app.current_session_id.as_ref() == Some(&session_id) {
                            switch_to_first_active_session(runtime, app).await;
                        } else {
                            select_session_row(app, &session_id);
                            app.state.render_scheduler.mark_dirty();
                        }
                    }
                    Err(e) => push_status_error(app, format!("[archive session error: {e}]")),
                }
            }

            Command::RestoreSession { session_id } => {
                match runtime.restore_archived_session(&session_id).await {
                    Ok(()) => {
                        if let Some(session) =
                            app.state.sessions.iter_mut().find(|s| s.id == session_id)
                        {
                            session.archived = false;
                            session.state = SessionState::Idle;
                        }
                        select_session_row(app, &session_id);
                        app.state.render_scheduler.mark_dirty();
                    }
                    Err(e) => push_status_error(app, format!("[restore session error: {e}]")),
                }
            }

            Command::DeleteSession { session_id } => {
                match runtime.permanently_delete_session(&session_id).await {
                    Ok(()) => {
                        app.state.sessions.retain(|s| s.id != session_id);
                        if app.current_session_id.as_ref() == Some(&session_id) {
                            switch_to_first_active_session(runtime, app).await;
                        } else {
                            clamp_session_selection(app);
                            app.state.render_scheduler.mark_dirty();
                        }
                    }
                    Err(e) => push_status_error(app, format!("[delete session error: {e}]")),
                }
            }
        }
    }
}

async fn switch_app_to_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    sid: agent_core::SessionId,
) {
    app.current_session_id = Some(sid.clone());

    for session in &mut app.state.sessions {
        if session.id == sid {
            session.state = SessionState::Active;
        } else if session.state == SessionState::Active {
            session.state = SessionState::Idle;
        }
    }
    select_session_row(app, &sid);

    let projection = runtime.get_session_projection(sid.clone()).await;
    let trace = runtime.get_trace(sid).await;

    if let Ok(proj) = projection {
        app.state.current_session = proj;
    }
    if let Ok(trc) = trace {
        app.domain_events = trc.into_iter().map(|t| t.event).collect();
    }

    app.state.render_scheduler.mark_dirty_immediate();
}

async fn switch_to_first_active_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
) {
    if let Some(session_id) = app
        .state
        .sessions
        .iter()
        .find(|session| !session.archived)
        .map(|session| session.id.clone())
    {
        switch_app_to_session(runtime, app, session_id).await;
    } else {
        app.current_session_id = None;
        app.state.current_session = agent_core::projection::SessionProjection::default();
        app.domain_events.clear();
        clamp_session_selection(app);
        app.state.render_scheduler.mark_dirty_immediate();
    }
}

fn select_session_row(app: &mut App, session_id: &agent_core::SessionId) {
    if let Some(index) = app
        .state
        .sessions
        .iter()
        .position(|session| &session.id == session_id)
    {
        app.sessions.state.select(Some(index));
    }
}

fn clamp_session_selection(app: &mut App) {
    let len = app.state.sessions.len();
    if len == 0 {
        app.sessions.state.select(None);
        return;
    }
    let selected = app.sessions.state.selected().unwrap_or(0).min(len - 1);
    app.sessions.state.select(Some(selected));
}

fn push_status_message(app: &mut App, content: String) {
    app.state
        .current_session
        .messages
        .push(agent_core::projection::ProjectedMessage {
            role: agent_core::projection::ProjectedRole::Assistant,
            content,
        });
    app.state.render_scheduler.mark_dirty();
}

fn push_status_error(app: &mut App, content: String) {
    push_status_message(app, content);
}

fn mcp_tool_entries(
    server_id: &str,
    tools: Vec<agent_mcp::McpToolDef>,
    disabled_tools: &std::collections::HashSet<String>,
) -> Vec<McpToolEntry> {
    tools
        .into_iter()
        .map(|tool| {
            let disabled = disabled_tools.contains(&tool.name);
            McpToolEntry {
                server_id: server_id.to_string(),
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
                disabled,
            }
        })
        .collect()
}

fn mcp_content_preview(blocks: &[agent_mcp::McpContentBlock]) -> String {
    let rendered = blocks
        .iter()
        .map(|block| match block {
            agent_mcp::McpContentBlock::Text { text } => text.clone(),
            agent_mcp::McpContentBlock::Image { mime_type, .. } => {
                format!("[image: {mime_type}]")
            }
            agent_mcp::McpContentBlock::Resource { resource } => {
                let text = resource
                    .text
                    .as_ref()
                    .map(|value| format!(" {}", value))
                    .unwrap_or_default();
                format!("[resource: {}]{}", resource.uri, text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if rendered.chars().count() > 800 {
        let preview: String = rendered.chars().take(800).collect();
        format!("{preview}...")
    } else {
        rendered
    }
}

// ---------------------------------------------------------------------------
// MCP overlay snapshot helper
// ---------------------------------------------------------------------------

/// Snapshot the runtime's MCP manager into a `Vec<McpServerEntry>` and
/// dispatch a `ShowMcpOverlay` effect so the overlay component re-renders.
///
/// Read-only over `McpServerManager`: status, trust, and tool counts are
/// captured without starting or stopping servers. If the runtime has no MCP
/// manager configured the overlay opens with an empty list.
async fn refresh_mcp_overlay(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
) {
    let entries = match runtime.mcp_manager() {
        Some(mcp_manager) => {
            let manager = mcp_manager.lock().await;
            let statuses = manager.server_statuses();

            // Count MCP tools per server from the tool registry. Adapter
            // ids are namespaced as `mcp.<server_id>.<tool_name>`.
            let tool_registry = runtime.tool_registry();
            let registry = tool_registry.lock().await;
            let definitions = registry.list_all().await;
            drop(registry);

            let mut entries: Vec<McpServerEntry> = Vec::with_capacity(statuses.len());
            for (server_id, status) in statuses {
                let trusted = manager.is_trusted(&server_id).await;
                let prefix = format!("mcp.{}.", server_id);
                let tool_count = definitions
                    .iter()
                    .filter(|def| def.tool_id.starts_with(&prefix))
                    .count();
                let status_view = match status {
                    agent_mcp::types::McpServerStatus::Stopped => McpServerStatusView::Stopped,
                    agent_mcp::types::McpServerStatus::Starting => McpServerStatusView::Starting,
                    agent_mcp::types::McpServerStatus::Running => McpServerStatusView::Running,
                    agent_mcp::types::McpServerStatus::Failed => McpServerStatusView::Failed,
                };
                entries.push(McpServerEntry {
                    server_id,
                    status: status_view,
                    trusted,
                    tool_count,
                });
            }
            entries.sort_by(|a, b| a.server_id.cmp(&b.server_id));
            entries
        }
        None => Vec::new(),
    };

    app::refresh_mcp_overlay(runtime, app, entries).await;
}

/// Build a `ModelOverlaySnapshot` from the runtime's config and dispatch the
/// `ShowModelOverlay` effect.
///
/// Read-only over `ProfileDef`/`ProfileInfo`. The overlay reflects the
/// currently active session's profile and reasoning effort so it can preselect
/// them on open.
fn refresh_model_overlay(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
) {
    let infos = runtime.config().profile_info();
    let profiles: Vec<ModelProfileEntry> = infos
        .into_iter()
        .map(|p| ModelProfileEntry {
            alias: p.alias,
            provider_display: p.provider_display,
            model_display: p.model_display,
            supports_reasoning: p.supports_reasoning,
        })
        .collect();
    let snapshot = ModelOverlaySnapshot {
        profiles,
        current_alias: Some(app.state.model_profile.clone()),
        current_effort: app.state.reasoning_effort.clone(),
    };
    app.dispatch_effects(vec![CrossPanelEffect::ShowModelOverlay(snapshot)]);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    eprintln!(
        "Kairox TUI {}",
        agent_core::build_info::BuildInfo::from_env()
    );

    // 2. Check size
    let size = terminal.size()?;
    if size.width < 80 || size.height < 24 {
        disable_raw_mode()?;
        crossterm::execute!(std::io::stdout(), LeaveAlternateScreen)?;
        eprintln!(
            "Terminal too small: {}x{}. Minimum: 80x24.",
            size.width, size.height
        );
        std::process::exit(1);
    }

    // 3. Load config and build runtime
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Config warning: {e}, using defaults");
        Config::defaults()
    });
    let router = config.build_router();
    let profiles = config.profile_names();
    let profile = config.default_profile();

    eprintln!("Available model profiles: {:?}", profiles);
    eprintln!("Using profile: {profile}");

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let home_dir = std::path::PathBuf::from(home);
    let data_dir = home_dir.join(".kairox");
    tokio::fs::create_dir_all(&data_dir).await?;
    let db_path = data_dir.join("kairox.sqlite");
    let database_url = format!(
        "sqlite:///{}",
        db_path.display().to_string().trim_start_matches('/')
    );
    let store = SqliteEventStore::connect(&database_url).await?;
    let mem_store = std::sync::Arc::new(SqliteMemoryStore::new(store.pool().clone()).await?)
        as std::sync::Arc<dyn agent_memory::MemoryStore>;
    let workspace_path = std::env::current_dir()?;
    let skill_roots = agent_runtime::skills::build_default_skill_roots(&home_dir, &workspace_path);
    let skill_settings_roots =
        agent_runtime::skills::build_default_skill_settings_roots(&home_dir, &workspace_path);
    let skill_registry = agent_skills::FileSkillRegistry::discover(skill_roots).await?;

    let ollama_clients = agent_config::build_ollama_clients(&config);
    let config_arc = std::sync::Arc::new(config);
    let runtime = Arc::new(
        LocalRuntime::new(store, router)
            .with_permission_mode(PermissionMode::Suggest)
            .with_context_limit(100_000)
            .with_memory_store(mem_store)
            .with_config(config_arc)
            .with_ollama_clients(ollama_clients)
            .with_skill_registry(Arc::new(skill_registry))
            .with_skill_settings_roots(skill_settings_roots)
            .with_skill_catalog(Some(data_dir.clone()))
            .with_builtin_tools(workspace_path.clone())
            .await,
    );

    // Try to restore previous workspace and sessions, or create fresh ones
    let workspace_path_str = workspace_path.display().to_string();

    let (workspace_id, mut app_sessions) = {
        // Try to find an existing workspace for this path
        let workspaces = runtime.list_workspaces().await.unwrap_or_default();
        let existing = workspaces.iter().find(|w| w.path == workspace_path_str);

        if let Some(ws) = existing {
            let sessions = runtime
                .list_sessions(&ws.workspace_id)
                .await
                .unwrap_or_default();
            let archived_sessions = runtime
                .list_archived_sessions(&ws.workspace_id)
                .await
                .unwrap_or_default();
            let mut session_infos: Vec<SessionInfo> = sessions
                .iter()
                .map(|s| SessionInfo {
                    id: s.session_id.clone(),
                    title: s.title.clone(),
                    model_profile: s.model_profile.clone(),
                    state: SessionState::Idle,
                    pinned: false,
                    archived: false,
                })
                .collect();
            session_infos.extend(archived_sessions.iter().map(|s| SessionInfo {
                id: s.session_id.clone(),
                title: s.title.clone(),
                model_profile: s.model_profile.clone(),
                state: SessionState::Idle,
                pinned: false,
                archived: true,
            }));
            (ws.workspace_id.clone(), session_infos)
        } else {
            let ws = runtime.open_workspace(workspace_path_str).await?;
            (ws.workspace_id, Vec::new())
        }
    };

    // If no sessions exist, create a new one
    if app_sessions.iter().all(|session| session.archived) {
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace_id.clone(),
                model_profile: profile.clone(),
                permission_mode: None,
            })
            .await?;
        app_sessions.push(SessionInfo {
            id: session_id,
            title: format!("Session using {profile}"),
            model_profile: profile.clone(),
            state: SessionState::Idle,
            pinned: false,
            archived: false,
        });
    }

    // 4. Create App with restored sessions
    let active_session_id = app_sessions
        .iter()
        .rfind(|session| !session.archived)
        .expect("at least one active session must exist")
        .id
        .clone();

    let mut app = App::new(&profile, PermissionMode::Suggest, workspace_id.clone());
    app.current_session_id = Some(active_session_id.clone());
    app.state.sessions = app_sessions;

    // Load the initial session projection and trace
    if let Ok(projection) = runtime
        .get_session_projection(active_session_id.clone())
        .await
    {
        app.state.current_session = projection;
    }
    if let Ok(trace) = runtime.get_trace(active_session_id.clone()).await {
        app.domain_events = trace.into_iter().map(|t| t.event).collect();
    }

    // Select the current session in the sessions panel
    if !app.state.sessions.is_empty() {
        let selected = app
            .state
            .sessions
            .iter()
            .position(|session| session.id == active_session_id)
            .unwrap_or_else(|| app.state.sessions.len() - 1);
        app.sessions.state.select(Some(selected));
    }

    app.sync_status_bar();
    app.sync_component_focus();

    // 5. Create channels + spawn tasks
    let (tx, mut rx) = mpsc::channel::<AppEvent>(256);

    // Domain event forwarder — subscribes to ALL runtime events
    let tx_events = tx.clone();
    let rt_handle = runtime.clone();
    let event_task = tokio::spawn(async move {
        let mut stream = rt_handle.subscribe_all();
        while let Some(event) = stream.next().await {
            if tx_events
                .send(AppEvent::DomainEvent(Box::new(event)))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Key reader — forwards crossterm key events
    let tx_keys = tx.clone();
    let key_task = tokio::spawn(async move {
        let mut reader = EventStream::new();
        while let Some(Ok(event)) = reader.next().await {
            if let Event::Key(key) = event {
                if tx_keys.send(AppEvent::Key(key)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Tick timer — fires every 16ms for render scheduling
    let tx_tick = tx;
    let tick_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(16));
        loop {
            interval.tick().await;
            if tx_tick.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // 6. Main loop
    loop {
        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Key(key) => {
                    let crossterm_event = Event::Key(key);
                    let commands = app.handle_crossterm_event(&crossterm_event);
                    dispatch_commands(&runtime, &mut app, commands).await;
                }
                AppEvent::DomainEvent(domain_event) => {
                    // Only process events for the current session
                    if let Some(ref sid) = app.current_session_id {
                        if domain_event.session_id == *sid {
                            app.handle_domain_event(&domain_event);

                            // Drain any messages the user queued while the
                            // session was busy. We drain on
                            // `AssistantMessageCompleted` to mirror the GUI
                            // "end-of-turn" signal — the runtime is ready to
                            // accept the next user turn at that point.
                            if matches!(
                                domain_event.payload,
                                agent_core::EventPayload::AssistantMessageCompleted { .. }
                            ) {
                                let queued = app.chat.drain_queue();
                                if !queued.is_empty() {
                                    if let Some(session_id) = app.current_session_id.clone() {
                                        let workspace_id = app.workspace_id.clone();
                                        let drain_cmds: Vec<Command> = queued
                                            .into_iter()
                                            .map(|q| Command::SendMessage {
                                                workspace_id: workspace_id.clone(),
                                                session_id: session_id.clone(),
                                                content: q.content,
                                                attachments: q.attachments,
                                            })
                                            .collect();
                                        dispatch_commands(&runtime, &mut app, drain_cmds).await;
                                    }
                                }
                            }
                        }
                    }
                }
                AppEvent::Tick => {
                    if app.state.render_scheduler.should_render() {
                        terminal.draw(|f| app.render(f))?;
                    }
                }
            }

            if app.quitting {
                break;
            }
        }
    }

    // 7. Cleanup
    event_task.abort();
    key_task.abort();
    tick_task.abort();

    drop(rx);

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
