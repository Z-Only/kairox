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
mod tests {
    use super::*;

    #[test]
    fn parses_catalog_sources_with_defaults() {
        let market = r#"
[[catalog_sources]]
id           = "mcp-registry"
display_name = "Model Context Protocol Servers"
kind         = "mcp_registry"
url          = "https://registry.modelcontextprotocol.io"
"#;
        let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
        assert_eq!(loaded.catalog_sources.len(), 1);
        let s = &loaded.catalog_sources[0];
        assert_eq!(s.id, "mcp-registry");
        assert_eq!(s.priority, 100);
        assert!(s.enabled);
        assert_eq!(s.default_trust, "community");
    }

    #[test]
    fn parses_multiple_sources_with_full_fields() {
        let market = r#"
[[catalog_sources]]
id            = "internal"
display_name  = "Internal"
kind          = "mcp_registry"
url           = "https://mcp.example.com/v0.1/servers"
api_key_env   = "INTERNAL_KEY"
priority      = 10
default_trust = "verified"
enabled       = true
cache_ttl_seconds = 600

[[catalog_sources]]
id           = "mcp-registry"
display_name = "Model Context Protocol Servers"
kind         = "mcp_registry"
url          = "https://registry.modelcontextprotocol.io"
priority     = 50
enabled      = false
"#;
        let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
        assert_eq!(loaded.catalog_sources.len(), 2);
        let internal = loaded
            .catalog_sources
            .iter()
            .find(|s| s.id == "internal")
            .unwrap();
        assert_eq!(internal.priority, 10);
        assert_eq!(internal.api_key_env.as_deref(), Some("INTERNAL_KEY"));
        assert_eq!(internal.cache_ttl_seconds, Some(600));
        let mcp = loaded
            .catalog_sources
            .iter()
            .find(|s| s.id == "mcp-registry")
            .unwrap();
        assert!(!mcp.enabled);
    }

    #[test]
    fn unknown_kind_is_silently_skipped() {
        let market = r#"
[[catalog_sources]]
id           = "x"
display_name = "X"
kind         = "wat"
url          = "https://x"
"#;
        let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
        assert!(
            !loaded.catalog_sources.iter().any(|s| s.id == "x"),
            "unsupported kind should be silently dropped"
        );
    }

    #[test]
    fn rejects_invalid_url_scheme() {
        let market = r#"
[[catalog_sources]]
id           = "x"
display_name = "X"
kind         = "mcp_registry"
url          = "ftp://x"
"#;
        let err = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap_err();
        assert!(format!("{err:?}").to_lowercase().contains("url"));
    }

    #[test]
    fn missing_marketplace_yields_empty_sources() {
        let loaded = load_with_marketplace_loaded("", None, "k.toml", "m.toml").unwrap();
        assert!(loaded.catalog_sources.is_empty());
    }

    #[test]
    fn catalog_source_loader_ignores_mcp_server_sections() {
        let market = r#"
[mcp_servers.foo]
type      = "stdio"
command   = "echo"
args      = []
"#;
        let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
        assert!(loaded.config.mcp_servers.is_empty());
        assert!(loaded.catalog_sources.is_empty());
    }

    #[test]
    fn defaults_ship_disabled_remote_sources() {
        let defaults = default_catalog_sources();
        let ids: Vec<&str> = defaults.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"mcp-registry"));
        assert_eq!(defaults.len(), 1);
        assert!(
            defaults.iter().all(|s| !s.enabled),
            "defaults must ship disabled so GUI does not auto-fetch on cold start",
        );
        assert!(
            defaults
                .iter()
                .all(|s| s.url.starts_with("https://") || s.url.starts_with("http://")),
            "default urls must be well-formed http(s)",
        );
        let mcp = defaults.iter().find(|s| s.id == "mcp-registry").unwrap();
        assert_eq!(mcp.kind, CatalogSourceKind::McpRegistry);
        assert_eq!(mcp.url, "https://registry.modelcontextprotocol.io");
    }

    #[test]
    fn merge_with_defaults_seeds_all_when_user_empty() {
        let merged = merge_with_defaults(Vec::new());
        let ids: Vec<&str> = merged.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"mcp-registry"));
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_with_defaults_user_overrides_default_by_id() {
        let user = vec![CatalogSourceConfig {
            id: "mcp-registry".into(),
            display_name: "My MCP Mirror".into(),
            kind: CatalogSourceKind::McpRegistry,
            url: "https://my-mirror.example/v0.1/servers".into(),
            api_key_env: Some("MY_KEY".into()),
            priority: 10,
            default_trust: "verified".into(),
            enabled: true,
            cache_ttl_seconds: Some(120),
        }];
        let merged = merge_with_defaults(user);
        let mcp: Vec<_> = merged.iter().filter(|s| s.id == "mcp-registry").collect();
        assert_eq!(mcp.len(), 1);
        assert_eq!(mcp[0].display_name, "My MCP Mirror");
        assert!(mcp[0].enabled);
        assert_eq!(mcp[0].priority, 10);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_with_defaults_preserves_user_ordering_then_appends_defaults() {
        let user = vec![CatalogSourceConfig {
            id: "custom-first".into(),
            display_name: "Custom".into(),
            kind: CatalogSourceKind::McpRegistry,
            url: "https://custom.example/v0.1/servers".into(),
            api_key_env: None,
            priority: 5,
            default_trust: "community".into(),
            enabled: true,
            cache_ttl_seconds: None,
        }];
        let merged = merge_with_defaults(user);
        assert_eq!(merged[0].id, "custom-first");
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].id, "mcp-registry");
    }
}
