//! Built-in model context window registry. Provides best-known
//! `context_window` and `output_limit` values for popular OpenAI and
//! Anthropic model ids. Used as the second tier of the three-layer
//! fallback (UserConfig > BuiltinRegistry > RuntimeProbe > Fallback).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LimitSource {
    UserConfig,
    BuiltinRegistry,
    RuntimeProbe,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ModelLimits {
    pub context_window: u64,
    pub output_limit: u64,
    pub source: LimitSource,
}

struct ModelInfo {
    pattern: &'static str,
    context_window: u64,
    output_limit: u64,
}

const OPENAI: &[ModelInfo] = &[
    ModelInfo {
        pattern: "gpt-4.1",
        context_window: 1_048_576,
        output_limit: 32_768,
    },
    ModelInfo {
        pattern: "gpt-4o-mini",
        context_window: 128_000,
        output_limit: 16_384,
    },
    ModelInfo {
        pattern: "gpt-4o",
        context_window: 128_000,
        output_limit: 16_384,
    },
    ModelInfo {
        pattern: "gpt-4-turbo",
        context_window: 128_000,
        output_limit: 4_096,
    },
    ModelInfo {
        pattern: "gpt-3.5-turbo",
        context_window: 16_385,
        output_limit: 4_096,
    },
    ModelInfo {
        pattern: "o1-mini",
        context_window: 128_000,
        output_limit: 65_536,
    },
    ModelInfo {
        pattern: "o1",
        context_window: 200_000,
        output_limit: 100_000,
    },
];

const ANTHROPIC: &[ModelInfo] = &[
    ModelInfo {
        pattern: "claude-opus-4",
        context_window: 200_000,
        output_limit: 8_192,
    },
    ModelInfo {
        pattern: "claude-sonnet-4",
        context_window: 200_000,
        output_limit: 8_192,
    },
    ModelInfo {
        pattern: "claude-3-7-sonnet",
        context_window: 200_000,
        output_limit: 64_000,
    },
    ModelInfo {
        pattern: "claude-3-5-sonnet",
        context_window: 200_000,
        output_limit: 8_192,
    },
    ModelInfo {
        pattern: "claude-3-5-haiku",
        context_window: 200_000,
        output_limit: 8_192,
    },
    ModelInfo {
        pattern: "claude-3-opus",
        context_window: 200_000,
        output_limit: 4_096,
    },
    ModelInfo {
        pattern: "claude-3-haiku",
        context_window: 200_000,
        output_limit: 4_096,
    },
];

const FALLBACK_OLLAMA: ModelLimits = ModelLimits {
    context_window: 8_192,
    output_limit: 2_048,
    source: LimitSource::Fallback,
};
const FALLBACK_FAKE: ModelLimits = ModelLimits {
    context_window: 4_096,
    // Small completions only — keep `output_reservation` (= output_limit +
    // 2k safety floor) well below `context_window` so the fake-driven
    // integration tests have a non-trivial input budget.
    output_limit: 256,
    source: LimitSource::Fallback,
};
const FALLBACK_GENERIC: ModelLimits = ModelLimits {
    context_window: 128_000,
    output_limit: 16_384,
    source: LimitSource::Fallback,
};

fn match_table(table: &'static [ModelInfo], model_id: &str) -> Option<ModelLimits> {
    let mut entries: Vec<&ModelInfo> = table.iter().collect();
    entries.sort_by_key(|info| std::cmp::Reverse(info.pattern.len()));
    for info in entries {
        if model_id.starts_with(info.pattern) {
            return Some(ModelLimits {
                context_window: info.context_window,
                output_limit: info.output_limit,
                source: LimitSource::BuiltinRegistry,
            });
        }
    }
    None
}

/// Look up the built-in limits for a (provider, model_id) pair.
pub fn lookup(provider: &str, model_id: &str) -> ModelLimits {
    match provider {
        "openai" | "openai_compatible" => match_table(OPENAI, model_id).unwrap_or(FALLBACK_GENERIC),
        "anthropic" => match_table(ANTHROPIC, model_id).unwrap_or(FALLBACK_GENERIC),
        "ollama" => FALLBACK_OLLAMA,
        "fake" => FALLBACK_FAKE,
        _ => FALLBACK_GENERIC,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_gpt_4o_with_builtin_source() {
        let limits = lookup("openai_compatible", "gpt-4o");
        assert_eq!(limits.context_window, 128_000);
        assert_eq!(limits.output_limit, 16_384);
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    }

    #[test]
    fn longest_prefix_wins_for_overlapping_anthropic_patterns() {
        let limits = lookup("anthropic", "claude-3-5-sonnet-20240620");
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.output_limit, 8_192);
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    }

    #[test]
    fn returns_provider_fallback_for_unknown_openai_model() {
        let limits = lookup("openai_compatible", "gpt-future-9000");
        assert_eq!(limits.source, LimitSource::Fallback);
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn ollama_provider_always_returns_conservative_fallback() {
        let limits = lookup("ollama", "llama3:70b");
        assert_eq!(limits.context_window, 8_192);
        assert_eq!(limits.source, LimitSource::Fallback);
    }

    #[test]
    fn fake_provider_returns_small_window() {
        let limits = lookup("fake", "fake");
        assert_eq!(limits.context_window, 4_096);
        assert_eq!(limits.source, LimitSource::Fallback);
    }

    #[test]
    fn unknown_provider_returns_generic_fallback() {
        let limits = lookup("custom", "anything");
        assert_eq!(limits.source, LimitSource::Fallback);
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_openai_gpt41_returns_builtin() {
        let limits = lookup("openai_compatible", "gpt-4.1");
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
        assert!(limits.context_window > 0);
    }

    #[test]
    fn lookup_openai_unknown_model_returns_fallback() {
        let limits = lookup("openai", "unknown-model-xyz");
        assert_eq!(limits.source, LimitSource::Fallback);
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_anthropic_claude_sonnet_4() {
        let limits = lookup("anthropic", "claude-sonnet-4-20250514");
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
        assert!(limits.context_window > 0);
    }

    #[test]
    fn lookup_anthropic_unknown_returns_fallback() {
        let limits = lookup("anthropic", "unknown-model");
        assert_eq!(limits.source, LimitSource::Fallback);
    }

    #[test]
    fn lookup_ollama_always_fallback() {
        let limits = lookup("ollama", "llama3:latest");
        assert_eq!(limits.source, LimitSource::Fallback);
        assert_eq!(limits.context_window, 8_192);
        assert_eq!(limits.output_limit, 2_048);
    }

    #[test]
    fn lookup_fake_always_fallback() {
        let limits = lookup("fake", "any-model");
        assert_eq!(limits.source, LimitSource::Fallback);
        assert_eq!(limits.context_window, 4_096);
        assert_eq!(limits.output_limit, 256);
    }

    #[test]
    fn lookup_unknown_provider_returns_generic_fallback() {
        let limits = lookup("deepseek", "deepseek-v3");
        assert_eq!(limits.source, LimitSource::Fallback);
        assert_eq!(limits.context_window, 128_000);
    }

    #[test]
    fn lookup_prefix_matching_chooses_longest_match() {
        let limits = lookup("anthropic", "claude-3-5-sonnet-20241022");
        assert_eq!(limits.source, LimitSource::BuiltinRegistry);
        assert_eq!(limits.context_window, 200_000);
        assert_eq!(limits.output_limit, 8_192);
    }

    #[test]
    fn limits_source_variants_distinct() {
        assert_ne!(LimitSource::UserConfig, LimitSource::Fallback);
        assert_ne!(LimitSource::UserConfig, LimitSource::BuiltinRegistry);
        assert_ne!(LimitSource::BuiltinRegistry, LimitSource::Fallback);
    }
}
