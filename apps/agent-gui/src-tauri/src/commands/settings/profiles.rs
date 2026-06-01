use super::config_runtime::{
    classify_model_error, classify_provider_failure_message, ConnectivityTestResult,
};
use super::*;
use futures::StreamExt;

#[tauri::command]
#[specta::specta]
pub async fn list_profiles(state: State<'_, GuiState>) -> Result<Vec<String>, String> {
    Ok(state.config.read().unwrap().profile_names())
}

#[tauri::command]
#[specta::specta]
pub async fn get_profile_info(state: State<'_, GuiState>) -> Result<Vec<ProfileInfo>, String> {
    Ok(state.config.read().unwrap().profile_info())
}

#[tauri::command]
#[specta::specta]
pub async fn list_profile_settings(
    state: State<'_, GuiState>,
    source_filter: Option<String>,
    project_root: Option<String>,
) -> Result<Vec<ProfileSettingsView>, String> {
    state
        .runtime
        .list_profile_settings_for_project(source_filter, project_root)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_effective_model_profiles(
    state: State<'_, GuiState>,
) -> Result<Vec<EffectiveProfileView>, String> {
    let config = state.config.read().map_err(|e| e.to_string())?;
    Ok(
        agent_config::build_effective_profile_settings_views(&config)
            .into_iter()
            .map(EffectiveProfileView::from_effective)
            .collect(),
    )
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_profile_settings(
    state: State<'_, GuiState>,
    input: ProfileSettingsInput,
) -> Result<ProfileSettingsView, String> {
    let view = state
        .runtime
        .upsert_profile_settings(input)
        .await
        .map_err(|error| error.to_string())?;
    state.refresh_config()?;
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub async fn set_profile_enabled(
    state: State<'_, GuiState>,
    alias: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_profile_enabled(alias, enabled)
        .await
        .map_err(|error| error.to_string())?;
    state.refresh_config()?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_profile_settings(
    state: State<'_, GuiState>,
    alias: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_profile_settings(alias)
        .await
        .map_err(|error| error.to_string())?;
    state.refresh_config()?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn test_model_connectivity(
    state: State<'_, GuiState>,
    alias: String,
    project_root: Option<String>,
) -> Result<ConnectivityTestResult, String> {
    let config = if let Some(project_root) = project_root.as_deref().filter(|path| !path.is_empty())
    {
        let base_config =
            agent_config::Config::load_with_project_root(Some(std::path::Path::new(project_root)))
                .map_err(|e| e.to_string())?;
        agent_runtime::ui_bootstrap::load_config_with_profiles_overlay(base_config, &state.home_dir)
            .map_err(|e| e.to_string())?
            .config
    } else {
        state.config.read().map_err(|e| e.to_string())?.clone()
    };
    let profile = config
        .profiles
        .iter()
        .find(|(name, _)| name == &alias)
        .map(|(_, profile)| profile.clone())
        .ok_or_else(|| format!("model profile '{}' not found", alias))?;

    Ok(probe_profile_chat_readiness(&alias, &profile, std::time::Duration::from_secs(20)).await)
}

async fn probe_profile_chat_readiness(
    alias: &str,
    profile: &agent_config::ProfileDef,
    timeout: std::time::Duration,
) -> ConnectivityTestResult {
    let subject = format!("Model {alias}");
    if !profile.enabled {
        return ConnectivityTestResult::failed("invalid_config", &subject, "profile is disabled");
    }
    if profile.provider == "fake" {
        return ConnectivityTestResult::chat_ready(&subject, profile.response.clone());
    }

    let mut probe_profile = profile.clone();
    probe_profile.output_limit = Some(probe_profile.output_limit.unwrap_or(8).min(8));
    probe_profile.max_tokens = Some(probe_profile.max_tokens.unwrap_or(8).min(8));
    probe_profile.temperature = Some(0.0);

    let mut probe_config = agent_config::Config::defaults();
    probe_config.profiles = vec![(alias.to_string(), probe_profile)];
    let router = probe_config.build_router();
    let request = agent_models::ModelRequest::user_text(alias, "Reply with OK.");

    let stream_result = match tokio::time::timeout(timeout, router.route(request)).await {
        Ok(result) => result,
        Err(_) => {
            return ConnectivityTestResult::failed(
                "network_error",
                &subject,
                format!("model probe timed out after {}s", timeout.as_secs()),
            );
        }
    };
    let mut stream = match stream_result {
        Ok(stream) => stream,
        Err(error) => return classify_model_error(&error, &subject),
    };

    let mut preview = String::new();
    loop {
        let event = match tokio::time::timeout(timeout, stream.next()).await {
            Ok(Some(event)) => event,
            Ok(None) => break,
            Err(_) => {
                return ConnectivityTestResult::failed(
                    "network_error",
                    &subject,
                    format!("model stream timed out after {}s", timeout.as_secs()),
                );
            }
        };

        match event {
            Ok(agent_models::ModelEvent::TokenDelta(delta)) => {
                if preview.len() < 200 {
                    preview.push_str(&delta);
                }
            }
            Ok(agent_models::ModelEvent::ToolCallRequested { .. }) => {
                return ConnectivityTestResult::chat_ready(&subject, non_empty_preview(preview));
            }
            Ok(agent_models::ModelEvent::Completed { .. }) => {
                return ConnectivityTestResult::chat_ready(&subject, non_empty_preview(preview));
            }
            Ok(agent_models::ModelEvent::Failed { message }) => {
                return classify_provider_failure_message(message, &subject);
            }
            Err(error) => return classify_model_error(&error, &subject),
        }
    }

    ConnectivityTestResult::chat_ready(&subject, non_empty_preview(preview))
}

fn non_empty_preview(preview: String) -> Option<String> {
    let trimmed = preview.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(200).collect())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn move_profile_in_order(
    state: State<'_, GuiState>,
    alias: String,
    direction: i32,
) -> Result<(), String> {
    state
        .runtime
        .move_profile_in_order(alias, direction)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileWithLimits {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    #[specta(type = u32)]
    pub context_window: u64,
    #[specta(type = u32)]
    pub output_limit: u64,
    /// Snake-case `LimitSource`: "user_config" | "builtin_registry" | "runtime_probe" | "fallback".
    pub limit_source: String,
    pub has_api_key: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn list_profiles_with_limits(
    state: State<'_, GuiState>,
) -> Result<Vec<ProfileWithLimits>, String> {
    let config = state.config.read().unwrap();
    let mut out = Vec::with_capacity(config.profiles.len());
    for (alias, profile) in &config.profiles {
        let limits = agent_config::resolve_limits(profile);
        let limit_source = match limits.source {
            agent_models::LimitSource::UserConfig => "user_config",
            agent_models::LimitSource::BuiltinRegistry => "builtin_registry",
            agent_models::LimitSource::RuntimeProbe => "runtime_probe",
            agent_models::LimitSource::Fallback => "fallback",
        };
        let has_api_key = profile.api_key.is_some()
            || profile
                .api_key_env
                .as_deref()
                .map(|env| std::env::var(env).is_ok())
                .unwrap_or(false)
            || matches!(profile.provider.as_str(), "ollama" | "fake");
        out.push(ProfileWithLimits {
            alias: alias.clone(),
            provider: profile.provider.clone(),
            model_id: profile.model_id.clone(),
            context_window: limits.context_window,
            output_limit: limits.output_limit,
            limit_source: limit_source.into(),
            has_api_key,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod profile_with_limits_tests {
    use super::*;

    #[test]
    fn profile_with_limits_serializes_expected_shape() {
        let p = ProfileWithLimits {
            alias: "fast".into(),
            provider: "openai".into(),
            model_id: "gpt-4o-mini".into(),
            context_window: 128_000,
            output_limit: 16_384,
            limit_source: "builtin_registry".into(),
            has_api_key: true,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"alias\":\"fast\""));
        assert!(json.contains("\"context_window\":128000"));
        assert!(json.contains("\"limit_source\":\"builtin_registry\""));
        assert!(json.contains("\"has_api_key\":true"));
    }
}

#[cfg(test)]
mod connectivity_tests {
    use super::*;

    fn fake_profile(response: &str) -> agent_config::ProfileDef {
        agent_config::ProfileDef {
            provider: "fake".into(),
            model_id: "fake-model".into(),
            base_url: None,
            api_key: None,
            api_key_env: None,
            context_window: None,
            output_limit: None,
            response: Some(response.into()),
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            headers: None,
            supports_tools: None,
            supports_vision: None,
            supports_reasoning: None,
            extra_params: None,
            server_tool_code_execution: None,
            server_tool_web_search: None,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn fake_profile_reports_chat_ready_without_network() {
        let result = probe_profile_chat_readiness(
            "fake",
            &fake_profile("Hello from the Kairox fake provider!"),
            std::time::Duration::from_secs(1),
        )
        .await;

        assert!(result.ok);
        assert_eq!(result.status, "chat_ready");
        assert_eq!(
            result.response_preview.as_deref(),
            Some("Hello from the Kairox fake provider!")
        );
    }
}
