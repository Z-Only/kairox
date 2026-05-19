mod order;
mod row;
mod view;
mod write;

use agent_core::CoreError;
use toml_edit::DocumentMut;

// ── Re-exports: public API ─────────────────────────────────────────────────

pub use order::move_profile_in_order;
pub use view::{list_profile_settings, writable_profiles_config_path};
pub use write::{
    delete_profile_in_file, set_profile_enabled_in_file, upsert_profile_settings_in_file,
};

// ── Shared helpers ─────────────────────────────────────────────────────────

pub(super) fn parse_document(raw: &str) -> agent_core::Result<DocumentMut> {
    raw.parse::<DocumentMut>().map_err(|error| {
        CoreError::InvalidState(format!("failed to parse profiles config: {error}"))
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::facade::ProfileSettingsInput;
    use std::path::PathBuf;

    fn write_profiles_config_fixture(raw: &str) -> PathBuf {
        let file = tempfile::NamedTempFile::new().expect("temp file should be created");
        let (_file, config_path) = file.keep().expect("temp file path should be kept");
        std::fs::write(&config_path, raw).expect("config fixture should be written");
        config_path
    }

    #[tokio::test]
    async fn upsert_writes_profile_settings() {
        let config_path = write_profiles_config_fixture("");
        let input = ProfileSettingsInput {
            alias: "my-model".to_string(),
            provider: "openai_compatible".to_string(),
            model_id: "gpt-4.1".to_string(),
            enabled: true,
            context_window: Some(128000),
            output_limit: Some(16384),
            temperature: Some(0.7),
            top_p: None,
            top_k: None,
            max_tokens: None,
            base_url: Some("https://api.openai.com/v1".to_string()),
            api_key_env: Some("OPENAI_API_KEY".to_string()),
        };

        let view = upsert_profile_settings_in_file(&config_path, &input)
            .await
            .expect("profile should be written");

        assert_eq!(view.alias, "my-model");
        assert_eq!(view.provider, "openai_compatible");
        assert_eq!(view.model_id, "gpt-4.1");
        assert!(view.enabled);
        assert_eq!(view.temperature, Some(0.7));

        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(raw.contains("[profiles.my-model]"));
        assert!(raw.contains("provider = \"openai_compatible\""));
        assert!(raw.contains("context_window = 128000"));
        assert!(raw.contains("temperature = "));
    }

    #[tokio::test]
    async fn set_profile_enabled_toggles_flag() {
        let config_path = write_profiles_config_fixture(
            "[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\nenabled = true\n",
        );
        let config = agent_config::Config::defaults();

        set_profile_enabled_in_file(&config_path, "fast", false, &config)
            .await
            .expect("profile should be disabled");

        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(raw.contains("enabled = false"));
    }

    #[tokio::test]
    async fn delete_profile_removes_table() {
        let config_path = write_profiles_config_fixture(
            "[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\nenabled = true\n",
        );

        delete_profile_in_file(&config_path, "fast")
            .await
            .expect("profile should be deleted");

        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(!raw.contains("[profiles.fast]"));
    }

    #[tokio::test]
    async fn upsert_preserves_other_profiles_and_unknown_fields() {
        let config_path = write_profiles_config_fixture(
            "[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\nunknown = \"keep\"\nenabled = true\n\n[other_section]\nkey = \"value\"\n",
        );
        let input = ProfileSettingsInput {
            alias: "new-model".to_string(),
            provider: "ollama".to_string(),
            model_id: "llama3".to_string(),
            enabled: false,
            context_window: None,
            output_limit: None,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            base_url: None,
            api_key_env: None,
        };

        upsert_profile_settings_in_file(&config_path, &input)
            .await
            .expect("profile should be written");

        let raw = tokio::fs::read_to_string(config_path)
            .await
            .expect("config should read");
        assert!(raw.contains("[profiles.fast]"));
        assert!(raw.contains("unknown = \"keep\""));
        assert!(raw.contains("[profiles.new-model]"));
        assert!(raw.contains("[other_section]"));
        assert!(raw.contains("key = \"value\""));
    }
}
