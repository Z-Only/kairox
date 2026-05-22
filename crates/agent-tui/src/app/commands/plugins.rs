use super::*;

pub(super) async fn dispatch<F>(runtime: &std::sync::Arc<F>, app: &mut App, command: Command)
where
    F: AppFacade + ?Sized,
{
    match command {
        Command::OpenPluginsOverlay => {
            refresh_plugins_overlay(runtime, app).await;
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::SetPluginEnabled {
            settings_id,
            enabled,
        } => {
            match PluginsFacade::set_plugin_enabled(runtime.as_ref(), settings_id.clone(), enabled)
                .await
            {
                Ok(()) => {
                    if app.plugin_overlay.is_visible() {
                        refresh_plugins_overlay(runtime, app).await;
                    } else {
                        let state = if enabled { "enabled" } else { "disabled" };
                        common::push_status_message(app, format!("{state} plugin {settings_id}"));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[plugin enable error: {error}]"));
                }
            }
        }
        Command::DeletePluginSettings { settings_id } => {
            match PluginsFacade::delete_plugin_settings(runtime.as_ref(), settings_id.clone()).await
            {
                Ok(()) => {
                    if app.plugin_overlay.is_visible() {
                        refresh_plugins_overlay(runtime, app).await;
                    } else {
                        common::push_status_message(app, format!("deleted plugin {settings_id}"));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[plugin delete error: {error}]"));
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
                    common::push_status_message(app, format!("[plugin source error: {error}]"));
                }
            }
        }
        Command::InstallPlugin { request } => {
            match PluginsFacade::install_plugin(runtime.as_ref(), request.clone()).await {
                Ok(plugin) => {
                    if app.plugin_overlay.is_visible() {
                        refresh_plugins_overlay(runtime, app).await;
                    } else {
                        common::push_status_message(app, format!("installed plugin {}", plugin.id));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[plugin install error: {error}]"));
                }
            }
        }
        _ => {}
    }
}

async fn refresh_plugins_overlay<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    let filters = app.plugin_overlay.catalog_filters();
    let plugins = match PluginsFacade::list_plugin_settings(runtime.as_ref()).await {
        Ok(plugins) => plugins,
        Err(error) => {
            common::push_status_message(app, format!("[plugins error: {error}]"));
            return;
        }
    };

    let sources = match PluginsFacade::list_plugin_marketplace_sources(runtime.as_ref()).await {
        Ok(sources) => sources,
        Err(error) => {
            common::push_status_message(app, format!("[plugin sources error: {error}]"));
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
            common::push_status_message(app, format!("[plugin catalog error: {error}]"));
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
