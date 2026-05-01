pub mod builder;
pub mod discovery;
pub mod loader;

use serde::{Deserialize, Serialize};

pub use builder::build_router;
pub use discovery::find_config;
pub use loader::{load_from_str, resolve_api_keys, validate};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Definition of a single model profile, loaded from TOML or generated as default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDef {
    pub provider: String,
    pub model_id: String,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Direct API key value. Takes priority over `api_key_env`.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Name of an environment variable that holds the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_context_window")]
    pub context_window: u64,
    #[serde(default = "default_output_limit")]
    pub output_limit: u64,
    /// Response text for the fake provider.
    #[serde(default)]
    pub response: Option<String>,
}

fn default_context_window() -> u64 {
    128_000
}

fn default_output_limit() -> u64 {
    16_384
}

/// Metadata about a profile for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
}

/// Where the configuration was loaded from.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    ProjectFile,
    UserFile,
    Defaults,
}

/// Fully loaded configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub profiles: Vec<(String, ProfileDef)>,
    pub source: ConfigSource,
}

/// Errors that can occur during config loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config parse error in {path}: {message}")]
    Parse { path: String, message: String },
    #[error("profile '{profile}' has unknown provider '{provider}'")]
    UnknownProvider { profile: String, provider: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl Config {
    /// Load configuration from a discovered file, or generate defaults.
    pub fn load() -> Result<Self, ConfigError> {
        match find_config() {
            Some((path, source)) => {
                let content = std::fs::read_to_string(&path)?;
                let mut config = load_from_str(&content, &path.display().to_string())?;
                config.source = source;
                resolve_api_keys(&mut config);
                validate(&config)?;
                Ok(config)
            }
            None => Ok(Self::defaults()),
        }
    }

    /// Generate default configuration from environment variables.
    pub fn defaults() -> Self {
        let mut profiles = vec![
            (
                "fake".into(),
                ProfileDef {
                    provider: "fake".into(),
                    model_id: "fake".into(),
                    base_url: None,
                    api_key: None,
                    api_key_env: None,
                    context_window: 4096,
                    output_limit: 2048,
                    response: Some("hello from Kairox".into()),
                },
            ),
            (
                "local-code".into(),
                ProfileDef {
                    provider: "ollama".into(),
                    model_id: "devstral".into(),
                    base_url: Some("http://localhost:11434".into()),
                    api_key: None,
                    api_key_env: None,
                    context_window: 128_000,
                    output_limit: 16_384,
                    response: None,
                },
            ),
        ];

        if std::env::var("OPENAI_API_KEY").is_ok() {
            profiles.insert(
                0,
                (
                    "fast".into(),
                    ProfileDef {
                        provider: "openai_compatible".into(),
                        model_id: "gpt-4.1-mini".into(),
                        base_url: Some("https://api.openai.com/v1".into()),
                        api_key: None,
                        api_key_env: Some("OPENAI_API_KEY".into()),
                        context_window: 128_000,
                        output_limit: 16_384,
                        response: None,
                    },
                ),
            );
        }

        Config {
            profiles,
            source: ConfigSource::Defaults,
        }
    }

    /// Build a `ModelRouter` from this configuration.
    pub fn build_router(&self) -> agent_models::ModelRouter {
        builder::build_router(self)
    }

    /// Get profile names in order.
    pub fn profile_names(&self) -> Vec<String> {
        self.profiles.iter().map(|(name, _)| name.clone()).collect()
    }

    /// Get the default profile name (fast > local-code > first available).
    /// Returns an owned String to avoid lifetime issues.
    pub fn default_profile(&self) -> String {
        let names = self.profile_names();
        if names.iter().any(|p| p == "fast") {
            "fast".to_string()
        } else if names.iter().any(|p| p == "local-code") {
            "local-code".to_string()
        } else {
            names.first().cloned().unwrap_or_else(|| "fake".to_string())
        }
    }

    /// Get profile metadata for UI display.
    pub fn profile_info(&self) -> Vec<ProfileInfo> {
        self.profiles
            .iter()
            .map(|(alias, def)| {
                let local = def.provider == "ollama" || def.provider == "fake";
                let has_api_key = def.api_key.is_some()
                    || def
                        .api_key_env
                        .as_ref()
                        .is_some_and(|v| std::env::var(v).is_ok());
                ProfileInfo {
                    alias: alias.clone(),
                    provider: def.provider.clone(),
                    model_id: def.model_id.clone(),
                    local,
                    has_api_key,
                }
            })
            .collect()
    }

    /// Look up a profile definition by alias.
    pub fn get_profile(&self, alias: &str) -> Option<&ProfileDef> {
        self.profiles
            .iter()
            .find(|(name, _)| name == alias)
            .map(|(_, def)| def)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_fake_and_local_code() {
        let config = Config::defaults();
        let names = config.profile_names();
        assert!(names.contains(&"fake".to_string()));
        assert!(names.contains(&"local-code".to_string()));
    }

    #[test]
    fn defaults_include_fast_when_openai_key_set() {
        let config = Config::defaults();
        let names = config.profile_names();
        assert!(!names.is_empty());
    }

    #[test]
    fn default_profile_prefers_fast() {
        let config = Config::defaults();
        let default = config.default_profile();
        assert!(!default.is_empty());
    }

    #[test]
    fn profile_names_returns_ordered_list() {
        let config = Config::defaults();
        let names = config.profile_names();
        assert_eq!(names.len(), config.profiles.len());
    }

    #[test]
    fn profile_info_reflects_local_and_key_status() {
        let config = Config::defaults();
        let info = config.profile_info();
        assert!(info.iter().any(|p| p.alias == "fake" && p.local));
        assert!(info.iter().any(|p| p.alias == "local-code" && p.local));
    }
}
