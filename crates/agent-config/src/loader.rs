//! TOML parsing, API key resolution, and validation.

use crate::{Config, ConfigError, ProfileDef};

/// Intermediate TOML structure for deserialization.
#[derive(Debug, serde::Deserialize)]
struct ConfigToml {
    #[serde(default)]
    profiles: toml::value::Table,
}

/// Intermediate profile structure for deserialization.
#[derive(Debug, serde::Deserialize)]
struct ProfileToml {
    provider: String,
    model_id: String,
    #[serde(default)]
    base_url: Option<String>,
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
}

/// Parse a TOML string into a Config.
pub fn load_from_str(content: &str, path_for_errors: &str) -> Result<Config, ConfigError> {
    let config_toml: ConfigToml = toml::from_str(content).map_err(|e| ConfigError::Parse {
        path: path_for_errors.to_string(),
        message: e.to_string(),
    })?;

    let mut profiles = Vec::new();

    for (alias, value) in &config_toml.profiles {
        let profile_toml: ProfileToml =
            value.clone().try_into().map_err(|e| ConfigError::Parse {
                path: path_for_errors.to_string(),
                message: format!("profile '{}': {}", alias, e),
            })?;

        let profile_def = ProfileDef {
            provider: profile_toml.provider,
            model_id: profile_toml.model_id,
            base_url: profile_toml.base_url,
            api_key: profile_toml.api_key,
            api_key_env: profile_toml.api_key_env,
            context_window: profile_toml.context_window.unwrap_or(128_000),
            output_limit: profile_toml.output_limit.unwrap_or(16_384),
            response: profile_toml.response,
        };

        profiles.push((alias.clone(), profile_def));
    }

    Ok(Config {
        profiles,
        source: crate::ConfigSource::ProjectFile, // Will be overridden by caller
    })
}

/// Resolve API keys: if `api_key_env` is set and `api_key` is not,
/// read the environment variable and populate `api_key`.
pub fn resolve_api_keys(config: &mut Config) {
    for (_alias, profile) in &mut config.profiles {
        if profile.api_key.is_none() {
            if let Some(ref env_var) = profile.api_key_env {
                if let Ok(key) = std::env::var(env_var) {
                    profile.api_key = Some(key);
                }
            }
        }
    }
}

/// Validate the configuration: check for unknown providers, missing fields, etc.
pub fn validate(config: &Config) -> Result<(), ConfigError> {
    let known_providers = ["openai_compatible", "anthropic", "ollama", "fake"];

    for (alias, profile) in &config.profiles {
        if !known_providers.contains(&profile.provider.as_str()) {
            return Err(ConfigError::UnknownProvider {
                profile: alias.clone(),
                provider: profile.provider.clone(),
            });
        }

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

        // fake provider doesn't need base_url or api_key
        // ollama is fine without api_key
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_toml_with_multiple_profiles() {
        let toml = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4.1-mini"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
context_window = 128_000
output_limit = 16_384

[profiles.local-code]
provider = "ollama"
model_id = "devstral"
base_url = "http://localhost:11434"
"#;
        let config = load_from_str(toml, "test.toml").unwrap();
        assert_eq!(config.profiles.len(), 2);

        let (fast_name, fast_def) = &config.profiles[0];
        assert_eq!(fast_name, "fast");
        assert_eq!(fast_def.provider, "openai_compatible");
        assert_eq!(fast_def.model_id, "gpt-4.1-mini");
        assert_eq!(
            fast_def.base_url,
            Some("https://api.openai.com/v1".to_string())
        );
        assert_eq!(fast_def.api_key_env, Some("OPENAI_API_KEY".to_string()));
        assert_eq!(fast_def.context_window, 128_000);

        let (local_name, local_def) = &config.profiles[1];
        assert_eq!(local_name, "local-code");
        assert_eq!(local_def.provider, "ollama");
        assert_eq!(local_def.model_id, "devstral");
        assert_eq!(local_def.context_window, 128_000); // default
    }

    #[test]
    fn parses_fake_provider_with_response() {
        let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"
response = "hello from Kairox"
"#;
        let config = load_from_str(toml, "test.toml").unwrap();
        let (_, fake_def) = &config.profiles[0];
        assert_eq!(fake_def.provider, "fake");
        assert_eq!(fake_def.response, Some("hello from Kairox".to_string()));
    }

    #[test]
    fn rejects_unknown_provider() {
        let toml = r#"
[profiles.bad]
provider = "unknown_provider"
model_id = "test"
"#;
        let config = load_from_str(toml, "test.toml").unwrap();
        let result = validate(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::UnknownProvider { profile, provider } => {
                assert_eq!(profile, "bad");
                assert_eq!(provider, "unknown_provider");
            }
            _ => panic!("expected UnknownProvider error"),
        }
    }

    #[test]
    fn rejects_openai_compatible_without_base_url() {
        let toml = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4.1-mini"
api_key_env = "OPENAI_API_KEY"
"#;
        let config = load_from_str(toml, "test.toml").unwrap();
        let result = validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn api_key_direct_takes_priority_over_env() {
        let toml = r#"
[profiles.test]
provider = "openai_compatible"
model_id = "test-model"
base_url = "https://api.example.com/v1"
api_key = "sk-direct-key"
api_key_env = "OPENAI_API_KEY"
"#;
        let mut config = load_from_str(toml, "test.toml").unwrap();
        resolve_api_keys(&mut config);
        let (_, def) = &config.profiles[0];
        // api_key was already set (direct), so it should remain unchanged
        assert_eq!(def.api_key, Some("sk-direct-key".to_string()));
    }

    #[test]
    fn api_key_env_resolves_from_environment() {
        let toml = r#"
[profiles.test]
provider = "openai_compatible"
model_id = "test-model"
base_url = "https://api.example.com/v1"
api_key_env = "KAIROX_TEST_KEY_VAR"
"#;
        std::env::set_var("KAIROX_TEST_KEY_VAR", "sk-from-env");
        let mut config = load_from_str(toml, "test.toml").unwrap();
        resolve_api_keys(&mut config);
        let (_, def) = &config.profiles[0];
        assert_eq!(def.api_key, Some("sk-from-env".to_string()));
        std::env::remove_var("KAIROX_TEST_KEY_VAR");
    }

    #[test]
    fn parse_error_on_invalid_toml() {
        let toml = "this is not valid toml {{{{";
        let result = load_from_str(toml, "bad.toml");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::Parse { path, .. } => assert_eq!(path, "bad.toml"),
            _ => panic!("expected Parse error"),
        }
    }
}
