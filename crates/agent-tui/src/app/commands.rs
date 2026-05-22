use agent_core::facade::{
    HookSettingsInput, HooksSettingsView, InstructionsUpdateInput, McpFacade, PluginsFacade,
    ProjectFacade, SkillCatalogQuery, SkillInstallTarget,
};
use agent_core::{
    ActivateSkillRequest, AppFacade, DeactivateSkillRequest, ProjectGitStatus,
    ProjectGitStatusKind, ProjectInstructionSummary, ProjectMeta,
};

use super::App;
use crate::app_state::SettingsConfigSource;
use crate::components::{
    AgentOverlaySnapshot, Command, CrossPanelEffect, McpOverlaySnapshot, McpServerEntry,
    ModelOverlaySnapshot, ModelProfileEntry, ModelProfileTestResult, PluginOverlaySnapshot,
    ProjectInfo, SkillEntry, SkillOverlaySnapshot,
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
            Command::SaveDraft { .. } => {}
            Command::OpenMcpOverlay => {
                refresh_mcp_overlay(runtime, app, Vec::new()).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenModelOverlay => {
                refresh_model_overlay(runtime, app).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenSkillsOverlay => {
                refresh_skills_overlay(runtime, app, None, None).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenPluginsOverlay => {
                refresh_plugins_overlay(runtime, app).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenInstructionsOverlay => {
                refresh_instructions_overlay(app);
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenSystemPromptOverlay => {
                refresh_system_prompt_overlay(app);
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenHooksOverlay => {
                refresh_hooks_overlay(app);
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::SetSettingsConfigSource { source } => {
                app.state.set_settings_config_source(source);
                let detail = match source {
                    SettingsConfigSource::User => "user config".to_string(),
                    SettingsConfigSource::Project => app
                        .state
                        .selected_settings_project()
                        .map(|project| format!("project config {}", project.display_name))
                        .unwrap_or_else(|| "project config".to_string()),
                };
                push_status_message(app, format!("settings source: {detail}"));
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::CycleSettingsProject { direction } => {
                app.state
                    .set_settings_config_source(SettingsConfigSource::Project);
                match cycle_settings_project(app, direction) {
                    Some(project) => {
                        let display_name = project.display_name.clone();
                        app.state.select_settings_project(project.id);
                        push_status_message(app, format!("settings project: {display_name}"));
                    }
                    None => {
                        push_status_message(app, "settings project unavailable".to_string());
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::SaveHookSettings { input } => {
                match save_hook_settings(app, input.clone()) {
                    Ok(()) => {
                        push_status_message(
                            app,
                            format!("saved hook {}.{}", input.event, input.id),
                        );
                        if app.hooks_overlay.is_visible() {
                            refresh_hooks_overlay(app);
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[hooks save error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::DeleteHookSettings { scope, event, id } => {
                match delete_hook_settings(app, scope, event.clone(), id.clone()) {
                    Ok(()) => {
                        push_status_message(app, format!("deleted hook {event}.{id}"));
                        if app.hooks_overlay.is_visible() {
                            refresh_hooks_overlay(app);
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[hooks delete error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenAgentSettingsOverlay => {
                refresh_agent_overlay(runtime, app).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::SaveAgentSettings { input } => {
                match AppFacade::upsert_agent_settings(runtime.as_ref(), input.clone()).await {
                    Ok(view) => {
                        push_status_message(app, format!("saved agent {}", view.name));
                        if app.agent_overlay.is_visible() {
                            refresh_agent_overlay(runtime, app).await;
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[agent save error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::DeleteAgentSettings { settings_id } => {
                match AppFacade::delete_agent_settings(runtime.as_ref(), settings_id.clone()).await
                {
                    Ok(()) => {
                        push_status_message(app, format!("deleted agent {settings_id}"));
                        if app.agent_overlay.is_visible() {
                            refresh_agent_overlay(runtime, app).await;
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[agent delete error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::CopyAgentSettings { settings_id, scope } => {
                match AppFacade::copy_agent_settings(runtime.as_ref(), settings_id.clone(), scope)
                    .await
                {
                    Ok(view) => {
                        push_status_message(app, format!("copied agent {}", view.name));
                        if app.agent_overlay.is_visible() {
                            refresh_agent_overlay(runtime, app).await;
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[agent copy error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenConfigDir => {
                match AppFacade::open_config_dir(runtime.as_ref()).await {
                    Ok(Some(path)) => {
                        open_directory_path(app, &path, "config dir");
                    }
                    Ok(None) => {
                        push_status_message(app, "config dir path unavailable".to_string());
                    }
                    Err(error) => {
                        push_status_message(app, format!("[config dir error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenAgentsDir => {
                match AppFacade::open_agents_dir(runtime.as_ref()).await {
                    Ok(Some(path)) => {
                        open_directory_path(app, &path, "agents dir");
                    }
                    Ok(None) => {
                        push_status_message(app, "agents dir path unavailable".to_string());
                    }
                    Err(error) => {
                        push_status_message(app, format!("[agents dir error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::OpenSkillsDir => {
                match AppFacade::open_skills_dir(runtime.as_ref()).await {
                    Ok(Some(path)) => {
                        open_directory_path(app, &path, "skills dir");
                    }
                    Ok(None) => {
                        push_status_message(app, "skills dir path unavailable".to_string());
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skills dir error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::SaveInstructions { scope, text } => {
                match save_instructions(app, scope, text) {
                    Ok(()) => {
                        refresh_instructions_overlay(app);
                    }
                    Err(error) => {
                        push_status_message(app, format!("[instructions error: {error}]"));
                    }
                }
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::ListSkills if app.skills_overlay.is_visible() => {
                refresh_skills_overlay_with_current_query(runtime, app).await;
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
                            refresh_skills_overlay_with_current_query(runtime, app).await;
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
                            refresh_skills_overlay_with_current_query(runtime, app).await;
                        } else {
                            push_status_message(app, format!("deactivated {skill_id}"));
                        }
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill deactivate error: {error}]"));
                    }
                }
            }
            Command::ListSkillCatalog { keyword, sources } if app.skills_overlay.is_visible() => {
                refresh_skills_overlay(runtime, app, keyword, sources).await;
            }
            Command::ListSkillCatalog { keyword, sources } => {
                let query = SkillCatalogQuery {
                    keyword: keyword.clone(),
                    sources: sources.clone(),
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
                            refresh_skills_overlay_with_current_query(runtime, app).await;
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
                            refresh_skills_overlay_with_current_query(runtime, app).await;
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
                            refresh_skills_overlay_with_current_query(runtime, app).await;
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
                            refresh_skills_overlay_with_current_query(runtime, app).await;
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
                            refresh_skills_overlay_with_current_query(runtime, app).await;
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
                    Ok(()) => refresh_skills_overlay_with_current_query(runtime, app).await,
                    Err(error) => {
                        push_status_message(app, format!("[skill source error: {error}]"));
                    }
                }
            }
            Command::AddSkillSource { config } => {
                match AppFacade::add_skill_source(runtime.as_ref(), config.clone()).await {
                    Ok(()) => {
                        push_status_message(app, format!("added skill source {}", config.id));
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill source add error: {error}]"));
                    }
                }
            }
            Command::RemoveSkillSource { source_id } => {
                match AppFacade::remove_skill_source(runtime.as_ref(), source_id.clone()).await {
                    Ok(()) => {
                        push_status_message(app, format!("removed skill source {source_id}"));
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill source remove error: {error}]"));
                    }
                }
            }
            Command::RefreshSkillCatalog { keyword, sources } => {
                match AppFacade::refresh_skill_catalog(runtime.as_ref()).await {
                    Ok(()) => {
                        if app.skills_overlay.is_visible() {
                            let (catalog_keyword, catalog_sources) =
                                if keyword.is_none() && sources.is_none() {
                                    app.skills_overlay.catalog_query()
                                } else {
                                    (keyword, sources)
                                };
                            refresh_skills_overlay(runtime, app, catalog_keyword, catalog_sources)
                                .await;
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
                match set_mcp_server_enabled_for_selected_source(
                    runtime,
                    app,
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
            Command::SaveMcpServerSettings { input } => {
                match upsert_mcp_server_for_selected_source(runtime, app, input.clone()).await {
                    Ok(view) => {
                        push_status_message(app, format!("saved MCP server {}", view.id));
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP save error: {error}]"));
                    }
                }
            }
            Command::DeleteMcpServerSettings { server_id } => {
                match delete_mcp_server_for_selected_source(runtime, app, server_id.clone()).await {
                    Ok(()) => {
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP delete error: {error}]"));
                    }
                }
            }
            Command::OpenMcpConfig => {
                match McpFacade::open_mcp_config_file(runtime.as_ref()).await {
                    Ok(Some(path)) => {
                        let path_buf = std::path::PathBuf::from(&path);
                        match open_path_in_system_file_manager(&path_buf) {
                            Ok(()) => {
                                push_status_message(
                                    app,
                                    format!("opened MCP config {}", path_buf.display()),
                                );
                            }
                            Err(error) => {
                                push_status_message(
                                    app,
                                    format!("[MCP config open error: {error}]"),
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        push_status_message(app, "MCP config path unavailable".to_string());
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP config error: {error}]"));
                    }
                }
            }
            Command::DisableMcpServerAtScope { server_id } => {
                match selected_project_config_path(app)
                    .and_then(|path| apply_mcp_scope_disabled(&path, &server_id, true))
                {
                    Ok(()) => {
                        push_status_message(
                            app,
                            format!("disabled MCP server {server_id} in project"),
                        );
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP project disable error: {error}]"));
                    }
                }
            }
            Command::EnableMcpServerAtScope { server_id } => {
                match selected_project_config_path(app)
                    .and_then(|path| apply_mcp_scope_disabled(&path, &server_id, false))
                {
                    Ok(()) => {
                        push_status_message(
                            app,
                            format!("enabled MCP server {server_id} in project"),
                        );
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP project enable error: {error}]"));
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
            Command::AddMcpCatalogSource { request } => {
                match McpFacade::add_catalog_source(runtime.as_ref(), request.clone()).await {
                    Ok(()) => {
                        push_status_message(app, format!("added MCP source {}", request.id));
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP source add error: {error}]"));
                    }
                }
            }
            Command::RemoveMcpCatalogSource { source_id } => {
                match McpFacade::remove_catalog_source(runtime.as_ref(), source_id.clone()).await {
                    Ok(()) => {
                        push_status_message(app, format!("removed MCP source {source_id}"));
                        refresh_mcp_overlay(runtime, app, Vec::new()).await;
                    }
                    Err(error) => {
                        push_status_message(app, format!("[MCP source remove error: {error}]"));
                    }
                }
            }
            Command::SetProfileEnabled { alias, enabled } => {
                match set_profile_enabled_for_selected_source(runtime, app, alias.clone(), enabled)
                    .await
                {
                    Ok(()) => {
                        let state = if enabled { "enabled" } else { "disabled" };
                        push_status_message(app, format!("{state} model profile {alias}"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[model profile enable error: {error}]"));
                    }
                }
            }
            Command::SaveProfileSettings { input } => {
                let alias = input.alias.clone();
                match upsert_profile_for_selected_source(runtime, app, input).await {
                    Ok(()) => {
                        push_status_message(app, format!("saved model profile {alias}"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[model profile save error: {error}]"));
                    }
                }
            }
            Command::DeleteProfileSettings { alias } => {
                match delete_profile_for_selected_source(runtime, app, alias.clone()).await {
                    Ok(()) => {
                        push_status_message(app, format!("deleted model profile {alias}"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[model profile delete error: {error}]"));
                    }
                }
            }
            Command::MoveProfileInOrder { alias, direction } => {
                match move_profile_in_selected_source(runtime, app, alias.clone(), direction).await
                {
                    Ok(()) => {
                        push_status_message(app, format!("moved model profile {alias}"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[model profile order error: {error}]"));
                    }
                }
            }
            Command::TestModelProfile { alias } => {
                match test_model_connectivity(runtime, app, alias.clone()).await {
                    Ok(result) => {
                        let message = if result.ok {
                            format!("model profile {alias} connectivity ok")
                        } else {
                            format!(
                                "model profile {alias} connectivity failed: {}",
                                result
                                    .message
                                    .as_deref()
                                    .unwrap_or("unknown connectivity error")
                            )
                        };
                        app.dispatch_effects(vec![CrossPanelEffect::ModelProfileTested(result)]);
                        push_status_message(app, message);
                    }
                    Err(error) => {
                        let result = ModelProfileTestResult {
                            alias: alias.clone(),
                            ok: false,
                            message: Some(error.to_string()),
                        };
                        app.dispatch_effects(vec![CrossPanelEffect::ModelProfileTested(result)]);
                        push_status_message(app, format!("[model profile test error: {error}]"));
                    }
                }
            }
            Command::TestModelProfileUrl { alias, base_url } => {
                let result = test_model_base_url_connectivity(alias.clone(), base_url).await;
                let message = if result.ok {
                    format!("model profile {alias} connectivity ok")
                } else {
                    format!(
                        "model profile {alias} connectivity failed: {}",
                        result
                            .message
                            .as_deref()
                            .unwrap_or("unknown connectivity error")
                    )
                };
                app.dispatch_effects(vec![CrossPanelEffect::ModelProfileTested(result)]);
                push_status_message(app, message);
            }
            Command::OpenProfilesConfig => {
                match AppFacade::open_profiles_config_file(runtime.as_ref()).await {
                    Ok(Some(path)) => {
                        let path_buf = std::path::PathBuf::from(&path);
                        match open_path_in_system_file_manager(&path_buf) {
                            Ok(()) => {
                                push_status_message(
                                    app,
                                    format!("opened profiles config {}", path_buf.display()),
                                );
                            }
                            Err(error) => {
                                push_status_message(
                                    app,
                                    format!("[profiles config open error: {error}]"),
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        push_status_message(app, "profiles config path unavailable".to_string());
                    }
                    Err(error) => {
                        push_status_message(app, format!("[profiles config error: {error}]"));
                    }
                }
            }
            Command::CreateBlankProject { display_name } => {
                match ProjectFacade::create_blank_project(
                    runtime.as_ref(),
                    app.workspace_id.clone(),
                    display_name.clone(),
                )
                .await
                {
                    Ok(project) => {
                        let project_info = project_info_from_meta(project.clone());
                        upsert_project(app, project_info);
                        refresh_project_status(runtime, app, project.project_id.clone()).await;
                        push_status_message(
                            app,
                            format!("created project {}", project.display_name),
                        );
                    }
                    Err(error) => {
                        push_status_message(app, format!("[project create error: {error}]"));
                    }
                }
            }
            Command::AddExistingProject { path } => {
                match ProjectFacade::add_existing_project(
                    runtime.as_ref(),
                    app.workspace_id.clone(),
                    path.clone(),
                )
                .await
                {
                    Ok(project) => {
                        let project_info = project_info_from_meta(project.clone());
                        upsert_project(app, project_info);
                        refresh_project_status(runtime, app, project.project_id.clone()).await;
                        push_status_message(
                            app,
                            format!("imported project {}", project.display_name),
                        );
                    }
                    Err(error) => {
                        push_status_message(app, format!("[project import error: {error}]"));
                    }
                }
            }
            Command::RenameProject {
                project_id,
                display_name,
            } => {
                match ProjectFacade::rename_project(
                    runtime.as_ref(),
                    project_id.clone(),
                    display_name.clone(),
                )
                .await
                {
                    Ok(()) => {
                        if let Some(project) = app
                            .state
                            .projects
                            .iter_mut()
                            .find(|project| project.id == project_id)
                        {
                            project.display_name = display_name;
                        }
                        app.state.render_scheduler.mark_dirty();
                    }
                    Err(error) => {
                        push_status_message(app, format!("[project rename error: {error}]"));
                    }
                }
            }
            Command::RemoveProject { project_id } => {
                match ProjectFacade::remove_project(runtime.as_ref(), project_id.clone()).await {
                    Ok(()) => {
                        app.state
                            .projects
                            .retain(|project| project.id != project_id);
                        for session in &mut app.state.sessions {
                            if session.project_id.as_ref() == Some(&project_id) {
                                session.archived = true;
                                session.visibility =
                                    Some(agent_core::ProjectSessionVisibility::Archived);
                            }
                        }
                        app.state.render_scheduler.mark_dirty();
                    }
                    Err(error) => {
                        push_status_message(app, format!("[project remove error: {error}]"));
                    }
                }
            }
            Command::MoveProject {
                project_id,
                direction,
            } => {
                if let Some(project_ids) = reordered_project_ids(app, &project_id, direction) {
                    match ProjectFacade::update_project_order(runtime.as_ref(), project_ids.clone())
                        .await
                    {
                        Ok(()) => {
                            apply_project_order(app, &project_ids);
                            app.state.render_scheduler.mark_dirty();
                        }
                        Err(error) => {
                            push_status_message(app, format!("[project reorder error: {error}]"));
                        }
                    }
                }
            }
            Command::SetProjectExpanded {
                project_id,
                expanded,
            } => {
                match ProjectFacade::update_project_expanded(
                    runtime.as_ref(),
                    project_id.clone(),
                    expanded,
                )
                .await
                {
                    Ok(()) => {
                        if let Some(project) = app
                            .state
                            .projects
                            .iter_mut()
                            .find(|project| project.id == project_id)
                        {
                            project.expanded = expanded;
                        }
                        app.state.render_scheduler.mark_dirty();
                    }
                    Err(error) => {
                        push_status_message(app, format!("[project expanded error: {error}]"));
                    }
                }
            }
            Command::RefreshProjectGitStatus { project_id } => {
                match ProjectFacade::get_project_git_status(runtime.as_ref(), project_id.clone())
                    .await
                {
                    Ok(status) => {
                        set_project_status(app, &project_id, status.clone());
                        push_status_message(app, project_git_status_message(&status));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[project git status error: {error}]"));
                    }
                }
            }
            Command::InitProjectGit { project_id } => {
                match ProjectFacade::init_project_git(runtime.as_ref(), project_id.clone()).await {
                    Ok(status) => {
                        set_project_status(app, &project_id, status.clone());
                        push_status_message(app, project_git_status_message(&status));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[project git init error: {error}]"));
                    }
                }
            }
            Command::ShowProjectInstructions { project_id } => {
                match ProjectFacade::get_project_instruction_summary(
                    runtime.as_ref(),
                    project_id.clone(),
                )
                .await
                {
                    Ok(summary) => {
                        set_project_instruction_summary(app, &project_id, summary.clone());
                        push_status_message(app, project_instruction_message(&summary));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[project instructions error: {error}]"));
                    }
                }
            }
            _ => {}
        }
    }
}

fn push_status_message(app: &mut App, content: String) {
    if content.trim().is_empty() {
        return;
    }
    app.state.push_status_message(content);
    if let Some(entry) = app.state.latest_status_message() {
        app.status_bar.push_notification(entry.message.clone());
    }
    app.state.render_scheduler.mark_dirty();
}

fn project_info_from_meta(project: ProjectMeta) -> ProjectInfo {
    ProjectInfo {
        id: project.project_id,
        display_name: project.display_name,
        root_path: project.root_path,
        expanded: project.expanded,
        git_status: None,
        instruction_summary: None,
    }
}

fn upsert_project(app: &mut App, project: ProjectInfo) {
    if let Some(existing) = app
        .state
        .projects
        .iter_mut()
        .find(|existing| existing.id == project.id)
    {
        *existing = project;
    } else {
        app.state.projects.push(project);
    }
    app.state.render_scheduler.mark_dirty();
}

async fn refresh_project_status<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    project_id: agent_core::ProjectId,
) where
    F: AppFacade + ?Sized,
{
    if let Ok(status) =
        ProjectFacade::get_project_git_status(runtime.as_ref(), project_id.clone()).await
    {
        set_project_status(app, &project_id, status);
    }
}

fn set_project_status(app: &mut App, project_id: &agent_core::ProjectId, status: ProjectGitStatus) {
    if let Some(project) = app
        .state
        .projects
        .iter_mut()
        .find(|project| &project.id == project_id)
    {
        project.git_status = Some(status);
    }
    app.state.render_scheduler.mark_dirty();
}

fn set_project_instruction_summary(
    app: &mut App,
    project_id: &agent_core::ProjectId,
    summary: ProjectInstructionSummary,
) {
    if let Some(project) = app
        .state
        .projects
        .iter_mut()
        .find(|project| &project.id == project_id)
    {
        project.instruction_summary = Some(summary);
    }
    app.state.render_scheduler.mark_dirty();
}

fn reordered_project_ids(
    app: &mut App,
    project_id: &agent_core::ProjectId,
    direction: i32,
) -> Option<Vec<agent_core::ProjectId>> {
    let index = app
        .state
        .projects
        .iter()
        .position(|project| &project.id == project_id)?;
    if app.state.projects.is_empty() {
        return None;
    }
    let last = app.state.projects.len() - 1;
    let next = if direction < 0 {
        index.saturating_sub(1)
    } else if direction > 0 {
        (index + 1).min(last)
    } else {
        index
    };
    if next == index {
        return None;
    }
    let mut project_ids = app
        .state
        .projects
        .iter()
        .map(|project| project.id.clone())
        .collect::<Vec<_>>();
    project_ids.swap(index, next);
    Some(project_ids)
}

fn apply_project_order(app: &mut App, project_ids: &[agent_core::ProjectId]) {
    let mut next_projects = Vec::with_capacity(app.state.projects.len());
    for project_id in project_ids {
        if let Some(project) = app
            .state
            .projects
            .iter()
            .find(|project| &project.id == project_id)
            .cloned()
        {
            next_projects.push(project);
        }
    }
    next_projects.extend(
        app.state
            .projects
            .iter()
            .filter(|project| {
                !project_ids
                    .iter()
                    .any(|project_id| project_id == &project.id)
            })
            .cloned(),
    );
    app.state.projects = next_projects;
}

fn project_git_status_message(status: &ProjectGitStatus) -> String {
    let branch = status
        .branch
        .as_deref()
        .map(|branch| format!(" on {branch}"))
        .unwrap_or_default();
    let kind = match status.kind {
        ProjectGitStatusKind::NotInitialized => "not initialized",
        ProjectGitStatusKind::Clean => "clean",
        ProjectGitStatusKind::Dirty => "dirty",
        ProjectGitStatusKind::Detached => "detached",
        ProjectGitStatusKind::MissingPath => "missing path",
        ProjectGitStatusKind::Error => "error",
    };
    let mut message = format!("git status: {kind}{branch} ({})", status.worktree_path);
    if let Some(detail) = status
        .message
        .as_deref()
        .filter(|detail| !detail.is_empty())
    {
        message.push_str(&format!(": {detail}"));
    }
    message
}

fn project_instruction_message(summary: &ProjectInstructionSummary) -> String {
    let sources = if summary.source_paths.is_empty() {
        "no instruction files".to_string()
    } else {
        summary.source_paths.join(", ")
    };
    let mut message = format!("project instructions: {sources}");
    if let Some(warning) = summary
        .warning
        .as_deref()
        .filter(|warning| !warning.is_empty())
    {
        message.push_str(&format!("\nwarning: {warning}"));
    }
    if let Some(contents) = summary
        .contents
        .as_deref()
        .filter(|contents| !contents.is_empty())
    {
        let preview: String = contents.chars().take(4000).collect();
        message.push_str("\n\n");
        message.push_str(&preview);
        if contents.chars().count() > preview.chars().count() {
            message.push_str("\n\n[...truncated]");
        }
    }
    message
}

fn user_config_path() -> std::path::PathBuf {
    std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".kairox")
        .join("config.toml")
}

fn project_config_path() -> Result<std::path::PathBuf, String> {
    std::env::current_dir()
        .map(|root| root.join(".kairox").join("config.toml"))
        .map_err(|error| format!("failed to resolve project config path: {error}"))
}

fn selected_project_config_path(app: &App) -> Result<std::path::PathBuf, String> {
    app.state
        .selected_settings_project_config_path()
        .map(Ok)
        .unwrap_or_else(project_config_path)
}

fn selected_project_config_path_for_source(
    app: &App,
) -> Result<Option<std::path::PathBuf>, String> {
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        selected_project_config_path(app).map(Some)
    } else {
        Ok(None)
    }
}

fn selected_project_root_for_source(app: &App) -> Option<String> {
    app.state
        .selected_settings_project_root()
        .map(|root| root.display().to_string())
}

fn cycle_settings_project(app: &App, direction: i32) -> Option<ProjectInfo> {
    if app.state.projects.is_empty() {
        return None;
    }
    let current_index = app
        .state
        .selected_settings_project_id()
        .and_then(|project_id| {
            app.state
                .projects
                .iter()
                .position(|project| &project.id == project_id)
        })
        .unwrap_or(0);
    let last = app.state.projects.len() - 1;
    let next_index = if direction < 0 {
        if current_index == 0 {
            last
        } else {
            current_index - 1
        }
    } else if direction > 0 {
        if current_index >= last {
            0
        } else {
            current_index + 1
        }
    } else {
        current_index
    };
    app.state.projects.get(next_index).cloned()
}

fn apply_mcp_scope_disabled(
    config_path: &std::path::Path,
    server_id: &str,
    disabled: bool,
) -> Result<(), String> {
    let raw = match std::fs::read_to_string(config_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("failed to read project config: {error}")),
    };
    let mut doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|error| format!("failed to parse project config: {error}"))?;

    let mut ids = doc
        .get("disabled_mcp_servers")
        .and_then(|value| value.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if disabled {
        if !ids.iter().any(|id| id == server_id) {
            ids.push(server_id.to_string());
        }
    } else {
        ids.retain(|id| id != server_id);
    }
    ids.sort();

    if ids.is_empty() {
        doc.remove("disabled_mcp_servers");
    } else {
        let mut array = toml_edit::Array::new();
        for id in ids {
            array.push(id);
        }
        doc["disabled_mcp_servers"] = toml_edit::value(array);
    }

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create project config dir {}: {error}",
                parent.display()
            )
        })?;
    }
    std::fs::write(config_path, doc.to_string())
        .map_err(|error| format!("failed to write project config: {error}"))
}

fn read_disabled_mcp_scope(
    config_path: &std::path::Path,
) -> Result<std::collections::HashSet<String>, String> {
    let raw = match std::fs::read_to_string(config_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("failed to read project config: {error}")),
    };
    let doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|error| format!("failed to parse project config: {error}"))?;
    Ok(doc
        .get("disabled_mcp_servers")
        .and_then(|value| value.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

fn apply_project_disabled_scope(
    settings: &mut [agent_core::facade::McpServerSettingsView],
    disabled_ids: &std::collections::HashSet<String>,
) {
    for setting in settings {
        if disabled_ids.contains(&setting.id) {
            setting.enabled = false;
        }
    }
}

fn load_instructions_view(app: &App) -> Result<agent_core::facade::InstructionsView, String> {
    let user_config_path = user_config_path();
    let user_instructions =
        agent_runtime::instructions_settings::read_instructions(&user_config_path)
            .map_err(|error| error.to_string())?;

    let project_instructions =
        if let Some(project_config_path) = selected_project_config_path_for_source(app)? {
            agent_runtime::instructions_settings::read_instructions(&project_config_path)
                .map_err(|error| error.to_string())?
        } else {
            None
        };

    Ok(
        agent_runtime::instructions_settings::build_instructions_view(
            user_instructions,
            project_instructions,
        ),
    )
}

fn refresh_instructions_overlay(app: &mut App) {
    let scope = app.state.settings_scope();
    match load_instructions_view(app) {
        Ok(view) => {
            app.dispatch_effects(vec![CrossPanelEffect::ShowInstructionsOverlay(view)]);
            app.instructions_overlay.set_active_scope(scope);
        }
        Err(error) => push_status_message(app, format!("[instructions error: {error}]")),
    }
}

fn refresh_system_prompt_overlay(app: &mut App) {
    match load_instructions_view(app) {
        Ok(view) => app.dispatch_effects(vec![CrossPanelEffect::ShowSystemPromptOverlay(view)]),
        Err(error) => push_status_message(app, format!("[instructions error: {error}]")),
    }
}

fn save_instructions(
    app: &App,
    scope: agent_core::ConfigScope,
    text: String,
) -> Result<(), String> {
    let input = InstructionsUpdateInput { scope, text };
    let user_config_path = user_config_path();
    let project_config_path = if scope == agent_core::ConfigScope::Project {
        Some(selected_project_config_path(app)?)
    } else {
        None
    };
    agent_runtime::instructions_settings::upsert_instructions(
        &input,
        &user_config_path,
        project_config_path.as_deref(),
    )
    .map_err(|error| error.to_string())
}

fn load_hooks_view(app: &App) -> Result<HooksSettingsView, String> {
    let user_config_path = user_config_path();
    let user = agent_runtime::hooks_settings::read_hooks_from_config(
        &user_config_path,
        agent_core::ConfigScope::User,
    )
    .map_err(|error| error.to_string())?;
    let project_config_path = selected_project_config_path_for_source(app)?;
    let project = if let Some(path) = project_config_path.as_deref() {
        agent_runtime::hooks_settings::read_hooks_from_config(
            path,
            agent_core::ConfigScope::Project,
        )
        .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };

    Ok(HooksSettingsView {
        user,
        project,
        templates: agent_runtime::hooks_settings::builtin_hook_templates(),
        user_config_path: user_config_path.display().to_string(),
        project_config_path: project_config_path.map(|path| path.display().to_string()),
    })
}

fn refresh_hooks_overlay(app: &mut App) {
    let scope = app.state.settings_scope();
    match load_hooks_view(app) {
        Ok(view) => {
            app.dispatch_effects(vec![CrossPanelEffect::ShowHooksOverlay(view)]);
            app.hooks_overlay.set_active_scope(scope);
        }
        Err(error) => push_status_message(app, format!("[hooks error: {error}]")),
    }
}

fn hooks_config_path_for_scope(
    app: &App,
    scope: agent_core::ConfigScope,
) -> Result<std::path::PathBuf, String> {
    match scope {
        agent_core::ConfigScope::User => Ok(user_config_path()),
        agent_core::ConfigScope::Project => selected_project_config_path(app),
        other => Err(format!(
            "hooks can only be managed at User or Project scope, got {other}"
        )),
    }
}

fn save_hook_settings(app: &App, input: HookSettingsInput) -> Result<(), String> {
    let config_path = hooks_config_path_for_scope(app, input.scope)?;
    agent_runtime::hooks_settings::upsert_hook(&input, &config_path)
        .map_err(|error| error.to_string())
}

fn delete_hook_settings(
    app: &App,
    scope: agent_core::ConfigScope,
    event: String,
    id: String,
) -> Result<(), String> {
    let config_path = hooks_config_path_for_scope(app, scope)?;
    agent_runtime::hooks_settings::delete_hook(&config_path, &event, &id)
        .map_err(|error| error.to_string())
}

async fn refresh_agent_overlay<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    match AppFacade::list_agent_settings(runtime.as_ref()).await {
        Ok(agents) => {
            app.dispatch_effects(vec![CrossPanelEffect::ShowAgentSettingsOverlay(
                AgentOverlaySnapshot { agents },
            )]);
        }
        Err(error) => {
            push_status_message(app, format!("[agent settings error: {error}]"));
        }
    }
}

async fn upsert_mcp_server_for_selected_source<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    input: agent_core::facade::McpServerSettingsInput,
) -> Result<agent_core::facade::McpServerSettingsView, String>
where
    F: AppFacade + ?Sized,
{
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        let config_path = selected_project_config_path(app)?;
        return agent_runtime::mcp_settings::upsert_mcp_server_settings(&config_path, input)
            .await
            .map_err(|error| error.to_string());
    }

    McpFacade::upsert_mcp_server_settings(runtime.as_ref(), input)
        .await
        .map_err(|error| error.to_string())
}

async fn set_mcp_server_enabled_for_selected_source<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    server_id: String,
    enabled: bool,
) -> Result<(), String>
where
    F: AppFacade + ?Sized,
{
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        let config_path = selected_project_config_path(app)?;
        return apply_mcp_scope_disabled(&config_path, &server_id, !enabled);
    }

    McpFacade::set_mcp_server_enabled(runtime.as_ref(), server_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

async fn delete_mcp_server_for_selected_source<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    server_id: String,
) -> Result<(), String>
where
    F: AppFacade + ?Sized,
{
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        let config_path = selected_project_config_path(app)?;
        return agent_runtime::mcp_settings::delete_mcp_server_settings(
            &config_path,
            None,
            &server_id,
        )
        .await
        .map_err(|error| error.to_string());
    }

    McpFacade::delete_mcp_server_settings(runtime.as_ref(), server_id)
        .await
        .map_err(|error| error.to_string())
}

async fn upsert_profile_for_selected_source<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    input: agent_core::facade::ProfileSettingsInput,
) -> Result<(), String>
where
    F: AppFacade + ?Sized,
{
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        let config_path = selected_project_config_path(app)?;
        agent_runtime::profile_settings::upsert_profile_settings_in_file(&config_path, &input)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        AppFacade::upsert_profile_settings(runtime.as_ref(), input)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

async fn set_profile_enabled_for_selected_source<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    alias: String,
    enabled: bool,
) -> Result<(), String>
where
    F: AppFacade + ?Sized,
{
    if app.state.settings_config_source() != SettingsConfigSource::Project {
        return AppFacade::set_profile_enabled(runtime.as_ref(), alias, enabled)
            .await
            .map_err(|error| error.to_string());
    }

    let input = profile_input_for_alias(runtime, app, alias, enabled).await?;
    let config_path = selected_project_config_path(app)?;
    agent_runtime::profile_settings::upsert_profile_settings_in_file(&config_path, &input)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn delete_profile_for_selected_source<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    alias: String,
) -> Result<(), String>
where
    F: AppFacade + ?Sized,
{
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        let config_path = selected_project_config_path(app)?;
        return agent_runtime::profile_settings::delete_profile_in_file(&config_path, &alias)
            .await
            .map_err(|error| error.to_string());
    }

    AppFacade::delete_profile_settings(runtime.as_ref(), alias)
        .await
        .map_err(|error| error.to_string())
}

async fn move_profile_in_selected_source<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    alias: String,
    direction: i32,
) -> Result<(), String>
where
    F: AppFacade + ?Sized,
{
    if app.state.settings_config_source() != SettingsConfigSource::Project {
        return AppFacade::move_profile_in_order(runtime.as_ref(), alias, direction)
            .await
            .map_err(|error| error.to_string());
    }

    let mut order = AppFacade::list_profile_settings_for_project(
        runtime.as_ref(),
        app.state.settings_source_filter(),
        selected_project_root_for_source(app),
    )
    .await
    .map_err(|error| error.to_string())?
    .into_iter()
    .map(|profile| profile.alias)
    .collect::<Vec<_>>();

    if let Some(pos) = order
        .iter()
        .position(|profile_alias| profile_alias == &alias)
    {
        let new_pos = if direction < 0 {
            pos.saturating_sub(1)
        } else {
            (pos + 1).min(order.len().saturating_sub(1))
        };
        if new_pos != pos {
            order.swap(pos, new_pos);
        }
    } else {
        order.push(alias);
    }

    let config_path = selected_project_config_path(app)?;
    agent_runtime::profile_settings::save_profile_display_order(&config_path, &order)
        .await
        .map_err(|error| error.to_string())
}

async fn profile_input_for_alias<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    alias: String,
    enabled: bool,
) -> Result<agent_core::facade::ProfileSettingsInput, String>
where
    F: AppFacade + ?Sized,
{
    let profile = AppFacade::list_profile_settings_for_project(
        runtime.as_ref(),
        app.state.settings_source_filter(),
        selected_project_root_for_source(app),
    )
    .await
    .map_err(|error| error.to_string())?
    .into_iter()
    .find(|profile| profile.alias == alias)
    .ok_or_else(|| format!("model profile '{alias}' not found"))?;

    Ok(agent_core::facade::ProfileSettingsInput {
        alias: profile.alias,
        provider: profile.provider,
        model_id: profile.model_id,
        enabled,
        context_window: profile.context_window,
        output_limit: profile.output_limit,
        temperature: profile.temperature,
        top_p: profile.top_p,
        top_k: profile.top_k,
        max_tokens: profile.max_tokens,
        base_url: profile.base_url,
        api_key_env: profile.api_key_env,
    })
}

async fn test_model_connectivity<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    alias: String,
) -> agent_core::Result<ModelProfileTestResult>
where
    F: AppFacade + ?Sized,
{
    let profiles = AppFacade::list_profile_settings_for_project(
        runtime.as_ref(),
        app.state.settings_source_filter(),
        selected_project_root_for_source(app),
    )
    .await?;
    let profile = profiles
        .into_iter()
        .find(|profile| profile.alias == alias)
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(format!("model profile '{alias}' not found"))
        })?;

    if let Some(base_url) = profile.base_url.as_deref().filter(|url| !url.is_empty()) {
        return Ok(test_model_base_url_connectivity(alias, base_url.to_string()).await);
    }

    Ok(ModelProfileTestResult {
        alias,
        ok: true,
        message: None,
    })
}

async fn test_model_base_url_connectivity(
    alias: String,
    base_url: String,
) -> ModelProfileTestResult {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return ModelProfileTestResult {
                alias,
                ok: false,
                message: Some(error.to_string()),
            };
        }
    };
    let endpoints = [
        base_url.to_string(),
        format!("{}/models", base_url.trim_end_matches('/')),
    ];

    let mut last_error = None;
    for endpoint in endpoints {
        match client.get(&endpoint).send().await {
            Ok(response)
                if response.status().is_success() || response.status().is_client_error() =>
            {
                return ModelProfileTestResult {
                    alias,
                    ok: true,
                    message: None,
                };
            }
            Ok(response) => {
                last_error = Some(format!("unexpected status: {}", response.status()));
            }
            Err(error) => {
                last_error = Some(format!("connection failed: {error}"));
            }
        }
    }

    ModelProfileTestResult {
        alias,
        ok: false,
        message: last_error,
    }
}

fn open_directory_path(app: &mut App, path: &str, label: &str) {
    let path_buf = std::path::PathBuf::from(path);
    match std::fs::create_dir_all(&path_buf)
        .map_err(|error| format!("failed to create {label} {}: {error}", path_buf.display()))
        .and_then(|()| open_path_in_system_file_manager(&path_buf))
    {
        Ok(()) => {
            push_status_message(app, format!("opened {label} {}", path_buf.display()));
        }
        Err(error) => {
            push_status_message(app, format!("[{label} open error: {error}]"));
        }
    }
}

fn open_path_in_system_file_manager(path: &std::path::Path) -> Result<(), String> {
    let mut command = system_file_manager_command(path);
    let status = command
        .status()
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;

    if status.success() {
        return Ok(());
    }

    Err(format!(
        "failed to open {}: system opener exited with {status}",
        path.display()
    ))
}

#[cfg(target_os = "macos")]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("explorer");
    command.arg(path);
    command
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(path);
    command
}

async fn refresh_skills_overlay<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    catalog_keyword: Option<String>,
    catalog_sources: Option<Vec<String>>,
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
            sources: catalog_sources,
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

async fn refresh_skills_overlay_with_current_query<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    let (catalog_keyword, catalog_sources) = app.skills_overlay.catalog_query();
    refresh_skills_overlay(runtime, app, catalog_keyword, catalog_sources).await;
}

pub async fn refresh_model_overlay<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    let settings = match AppFacade::list_profile_settings_for_project(
        runtime.as_ref(),
        app.state.settings_source_filter(),
        selected_project_root_for_source(app),
    )
    .await
    {
        Ok(settings) => settings,
        Err(error) => {
            push_status_message(app, format!("[model settings error: {error}]"));
            return;
        }
    };
    let profiles = settings
        .into_iter()
        .map(|profile| ModelProfileEntry {
            alias: profile.alias,
            provider_display: profile.provider,
            model_display: profile.model_id,
            context_window: profile.context_window,
            output_limit: profile.output_limit,
            temperature: profile.temperature,
            top_p: profile.top_p,
            top_k: profile.top_k,
            max_tokens: profile.max_tokens,
            base_url: profile.base_url,
            api_key_env: profile.api_key_env,
            supports_reasoning: false,
            enabled: profile.enabled,
            writable: profile.writable,
            source: profile.source,
            has_api_key: profile.has_api_key,
        })
        .collect();

    app.dispatch_effects(vec![CrossPanelEffect::ShowModelOverlay(
        ModelOverlaySnapshot {
            profiles,
            current_alias: Some(app.state.model_profile.clone()),
            current_effort: app.state.reasoning_effort.clone(),
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
    let mut settings = match McpFacade::list_mcp_server_settings_for_project(
        runtime.as_ref(),
        app.state.settings_source_filter(),
        selected_project_root_for_source(app),
    )
    .await
    {
        Ok(settings) => settings,
        Err(error) => {
            push_status_message(app, format!("[MCP settings error: {error}]"));
            Vec::new()
        }
    };
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        match selected_project_config_path(app).and_then(|path| read_disabled_mcp_scope(&path)) {
            Ok(disabled_ids) => apply_project_disabled_scope(&mut settings, &disabled_ids),
            Err(error) => push_status_message(app, format!("[MCP project scope error: {error}]")),
        }
    }

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
    let filters = app.plugin_overlay.catalog_filters();
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

    let catalog = match PluginsFacade::list_plugin_catalog(
        runtime.as_ref(),
        filters.marketplace_id,
        filters.keyword,
    )
    .await
    {
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
