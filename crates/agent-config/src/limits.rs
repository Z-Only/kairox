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
#[path = "limits_tests.rs"]
mod tests;
