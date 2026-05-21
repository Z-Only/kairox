use agent_core::facade::{McpFacade, PluginsFacade, SkillCatalogQuery, SkillInstallTarget};
use agent_core::projection::{ProjectedMessage, ProjectedRole};
use agent_core::{ActivateSkillRequest, AppFacade, DeactivateSkillRequest};

use super::App;
use crate::components::{
    Command, CrossPanelEffect, McpOverlaySnapshot, McpServerEntry, PluginOverlaySnapshot,
    SkillEntry, SkillOverlaySnapshot,
};

pub async fn dispatch_commands<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    commands: Vec<Command>,
) where
    F: AppFacade + ?Sized,
{
    for command in commands {
        match command {
            Command::OpenMcpOverlay => {
                refresh_mcp_overlay(runtime, app, Vec::new()).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenSkillsOverlay => {
                refresh_skills_overlay(runtime, app, None).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenPluginsOverlay => {
                refresh_plugins_overlay(runtime, app).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::ListSkills if app.skills_overlay.is_visible() => {
                refresh_skills_overlay(runtime, app, None).await;
            }
            Command::ListSkills => match AppFacade::list_skills(runtime.as_ref()).await {
                Ok(skills) if skills.is_empty() => {
                    push_status_message(app, "No skills discovered".to_string());
                }
                Ok(skills) => {
                    let skill_lines = skills
                        .iter()
                        .map(|skill| format!("- {}: {}", skill.id, skill.description))
                        .collect::<Vec<_>>()
                        .join("\n");
                    push_status_message(app, format!("Available skills:\n{skill_lines}"));
                }
                Err(error) => {
                    push_status_message(app, format!("[skills error: {error}]"));
                }
            },
            Command::ShowSkill { skill_id } => {
                match AppFacade::get_skill(runtime.as_ref(), skill_id.clone()).await {
                    Ok(Some(skill)) => {
                        if app.skills_overlay.is_visible() {
                            app.dispatch_effects(vec![CrossPanelEffect::ShowSkillBody {
                                skill_id: skill.view.id.clone(),
                                body: skill.body_markdown.clone(),
                            }]);
                            app.state.render_scheduler.mark_dirty();
                        } else {
                            push_status_message(
                                app,
                                format!(
                                    "Skill {}\n{}\n\n{}",
                                    skill.view.id, skill.view.description, skill.body_markdown
                                ),
                            );
                        }
                    }
                    Ok(None) => {
                        push_status_message(app, format!("[skill not found: {skill_id}]"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill show error: {error}]"));
                    }
                }
            }
            Command::ActivateSkill {
                workspace_id,
                session_id,
                skill_id,
            } => {
                let request = ActivateSkillRequest {
                    workspace_id,
                    session_id,
                    skill_id: skill_id.clone(),
                };
                match AppFacade::activate_skill(runtime.as_ref(), request).await {
                    Ok(active_skill) => {
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app, None).await;
                        } else {
                            push_status_message(
                                app,
                                format!("activated {}", active_skill.skill_id),
                            );
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill activate error: {error}]"));
                    }
                }
            }
            Command::DeactivateSkill {
                workspace_id,
                session_id,
                skill_id,
            } => {
                let request = DeactivateSkillRequest {
                    workspace_id,
                    session_id,
                    skill_id: skill_id.clone(),
                };
                match AppFacade::deactivate_skill(runtime.as_ref(), request).await {
                    Ok(()) => {
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app, None).await;
                        } else {
                            push_status_message(app, format!("deactivated {skill_id}"));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill deactivate error: {error}]"));
                    }
                }
            }
            Command::ListSkillCatalog { keyword } => {
                let query = SkillCatalogQuery {
                    keyword: keyword.clone(),
                    sources: None,
                    limit: Some(50),
                };
                match AppFacade::list_skill_catalog(runtime.as_ref(), query).await {
                    Ok(entries) if entries.is_empty() => {
                        let suffix = keyword
                            .as_deref()
                            .map(|value| format!(" for {value}"))
                            .unwrap_or_default();
                        push_status_message(app, format!("No catalog skills found{suffix}"));
                    }
                    Ok(entries) => {
                        let skill_lines = entries
                            .iter()
                            .map(|entry| {
                                format!(
                                    "- {}: {} [{}]",
                                    entry.name, entry.description, entry.source
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        push_status_message(app, format!("Catalog skills:\n{skill_lines}"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill catalog error: {error}]"));
                    }
                }
            }
            Command::InstallRemoteSkill { request } => {
                match AppFacade::install_remote_skill(runtime.as_ref(), request.clone()).await {
                    Ok(skill) => {
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app, None).await;
                        } else {
                            push_status_message(app, format!("installed skill {}", skill.id));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill install error: {error}]"));
                    }
                }
            }
            Command::InstallGithubSkill { request } => {
                match AppFacade::install_github_skill(runtime.as_ref(), request.clone()).await {
                    Ok(skill) => {
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app, None).await;
                        } else {
                            push_status_message(app, format!("installed skill {}", skill.id));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill install error: {error}]"));
                    }
                }
            }
            Command::UpdateSkillSettings { skill_id } => {
                match AppFacade::update_skill(runtime.as_ref(), skill_id.clone()).await {
                    Ok(skill) => {
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app, None).await;
                        } else {
                            push_status_message(app, format!("updated skill {}", skill.id));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill update error: {error}]"));
                    }
                }
            }
            Command::DeleteSkillSettings { skill_id } => {
                match AppFacade::delete_skill_settings(runtime.as_ref(), skill_id.clone()).await {
                    Ok(()) => {
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app, None).await;
                        } else {
                            push_status_message(app, format!("deleted skill {skill_id}"));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill delete error: {error}]"));
                    }
                }
            }
            Command::SetSkillEnabled { skill_id, enabled } => {
                match AppFacade::set_skill_enabled(runtime.as_ref(), skill_id.clone(), enabled)
                    .await
                {
                    Ok(()) => {
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app, None).await;
                        } else {
                            let state = if enabled { "enabled" } else { "disabled" };
                            push_status_message(app, format!("{state} skill {skill_id}"));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill enable error: {error}]"));
                    }
                }
            }
            Command::SetSkillSourceEnabled { source_id, enabled } => {
                match AppFacade::set_skill_source_enabled(
                    runtime.as_ref(),
                    source_id.clone(),
                    enabled,
                )
                .await
                {
                    Ok(()) => {
                        refresh_skills_overlay(runtime, app, None).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill source error: {error}]"));
                    }
                }
            }
            Command::RefreshSkillCatalog => {
                match AppFacade::refresh_skill_catalog(runtime.as_ref()).await {
                    Ok(()) => {
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app, None).await;
                        } else {
                            push_status_message(app, "refreshed skill catalog".to_string());
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill catalog refresh error: {error}]"));
                    }
                }
            }
            Command::SetPluginEnabled {
                settings_id,
                enabled,
            } => {
                match PluginsFacade::set_plugin_enabled(
                    runtime.as_ref(),
                    settings_id.clone(),
                    enabled,
                )
                .await
                {
                    Ok(()) => {
                        if app.plugin_overlay.is_visible() {
                            refresh_plugins_overlay(runtime, app).await;
                        } else {
                            let state = if enabled { "enabled" } else { "disabled" };
                            push_status_message(app, format!("{state} plugin {settings_id}"));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[plugin enable error: {error}]"));
                    }
                }
            }
            Command::DeletePluginSettings { settings_id } => {
                match PluginsFacade::delete_plugin_settings(runtime.as_ref(), settings_id.clone())
                    .await
                {
                    Ok(()) => {
                        if app.plugin_overlay.is_visible() {
                            refresh_plugins_overlay(runtime, app).await;
                        } else {
                            push_status_message(app, format!("deleted plugin {settings_id}"));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[plugin delete error: {error}]"));
                    }
                }
            }
            Command::SetPluginMarketplaceSourceEnabled { source_id, enabled } => {
                match PluginsFacade::set_plugin_marketplace_source_enabled(
                    runtime.as_ref(),
                    source_id.clone(),
                    enabled,
                )
                .await
                {
                    Ok(()) => {
                        refresh_plugins_overlay(runtime, app).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[plugin source error: {error}]"));
                    }
                }
            }
            Command::InstallPlugin { request } => {
                match PluginsFacade::install_plugin(runtime.as_ref(), request.clone()).await {
                    Ok(plugin) => {
                        if app.plugin_overlay.is_visible() {
                            refresh_plugins_overlay(runtime, app).await;
                        } else {
                            push_status_message(app, format!("installed plugin {}", plugin.id));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[plugin install error: {error}]"));
                    }
                }
            }
            Command::SetMcpServerEnabled { server_id, enabled } => {
                match McpFacade::set_mcp_server_enabled(
                    runtime.as_ref(),
                    server_id.clone(),
                    enabled,
                )
                .await
                {
                    Ok(()) => {
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP settings error: {error}]"));
                    }
                }
            }
            Command::DeleteMcpServerSettings { server_id } => {
                match McpFacade::delete_mcp_server_settings(runtime.as_ref(), server_id.clone())
                    .await
                {
                    Ok(()) => {
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP delete error: {error}]"));
                    }
                }
            }
            Command::InstallMcpServer { request } => {
                match McpFacade::install_catalog_entry(runtime.as_ref(), request.clone()).await {
                    Ok(outcome) => {
                        if !app.mcp_overlay.is_visible() {
                            push_status_message(
                                app,
                                format!("MCP install {} {:?}", outcome.kind, outcome.server_id),
                            );
                        }
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP install error: {error}]"));
                    }
                }
            }
            Command::UninstallMcpServer { server_id } => {
                match McpFacade::uninstall_catalog_entry(runtime.as_ref(), server_id.clone()).await
                {
                    Ok(()) => {
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP uninstall error: {error}]"));
                    }
                }
            }
            Command::SetMcpCatalogSourceEnabled { source_id, enabled } => {
                match McpFacade::set_catalog_source_enabled(
                    runtime.as_ref(),
                    source_id.clone(),
                    enabled,
                )
                .await
                {
                    Ok(()) => {
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP source error: {error}]"));
                    }
                }
            }
            _ => {}
        }
    }
}

fn push_status_message(app: &mut App, content: String) {
    app.state.current_session.messages.push(ProjectedMessage {
        role: ProjectedRole::Assistant,
        content,
    });
    app.state.render_scheduler.mark_dirty();
}

async fn refresh_skills_overlay<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    catalog_keyword: Option<String>,
) where
    F: AppFacade + ?Sized,
{
    let skills = match AppFacade::list_skills(runtime.as_ref()).await {
        Ok(skills) => skills,
        Err(error) => {
            push_status_message(app, format!("[skills error: {error}]"));
            return;
        }
    };

    let active_ids: std::collections::HashSet<String> =
        if let Some(session_id) = app.current_session_id.clone() {
            match AppFacade::list_active_skills(runtime.as_ref(), session_id).await {
                Ok(list) => list.into_iter().map(|a| a.skill_id).collect(),
                Err(error) => {
                    push_status_message(app, format!("[active skills error: {error}]"));
                    std::collections::HashSet::new()
                }
            }
        } else {
            std::collections::HashSet::new()
        };

    let entries: Vec<SkillEntry> = skills
        .into_iter()
        .map(|s| {
            let active = active_ids.contains(&s.id);
            SkillEntry {
                id: s.id,
                name: s.name,
                description: s.description,
                source: s.source,
                activation_mode: s.activation_mode,
                active,
            }
        })
        .collect();

    let installed = match AppFacade::list_skill_settings(runtime.as_ref()).await {
        Ok(installed) => installed,
        Err(error) => {
            push_status_message(app, format!("[skill settings error: {error}]"));
            Vec::new()
        }
    };

    let catalog = match AppFacade::list_skill_catalog(
        runtime.as_ref(),
        SkillCatalogQuery {
            keyword: catalog_keyword,
            sources: None,
            limit: Some(50),
        },
    )
    .await
    {
        Ok(catalog) => catalog,
        Err(error) => {
            push_status_message(app, format!("[skill catalog error: {error}]"));
            Vec::new()
        }
    };

    let sources = match AppFacade::list_skill_sources(runtime.as_ref()).await {
        Ok(sources) => sources,
        Err(error) => {
            push_status_message(app, format!("[skill sources error: {error}]"));
            Vec::new()
        }
    };

    app.dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
        SkillOverlaySnapshot {
            discovered: entries,
            installed,
            catalog,
            sources,
            install_target: SkillInstallTarget::User,
        },
    )]);
}

pub async fn refresh_mcp_overlay<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    runtime_servers: Vec<McpServerEntry>,
) where
    F: AppFacade + ?Sized,
{
    let settings = match McpFacade::list_mcp_server_settings(runtime.as_ref(), None).await {
        Ok(settings) => settings,
        Err(error) => {
            push_status_message(app, format!("[MCP settings error: {error}]"));
            Vec::new()
        }
    };

    let installed = match McpFacade::list_installed_entries(runtime.as_ref()).await {
        Ok(installed) => installed,
        Err(error) => {
            push_status_message(app, format!("[MCP installed error: {error}]"));
            Vec::new()
        }
    };

    let catalog = match McpFacade::list_catalog(
        runtime.as_ref(),
        agent_core::facade::CatalogQuery {
            limit: Some(100),
            ..Default::default()
        },
    )
    .await
    {
        Ok(catalog) => catalog,
        Err(error) => {
            push_status_message(app, format!("[MCP catalog error: {error}]"));
            Vec::new()
        }
    };

    let sources = match McpFacade::list_catalog_sources(runtime.as_ref()).await {
        Ok(sources) => sources,
        Err(error) => {
            push_status_message(app, format!("[MCP sources error: {error}]"));
            Vec::new()
        }
    };

    app.dispatch_effects(vec![CrossPanelEffect::ShowMcpOverlay(McpOverlaySnapshot {
        runtime_servers,
        settings,
        installed,
        catalog,
        sources,
    })]);
}

async fn refresh_plugins_overlay<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    let plugins = match PluginsFacade::list_plugin_settings(runtime.as_ref()).await {
        Ok(plugins) => plugins,
        Err(error) => {
            push_status_message(app, format!("[plugins error: {error}]"));
            return;
        }
    };

    let sources = match PluginsFacade::list_plugin_marketplace_sources(runtime.as_ref()).await {
        Ok(sources) => sources,
        Err(error) => {
            push_status_message(app, format!("[plugin sources error: {error}]"));
            Vec::new()
        }
    };

    let catalog = match PluginsFacade::list_plugin_catalog(runtime.as_ref(), None, None).await {
        Ok(catalog) => catalog,
        Err(error) => {
            push_status_message(app, format!("[plugin catalog error: {error}]"));
            Vec::new()
        }
    };

    app.dispatch_effects(vec![CrossPanelEffect::ShowPluginsOverlay(
        PluginOverlaySnapshot {
            plugins,
            catalog,
            sources,
            install_target: agent_core::facade::PluginInstallTarget::User,
        },
    )]);
}
