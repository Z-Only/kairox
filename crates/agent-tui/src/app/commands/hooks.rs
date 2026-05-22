use super::*;

pub(super) fn dispatch(app: &mut App, command: Command) {
    match command {
        Command::OpenHooksOverlay => {
            refresh_hooks_overlay(app);
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::SaveHookSettings { input } => {
            match save_hook_settings(app, input.clone()) {
                Ok(()) => {
                    common::push_status_message(
                        app,
                        format!("saved hook {}.{}", input.event, input.id),
                    );
                    if app.hooks_overlay.is_visible() {
                        refresh_hooks_overlay(app);
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[hooks save error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::DeleteHookSettings { scope, event, id } => {
            match delete_hook_settings(app, scope, event.clone(), id.clone()) {
                Ok(()) => {
                    common::push_status_message(app, format!("deleted hook {event}.{id}"));
                    if app.hooks_overlay.is_visible() {
                        refresh_hooks_overlay(app);
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[hooks delete error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        _ => {}
    }
}

fn load_hooks_view(app: &App) -> Result<HooksSettingsView, String> {
    let user_config_path = common::user_config_path();
    let user = agent_runtime::hooks_settings::read_hooks_from_config(
        &user_config_path,
        agent_core::ConfigScope::User,
    )
    .map_err(|error| error.to_string())?;
    let project_config_path = common::selected_project_config_path_for_source(app)?;
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
        Err(error) => common::push_status_message(app, format!("[hooks error: {error}]")),
    }
}

fn hooks_config_path_for_scope(
    app: &App,
    scope: agent_core::ConfigScope,
) -> Result<std::path::PathBuf, String> {
    match scope {
        agent_core::ConfigScope::User => Ok(common::user_config_path()),
        agent_core::ConfigScope::Project => common::selected_project_config_path(app),
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
