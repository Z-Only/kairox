//! Three-layer fallback for `ModelLimits`:
//! 1. UserConfig — explicit `context_window` / `output_limit` in the TOML profile
//! 2. BuiltinRegistry — match on (provider, model_id) via `agent_models::lookup_limits`
//! 3. Fallback — provider-specific conservative default returned by the registry
//!
//! The optional fourth tier — RuntimeProbe — is applied by `agent-runtime` after a
//! session initialises (because only the runtime owns a live `OllamaClient`).

use crate::ProfileDef;
use agent_models::{lookup_limits, LimitSource, ModelLimits};

pub fn resolve_limits(profile: &ProfileDef) -> ModelLimits {
    if let (Some(ctx), Some(out)) = (profile.context_window, profile.output_limit) {
        return ModelLimits {
            context_window: ctx,
            output_limit: out,
            source: LimitSource::UserConfig,
        };
    }
    let from_table = lookup_limits(&profile.provider, &profile.model_id);
    if let Some(ctx) = profile.context_window {
        return ModelLimits {
            context_window: ctx,
            output_limit: from_table.output_limit,
            source: LimitSource::UserConfig,
        };
    }
    if let Some(out) = profile.output_limit {
        return ModelLimits {
            context_window: from_table.context_window,
            output_limit: out,
            source: LimitSource::UserConfig,
        };
    }
    from_table
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(provider: &str, model_id: &str, ctx: Option<u64>, out: Option<u64>) -> ProfileDef {
        ProfileDef {
            provider: provider.into(),
            model_id: model_id.into(),
            base_url: None,
            api_key: None,
            api_key_env: None,
            context_window: ctx,
            output_limit: out,
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
            enabled: true,
        }
    }

    #[test]
    fn user_config_wins_when_both_fields_set() {
        let limits = resolve_limits(&profile(
            "anthropic",
            "claude-sonnet-4",
            Some(50_000),
            Some(4_000),
        ));
        assert_eq!(limits.context_window, 50_000);
        assert_eq!(limits.output_limit, 4_000);
        assert_eq!(limits.source, LimitSource::UserConfig);
    }

    #[test]
    fn builtin_registry_used_when_user_omits_both_fields() {
        let limits = resolve_limits(&profile("anthropic", "claude-sonnet-4", None, None));
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.output_limit, 8_192);
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    }

    #[test]
    fn partial_override_keeps_other_field_from_registry() {
        let limits = resolve_limits(&profile("openai_compatible", "gpt-4o", Some(64_000), None));
        assert_eq!(limits.context_window, 64_000);
        assert_eq!(limits.output_limit, 16_384); // from registry
        assert_eq!(limits.source, LimitSource::UserConfig);
    }

    #[test]
    fn unknown_model_falls_back_to_provider_default() {
        let limits = resolve_limits(&profile("ollama", "weird-model", None, None));
        assert_eq!(limits.context_window, 8_192);
        assert_eq!(limits.source, LimitSource::Fallback);
    }
}
