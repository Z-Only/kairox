use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_config::{Config, ProfileDef};
use agent_core::facade::{ProfileSettingsInput, ProfileSettingsView};
use agent_core::CoreError;
use toml_edit::{value, DocumentMut, Item, Table};

const PROFILES_FILE_NAME: &str = "profiles.toml";

pub fn writable_profiles_config_path(
    config_dir: Option<&Path>,
) -> agent_core::Result<Option<PathBuf>> {
    Ok(config_dir.map(|dir| dir.join(PROFILES_FILE_NAME)))
}

#[derive(Debug, Clone)]
struct ProfileSettingsRow {
    provider: String,
    model_id: String,
    enabled: bool,
    context_window: Option<u64>,
    output_limit: Option<u64>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    max_tokens: Option<u64>,
    base_url: Option<String>,
    api_key_env: Option<String>,
    api_key: Option<String>,
    /// Where this profile was found: "defaults", "profiles_toml", "user_config", "project_config"
    source: String,
    writable: bool,
}

pub async fn list_profile_settings(
    config: &Config,
    profiles_toml_path: Option<&Path>,
    user_config_path: Option<&Path>,
    project_config_path: Option<&Path>,
    source_filter: Option<&str>,
) -> agent_core::Result<Vec<ProfileSettingsView>> {
    let mut rows: BTreeMap<String, ProfileSettingsRow> = BTreeMap::new();

    // Layer 1: defaults (lowest priority)
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
                source: "defaults".to_string(),
                writable: false,
            },
        );
    }

    // Layer 2: profiles.toml overrides defaults
    if let Some(path) = profiles_toml_path {
        if path.exists() {
            let raw = tokio::fs::read_to_string(path).await.map_err(|error| {
                CoreError::InvalidState(format!("failed to read profiles config: {error}"))
            })?;
            let document = parse_document(&raw)?;
            if let Some(profiles) = document["profiles"].as_table() {
                for (alias, item) in profiles.iter() {
                    let alias_str = alias.to_string();
                    let row = profile_row_from_toml_table(item, "profiles_toml", true);
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
                if let Some(file_rows) = rows_from_config_toml(path, "user_config", false).await? {
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
                api_key_env: row.api_key_env,
                has_api_key,
                writable: row.writable,
                config_path: profiles_toml_path.map(|p| p.display().to_string()),
                source: row.source,
            }
        })
        .filter(|view| view.source != "defaults" || view.enabled)
        .collect();

    let mut display_order: Vec<String> = Vec::new();
    if let Some(path) = profiles_toml_path {
        if path.exists() {
            if let Ok(raw) = tokio::fs::read_to_string(path).await {
                if let Ok(doc) = raw.parse::<DocumentMut>() {
                    display_order = load_display_order(&doc);
                }
            }
        }
    }
    sort_by_display_order(&mut views, &display_order);
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
    let document = parse_document(&raw)?;
    let Some(profiles) = document["profiles"].as_table() else {
        return Ok(None);
    };

    let mut rows = BTreeMap::new();
    for (alias, item) in profiles.iter() {
        let alias_str = alias.to_string();
        let row = profile_row_from_toml_table(item, source, writable);
        rows.insert(alias_str, row);
    }
    Ok(Some(rows))
}

fn profile_row_from_toml_table(item: &Item, source: &str, writable: bool) -> ProfileSettingsRow {
    let table = item.as_table();
    ProfileSettingsRow {
        provider: table
            .and_then(|t| t.get("provider"))
            .and_then(Item::as_str)
            .unwrap_or("")
            .to_string(),
        model_id: table
            .and_then(|t| t.get("model_id"))
            .and_then(Item::as_str)
            .unwrap_or("")
            .to_string(),
        enabled: table
            .and_then(|t| t.get("enabled"))
            .and_then(Item::as_bool)
            .unwrap_or(true),
        context_window: table
            .and_then(|t| t.get("context_window"))
            .and_then(Item::as_integer)
            .map(|v| v as u64),
        output_limit: table
            .and_then(|t| t.get("output_limit"))
            .and_then(Item::as_integer)
            .map(|v| v as u64),
        temperature: table
            .and_then(|t| t.get("temperature"))
            .and_then(Item::as_float)
            .map(|v| v as f32),
        top_p: table
            .and_then(|t| t.get("top_p"))
            .and_then(Item::as_float)
            .map(|v| v as f32),
        top_k: table
            .and_then(|t| t.get("top_k"))
            .and_then(Item::as_integer)
            .map(|v| v as u32),
        max_tokens: table
            .and_then(|t| t.get("max_tokens"))
            .and_then(Item::as_integer)
            .map(|v| v as u64),
        base_url: table
            .and_then(|t| t.get("base_url"))
            .and_then(Item::as_str)
            .map(ToString::to_string),
        api_key_env: table
            .and_then(|t| t.get("api_key_env"))
            .and_then(Item::as_str)
            .map(ToString::to_string),
        api_key: table
            .and_then(|t| t.get("api_key"))
            .and_then(Item::as_str)
            .map(ToString::to_string),
        source: source.to_string(),
        writable,
    }
}

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
}

// -- display ordering helpers --

fn load_display_order(document: &DocumentMut) -> Vec<String> {
    document
        .get("display_order")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn save_display_order(document: &mut DocumentMut, order: &[String]) {
    let array =
        toml_edit::Array::from_iter(order.iter().map(|s| toml_edit::Value::from(s.clone())));
    document["display_order"] = toml_edit::Item::Value(toml_edit::Value::Array(array));
}

fn sort_by_display_order(views: &mut [ProfileSettingsView], display_order: &[String]) {
    views.sort_by(|a, b| {
        let pos_a = display_order.iter().position(|s| s == &a.alias);
        let pos_b = display_order.iter().position(|s| s == &b.alias);
        match (pos_a, pos_b) {
            (Some(pa), Some(pb)) => pa.cmp(&pb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.alias.cmp(&b.alias),
        }
    });
}

pub async fn move_profile_in_order(
    config_path: &Path,
    alias: &str,
    direction: i32, // -1 for up, +1 for down
) -> agent_core::Result<()> {
    mutate_profiles_config(config_path, |document| {
        let mut order = load_display_order(document);
        if let Some(pos) = order.iter().position(|s| s == alias) {
            let new_pos = if direction < 0 {
                pos.saturating_sub(1)
            } else {
                (pos + 1).min(order.len().saturating_sub(1))
            };
            if new_pos != pos {
                order.swap(pos, new_pos);
                save_display_order(document, &order);
            }
        } else {
            // Profile not in order yet — add it at the end
            order.push(alias.to_string());
            save_display_order(document, &order);
        }
        Ok(())
    })
    .await
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
    let document = parse_document(&raw)?;
    let profiles = document["profiles"].as_table().ok_or_else(|| {
        CoreError::InvalidState("profiles table missing after upsert".to_string())
    })?;
    let item = profiles
        .get(alias)
        .ok_or_else(|| CoreError::InvalidState(format!("saved profile not found: {alias}")))?;
    let row = profile_row_from_toml_table(item, "profiles_toml", true);
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

async fn mutate_profiles_config<F>(config_path: &Path, mutate: F) -> agent_core::Result<()>
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
    let mut document = parse_document(&raw)?;
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

fn parse_document(raw: &str) -> agent_core::Result<DocumentMut> {
    raw.parse::<DocumentMut>().map_err(|error| {
        CoreError::InvalidState(format!("failed to parse profiles config: {error}"))
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let config = Config::defaults();

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
