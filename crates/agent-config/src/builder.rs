//! Build a ModelRouter from a Config.

use crate::{Config, ProfileDef};
use agent_models::{
    AnthropicClient, AnthropicConfig, FakeModelClient, ModelCapabilities, ModelClient,
    ModelProfile, ModelRouter, OllamaClient, OllamaConfig, OpenAiCompatibleClient,
    OpenAiCompatibleConfig,
};
use std::sync::Arc;

const CLAUDE_CODE_CLIENT_IDENTITY: &str = "claude_code";
const CLAUDE_CODE_BETA: &str = "claude-code-20250219";
const CODE_EXECUTION_BETA: &str = "code-execution-2025-08-25";
const CLAUDE_CODE_APP_NAME: &str = "claude-code";
const CLAUDE_CODE_APP_VERSION: &str = "1.0.0";

/// Build a `ModelRouter` from the given `Config`, registering a `ModelClient`
/// for each profile.
pub fn build_router(config: &Config) -> ModelRouter {
    let mut router = ModelRouter::new();

    for (alias, def) in &config.profiles {
        if !def.enabled {
            continue;
        }
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
        if !def.enabled || def.provider != "ollama" {
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

/// Map a profile definition to a client family.
/// Known providers map to their specific client. Custom providers normally map
/// to `openai_compatible`, but Anthropic-compatible gateways often keep their
/// own provider name while exposing an `/anthropic` base URL.
fn provider_family(def: &ProfileDef) -> &'static str {
    let provider = def.provider.to_ascii_lowercase();
    match provider.as_str() {
        "anthropic" => "anthropic",
        "ollama" => "ollama",
        "fake" => "fake",
        "openai_compatible" => "openai_compatible",
        _ if provider.contains("anthropic") || uses_anthropic_base_url(def.base_url.as_deref()) => {
            "anthropic"
        }
        _ => "openai_compatible",
    }
}

fn uses_anthropic_base_url(base_url: Option<&str>) -> bool {
    base_url
        .map(|url| {
            let normalized = url.trim_end_matches('/').to_ascii_lowercase();
            normalized.ends_with("/anthropic") || normalized.contains("/anthropic/")
        })
        .unwrap_or(false)
}

fn build_profile(alias: &str, def: &ProfileDef) -> ModelProfile {
    let family = provider_family(def);

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
    capabilities.reasoning_controls = crate::profile_supports_reasoning(def);

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

fn profile_headers(def: &ProfileDef) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if def
        .client_identity
        .as_deref()
        .is_some_and(is_claude_code_client_identity)
    {
        append_anthropic_beta(&mut headers, CLAUDE_CODE_BETA);
        headers.extend([
            ("x-app-name".to_string(), CLAUDE_CODE_APP_NAME.to_string()),
            ("x-app-ver".to_string(), CLAUDE_CODE_APP_VERSION.to_string()),
            ("x-app".to_string(), CLAUDE_CODE_APP_NAME.to_string()),
            ("user-agent".to_string(), CLAUDE_CODE_APP_NAME.to_string()),
        ]);
    }

    if def.server_tool_code_execution.unwrap_or(false) {
        append_anthropic_beta(&mut headers, CODE_EXECUTION_BETA);
    }

    if let Some(custom_headers) = &def.headers {
        for (key, value) in custom_headers {
            if key.eq_ignore_ascii_case("anthropic-beta") {
                for beta in value.split(',') {
                    append_anthropic_beta(&mut headers, beta);
                }
            } else {
                upsert_header(&mut headers, key.clone(), value.clone());
            }
        }
    }

    headers
}

fn is_claude_code_client_identity(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    normalized == CLAUDE_CODE_CLIENT_IDENTITY
}

fn upsert_header(headers: &mut Vec<(String, String)>, key: String, value: String) {
    if let Some((_, existing_value)) = headers
        .iter_mut()
        .find(|(existing_key, _)| existing_key.eq_ignore_ascii_case(&key))
    {
        *existing_value = value;
    } else {
        headers.push((key, value));
    }
}

fn append_anthropic_beta(headers: &mut Vec<(String, String)>, beta: &str) {
    let beta = beta.trim();
    if beta.is_empty() {
        return;
    }
    if let Some((_, existing_value)) = headers
        .iter_mut()
        .find(|(existing_key, _)| existing_key.eq_ignore_ascii_case("anthropic-beta"))
    {
        let already_present = existing_value
            .split(',')
            .map(str::trim)
            .any(|existing| existing == beta);
        if !already_present {
            if !existing_value.trim().is_empty() {
                existing_value.push(',');
            }
            existing_value.push_str(beta);
        }
    } else {
        headers.push(("anthropic-beta".to_string(), beta.to_string()));
    }
}

fn build_client(alias: &str, def: &ProfileDef) -> Box<dyn ModelClient> {
    match provider_family(def) {
        "openai_compatible" => {
            let base_url = def
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let api_key_env = resolve_api_key_env(alias, def);

            let headers = profile_headers(def);

            let extra_params: Option<serde_json::Value> = def.extra_params.as_ref().map(|v| {
                let json_str = serde_json::to_string(v).unwrap_or_else(|_| "null".to_string());
                serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null)
            });

            let config = OpenAiCompatibleConfig {
                base_url,
                api_key_env,
                default_model: def.model_id.clone(),
                headers,
                capability_overrides: None,
                temperature: def.temperature,
                top_p: def.top_p,
                extra_params,
            };
            Box::new(OpenAiCompatibleClient::new(config))
        }
        "anthropic" => {
            let base_url = def
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            let api_key_env = resolve_api_key_env(alias, def);

            let headers = profile_headers(def);

            let extra_params: Option<serde_json::Value> = def.extra_params.as_ref().map(|v| {
                let json_str = serde_json::to_string(v).unwrap_or_else(|_| "null".to_string());
                serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null)
            });

            let config = AnthropicConfig {
                base_url,
                api_key_env,
                default_model: def.model_id.clone(),
                max_tokens: def
                    .output_limit
                    .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
                headers,
                capability_overrides: None,
                temperature: def.temperature,
                top_p: def.top_p,
                top_k: def.top_k,
                extra_params,
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
#[path = "builder_tests.rs"]
mod tests;
