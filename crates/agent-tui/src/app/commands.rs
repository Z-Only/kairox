use agent_core::facade::{
    InstructionsUpdateInput, McpFacade, PluginsFacade, ProjectFacade, SkillCatalogQuery,
    SkillInstallTarget,
};
use agent_core::projection::{ProjectedMessage, ProjectedRole};
use agent_core::{
    ActivateSkillRequest, AppFacade, DeactivateSkillRequest, ProjectGitStatus,
    ProjectGitStatusKind, ProjectInstructionSummary, ProjectMeta,
};

use super::App;
use crate::components::{
    AgentOverlaySnapshot, Command, CrossPanelEffect, McpOverlaySnapshot, McpServerEntry,
    ModelProfileTestResult, PluginOverlaySnapshot, ProjectInfo, SkillEntry, SkillOverlaySnapshot,
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
            Command::OpenSkillsOverlay => {
                refresh_skills_overlay(runtime, app, None).await;
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
            Command::OpenAgentsDir => {
                match AppFacade::open_agents_dir(runtime.as_ref()).await {
                    Ok(Some(path)) => {
                        let path_buf = std::path::PathBuf::from(&path);
                        match std::fs::create_dir_all(&path_buf)
                            .map_err(|error| {
                                format!(
                                    "failed to create agents dir {}: {error}",
                                    path_buf.display()
                                )
                            })
                            .and_then(|()| open_path_in_system_file_manager(&path_buf))
                        {
                            Ok(()) => {
                                push_status_message(
                                    app,
                                    format!("opened agents dir {}", path_buf.display()),
                                );
                            }
                            Err(error) => {
                                push_status_message(
                                    app,
                                    format!("[agents dir open error: {error}]"),
                                );
                            }
                        }
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
            Command::SaveInstructions { scope, text } => {
                match save_instructions(scope, text) {
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
            Command::SetProfileEnabled { alias, enabled } => {
                match AppFacade::set_profile_enabled(runtime.as_ref(), alias.clone(), enabled).await
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
            Command::DeleteProfileSettings { alias } => {
                match AppFacade::delete_profile_settings(runtime.as_ref(), alias.clone()).await {
                    Ok(()) => {
                        push_status_message(app, format!("deleted model profile {alias}"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[model profile delete error: {error}]"));
                    }
                }
            }
            Command::MoveProfileInOrder { alias, direction } => {
                match AppFacade::move_profile_in_order(runtime.as_ref(), alias.clone(), direction)
                    .await
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
                match test_model_connectivity(runtime, alias.clone()).await {
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
    app.state.current_session.messages.push(ProjectedMessage {
        role: ProjectedRole::Assistant,
        content,
    });
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

fn load_instructions_view() -> Result<agent_core::facade::InstructionsView, String> {
    let user_config_path = user_config_path();
    let user_instructions =
        agent_runtime::instructions_settings::read_instructions(&user_config_path)
            .map_err(|error| error.to_string())?;

    let project_config_path = project_config_path()?;
    let project_instructions =
        agent_runtime::instructions_settings::read_instructions(&project_config_path)
            .map_err(|error| error.to_string())?;

    Ok(
        agent_runtime::instructions_settings::build_instructions_view(
            user_instructions,
            project_instructions,
        ),
    )
}

fn refresh_instructions_overlay(app: &mut App) {
    match load_instructions_view() {
        Ok(view) => app.dispatch_effects(vec![CrossPanelEffect::ShowInstructionsOverlay(view)]),
        Err(error) => push_status_message(app, format!("[instructions error: {error}]")),
    }
}

fn save_instructions(scope: agent_core::ConfigScope, text: String) -> Result<(), String> {
    let input = InstructionsUpdateInput { scope, text };
    let user_config_path = user_config_path();
    let project_config_path = project_config_path()?;
    agent_runtime::instructions_settings::upsert_instructions(
        &input,
        &user_config_path,
        Some(project_config_path.as_path()),
    )
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

async fn test_model_connectivity<F>(
    runtime: &std::sync::Arc<F>,
    alias: String,
) -> agent_core::Result<ModelProfileTestResult>
where
    F: AppFacade + ?Sized,
{
    let profiles = AppFacade::list_profile_settings(runtime.as_ref(), None).await?;
    let profile = profiles
        .into_iter()
        .find(|profile| profile.alias == alias)
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(format!("model profile '{alias}' not found"))
        })?;

    if let Some(base_url) = profile.base_url.as_deref().filter(|url| !url.is_empty()) {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
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
                    return Ok(ModelProfileTestResult {
                        alias,
                        ok: true,
                        message: None,
                    });
                }
                Ok(response) => {
                    last_error = Some(format!("unexpected status: {}", response.status()));
                }
                Err(error) => {
                    last_error = Some(format!("connection failed: {error}"));
                }
            }
        }

        return Ok(ModelProfileTestResult {
            alias,
            ok: false,
            message: last_error,
        });
    }

    Ok(ModelProfileTestResult {
        alias,
        ok: true,
        message: None,
    })
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
