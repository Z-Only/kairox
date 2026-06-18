use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_config::{Config, ConfigSource};
use agent_core::facade::ProfileSettingsView;
use agent_core::CoreError;
use toml_edit::DocumentMut;

use super::order;
use super::row::{self, ProfileSettingsRow};

const CONFIG_FILE_NAME: &str = "config.toml";

pub fn writable_profiles_config_path(
    config_dir: Option<&Path>,
) -> agent_core::Result<Option<PathBuf>> {
    Ok(config_dir.map(|dir| dir.join(CONFIG_FILE_NAME)))
}

pub async fn list_profile_settings(
    config: &Config,
    profiles_toml_path: Option<&Path>,
    user_config_path: Option<&Path>,
    project_config_path: Option<&Path>,
    source_filter: Option<&str>,
) -> agent_core::Result<Vec<ProfileSettingsView>> {
    let mut rows: BTreeMap<String, ProfileSettingsRow> = BTreeMap::new();

    // Layer 1: built-in defaults (lowest priority). `config.profiles` is an
    // already-merged effective view, so using it here would relabel user or
    // project profiles as "defaults" in the settings UI.
    for (alias, def) in Config::defaults().profiles {
        rows.insert(
            alias,
            ProfileSettingsRow {
                provider: def.provider,
                model_id: def.model_id,
                enabled: def.enabled,
                context_window: def.context_window,
                output_limit: def.output_limit,
                temperature: def.temperature,
                top_p: def.top_p,
                top_k: def.top_k,
                max_tokens: def.max_tokens,
                base_url: def.base_url,
                api_key_env: def.api_key_env,
                api_key: def.api_key,
                client_identity: def.client_identity,
                supports_reasoning: def.supports_reasoning,
                source: "defaults".to_string(),
                writable: false,
            },
        );
    }

    if config.source == ConfigSource::Defaults {
        for (alias, def) in &config.profiles {
            rows.insert(
                alias.clone(),
                ProfileSettingsRow {
                    provider: def.provider.clone(),
                    model_id: def.model_id.clone(),
                    enabled: def.enabled,
                    context_window: def.context_window,
                    output_limit: def.output_limit,
                    temperature: def.temperature,
                    top_p: def.top_p,
                    top_k: def.top_k,
                    max_tokens: def.max_tokens,
                    base_url: def.base_url.clone(),
                    api_key_env: def.api_key_env.clone(),
                    api_key: def.api_key.clone(),
                    client_identity: def.client_identity.clone(),
                    supports_reasoning: def.supports_reasoning,
                    source: "defaults".to_string(),
                    writable: false,
                },
            );
        }
    }

    // Layer 2: profiles.toml overrides defaults
    if let Some(path) = profiles_toml_path {
        if path.exists() {
            let raw = tokio::fs::read_to_string(path).await.map_err(|error| {
                CoreError::InvalidState(format!("failed to read profiles config: {error}"))
            })?;
            let document = super::parse_document(&raw)?;
            if let Some(profiles) = document.get("profiles").and_then(|item| item.as_table()) {
                for (alias, item) in profiles.iter() {
                    let alias_str = alias.to_string();
                    let row = row::profile_row_from_toml_table(item, "profiles_toml", true);
                    rows.insert(alias_str, row);
                }
            }
        }
    }

    // Layer 3: user config.toml overrides profiles.toml
    // (skip when source_filter == "project")
    if source_filter != Some("project") {
        if let Some(path) = user_config_path {
            if path.exists() {
                if let Some(file_rows) = rows_from_config_toml(path, "user_config", true).await? {
                    for (alias, row) in file_rows {
                        rows.insert(alias, row);
                    }
                }
            }
        }
    }

    // Layer 4: project config.toml overrides everything (highest priority)
    // (skip when source_filter == "user")
    if source_filter != Some("user") {
        if let Some(path) = project_config_path {
            if path.exists() {
                if let Some(file_rows) =
                    rows_from_config_toml(path, "project_config", false).await?
                {
                    for (alias, row) in file_rows {
                        rows.insert(alias, row);
                    }
                }
            }
        }
    }

    let mut views: Vec<ProfileSettingsView> = rows
        .into_iter()
        .map(|(alias, row)| {
            let has_api_key = row.api_key.is_some()
                || row
                    .api_key_env
                    .as_ref()
                    .is_some_and(|v| std::env::var(v).is_ok());
            let config_path = match row.source.as_str() {
                "profiles_toml" => profiles_toml_path,
                "user_config" => user_config_path,
                "project_config" => project_config_path,
                _ => None,
            };
            ProfileSettingsView {
                alias: alias.clone(),
                provider: row.provider,
                model_id: row.model_id,
                enabled: row.enabled,
                context_window: row.context_window,
                output_limit: row.output_limit,
                temperature: row.temperature,
                top_p: row.top_p,
                top_k: row.top_k,
                max_tokens: row.max_tokens,
                base_url: row.base_url,
                api_key: None, // masked for security; use has_api_key to check presence
                api_key_env: row.api_key_env,
                client_identity: row.client_identity,
                supports_reasoning: row.supports_reasoning,
                has_api_key,
                writable: row.writable,
                config_path: config_path.map(|p| p.display().to_string()),
                source: row.source,
            }
        })
        .filter(|view| view.source != "defaults" || view.enabled)
        .collect();

    let mut display_order: Vec<String> = Vec::new();
    let display_order_path = if source_filter == Some("project") {
        project_config_path.or(user_config_path)
    } else {
        user_config_path.or(profiles_toml_path)
    };
    if let Some(path) = display_order_path {
        if path.exists() {
            if let Ok(raw) = tokio::fs::read_to_string(path).await {
                if let Ok(doc) = raw.parse::<DocumentMut>() {
                    display_order = order::load_display_order_from_doc(&doc);
                }
            }
        }
    }
    order::sort_by_display_order(&mut views, &display_order);
    Ok(views)
}

async fn rows_from_config_toml(
    path: &Path,
    source: &str,
    writable: bool,
) -> agent_core::Result<Option<BTreeMap<String, ProfileSettingsRow>>> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to read config: {error}")))?;
    let document = super::parse_document(&raw)?;
    let Some(profiles) = document.get("profiles").and_then(|item| item.as_table()) else {
        return Ok(None);
    };

    let mut rows = BTreeMap::new();
    for (alias, item) in profiles.iter() {
        let alias_str = alias.to_string();
        let row = row::profile_row_from_toml_table(item, source, writable);
        rows.insert(alias_str, row);
    }
    Ok(Some(rows))
}
