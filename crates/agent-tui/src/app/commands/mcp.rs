use super::*;

pub(super) async fn dispatch<F>(runtime: &std::sync::Arc<F>, app: &mut App, command: Command)
where
    F: AppFacade + ?Sized,
{
    match command {
        Command::OpenMcpOverlay => {
            refresh_mcp_overlay(runtime, app, Vec::new()).await;
            app.state.render_scheduler.mark_dirty_immediate();
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
                    common::push_status_message(app, format!("[MCP settings error: {error}]"));
                }
            }
        }
        Command::SaveMcpServerSettings { input } => {
            match upsert_mcp_server_for_selected_source(runtime, app, input.clone()).await {
                Ok(view) => {
                    common::push_status_message(app, format!("saved MCP server {}", view.id));
                    refresh_mcp_overlay(runtime, app, Vec::new()).await;
                }
                Err(error) => {
                    common::push_status_message(app, format!("[MCP save error: {error}]"));
                }
            }
        }
        Command::DeleteMcpServerSettings { server_id } => {
            match delete_mcp_server_for_selected_source(runtime, app, server_id.clone()).await {
                Ok(()) => {
                    refresh_mcp_overlay(runtime, app, Vec::new()).await;
                }
                Err(error) => {
                    common::push_status_message(app, format!("[MCP delete error: {error}]"));
                }
            }
        }
        Command::OpenMcpConfig => match McpFacade::open_mcp_config_file(runtime.as_ref()).await {
            Ok(Some(path)) => {
                let path_buf = std::path::PathBuf::from(&path);
                match common::open_path_in_system_file_manager(&path_buf) {
                    Ok(()) => {
                        common::push_status_message(
                            app,
                            format!("opened MCP config {}", path_buf.display()),
                        );
                    }
                    Err(error) => {
                        common::push_status_message(
                            app,
                            format!("[MCP config open error: {error}]"),
                        );
                    }
                }
            }
            Ok(None) => {
                common::push_status_message(app, "MCP config path unavailable".to_string());
            }
            Err(error) => {
                common::push_status_message(app, format!("[MCP config error: {error}]"));
            }
        },
        Command::DisableMcpServerAtScope { server_id } => {
            match common::selected_project_config_path(app)
                .and_then(|path| apply_mcp_scope_disabled(&path, &server_id, true))
            {
                Ok(()) => {
                    common::push_status_message(
                        app,
                        format!("disabled MCP server {server_id} in project"),
                    );
                    refresh_mcp_overlay(runtime, app, Vec::new()).await;
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[MCP project disable error: {error}]"),
                    );
                }
            }
        }
        Command::EnableMcpServerAtScope { server_id } => {
            match common::selected_project_config_path(app)
                .and_then(|path| apply_mcp_scope_disabled(&path, &server_id, false))
            {
                Ok(()) => {
                    common::push_status_message(
                        app,
                        format!("enabled MCP server {server_id} in project"),
                    );
                    refresh_mcp_overlay(runtime, app, Vec::new()).await;
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[MCP project enable error: {error}]"),
                    );
                }
            }
        }
        Command::InstallMcpServer { request } => {
            app.mcp_overlay.mark_catalog_install_started(&request);
            app.state.render_scheduler.mark_dirty_immediate();
            match McpFacade::install_catalog_entry(runtime.as_ref(), request.clone()).await {
                Ok(outcome) => {
                    app.mcp_overlay
                        .mark_catalog_install_outcome(&request, &outcome);
                    if !app.mcp_overlay.is_visible() {
                        common::push_status_message(
                            app,
                            format!("MCP install {} {:?}", outcome.kind, outcome.server_id),
                        );
                    }
                    refresh_mcp_overlay(runtime, app, Vec::new()).await;
                }
                Err(error) => {
                    app.mcp_overlay
                        .mark_catalog_install_failed(&request, error.to_string());
                    common::push_status_message(app, format!("[MCP install error: {error}]"));
                    app.state.render_scheduler.mark_dirty_immediate();
                }
            }
        }
        Command::UninstallMcpServer { server_id } => {
            match McpFacade::uninstall_catalog_entry(runtime.as_ref(), server_id.clone()).await {
                Ok(()) => {
                    refresh_mcp_overlay(runtime, app, Vec::new()).await;
                }
                Err(error) => {
                    common::push_status_message(app, format!("[MCP uninstall error: {error}]"));
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
                    common::push_status_message(app, format!("[MCP source error: {error}]"));
                }
            }
        }
        Command::AddMcpCatalogSource { request } => {
            match McpFacade::add_catalog_source(runtime.as_ref(), request.clone()).await {
                Ok(()) => {
                    common::push_status_message(app, format!("added MCP source {}", request.id));
                    refresh_mcp_overlay(runtime, app, Vec::new()).await;
                }
                Err(error) => {
                    common::push_status_message(app, format!("[MCP source add error: {error}]"));
                }
            }
        }
        Command::RemoveMcpCatalogSource { source_id } => {
            match McpFacade::remove_catalog_source(runtime.as_ref(), source_id.clone()).await {
                Ok(()) => {
                    common::push_status_message(app, format!("removed MCP source {source_id}"));
                    refresh_mcp_overlay(runtime, app, Vec::new()).await;
                }
                Err(error) => {
                    common::push_status_message(app, format!("[MCP source remove error: {error}]"));
                }
            }
        }
        _ => {}
    }
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

async fn upsert_mcp_server_for_selected_source<F>(
    runtime: &std::sync::Arc<F>,
    app: &App,
    input: agent_core::facade::McpServerSettingsInput,
) -> Result<agent_core::facade::McpServerSettingsView, String>
where
    F: AppFacade + ?Sized,
{
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        let config_path = common::selected_project_config_path(app)?;
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
        let config_path = common::selected_project_config_path(app)?;
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
        let config_path = common::selected_project_config_path(app)?;
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
        common::selected_project_root_for_source(app),
    )
    .await
    {
        Ok(settings) => settings,
        Err(error) => {
            common::push_status_message(app, format!("[MCP settings error: {error}]"));
            Vec::new()
        }
    };
    if app.state.settings_config_source() == SettingsConfigSource::Project {
        match common::selected_project_config_path(app)
            .and_then(|path| read_disabled_mcp_scope(&path))
        {
            Ok(disabled_ids) => apply_project_disabled_scope(&mut settings, &disabled_ids),
            Err(error) => {
                common::push_status_message(app, format!("[MCP project scope error: {error}]"))
            }
        }
    }

    let installed = match McpFacade::list_installed_entries(runtime.as_ref()).await {
        Ok(installed) => installed,
        Err(error) => {
            common::push_status_message(app, format!("[MCP installed error: {error}]"));
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
            common::push_status_message(app, format!("[MCP catalog error: {error}]"));
            Vec::new()
        }
    };

    let sources = match McpFacade::list_catalog_sources(runtime.as_ref()).await {
        Ok(sources) => sources,
        Err(error) => {
            common::push_status_message(app, format!("[MCP sources error: {error}]"));
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
