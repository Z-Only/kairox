use crate::{Config, ConfigError, ProfileDef};

/// Intermediate profile structure for deserialization.
#[derive(Debug, serde::Deserialize)]
struct ProfileToml {
    provider: String,
    model_id: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    connect_timeout_secs: Option<u64>,
    #[serde(default)]
    request_timeout_secs: Option<u64>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default)]
    context_window: Option<u64>,
    #[serde(default)]
    output_limit: Option<u64>,
    #[serde(default)]
    response: Option<String>,
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    top_k: Option<u32>,
    #[serde(default)]
    headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    client_identity: Option<String>,
    #[serde(default)]
    supports_tools: Option<bool>,
    #[serde(default)]
    supports_vision: Option<bool>,
    #[serde(default)]
    supports_reasoning: Option<bool>,
    #[serde(default)]
    extra_params: Option<toml::Value>,
    #[serde(default)]
    server_tool_code_execution: Option<bool>,
    #[serde(default)]
    server_tool_web_search: Option<bool>,
    #[serde(default = "crate::default_true")]
    enabled: bool,
}

pub(super) fn parse_profiles(
    profiles_toml: &toml::value::Table,
    path_for_errors: &str,
) -> Result<Vec<(String, ProfileDef)>, ConfigError> {
    let mut profiles = Vec::new();

    for (alias, value) in profiles_toml {
        let profile_toml: ProfileToml =
            value.clone().try_into().map_err(|e| ConfigError::Parse {
                path: path_for_errors.to_string(),
                message: format!("profile '{}': {}", alias, e),
            })?;

        let profile_def = ProfileDef {
            provider: profile_toml.provider,
            model_id: profile_toml.model_id,
            base_url: profile_toml.base_url,
            connect_timeout_secs: profile_toml.connect_timeout_secs,
            request_timeout_secs: profile_toml.request_timeout_secs,
            api_key: profile_toml.api_key,
            api_key_env: profile_toml.api_key_env,
            context_window: profile_toml.context_window,
            output_limit: profile_toml.output_limit,
            response: profile_toml.response,
            max_tokens: profile_toml.max_tokens,
            temperature: profile_toml.temperature,
            top_p: profile_toml.top_p,
            top_k: profile_toml.top_k,
            headers: profile_toml.headers,
            client_identity: profile_toml.client_identity,
            supports_tools: profile_toml.supports_tools,
            supports_vision: profile_toml.supports_vision,
            supports_reasoning: profile_toml.supports_reasoning,
            extra_params: profile_toml.extra_params,
            server_tool_code_execution: profile_toml.server_tool_code_execution,
            server_tool_web_search: profile_toml.server_tool_web_search,
            enabled: profile_toml.enabled,
        };

        profiles.push((alias.clone(), profile_def));
    }

    Ok(profiles)
}

/// Validate the configuration: check for missing required fields, etc.
pub fn validate(config: &Config) -> Result<(), ConfigError> {
    for (alias, profile) in &config.profiles {
        // openai_compatible requires base_url
        if profile.provider == "openai_compatible" && profile.base_url.is_none() {
            return Err(ConfigError::Parse {
                path: "config".to_string(),
                message: format!(
                    "profile '{}' uses 'openai_compatible' provider but missing 'base_url'",
                    alias
                ),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
