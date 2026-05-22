use std::sync::Arc;
use std::time::Duration;

use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app;
use crate::app::App;
use crate::components::{
    Command, CrossPanelEffect, McpConnectivityEntry, McpPromptEntry, McpResourceEntry,
    McpServerEntry, McpServerStatusView, McpToolEntry,
};

use super::{push_status_error, push_status_message};

pub(crate) async fn dispatch(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    command: Command,
) {
    match command {
        Command::OpenMcpOverlay => {
            refresh_mcp_overlay(runtime, app).await;
            app.state.render_scheduler.mark_dirty_immediate();
        }

        Command::TrustMcpServer { server_id } => {
            if let Some(mcp_manager) = runtime.mcp_manager() {
                let manager = mcp_manager.lock().await;
                let result = manager.trust_server(&server_id).await;
                drop(manager);
                if let Err(e) = result {
                    push_status_message(app, format!("[MCP trust error: {e}]"));
                } else {
                    push_status_message(app, format!("MCP server '{}' is now trusted", server_id));
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
                    push_status_message(app, format!("MCP server '{}' trust revoked", server_id));
                    refresh_mcp_overlay(runtime, app).await;
                }
            }
        }

        Command::StartMcpServer { server_id } => {
            if let Some(mcp_manager) = runtime.mcp_manager() {
                let mut manager = mcp_manager.lock().await;
                match manager.ensure_server(&server_id).await {
                    Ok(_) => {
                        push_status_message(app, format!("MCP server '{}' started", server_id));
                    }
                    Err(e) => {
                        push_status_error(app, format!("[MCP start error: {e}]"));
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
                        push_status_message(app, format!("MCP server '{}' stopped", server_id));
                    }
                    Err(e) => {
                        push_status_error(app, format!("[MCP stop error: {e}]"));
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
                        push_status_message(app, format!("MCP server '{}' refreshed", server_id));
                        refresh_mcp_overlay(runtime, app).await;
                    }
                    Err(e) => {
                        drop(manager);
                        push_status_message(app, format!("[MCP refresh error: {e}]"));
                    }
                }
            }
        }

        Command::CheckMcpHealth { server_id } => match runtime.check_mcp_health(&server_id).await {
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
                        format!("MCP server '{}' healthy ({} tools)", server_id, tool_count),
                    );
                } else {
                    let reason = error.unwrap_or_else(|| "unknown error".to_string());
                    push_status_message(app, format!("[MCP health error: {server_id}: {reason}]"));
                }
            }
            Err(e) => {
                push_status_message(app, format!("[MCP health error: {e}]"));
            }
        },

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
        } => match runtime
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
                            tools: mcp_tool_entries(&server_id, result.tools, &disabled_tools),
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
        },

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
                        push_status_message(app, format!("MCP resource '{}'\n{}", uri, preview));
                    }
                    Err(e) => {
                        push_status_message(app, format!("[MCP resource read error: {e}]"));
                    }
                }
            }
        }

        _ => {}
    }
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

/// Snapshot the runtime's MCP manager into a `Vec<McpServerEntry>` and
/// dispatch a `ShowMcpOverlay` effect so the overlay component re-renders.
///
/// Read-only over `McpServerManager`: status, trust, and tool counts are
/// captured without starting or stopping servers. If the runtime has no MCP
/// manager configured the overlay opens with an empty list.
pub(crate) async fn refresh_mcp_overlay(
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
