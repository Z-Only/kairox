use super::*;

pub(super) fn dispatch(app: &mut App, command: Command) {
    match command {
        Command::OpenInstructionsOverlay => {
            refresh_instructions_overlay(app);
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::OpenSystemPromptOverlay => {
            refresh_system_prompt_overlay(app);
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::SaveInstructions { scope, text } => {
            match save_instructions(app, scope, text) {
                Ok(()) => {
                    refresh_instructions_overlay(app);
                }
                Err(error) => {
                    common::push_status_message(app, format!("[instructions error: {error}]"));
                }
            }
            app.state.render_scheduler.mark_dirty_immediate();
        }
        _ => {}
    }
}

fn load_instructions_view(app: &App) -> Result<agent_core::facade::InstructionsView, String> {
    let user_config_path = common::user_config_path();
    let user_instructions =
        agent_runtime::instructions_settings::read_instructions(&user_config_path)
            .map_err(|error| error.to_string())?;

    let project_instructions =
        if let Some(project_config_path) = common::selected_project_config_path_for_source(app)? {
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
        Err(error) => common::push_status_message(app, format!("[instructions error: {error}]")),
    }
}

fn refresh_system_prompt_overlay(app: &mut App) {
    match load_instructions_view(app) {
        Ok(view) => app.dispatch_effects(vec![CrossPanelEffect::ShowSystemPromptOverlay(view)]),
        Err(error) => common::push_status_message(app, format!("[instructions error: {error}]")),
    }
}

fn save_instructions(
    app: &App,
    scope: agent_core::ConfigScope,
    text: String,
) -> Result<(), String> {
    let input = InstructionsUpdateInput { scope, text };
    let user_config_path = common::user_config_path();
    let project_config_path = if scope == agent_core::ConfigScope::Project {
        Some(common::selected_project_config_path(app)?)
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
