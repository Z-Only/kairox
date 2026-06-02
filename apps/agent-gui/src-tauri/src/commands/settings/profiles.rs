use super::config_runtime::{
    classify_model_error, classify_provider_failure_message, ConnectivityTestResult,
};
use super::*;
use futures::{stream::BoxStream, StreamExt};

const EMPTY_MODEL_RESPONSE_ERROR: &str =
    "model returned an empty response; check model availability, quota, or plan";

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

    let probe_profile = probe_profile(profile);

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
    let stream = match stream_result {
        Ok(stream) => stream,
        Err(error) => return classify_model_error(&error, &subject),
    };

    probe_chat_stream(&subject, stream, timeout).await
}

async fn probe_chat_stream(
    subject: &str,
    mut stream: BoxStream<'static, agent_models::Result<agent_models::ModelEvent>>,
    timeout: std::time::Duration,
) -> ConnectivityTestResult {
    let mut preview = String::new();
    let mut saw_output = false;

    loop {
        let event = match tokio::time::timeout(timeout, stream.next()).await {
            Ok(Some(event)) => event,
            Ok(None) => break,
            Err(_) => {
                return ConnectivityTestResult::failed(
                    "network_error",
                    subject,
                    format!("model stream timed out after {}s", timeout.as_secs()),
                );
            }
        };

        match event {
            Ok(agent_models::ModelEvent::TokenDelta(delta)) => {
                if !delta.trim().is_empty() {
                    saw_output = true;
                }
                if preview.len() < 200 {
                    preview.push_str(&delta);
                }
            }
            Ok(agent_models::ModelEvent::ToolCallRequested { .. }) => {
                return ConnectivityTestResult::chat_ready(subject, non_empty_preview(preview));
            }
            Ok(agent_models::ModelEvent::Completed { .. }) => {
                if saw_output {
                    return ConnectivityTestResult::chat_ready(subject, non_empty_preview(preview));
                }
            }
            Ok(agent_models::ModelEvent::Failed { message }) => {
                return classify_provider_failure_message(message, subject);
            }
            Err(error) => return classify_model_error(&error, subject),
        }
    }

    if saw_output {
        ConnectivityTestResult::chat_ready(subject, non_empty_preview(preview))
    } else {
        ConnectivityTestResult::failed("empty_response", subject, EMPTY_MODEL_RESPONSE_ERROR)
    }
}

fn probe_profile(profile: &agent_config::ProfileDef) -> agent_config::ProfileDef {
    let mut probe_profile = profile.clone();
    probe_profile.output_limit = Some(probe_profile.output_limit.unwrap_or(8).min(8));
    probe_profile.max_tokens = Some(probe_profile.max_tokens.unwrap_or(8).min(8));
    probe_profile
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
            client_identity: None,
            supports_tools: None,
            supports_vision: None,
            supports_reasoning: None,
            extra_params: None,
            server_tool_code_execution: None,
            server_tool_web_search: None,
            enabled: true,
        }
    }

    fn anthropic_profile(base_url: String) -> agent_config::ProfileDef {
        agent_config::ProfileDef {
            provider: "anthropic".into(),
            model_id: "anthropic/claude-haiku-4.5".into(),
            base_url: Some(base_url),
            api_key: Some("test-key".into()),
            api_key_env: None,
            context_window: None,
            output_limit: None,
            response: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            headers: None,
            client_identity: None,
            supports_tools: None,
            supports_vision: None,
            supports_reasoning: None,
            extra_params: None,
            server_tool_code_execution: None,
            server_tool_web_search: None,
            enabled: true,
        }
    }

    async fn serve_anthropic_once(body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let bytes = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..bytes]);
            assert!(request.starts_with("POST /v1/messages "));
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{addr}")
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

    #[tokio::test]
    async fn anthropic_probe_requires_actual_chat_output() {
        std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
        let base_url = serve_anthropic_once(
            r#"{
                "id": "msg_test",
                "type": "message",
                "role": "assistant",
                "model": "anthropic/claude-haiku-4.5",
                "content": [],
                "stop_reason": "end_turn",
                "usage": { "input_tokens": 3, "output_tokens": 0 }
            }"#,
        )
        .await;
        let profile = anthropic_profile(base_url);

        let result = probe_profile_chat_readiness(
            "claude-haiku",
            &profile,
            std::time::Duration::from_secs(2),
        )
        .await;

        assert!(!result.ok);
        assert_eq!(result.status, "empty_response");
        assert!(
            result
                .error
                .as_deref()
                .is_some_and(|error| error.contains("empty response")),
            "expected empty response error, got {result:?}"
        );
    }

    #[test]
    fn probe_profile_preserves_temperature() {
        let mut profile = fake_profile("ok");
        profile.output_limit = Some(16_384);
        profile.max_tokens = Some(16_384);
        profile.temperature = None;

        let probe = super::probe_profile(&profile);

        assert_eq!(probe.output_limit, Some(8));
        assert_eq!(probe.max_tokens, Some(8));
        assert_eq!(probe.temperature, None);
    }
}
