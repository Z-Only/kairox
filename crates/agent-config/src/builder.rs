//! Build a ModelRouter from a Config.

use crate::{Config, ProfileDef};
use agent_models::{
    AnthropicClient, AnthropicConfig, FakeModelClient, ModelCapabilities, ModelClient,
    ModelProfile, ModelRouter, OllamaClient, OllamaConfig, OpenAiCompatibleClient,
    OpenAiCompatibleConfig,
};
use std::sync::Arc;

/// Build a `ModelRouter` from the given `Config`, registering a `ModelClient`
/// for each profile.
pub fn build_router(config: &Config) -> ModelRouter {
    let mut router = ModelRouter::new();

    for (alias, def) in &config.profiles {
        let profile = build_profile(alias, def);
        let client = build_client(alias, def);
        router.register(profile, Arc::from(client));
    }

    router
}

/// Build a map of profile alias → typed `Arc<OllamaClient>` for every profile
/// whose `provider == "ollama"`. The runtime keeps these typed handles so it
/// can call `probe_context_window` on session init — `ModelRouter` only stores
/// `Arc<dyn ModelClient>` and would otherwise force an `Any` downcast.
pub fn build_ollama_clients(
    config: &Config,
) -> std::collections::HashMap<String, Arc<OllamaClient>> {
    let mut clients = std::collections::HashMap::new();
    for (alias, def) in &config.profiles {
        if def.provider != "ollama" {
            continue;
        }
        let ollama_config = OllamaConfig {
            base_url: def
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
            default_model: def.model_id.clone(),
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
        };
        clients.insert(alias.clone(), Arc::new(OllamaClient::new(ollama_config)));
    }
    clients
}

/// Map a provider name to a client family.
/// Known providers map to their specific client; everything else maps to
/// `openai_compatible` since most third-party APIs follow the OpenAI protocol.
fn provider_family(provider: &str) -> &str {
    match provider {
        "anthropic" => "anthropic",
        "ollama" => "ollama",
        "fake" => "fake",
        "openai_compatible" => "openai_compatible",
        _ => "openai_compatible",
    }
}

fn build_profile(alias: &str, def: &ProfileDef) -> ModelProfile {
    let family = provider_family(&def.provider);

    let mut capabilities = match family {
        "openai_compatible" => ModelCapabilities {
            streaming: true,
            tool_calling: true,
            json_schema: true,
            vision: false,
            reasoning_controls: false,
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            output_limit: def
                .output_limit
                .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
            local_model: false,
        },
        "anthropic" => ModelCapabilities {
            streaming: true,
            tool_calling: true,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            output_limit: def
                .output_limit
                .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
            local_model: false,
        },
        "ollama" => ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            output_limit: def
                .output_limit
                .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
            local_model: true,
        },
        "fake" => ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            output_limit: def
                .output_limit
                .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
            local_model: true,
        },
        _ => unreachable!("provider_family always returns a known family"),
    };

    // Apply capability overrides from ProfileDef
    if let Some(v) = def.supports_tools {
        capabilities.tool_calling = v;
    }
    if let Some(v) = def.supports_vision {
        capabilities.vision = v;
    }
    if let Some(v) = def.supports_reasoning {
        capabilities.reasoning_controls = v;
    }

    ModelProfile {
        alias: alias.to_string(),
        provider: def.provider.clone(),
        model_id: def.model_id.clone(),
        capabilities,
    }
}

/// Resolve API key: direct key takes priority, otherwise read from env var.
fn resolve_api_key_env(alias: &str, def: &ProfileDef) -> String {
    if def.api_key.is_some() {
        let env_name = format!("KAIROX_KEY_{}", alias.replace('-', "_").to_uppercase());
        if let Some(ref key) = def.api_key {
            std::env::set_var(&env_name, key);
        }
        env_name
    } else {
        def.api_key_env
            .clone()
            .unwrap_or_else(|| "OPENAI_API_KEY".to_string())
    }
}

fn build_client(alias: &str, def: &ProfileDef) -> Box<dyn ModelClient> {
    match provider_family(&def.provider) {
        "openai_compatible" => {
            let base_url = def
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let api_key_env = resolve_api_key_env(alias, def);

            let config = OpenAiCompatibleConfig {
                base_url,
                api_key_env,
                default_model: def.model_id.clone(),
                headers: Vec::new(),
                capability_overrides: None,
                temperature: None,
                top_p: None,
                extra_params: None,
            };
            Box::new(OpenAiCompatibleClient::new(config))
        }
        "anthropic" => {
            let base_url = def
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            let api_key_env = resolve_api_key_env(alias, def);

            let config = AnthropicConfig {
                base_url,
                api_key_env,
                default_model: def.model_id.clone(),
                max_tokens: def
                    .output_limit
                    .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
                headers: Vec::new(),
                capability_overrides: None,
                temperature: None,
                top_p: None,
                top_k: None,
                extra_params: None,
            };
            Box::new(AnthropicClient::new(config))
        }
        "ollama" => {
            let config = OllamaConfig {
                base_url: def
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                default_model: def.model_id.clone(),
                context_window: def
                    .context_window
                    .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            };
            Box::new(OllamaClient::new(config))
        }
        "fake" => {
            let response = def
                .response
                .clone()
                .unwrap_or_else(|| "hello from Kairox".to_string());
            Box::new(FakeModelClient::new(vec![response]))
        }
        _ => unreachable!("provider_family always returns a known family"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_models::ModelClient;
    use futures::StreamExt;

    #[test]
    fn build_router_registers_all_profiles() {
        let config = Config::defaults();
        let router = build_router(&config);
        let profiles = router.list_profiles();
        assert!(profiles.len() >= 2);
        assert!(profiles.iter().any(|p| p.alias == "fake"));
        assert!(profiles.iter().any(|p| p.alias == "local-code"));
    }

    #[tokio::test]
    async fn fake_profile_produces_tokens() {
        let config = Config::defaults();
        let router = build_router(&config);

        let mut stream = router
            .stream(agent_models::ModelRequest::user_text("fake", "hello"))
            .await
            .unwrap();

        let mut tokens = Vec::new();
        while let Some(event) = stream.next().await {
            match event {
                Ok(agent_models::ModelEvent::TokenDelta(d)) => tokens.push(d),
                Ok(agent_models::ModelEvent::Completed { .. }) => break,
                _ => {}
            }
        }

        assert!(!tokens.is_empty());
    }

    #[tokio::test]
    async fn unknown_profile_returns_error() {
        let config = Config::defaults();
        let router = build_router(&config);

        let result = router
            .stream(agent_models::ModelRequest::user_text(
                "nonexistent",
                "hello",
            ))
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn build_profile_sets_capabilities_per_provider() {
        let fast_def = ProfileDef {
            provider: "anthropic".into(),
            model_id: "claude-sonnet-4-20250514".into(),
            base_url: Some("https://api.anthropic.com".into()),
            api_key: None,
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            context_window: Some(200_000),
            output_limit: Some(16_384),
            response: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            headers: None,
            supports_tools: None,
            supports_vision: None,
            supports_reasoning: None,
            extra_params: None,
        };
        let profile = build_profile("fast", &fast_def);
        assert_eq!(profile.alias, "fast");
        assert!(profile.capabilities.tool_calling);
        assert!(!profile.capabilities.local_model);

        let ollama_def = ProfileDef {
            provider: "ollama".into(),
            model_id: "devstral".into(),
            base_url: Some("http://localhost:11434".into()),
            api_key: None,
            api_key_env: None,
            context_window: Some(128_000),
            output_limit: Some(16_384),
            response: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            headers: None,
            supports_tools: None,
            supports_vision: None,
            supports_reasoning: None,
            extra_params: None,
        };
        let profile = build_profile("local-code", &ollama_def);
        assert!(!profile.capabilities.tool_calling);
        assert!(profile.capabilities.local_model);
    }

    #[test]
    fn provider_family_maps_correctly() {
        assert_eq!(provider_family("anthropic"), "anthropic");
        assert_eq!(provider_family("ollama"), "ollama");
        assert_eq!(provider_family("fake"), "fake");
        assert_eq!(provider_family("openai_compatible"), "openai_compatible");
        assert_eq!(provider_family("deepseek"), "openai_compatible");
        assert_eq!(provider_family("groq"), "openai_compatible");
        assert_eq!(provider_family("xai"), "openai_compatible");
        assert_eq!(provider_family("unknown-thing"), "openai_compatible");
    }

    #[test]
    fn capability_overrides_from_profile_def() {
        let def = ProfileDef {
            provider: "deepseek".into(),
            model_id: "deepseek-chat".into(),
            base_url: Some("https://api.deepseek.com/v1".into()),
            api_key: None,
            api_key_env: Some("DEEPSEEK_API_KEY".into()),
            context_window: Some(128_000),
            output_limit: Some(32_768),
            response: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            headers: None,
            supports_tools: Some(false),
            supports_vision: Some(true),
            supports_reasoning: None,
            extra_params: None,
        };
        let profile = build_profile("deepseek", &def);
        // Overridden
        assert!(!profile.capabilities.tool_calling);
        assert!(profile.capabilities.vision);
        // Not overridden -- uses provider default (openai_compatible defaults)
        assert!(!profile.capabilities.reasoning_controls);
    }
}
