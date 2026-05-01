//! Build a ModelRouter from a Config.

use crate::{Config, ProfileDef};
use agent_models::{
    FakeModelClient, ModelCapabilities, ModelClient, ModelProfile, ModelRouter, OllamaClient,
    OllamaConfig, OpenAiCompatibleClient, OpenAiCompatibleConfig,
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

fn build_profile(alias: &str, def: &ProfileDef) -> ModelProfile {
    let capabilities = match def.provider.as_str() {
        "openai_compatible" => ModelCapabilities {
            streaming: true,
            tool_calling: true,
            json_schema: true,
            vision: false,
            reasoning_controls: false,
            context_window: def.context_window,
            output_limit: def.output_limit,
            local_model: false,
        },
        "ollama" => ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def.context_window,
            output_limit: def.output_limit,
            local_model: true,
        },
        "fake" => ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def.context_window,
            output_limit: def.output_limit,
            local_model: true,
        },
        _ => ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def.context_window,
            output_limit: def.output_limit,
            local_model: false,
        },
    };

    ModelProfile {
        alias: alias.to_string(),
        provider: def.provider.clone(),
        model_id: def.model_id.clone(),
        capabilities,
    }
}

fn build_client(alias: &str, def: &ProfileDef) -> Box<dyn ModelClient> {
    match def.provider.as_str() {
        "openai_compatible" => {
            let base_url = def
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

            // If api_key is set directly (not via env var), create a synthetic env var
            // so that OpenAiCompatibleConfig can read it at request time.
            // This avoids mutating global env vars in a conflicting way.
            let api_key_env = if def.api_key.is_some() {
                let env_name = format!("KAIROX_KEY_{}", alias.replace('-', "_").to_uppercase());
                if let Some(ref key) = def.api_key {
                    std::env::set_var(&env_name, key);
                }
                env_name
            } else {
                def.api_key_env
                    .clone()
                    .unwrap_or_else(|| "OPENAI_API_KEY".to_string())
            };

            let config = OpenAiCompatibleConfig {
                base_url,
                api_key_env,
                default_model: def.model_id.clone(),
                headers: Vec::new(),
                capability_overrides: None,
            };
            Box::new(OpenAiCompatibleClient::new(config))
        }
        "ollama" => {
            let config = OllamaConfig {
                base_url: def
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                default_model: def.model_id.clone(),
                context_window: def.context_window,
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
        _ => {
            tracing::warn!(
                "Unknown provider '{}' for profile '{}', using fake client",
                def.provider,
                alias
            );
            Box::new(FakeModelClient::new(vec![
                "unknown provider fallback".into()
            ]))
        }
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
        assert!(profiles.len() >= 2); // at least fake and local-code
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
            provider: "openai_compatible".into(),
            model_id: "gpt-4.1-mini".into(),
            base_url: Some("https://api.openai.com/v1".into()),
            api_key: None,
            api_key_env: Some("OPENAI_API_KEY".into()),
            context_window: 128_000,
            output_limit: 16_384,
            response: None,
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
            context_window: 128_000,
            output_limit: 16_384,
            response: None,
        };
        let profile = build_profile("local-code", &ollama_def);
        assert!(!profile.capabilities.tool_calling);
        assert!(profile.capabilities.local_model);
    }
}
