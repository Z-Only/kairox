use std::path::Path;

use agent_config::{Config, ProfileDef};
use agent_core::facade::{ProfileSettingsInput, ProfileSettingsView};
use agent_core::CoreError;
use toml_edit::{value, DocumentMut, Item, Table};

use super::row;

pub async fn upsert_profile_settings_in_file(
    config_path: &Path,
    input: &ProfileSettingsInput,
) -> agent_core::Result<ProfileSettingsView> {
    mutate_profiles_config(config_path, |document| {
        upsert_profile_table(document, input);
        Ok(())
    })
    .await?;
    settings_view_from_file(config_path, &input.alias).await
}

pub async fn set_profile_enabled_in_file(
    config_path: &Path,
    alias: &str,
    enabled: bool,
    config: &Config,
) -> agent_core::Result<()> {
    mutate_profiles_config(config_path, |document| {
        // If the profile doesn't exist yet in profiles.toml, seed it with
        // the full definition from the merged Config so we don't override
        // defaults with an empty table.
        let exists_in_file = document["profiles"]
            .as_table()
            .map(|t| t.contains_key(alias))
            .unwrap_or(false);
        if !exists_in_file {
            if let Some(def) = config.get_profile(alias) {
                let table = ensure_profile_table(document, alias);
                seed_profile_table(table, def);
            }
        }
        let profile_table = ensure_profile_table(document, alias);
        profile_table["enabled"] = value(enabled);
        Ok(())
    })
    .await
}

fn seed_profile_table(table: &mut Table, def: &ProfileDef) {
    table["provider"] = value(def.provider.clone());
    table["model_id"] = value(def.model_id.clone());
    table["enabled"] = value(def.enabled);
    if let Some(v) = def.context_window {
        table["context_window"] = value(v as i64);
    }
    if let Some(v) = def.output_limit {
        table["output_limit"] = value(v as i64);
    }
    if let Some(v) = def.temperature {
        table["temperature"] = value(v as f64);
    }
    if let Some(v) = def.top_p {
        table["top_p"] = value(v as f64);
    }
    if let Some(v) = def.top_k {
        table["top_k"] = value(v as i64);
    }
    if let Some(v) = def.max_tokens {
        table["max_tokens"] = value(v as i64);
    }
    if let Some(ref v) = def.base_url {
        if !v.is_empty() {
            table["base_url"] = value(v.clone());
        }
    }
    if let Some(ref v) = def.api_key_env {
        if !v.is_empty() {
            table["api_key_env"] = value(v.clone());
        }
    }
    if let Some(ref v) = def.api_key {
        if !v.is_empty() {
            table["api_key"] = value(v.clone());
        }
    }
}

pub async fn delete_profile_in_file(config_path: &Path, alias: &str) -> agent_core::Result<()> {
    mutate_profiles_config(config_path, |document| {
        if let Some(profiles) = document["profiles"].as_table_mut() {
            profiles.remove(alias);
        }
        Ok(())
    })
    .await
}

async fn settings_view_from_file(
    config_path: &Path,
    alias: &str,
) -> agent_core::Result<ProfileSettingsView> {
    let raw = tokio::fs::read_to_string(config_path)
        .await
        .map_err(|error| {
            CoreError::InvalidState(format!("failed to read profiles config: {error}"))
        })?;
    let document = super::parse_document(&raw)?;
    let profiles = document["profiles"].as_table().ok_or_else(|| {
        CoreError::InvalidState("profiles table missing after upsert".to_string())
    })?;
    let item = profiles
        .get(alias)
        .ok_or_else(|| CoreError::InvalidState(format!("saved profile not found: {alias}")))?;
    let row = row::profile_row_from_toml_table(item, "profiles_toml", true);
    Ok(ProfileSettingsView {
        alias: alias.to_string(),
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
        api_key_env: row.api_key_env,
        has_api_key: false,
        writable: true,
        config_path: Some(config_path.display().to_string()),
        source: row.source,
    })
}

pub(super) async fn mutate_profiles_config<F>(
    config_path: &Path,
    mutate: F,
) -> agent_core::Result<()>
where
    F: FnOnce(&mut DocumentMut) -> agent_core::Result<()>,
{
    let raw = match tokio::fs::read_to_string(config_path).await {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(CoreError::InvalidState(format!(
                "failed to read profiles config: {error}"
            )))
        }
    };
    let mut document = super::parse_document(&raw)?;
    mutate(&mut document)?;

    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            CoreError::InvalidState(format!(
                "failed to create profiles config directory: {error}"
            ))
        })?;
    }
    tokio::fs::write(config_path, document.to_string())
        .await
        .map_err(|error| {
            CoreError::InvalidState(format!("failed to write profiles config: {error}"))
        })
}

fn upsert_profile_table(document: &mut DocumentMut, input: &ProfileSettingsInput) {
    let profile_table = ensure_profile_table(document, &input.alias);
    profile_table["provider"] = value(input.provider.clone());
    profile_table["model_id"] = value(input.model_id.clone());
    profile_table["enabled"] = value(input.enabled);

    set_optional_int(profile_table, "context_window", input.context_window);
    set_optional_int(profile_table, "output_limit", input.output_limit);
    set_optional_float(profile_table, "temperature", input.temperature);
    set_optional_float(profile_table, "top_p", input.top_p);
    set_optional_int_32(profile_table, "top_k", input.top_k);
    set_optional_int(profile_table, "max_tokens", input.max_tokens);
    set_optional_string(profile_table, "base_url", &input.base_url);
    set_optional_string(profile_table, "api_key_env", &input.api_key_env);
}

fn ensure_profile_table<'a>(document: &'a mut DocumentMut, alias: &str) -> &'a mut Table {
    let profiles_table = ensure_profiles_table(document);
    if !profiles_table.contains_key(alias) || !profiles_table[alias].is_table() {
        profiles_table[alias] = Item::Table(Table::new());
    }
    profiles_table[alias]
        .as_table_mut()
        .expect("profile table should exist")
}

fn ensure_profiles_table(document: &mut DocumentMut) -> &mut Table {
    if !document.as_table().contains_key("profiles") || !document["profiles"].is_table() {
        document["profiles"] = Item::Table(Table::new());
    }
    document["profiles"]
        .as_table_mut()
        .expect("profiles table should exist")
}

fn set_optional_int(table: &mut Table, key: &str, val: Option<u64>) {
    match val {
        Some(v) => table[key] = value(v as i64),
        None => {
            table.remove(key);
        }
    }
}

fn set_optional_int_32(table: &mut Table, key: &str, val: Option<u32>) {
    match val {
        Some(v) => table[key] = value(v as i64),
        None => {
            table.remove(key);
        }
    }
}

fn set_optional_float(table: &mut Table, key: &str, val: Option<f32>) {
    match val {
        Some(v) => table[key] = value(v as f64),
        None => {
            table.remove(key);
        }
    }
}

fn set_optional_string(table: &mut Table, key: &str, val: &Option<String>) {
    match val {
        Some(v) if !v.is_empty() => table[key] = value(v.clone()),
        _ => {
            table.remove(key);
        }
    }
}
