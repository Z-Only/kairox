use crate::{Config, ConfigError};

/// Adapter kind for a remote catalog source. Mirrors
/// `agent_mcp::RemoteSourceKind` but lives here so `agent-config` does not
/// need to depend on `agent-mcp` (cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSourceKind {
    McpRegistry,
}

/// A user-configured remote catalog source, parsed from `[[catalog_sources]]`
/// in `config.toml`. Mirrors `agent_mcp::RemoteSourceConfig`; the
/// runtime layer translates between the two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSourceConfig {
    pub id: String,
    pub display_name: String,
    pub kind: CatalogSourceKind,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: u32,
    pub default_trust: String,
    pub enabled: bool,
    pub cache_ttl_seconds: Option<u64>,
}

/// Result bundle returned by [`load_with_marketplace_loaded`].
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    pub catalog_sources: Vec<CatalogSourceConfig>,
}

#[derive(Debug, serde::Deserialize)]
struct MarketplaceTomlInner {
    #[serde(default, rename = "mcp_servers")]
    _mcp_servers: toml::value::Table,
    #[serde(default)]
    catalog_sources: Vec<RawCatalogSource>,
}

#[derive(Debug, serde::Deserialize)]
struct RawCatalogSource {
    id: String,
    display_name: String,
    kind: String,
    url: String,
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default = "default_priority")]
    priority: u32,
    #[serde(default = "default_trust_str")]
    default_trust: String,
    #[serde(default = "crate::default_true")]
    enabled: bool,
    #[serde(default)]
    cache_ttl_seconds: Option<u64>,
}

fn default_priority() -> u32 {
    100
}

fn default_trust_str() -> String {
    "community".into()
}

fn raw_to_source(raw: RawCatalogSource) -> Result<Option<CatalogSourceConfig>, ConfigError> {
    let kind = match raw.kind.as_str() {
        "mcp_registry" => CatalogSourceKind::McpRegistry,
        other => {
            eprintln!(
                "[agent-config] catalog_sources[{}]: skipping unsupported kind '{other}'",
                raw.id
            );
            return Ok(None);
        }
    };
    if !raw.url.starts_with("http://") && !raw.url.starts_with("https://") {
        return Err(ConfigError::Parse {
            path: "marketplace".into(),
            message: format!(
                "catalog_sources[{}]: url must start with http:// or https://",
                raw.id
            ),
        });
    }
    Ok(Some(CatalogSourceConfig {
        id: raw.id,
        display_name: raw.display_name,
        kind,
        url: raw.url,
        api_key_env: raw.api_key_env,
        priority: raw.priority,
        default_trust: raw.default_trust,
        enabled: raw.enabled,
        cache_ttl_seconds: raw.cache_ttl_seconds,
    }))
}

/// Parse only the `[[catalog_sources]]` array from a config TOML
/// string. Returns an empty `Vec` if the section is missing. Entries
/// with unrecognised `kind` values are silently skipped so that old
/// config files written by a newer (or older) version of Kairox don't
/// prevent the application from starting.
pub fn parse_catalog_sources(
    marketplace_content: &str,
) -> Result<Vec<CatalogSourceConfig>, ConfigError> {
    let inner: MarketplaceTomlInner =
        toml::from_str(marketplace_content).map_err(|e| ConfigError::Parse {
            path: "marketplace".into(),
            message: e.to_string(),
        })?;
    inner
        .catalog_sources
        .into_iter()
        .filter_map(|raw| raw_to_source(raw).transpose())
        .collect()
}

/// Load main config + optional catalog source TOML.
pub fn load_with_marketplace_loaded(
    main_content: &str,
    marketplace_content: Option<&str>,
    main_path: &str,
    marketplace_path: &str,
) -> Result<LoadedConfig, ConfigError> {
    let config = super::load_with_marketplace_overlay(
        main_content,
        marketplace_content,
        main_path,
        marketplace_path,
    )?;
    let catalog_sources = match marketplace_content {
        Some(m) => parse_catalog_sources(m)?,
        None => vec![],
    };
    Ok(LoadedConfig {
        config,
        catalog_sources,
    })
}

/// Default remote catalog sources shipped with Kairox so that the GUI
/// marketplace tab has visible subscriptions out of the box.
///
/// All defaults are `enabled = false`; users opt in by enabling them via
/// the GUI settings page (or by adding overriding entries to `config.toml`).
/// User-defined sources with the same id replace the
/// matching default — see [`merge_with_defaults`].
pub fn default_catalog_sources() -> Vec<CatalogSourceConfig> {
    vec![CatalogSourceConfig {
        id: "mcp-registry".into(),
        display_name: "Model Context Protocol Servers".into(),
        kind: CatalogSourceKind::McpRegistry,
        url: "https://registry.modelcontextprotocol.io".into(),
        api_key_env: None,
        priority: 50,
        default_trust: "community".into(),
        enabled: false,
        cache_ttl_seconds: None,
    }]
}

/// Merge user-configured catalog sources with the built-in defaults.
///
/// Strategy: union by `id`. Any default whose `id` already appears in
/// `user_sources` is dropped (user overrides win), and remaining defaults
/// are appended after the user-provided entries. This preserves the
/// user's listing order while ensuring the predefined subscriptions are
/// always visible in the GUI even when no user config is present.
pub fn merge_with_defaults(user_sources: Vec<CatalogSourceConfig>) -> Vec<CatalogSourceConfig> {
    let user_ids: std::collections::HashSet<String> =
        user_sources.iter().map(|s| s.id.clone()).collect();
    let mut merged = user_sources;
    for default in default_catalog_sources() {
        if !user_ids.contains(&default.id) {
            merged.push(default);
        }
    }
    merged
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
