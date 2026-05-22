use super::*;

pub(super) async fn dispatch<F>(runtime: &std::sync::Arc<F>, app: &mut App, command: Command)
where
    F: AppFacade + ?Sized,
{
    match command {
        Command::OpenAgentSettingsOverlay => {
            refresh_agent_overlay(runtime, app).await;
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::SaveAgentSettings { input } => {
            match AppFacade::upsert_agent_settings(runtime.as_ref(), input.clone()).await {
                Ok(view) => {
                    common::push_status_message(app, format!("saved agent {}", view.name));
                    if app.agent_overlay.is_visible() {
                        refresh_agent_overlay(runtime, app).await;
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[agent save error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::DeleteAgentSettings { settings_id } => {
            match AppFacade::delete_agent_settings(runtime.as_ref(), settings_id.clone()).await {
                Ok(()) => {
                    common::push_status_message(app, format!("deleted agent {settings_id}"));
                    if app.agent_overlay.is_visible() {
                        refresh_agent_overlay(runtime, app).await;
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[agent delete error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::CopyAgentSettings { settings_id, scope } => {
            match AppFacade::copy_agent_settings(runtime.as_ref(), settings_id.clone(), scope).await
            {
                Ok(view) => {
                    common::push_status_message(app, format!("copied agent {}", view.name));
                    if app.agent_overlay.is_visible() {
                        refresh_agent_overlay(runtime, app).await;
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[agent copy error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        _ => {}
    }
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
            common::push_status_message(app, format!("[agent settings error: {error}]"));
        }
    }
}
