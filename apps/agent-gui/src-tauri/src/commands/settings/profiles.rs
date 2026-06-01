use super::config_runtime::ConnectivityTestResult;
use super::*;

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
) -> Result<ConnectivityTestResult, String> {
    // Get the profile settings to verify it exists and is configured.
    let profiles = state
        .runtime
        .list_profile_settings(None)
        .await
        .map_err(|e| e.to_string())?;

    let profile = profiles
        .into_iter()
        .find(|p| p.alias == alias)
        .ok_or_else(|| format!("model profile '{}' not found", alias))?;

    // If the profile has a base_url configured, try to reach it.
    if let Some(base_url) = &profile.base_url {
        if !base_url.is_empty() {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())?;

            // Try common endpoints: the base URL itself, then /models.
            let endpoints = [
                base_url.clone(),
                format!("{}/models", base_url.trim_end_matches('/')),
            ];

            let mut last_error: Option<String> = None;
            for endpoint in &endpoints {
                match client.get(endpoint).send().await {
                    Ok(response) => {
                        if response.status().is_success() || response.status().is_client_error() {
                            return Ok(ConnectivityTestResult {
                                ok: true,
                                error: None,
                            });
                        }
                        last_error = Some(format!("unexpected status: {}", response.status()));
                    }
                    Err(e) => {
                        last_error = Some(format!("connection failed: {e}"));
                    }
                }
            }

            return Ok(ConnectivityTestResult {
                ok: false,
                error: last_error,
            });
        }
    }

    // No custom base_url — the profile uses default provider endpoints.
    // Assume it's reachable if the config is valid.
    Ok(ConnectivityTestResult {
        ok: true,
        error: None,
    })
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
