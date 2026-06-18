use super::*;

pub(super) async fn dispatch<F>(runtime: &std::sync::Arc<F>, app: &mut App, command: Command)
where
    F: AppFacade + ?Sized,
{
    match command {
        Command::OpenModelOverlay => {
            refresh_model_overlay(runtime, app).await;
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::SetProfileEnabled { alias, enabled } => {
            match set_profile_enabled_for_selected_source(runtime, app, alias.clone(), enabled)
                .await
            {
                Ok(()) => {
                    let state = if enabled { "enabled" } else { "disabled" };
                    common::push_status_message(app, format!("{state} model profile {alias}"));
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[model profile enable error: {error}]"),
                    );
                }
            }
        }
        Command::SaveProfileSettings { input } => {
            let alias = input.alias.clone();
            match upsert_profile_for_selected_source(runtime, app, input).await {
                Ok(()) => {
                    common::push_status_message(app, format!("saved model profile {alias}"));
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[model profile save error: {error}]"),
                    );
                }
            }
        }
        Command::DeleteProfileSettings { alias } => {
            match delete_profile_for_selected_source(runtime, app, alias.clone()).await {
                Ok(()) => {
                    common::push_status_message(app, format!("deleted model profile {alias}"));
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[model profile delete error: {error}]"),
                    );
                }
            }
        }
        Command::MoveProfileInOrder { alias, direction } => {
            match move_profile_in_selected_source(runtime, app, alias.clone(), direction).await {
                Ok(()) => {
                    common::push_status_message(app, format!("moved model profile {alias}"));
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[model profile order error: {error}]"),
                    );
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
                    common::push_status_message(app, message);
                }
                Err(error) => {
                    let result = ModelProfileTestResult {
                        alias: alias.clone(),
                        ok: false,
                        message: Some(error.to_string()),
                    };
                    app.dispatch_effects(vec![CrossPanelEffect::ModelProfileTested(result)]);
                    common::push_status_message(
                        app,
                        format!("[model profile test error: {error}]"),
                    );
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
            common::push_status_message(app, message);
        }
        Command::OpenProfilesConfig => {
            match AppFacade::open_profiles_config_file(runtime.as_ref()).await {
                Ok(Some(path)) => {
                    let path_buf = std::path::PathBuf::from(&path);
                    match common::open_path_in_system_file_manager(&path_buf) {
                        Ok(()) => {
                            common::push_status_message(
                                app,
                                format!("opened profiles config {}", path_buf.display()),
                            );
                        }
                        Err(error) => {
                            common::push_status_message(
                                app,
                                format!("[profiles config open error: {error}]"),
                            );
                        }
                    }
                }
                Ok(None) => {
                    common::push_status_message(
                        app,
                        "profiles config path unavailable".to_string(),
                    );
                }
                Err(error) => {
                    common::push_status_message(app, format!("[profiles config error: {error}]"));
                }
            }
        }
        _ => {}
    }
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
        let config_path = common::selected_project_config_path(app)?;
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
    let config_path = common::selected_project_config_path(app)?;
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
        let config_path = common::selected_project_config_path(app)?;
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
        common::selected_project_root_for_source(app),
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

    let config_path = common::selected_project_config_path(app)?;
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
        common::selected_project_root_for_source(app),
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
        api_key: profile.api_key,
        api_key_env: profile.api_key_env,
        client_identity: profile.client_identity,
        supports_reasoning: profile.supports_reasoning,
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
        common::selected_project_root_for_source(app),
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

pub(super) async fn command_palette_model_profiles<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
) -> Vec<ModelProfileEntry>
where
    F: AppFacade + ?Sized,
{
    match AppFacade::list_profile_settings_for_project(
        runtime.as_ref(),
        app.state.settings_source_filter(),
        common::selected_project_root_for_source(app),
    )
    .await
    {
        Ok(settings) => settings
            .into_iter()
            .filter(|profile| profile.enabled)
            .map(model_profile_entry_from_settings)
            .collect(),
        Err(error) => {
            common::push_status_message(app, format!("[model settings error: {error}]"));
            Vec::new()
        }
    }
}

pub async fn refresh_model_overlay<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    let settings = match AppFacade::list_profile_settings_for_project(
        runtime.as_ref(),
        app.state.settings_source_filter(),
        common::selected_project_root_for_source(app),
    )
    .await
    {
        Ok(settings) => settings,
        Err(error) => {
            common::push_status_message(app, format!("[model settings error: {error}]"));
            return;
        }
    };
    let profiles = settings
        .into_iter()
        .map(model_profile_entry_from_settings)
        .collect();

    app.dispatch_effects(vec![CrossPanelEffect::ShowModelOverlay(
        ModelOverlaySnapshot {
            profiles,
            current_alias: Some(app.state.model_profile.clone()),
            current_effort: app.state.reasoning_effort.clone(),
        },
    )]);
}

fn model_profile_entry_from_settings(
    profile: agent_core::facade::ProfileSettingsView,
) -> ModelProfileEntry {
    ModelProfileEntry {
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
        client_identity: profile.client_identity,
        supports_reasoning: profile.supports_reasoning.unwrap_or(false),
        supports_reasoning_override: profile.supports_reasoning,
        enabled: profile.enabled,
        writable: profile.writable,
        source: profile.source,
        has_api_key: profile.has_api_key,
    }
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
