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
async fn list_profile_settings_does_not_label_project_profiles_as_defaults_in_user_scope() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let config_dir = project_dir.path().join(".kairox");
    std::fs::create_dir_all(&config_dir).expect("config dir should be created");
    let project_config_path = config_dir.join("config.toml");
    std::fs::write(
        &project_config_path,
        r#"
[profiles.project-only]
provider = "anthropic"
model_id = "claude-opus"
enabled = true
"#,
    )
    .expect("project config should be written");
    let effective_config = agent_config::load_from_str(
        r#"
[profiles.project-only]
provider = "anthropic"
model_id = "claude-opus"
enabled = true
"#,
        "effective-project.toml",
    )
    .expect("effective project config should parse");

    let user_views = list_profile_settings(
        &effective_config,
        None,
        None,
        Some(&project_config_path),
        Some("user"),
    )
    .await
    .expect("user profile settings should list");

    assert!(
        user_views
            .iter()
            .all(|profile| profile.alias != "project-only"),
        "project-only profile must not leak into the user scope as Defaults: {user_views:?}"
    );

    let project_views = list_profile_settings(
        &effective_config,
        None,
        None,
        Some(&project_config_path),
        Some("project"),
    )
    .await
    .expect("project profile settings should list");
    let project_profile = project_views
        .iter()
        .find(|profile| profile.alias == "project-only")
        .expect("project-only profile should be visible in project scope");

    assert_eq!(project_profile.source, "project_config");
}

#[tokio::test]
async fn list_profile_settings_preserves_runtime_defaults_profiles() {
    let mut config = agent_config::load_from_str(
        r#"
[profiles.reasoning]
provider = "fake"
model_id = "fake-reasoning"
enabled = true
supports_reasoning = true
"#,
        "runtime-defaults.toml",
    )
    .expect("runtime config should parse");
    config.source = agent_config::ConfigSource::Defaults;

    let views = list_profile_settings(&config, None, None, None, None)
        .await
        .expect("profile settings should list");
    let profile = views
        .iter()
        .find(|profile| profile.alias == "reasoning")
        .expect("runtime defaults profile should stay visible");

    assert_eq!(profile.source, "defaults");
    assert!(!profile.writable);
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
        api_key: None,
        api_key_env: Some("OPENAI_API_KEY".to_string()),
        client_identity: Some("claude_code".to_string()),
        supports_reasoning: Some(true),
    };

    let view = upsert_profile_settings_in_file(&config_path, &input)
        .await
        .expect("profile should be written");

    assert_eq!(view.alias, "my-model");
    assert_eq!(view.provider, "openai_compatible");
    assert_eq!(view.model_id, "gpt-4.1");
    assert!(view.enabled);
    assert_eq!(view.source, "user_config");
    assert_eq!(view.temperature, Some(0.7));
    assert_eq!(view.supports_reasoning, Some(true));

    let raw = tokio::fs::read_to_string(config_path)
        .await
        .expect("config should read");
    assert!(raw.contains("[profiles.my-model]"));
    assert!(raw.contains("provider = \"openai_compatible\""));
    assert!(raw.contains("context_window = 128000"));
    assert!(raw.contains("temperature = "));
    assert!(raw.contains("client_identity = \"claude_code\""));
    assert!(raw.contains("supports_reasoning = true"));
}

#[tokio::test]
async fn upsert_masks_api_key_but_reports_presence() {
    let config_path = write_profiles_config_fixture("");
    let input = ProfileSettingsInput {
        alias: "keyed".to_string(),
        provider: "openai_compatible".to_string(),
        model_id: "gpt-4.1".to_string(),
        enabled: true,
        context_window: None,
        output_limit: None,
        temperature: None,
        top_p: None,
        top_k: None,
        max_tokens: None,
        base_url: Some("https://api.example.test/v1".to_string()),
        api_key: Some("secret-key".to_string()),
        api_key_env: None,
        client_identity: None,
        supports_reasoning: None,
    };

    let view = upsert_profile_settings_in_file(&config_path, &input)
        .await
        .expect("profile should be written");

    assert_eq!(view.api_key, None);
    assert!(view.has_api_key);
}

#[tokio::test]
async fn list_profile_settings_exposes_supports_reasoning_override() {
    let config_path = write_profiles_config_fixture(
        "[profiles.reasoning]\nprovider = \"anthropic\"\nmodel_id = \"claude-opus-4-6\"\nsupports_reasoning = true\n",
    );

    let views = list_profile_settings(
        &agent_config::Config::defaults(),
        Some(&config_path),
        None,
        None,
        None,
    )
    .await
    .expect("profile settings should list");
    let profile = views
        .iter()
        .find(|profile| profile.alias == "reasoning")
        .expect("profile should be visible");

    assert_eq!(profile.supports_reasoning, Some(true));
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
        api_key: None,
        api_key_env: None,
        client_identity: None,
        supports_reasoning: None,
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
