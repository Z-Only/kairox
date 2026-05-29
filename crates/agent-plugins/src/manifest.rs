use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::{PluginError, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginManifestKind {
    Kairox,
    Codex,
    Claude,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PluginInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub category: Option<String>,
    pub brand_color: Option<String>,
    pub logo: Option<String>,
    pub composer_icon: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PluginComponentInventory {
    pub skill_count: usize,
    pub skill_names: Vec<String>,
    pub mcp_server_count: usize,
    pub app_count: usize,
    pub agent_count: usize,
    pub hook_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginManifestView {
    pub name: String,
    pub version: Option<String>,
    pub description: String,
    pub author_name: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub keywords: Vec<String>,
    pub interface: PluginInterface,
    pub inventory: PluginComponentInventory,
    pub manifest_kind: PluginManifestKind,
    pub manifest_path: PathBuf,
    pub plugin_root: PathBuf,
    pub valid: bool,
    pub validation_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPluginManifest {
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    author: Option<RawAuthor>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    repository: Option<Value>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    skills: Option<Value>,
    #[serde(default, rename = "mcpServers")]
    mcp_servers: Option<Value>,
    #[serde(default)]
    apps: Option<Value>,
    #[serde(default)]
    agents: Option<Value>,
    #[serde(default)]
    hooks: Option<Value>,
    #[serde(default)]
    interface: Option<RawPluginInterface>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawAuthor {
    String(String),
    Object { name: Option<String> },
}

#[derive(Debug, Default, Deserialize)]
struct RawPluginInterface {
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
    #[serde(default, rename = "shortDescription")]
    short_description: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default, rename = "brandColor")]
    brand_color: Option<String>,
    #[serde(default)]
    logo: Option<String>,
    #[serde(default, rename = "composerIcon")]
    composer_icon: Option<String>,
}

pub async fn read_plugin_manifest(plugin_root: &Path) -> Result<PluginManifestView> {
    let (kind, manifest_path) =
        resolve_manifest_path(plugin_root).ok_or(PluginError::ManifestNotFound)?;
    let raw_json = tokio::fs::read_to_string(&manifest_path).await?;
    let parsed: RawPluginManifest = serde_json::from_str(&raw_json)
        .map_err(|error| PluginError::InvalidManifest(error.to_string()))?;
    Ok(view_from_raw(plugin_root, kind, manifest_path, parsed).await)
}

fn resolve_manifest_path(plugin_root: &Path) -> Option<(PluginManifestKind, PathBuf)> {
    [
        (
            PluginManifestKind::Kairox,
            plugin_root.join(".kairox-plugin/plugin.json"),
        ),
        (
            PluginManifestKind::Codex,
            plugin_root.join(".codex-plugin/plugin.json"),
        ),
        (
            PluginManifestKind::Claude,
            plugin_root.join(".claude-plugin/plugin.json"),
        ),
    ]
    .into_iter()
    .find(|(_, path)| path.is_file())
}

async fn view_from_raw(
    plugin_root: &Path,
    manifest_kind: PluginManifestKind,
    manifest_path: PathBuf,
    raw: RawPluginManifest,
) -> PluginManifestView {
    let fallback_name = fallback_plugin_name(plugin_root);
    let (name, valid, validation_error) = match raw.name.clone() {
        Some(name) if !name.trim().is_empty() => (name, true, None),
        _ => (
            fallback_name,
            false,
            Some("missing required plugin field: name".to_string()),
        ),
    };
    let description = raw.description.clone().unwrap_or_default();
    let inventory = build_inventory(plugin_root, &raw).await;
    let interface = raw.interface.unwrap_or_default();

    PluginManifestView {
        name,
        version: raw.version,
        description,
        author_name: raw.author.and_then(author_name),
        homepage: raw.homepage,
        repository: raw.repository.and_then(repository_string),
        license: raw.license,
        keywords: raw.keywords,
        interface: PluginInterface {
            display_name: interface.display_name,
            short_description: interface.short_description,
            category: interface.category,
            brand_color: interface.brand_color,
            logo: interface.logo,
            composer_icon: interface.composer_icon,
        },
        inventory,
        manifest_kind,
        manifest_path,
        plugin_root: plugin_root.to_path_buf(),
        valid,
        validation_error,
    }
}

fn author_name(author: RawAuthor) -> Option<String> {
    match author {
        RawAuthor::String(value) => Some(value),
        RawAuthor::Object { name } => name,
    }
}

fn repository_string(value: Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value),
        Value::Object(object) => object
            .get("url")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        _ => None,
    }
}

async fn build_inventory(plugin_root: &Path, raw: &RawPluginManifest) -> PluginComponentInventory {
    let mut inventory = PluginComponentInventory::default();
    let skills_dir = plugin_root.join("skills");
    if let Ok(mut entries) = tokio::fs::read_dir(&skills_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let skill_name = entry.file_name().to_string_lossy().to_string();
            if entry.path().join("SKILL.md").is_file() {
                inventory.skill_names.push(skill_name);
            }
        }
    }
    inventory.skill_names.sort();
    inventory.skill_count = inventory.skill_names.len();
    if inventory.skill_count == 0 {
        inventory.skill_count = count_component_value(raw.skills.as_ref());
    }

    inventory.mcp_server_count = count_mcp_servers(plugin_root, raw).await;
    inventory.app_count = count_component_value(raw.apps.as_ref());
    inventory.agent_count = count_component_value(raw.agents.as_ref())
        .max(count_files(plugin_root.join("agents")).await);
    inventory.hook_count = count_component_value(raw.hooks.as_ref());
    inventory
}

async fn count_mcp_servers(plugin_root: &Path, raw: &RawPluginManifest) -> usize {
    let manifest_count = count_component_value(raw.mcp_servers.as_ref());
    let mcp_path = plugin_root.join(".mcp.json");
    let file_count = match tokio::fs::read_to_string(&mcp_path).await {
        Ok(raw_json) => serde_json::from_str::<Value>(&raw_json)
            .ok()
            .and_then(|json| json.get("mcpServers").cloned())
            .as_ref()
            .map_or(0, |value| count_component_value(Some(value))),
        Err(_) => 0,
    };
    manifest_count.max(file_count)
}

fn count_component_value(value: Option<&Value>) -> usize {
    match value {
        Some(Value::Array(values)) => values.len(),
        Some(Value::Object(values)) => values.len(),
        Some(Value::String(_)) => 1,
        Some(_) | None => 0,
    }
}

async fn count_files(path: PathBuf) -> usize {
    let mut count = 0;
    if let Ok(mut entries) = tokio::fs::read_dir(path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().is_file() {
                count += 1;
            }
        }
    }
    count
}

fn fallback_plugin_name(plugin_root: &Path) -> String {
    plugin_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_owned()
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
