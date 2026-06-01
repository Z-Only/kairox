use std::sync::Arc;

use agent_core::AppFacade;
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app::App;
use crate::components::{Command, CrossPanelEffect, ModelOverlaySnapshot, ModelProfileEntry};

use super::push_status_error;

pub(crate) async fn dispatch(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    command: Command,
) {
    match command {
        Command::CompactSession {
            workspace_id: _,
            session_id,
        } => {
            if let Err(e) = runtime
                .compact_session(session_id, agent_core::CompactionReason::UserRequested)
                .await
            {
                push_status_error(app, format!("[compact error: {e}]"));
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
                push_status_error(app, format!("[switch_model error: {e}]"));
            }
        }

        Command::OpenModelOverlay => {
            refresh_model_overlay(runtime, app).await;
        }

        _ => {}
    }
}

/// Build a `ModelOverlaySnapshot` from the runtime's config and dispatch the
/// `ShowModelOverlay` effect.
///
/// Uses the profile settings facade so disabled and writable profiles appear
/// alongside the active session profile and reasoning effort.
pub(crate) async fn refresh_model_overlay(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
) {
    let project_root = app
        .state
        .selected_settings_project_root()
        .map(|root| root.display().to_string());
    let settings = match AppFacade::list_profile_settings_for_project(
        runtime.as_ref(),
        app.state.settings_source_filter(),
        project_root,
    )
    .await
    {
        Ok(settings) => settings,
        Err(error) => {
            push_status_error(app, format!("[model settings error: {error}]"));
            return;
        }
    };
    let config = runtime.config();
    let profiles: Vec<ModelProfileEntry> = settings
        .into_iter()
        .map(|p| {
            // Resolve context-window limits from the builtin registry when
            // the user config doesn't set them explicitly, matching the GUI's
            // `list_profiles_with_limits` behaviour.
            let (context_window, output_limit) = config
                .get_profile(&p.alias)
                .map(|def| {
                    let limits = agent_config::resolve_limits(def);
                    (Some(limits.context_window), Some(limits.output_limit))
                })
                .unwrap_or((p.context_window, p.output_limit));
            ModelProfileEntry {
                supports_reasoning: config
                    .get_profile(&p.alias)
                    .map(agent_config::profile_supports_reasoning)
                    .unwrap_or(false),
                alias: p.alias,
                provider_display: p.provider,
                model_display: p.model_id,
                context_window,
                output_limit,
                temperature: p.temperature,
                top_p: p.top_p,
                top_k: p.top_k,
                max_tokens: p.max_tokens,
                base_url: p.base_url,
                api_key_env: p.api_key_env,
                client_identity: p.client_identity,
                enabled: p.enabled,
                writable: p.writable,
                source: p.source,
                has_api_key: p.has_api_key,
            }
        })
        .collect();
    let snapshot = ModelOverlaySnapshot {
        profiles,
        current_alias: Some(app.state.model_profile.clone()),
        current_effort: app.state.reasoning_effort.clone(),
    };
    app.dispatch_effects(vec![CrossPanelEffect::ShowModelOverlay(snapshot)]);
}
