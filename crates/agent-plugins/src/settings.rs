use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::manifest::{read_plugin_manifest, PluginManifestView};
use crate::{PluginError, Result};

const PLUGINS_STATE_FILE_NAME: &str = "plugins-state.toml";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginScope {
    Builtin,
    User,
    Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginRoot {
    pub scope: PluginScope,
    pub path: PathBuf,
}

impl PluginRoot {
    pub fn new(scope: PluginScope, path: impl Into<PathBuf>) -> Self {
        Self {
            scope,
            path: path.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginSettingsProjection {
    pub plugins: Vec<PluginSettingsView>,
    pub state_errors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginSettingsView {
    pub settings_id: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub scope: PluginScope,
    pub path: String,
    pub enabled: bool,
    pub install_source: Option<String>,
    pub marketplace: Option<String>,
    pub effective: bool,
    pub shadowed_by: Option<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub manifest: PluginManifestView,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct PluginsStateFile {
    #[serde(default)]
    plugins: BTreeMap<String, PluginStateEntry>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct PluginStateEntry {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    install_source: Option<String>,
    #[serde(default)]
    marketplace: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

fn default_true() -> bool {
    true
}

pub async fn discover_plugin_settings(roots: Vec<PluginRoot>) -> Result<PluginSettingsProjection> {
    let mut plugins = Vec::new();
    let mut state_errors = Vec::new();

    for root in roots {
        if !tokio::fs::try_exists(&root.path).await? {
            continue;
        }
        let state_path = root.path.join(PLUGINS_STATE_FILE_NAME);
        let state = match read_plugins_state(&state_path).await {
            Ok(state) => state,
            Err(error) => {
                state_errors.push(format!("{}: {error}", state_path.display()));
                PluginsStateFile::default()
            }
        };

        let mut entries = tokio::fs::read_dir(&root.path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            let plugin_root = entry.path();
            let manifest = match read_plugin_manifest(&plugin_root).await {
                Ok(manifest) => manifest,
                Err(PluginError::ManifestNotFound) => continue,
                Err(error) => {
                    state_errors.push(format!("{}: {error}", plugin_root.display()));
                    continue;
                }
            };
            plugins.push(view_from_manifest(root.scope, manifest, &state));
        }
    }

    apply_effective_markers(&mut plugins);
    plugins.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| scope_priority(right.scope).cmp(&scope_priority(left.scope)))
            .then_with(|| left.path.cmp(&right.path))
    });

    Ok(PluginSettingsProjection {
        plugins,
        state_errors,
    })
}

pub async fn write_plugin_state(
    root: &Path,
    plugin_id: &str,
    enabled: bool,
    install_source: Option<&str>,
    marketplace: Option<&str>,
) -> Result<()> {
    let state_path = root.join(PLUGINS_STATE_FILE_NAME);
    let mut state = read_plugins_state(&state_path).await.unwrap_or_default();
    state.plugins.insert(
        plugin_id.to_owned(),
        PluginStateEntry {
            enabled,
            install_source: install_source.map(ToOwned::to_owned),
            marketplace: marketplace.map(ToOwned::to_owned),
            version: None,
        },
    );
    write_plugins_state(&state_path, &state).await
}

async fn read_plugins_state(path: &Path) -> Result<PluginsStateFile> {
    let raw = match tokio::fs::read_to_string(path).await {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PluginsStateFile::default());
        }
        Err(error) => return Err(error.into()),
    };
    toml::from_str(&raw).map_err(|error| PluginError::InvalidStateFile(error.to_string()))
}

async fn write_plugins_state(path: &Path, state: &PluginsStateFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let raw = toml::to_string_pretty(state)
        .map_err(|error| PluginError::InvalidStateFile(error.to_string()))?;
    tokio::fs::write(path, raw).await?;
    Ok(())
}

fn view_from_manifest(
    scope: PluginScope,
    manifest: PluginManifestView,
    state: &PluginsStateFile,
) -> PluginSettingsView {
    let state_entry = state.plugins.get(&manifest.name);
    let settings_id = format!("{}:{}", scope_label(scope), manifest.name);
    PluginSettingsView {
        settings_id,
        id: manifest.name.clone(),
        name: manifest
            .interface
            .display_name
            .clone()
            .unwrap_or_else(|| manifest.name.clone()),
        description: manifest.description.clone(),
        version: manifest.version.clone(),
        scope,
        path: manifest.plugin_root.display().to_string(),
        enabled: state_entry.map(|entry| entry.enabled).unwrap_or(true),
        install_source: state_entry.and_then(|entry| entry.install_source.clone()),
        marketplace: state_entry.and_then(|entry| entry.marketplace.clone()),
        effective: false,
        shadowed_by: None,
        valid: manifest.valid,
        validation_error: manifest.validation_error.clone(),
        manifest,
    }
}

fn apply_effective_markers(plugins: &mut [PluginSettingsView]) {
    let mut effective_scope_by_id = BTreeMap::new();
    for plugin in plugins.iter() {
        let existing = effective_scope_by_id
            .get(&plugin.id)
            .copied()
            .unwrap_or(plugin.scope);
        if scope_priority(plugin.scope) >= scope_priority(existing) {
            effective_scope_by_id.insert(plugin.id.clone(), plugin.scope);
        }
    }

    for plugin in plugins.iter_mut() {
        let effective_scope = effective_scope_by_id
            .get(&plugin.id)
            .copied()
            .unwrap_or(plugin.scope);
        plugin.effective = plugin.scope == effective_scope;
        plugin.shadowed_by = if plugin.effective {
            None
        } else {
            Some(scope_label(effective_scope).to_owned())
        };
    }
}

fn scope_priority(scope: PluginScope) -> u8 {
    match scope {
        PluginScope::Builtin => 0,
        PluginScope::User => 1,
        PluginScope::Project => 2,
    }
}

fn scope_label(scope: PluginScope) -> &'static str {
    match scope {
        PluginScope::Builtin => "builtin",
        PluginScope::User => "user",
        PluginScope::Project => "project",
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
