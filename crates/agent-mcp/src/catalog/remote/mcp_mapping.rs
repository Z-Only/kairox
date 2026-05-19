//! MCP Registry API DTOs and mapping to internal [`ServerEntry`] schema.
//!
//! Pure mapping layer — no IO, no caching, no network. Separated from
//! [`super::mcp_registry::McpRegistryProvider`] so the mapping logic can
//! be tested independently of the provider's fetch/cache/lock machinery.

use crate::catalog::remote::RemoteError;
use crate::catalog::{
    EnvVarSpec, InstallSpec, RuntimeKind, RuntimeRequirement, ServerEntry, TrustLevel,
};
use serde::Deserialize;
use std::collections::BTreeMap;

// ── API response types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct McpListResponse {
    #[serde(default)]
    pub(super) servers: Vec<McpServerWrapper>,
    #[serde(default)]
    pub(super) metadata: Option<McpMetadata>,
}

#[derive(Debug, Deserialize)]
pub(super) struct McpMetadata {
    #[serde(default, rename = "nextCursor")]
    pub(super) next_cursor: Option<String>,
}

/// Each item in `servers` wraps a `server` object and `_meta`.
#[derive(Debug, Deserialize)]
pub(super) struct McpServerWrapper {
    pub(super) server: McpServer,
    #[serde(default, rename = "_meta")]
    pub(super) meta: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct McpServer {
    /// Scoped name like `com.example/my-server`.
    pub(super) name: String,
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) version: Option<String>,
    #[serde(default, rename = "websiteUrl")]
    pub(super) website_url: Option<String>,
    #[serde(default)]
    pub(super) remotes: Vec<McpRemote>,
    #[serde(default)]
    pub(super) packages: Vec<McpPackage>,
    #[serde(default)]
    pub(super) repository: Option<McpRepository>,
}

#[derive(Debug, Deserialize)]
pub(super) struct McpRemote {
    #[serde(rename = "type")]
    pub(super) transport_type: String,
    pub(super) url: String,
    #[serde(default)]
    pub(super) headers: Vec<McpRemoteHeader>,
}

#[derive(Debug, Deserialize)]
pub(super) struct McpRemoteHeader {
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default, rename = "isRequired")]
    pub(super) is_required: Option<bool>,
    #[serde(default, rename = "isSecret")]
    pub(super) is_secret: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct McpPackage {
    #[serde(rename = "registryType")]
    pub(super) registry_type: String,
    pub(super) identifier: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) version: Option<String>,
    #[serde(default)]
    pub(super) transport: Option<McpPackageTransport>,
    #[serde(default, rename = "environmentVariables")]
    pub(super) environment_variables: Vec<McpEnvVar>,
}

#[derive(Debug, Deserialize)]
pub(super) struct McpPackageTransport {
    #[serde(rename = "type")]
    pub(super) transport_type: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct McpEnvVar {
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default, rename = "isRequired")]
    pub(super) is_required: Option<bool>,
    #[serde(default, rename = "isSecret")]
    pub(super) is_secret: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct McpRepository {
    #[serde(default)]
    pub(super) url: Option<String>,
}

// ── Helpers ──────────────────────────────────────────────────────────

pub(super) fn first_sentence(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    let cut = trimmed
        .find(['.', '\n'])
        .map(|i| &trimmed[..i])
        .unwrap_or(trimmed);
    if cut.chars().count() <= max_chars {
        cut.to_string()
    } else {
        cut.chars().take(max_chars).collect::<String>() + "…"
    }
}

/// Returns `true` when the `_meta` block indicates this is the latest
/// published version. Entries without the flag default to `true` so we
/// don't accidentally drop servers whose registry metadata is incomplete.
pub(super) fn is_latest(meta: &Option<serde_json::Value>) -> bool {
    meta.as_ref()
        .and_then(|m| m.get("io.modelcontextprotocol.registry/official"))
        .and_then(|official| official.get("isLatest"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

pub(super) fn infer_runtime_from_package(pkg: &McpPackage) -> Option<RuntimeRequirement> {
    let kind = match pkg.registry_type.as_str() {
        "npm" => RuntimeKind::Node,
        "pypi" => RuntimeKind::Python,
        _ => return None,
    };
    Some(RuntimeRequirement {
        kind,
        min_version: None,
        install_hint: None,
    })
}

// ── Mapping ──────────────────────────────────────────────────────────

pub(super) fn map_mcp_to_entry(
    source_id: &str,
    wrapper: &McpServerWrapper,
    trust_ceiling: TrustLevel,
) -> Result<ServerEntry, RemoteError> {
    let srv = &wrapper.server;
    let id = srv.name.clone();
    let display_name = srv
        .title
        .clone()
        .unwrap_or_else(|| srv.name.rsplit('/').next().unwrap_or(&srv.name).to_string());
    let description = srv.description.clone().unwrap_or_default();
    let summary = if description.is_empty() {
        display_name.clone()
    } else {
        first_sentence(&description, 200)
    };

    // Build InstallSpec: prefer remote endpoints, fall back to packages.
    let install = if let Some(remote) = srv.remotes.first() {
        let headers: BTreeMap<String, String> = remote
            .headers
            .iter()
            .filter_map(|h| h.name.clone().map(|n| (n, String::new())))
            .collect();
        match remote.transport_type.as_str() {
            "streamable-http" => InstallSpec::StreamableHttp {
                url: remote.url.clone(),
                headers,
            },
            _ => InstallSpec::Sse {
                url: remote.url.clone(),
                headers,
            },
        }
    } else if let Some(pkg) = srv.packages.first() {
        build_install_from_package(pkg)
    } else {
        // No connection info at all — placeholder.
        InstallSpec::Stdio {
            command: srv.name.clone(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        }
    };

    // Trust: the official registry is curated; treat all entries as
    // community level, clamped by the source ceiling.
    let trust = TrustLevel::Community.min(trust_ceiling);

    // Runtime requirements inferred from packages.
    let mut requirements: Vec<RuntimeRequirement> = Vec::new();
    for pkg in &srv.packages {
        if let Some(req) = infer_runtime_from_package(pkg) {
            if !requirements.iter().any(|r| r.kind == req.kind) {
                requirements.push(req);
            }
        }
    }

    // Environment variables from packages.
    let mut default_env: Vec<EnvVarSpec> = Vec::new();
    for pkg in &srv.packages {
        for ev in &pkg.environment_variables {
            let key = match &ev.name {
                Some(k) if !k.is_empty() => k.clone(),
                _ => continue,
            };
            if default_env.iter().any(|e| e.key == key) {
                continue;
            }
            default_env.push(EnvVarSpec {
                key: key.clone(),
                label: key,
                description: ev.description.clone().unwrap_or_default(),
                required: ev.is_required.unwrap_or(false),
                secret: ev.is_secret.unwrap_or(false),
                default: None,
            });
        }
    }

    // Headers from remote endpoints — surfaced as configurable fields.
    for remote in &srv.remotes {
        for h in &remote.headers {
            let name = match &h.name {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };
            if default_env.iter().any(|e| e.key == *name) {
                continue;
            }
            default_env.push(EnvVarSpec {
                key: name.clone(),
                label: name.clone(),
                description: h.description.clone().unwrap_or_default(),
                required: h.is_required.unwrap_or(false),
                secret: h.is_secret.unwrap_or(false),
                default: None,
            });
        }
    }

    let homepage = srv
        .website_url
        .clone()
        .or_else(|| srv.repository.as_ref().and_then(|r| r.url.clone()));

    Ok(ServerEntry {
        id,
        source: source_id.to_string(),
        display_name,
        summary,
        description,
        categories: vec![],
        tags: vec![],
        author: None,
        homepage,
        version: srv.version.clone(),
        install,
        requirements,
        trust,
        default_env,
        icon: None,
        verified: false,
    })
}

pub(super) fn build_install_from_package(pkg: &McpPackage) -> InstallSpec {
    let is_stdio = pkg
        .transport
        .as_ref()
        .map(|t| t.transport_type == "stdio")
        .unwrap_or(true);

    if is_stdio {
        let (command, args) = match pkg.registry_type.as_str() {
            "npm" => (
                "npx".to_string(),
                vec!["-y".to_string(), pkg.identifier.clone()],
            ),
            "pypi" => ("uvx".to_string(), vec![pkg.identifier.clone()]),
            _ => (pkg.identifier.clone(), vec![]),
        };
        InstallSpec::Stdio {
            command,
            args,
            env: BTreeMap::new(),
            cwd: None,
        }
    } else {
        // Non-stdio package transport — unlikely but handle gracefully.
        InstallSpec::Stdio {
            command: pkg.identifier.clone(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_wrapper(name: &str, is_latest: bool) -> serde_json::Value {
        json!({
            "server": {
                "name": name,
                "description": "A test server.",
                "title": "Test Server",
                "version": "1.0.0",
                "remotes": [{"type": "streamable-http", "url": "https://example.com/mcp"}]
            },
            "_meta": {
                "io.modelcontextprotocol.registry/official": {
                    "status": "active",
                    "isLatest": is_latest
                }
            }
        })
    }

    fn parse_wrapper(val: &serde_json::Value) -> McpServerWrapper {
        serde_json::from_value(val.clone()).unwrap()
    }

    #[test]
    fn maps_remote_server_to_streamable_http() {
        let val = sample_wrapper("com.example/my-server", true);
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(entry.id, "com.example/my-server");
        assert_eq!(entry.display_name, "Test Server");
        assert_eq!(entry.summary, "A test server");
        assert_eq!(entry.source, "mcp-registry");
        match &entry.install {
            InstallSpec::StreamableHttp { url, .. } => {
                assert_eq!(url, "https://example.com/mcp");
            }
            _ => panic!("expected StreamableHttp install"),
        }
    }

    #[test]
    fn maps_remote_with_headers() {
        let val = json!({
            "server": {
                "name": "ai.example/app",
                "description": "API server.",
                "remotes": [{
                    "type": "sse",
                    "url": "https://api.example.com/mcp",
                    "headers": [
                        {"name": "Authorization", "description": "Bearer token", "isRequired": true, "isSecret": true}
                    ]
                }]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(entry.id, "ai.example/app");
        match &entry.install {
            InstallSpec::Sse { url, headers } => {
                assert_eq!(url, "https://api.example.com/mcp");
                assert!(headers.contains_key("Authorization"));
            }
            _ => panic!("expected SSE install"),
        }
        assert!(entry
            .default_env
            .iter()
            .any(|e| e.key == "Authorization" && e.required && e.secret));
    }

    #[test]
    fn maps_npm_package_to_stdio() {
        let val = json!({
            "server": {
                "name": "org.example/cli-tool",
                "description": "CLI tool.",
                "version": "2.0.0",
                "packages": [{
                    "registryType": "npm",
                    "identifier": "@example/cli-tool",
                    "version": "2.0.0",
                    "transport": {"type": "stdio"},
                    "environmentVariables": [
                        {"name": "API_KEY", "description": "key", "isRequired": true, "isSecret": true}
                    ]
                }]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        match &entry.install {
            InstallSpec::Stdio { command, args, .. } => {
                assert_eq!(command, "npx");
                assert_eq!(args, &["-y", "@example/cli-tool"]);
            }
            _ => panic!("expected Stdio install"),
        }
        assert_eq!(entry.requirements.len(), 1);
        assert_eq!(entry.requirements[0].kind, RuntimeKind::Node);
        let env = &entry.default_env;
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].key, "API_KEY");
        assert!(env[0].required);
        assert!(env[0].secret);
    }

    #[test]
    fn maps_pypi_package_to_uvx() {
        let val = json!({
            "server": {
                "name": "org.example/py-tool",
                "description": "Python tool.",
                "version": "1.0.0",
                "packages": [{
                    "registryType": "pypi",
                    "identifier": "py-tool",
                    "version": "1.0.0",
                    "transport": {"type": "stdio"}
                }]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        match &entry.install {
            InstallSpec::Stdio { command, args, .. } => {
                assert_eq!(command, "uvx");
                assert_eq!(args, &["py-tool"]);
            }
            _ => panic!("expected Stdio install"),
        }
        assert_eq!(entry.requirements[0].kind, RuntimeKind::Python);
    }

    #[test]
    fn filters_non_latest_entries() {
        let old = sample_wrapper("com.example/server", false);
        let wrapper = parse_wrapper(&old);
        assert!(!is_latest(&wrapper.meta));

        let latest = sample_wrapper("com.example/server", true);
        let wrapper = parse_wrapper(&latest);
        assert!(is_latest(&wrapper.meta));
    }

    #[test]
    fn missing_meta_defaults_to_latest() {
        let val = json!({
            "server": {
                "name": "com.example/no-meta",
                "version": "1.0.0"
            }
        });
        let wrapper = parse_wrapper(&val);
        assert!(is_latest(&wrapper.meta));
    }

    #[test]
    fn trust_ceiling_is_applied() {
        let val = sample_wrapper("com.example/server", true);
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(entry.trust, TrustLevel::Community);
    }

    #[test]
    fn title_fallback_to_name_suffix() {
        let val = json!({
            "server": {
                "name": "com.example/cool-server",
                "version": "1.0.0"
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(entry.display_name, "cool-server");
    }

    #[test]
    fn homepage_from_website_url() {
        let val = json!({
            "server": {
                "name": "com.example/srv",
                "version": "1.0.0",
                "websiteUrl": "https://example.com"
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(entry.homepage.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn homepage_fallback_to_repository() {
        let val = json!({
            "server": {
                "name": "com.example/srv",
                "version": "1.0.0",
                "repository": {"url": "https://github.com/example/srv"}
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(
            entry.homepage.as_deref(),
            Some("https://github.com/example/srv")
        );
    }

    // ── New edge-case tests ──────────────────────────────────────────

    #[test]
    fn is_latest_meta_missing_official_key() {
        let meta = json!({"other": "value"});
        assert!(is_latest(&Some(meta)));
    }

    #[test]
    fn is_latest_meta_official_missing_is_latest() {
        let meta = json!({
            "io.modelcontextprotocol.registry/official": {
                "status": "active"
            }
        });
        assert!(is_latest(&Some(meta)));
    }

    #[test]
    fn is_latest_meta_none() {
        assert!(is_latest(&None));
    }

    #[test]
    fn is_latest_explicit_false() {
        let meta = json!({
            "io.modelcontextprotocol.registry/official": {
                "isLatest": false
            }
        });
        assert!(!is_latest(&Some(meta)));
    }

    #[test]
    fn remote_takes_precedence_over_package() {
        let val = json!({
            "server": {
                "name": "ai.example/hybrid",
                "description": "Has both remote and package.",
                "remotes": [{"type": "sse", "url": "https://remote.example.com/mcp"}],
                "packages": [{
                    "registryType": "npm",
                    "identifier": "@example/hybrid",
                    "version": "1.0.0",
                    "transport": {"type": "stdio"}
                }]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        // Remote wins.
        match &entry.install {
            InstallSpec::Sse { url, .. } => {
                assert_eq!(url, "https://remote.example.com/mcp");
            }
            _ => panic!("expected SSE from remote, not package"),
        }
    }

    #[test]
    fn package_only_without_remote() {
        let val = json!({
            "server": {
                "name": "ai.example/pkg-only",
                "description": "Only a package.",
                "packages": [{
                    "registryType": "npm",
                    "identifier": "@example/pkg-only",
                    "version": "1.0.0",
                    "transport": {"type": "stdio"}
                }]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        match &entry.install {
            InstallSpec::Stdio { command, args, .. } => {
                assert_eq!(command, "npx");
                assert_eq!(args, &["-y", "@example/pkg-only"]);
            }
            _ => panic!("expected Stdio from package"),
        }
    }

    #[test]
    fn no_remote_no_package_placeholder() {
        let val = json!({
            "server": {
                "name": "com.example/bare",
                "version": "1.0.0"
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        match &entry.install {
            InstallSpec::Stdio { command, args, .. } => {
                assert_eq!(command, "com.example/bare");
                assert!(args.is_empty());
            }
            _ => panic!("expected placeholder Stdio"),
        }
    }

    #[test]
    fn deduplicates_runtime_requirements() {
        let val = json!({
            "server": {
                "name": "ai.example/dual-pkg",
                "description": "Two npm packages.",
                "packages": [
                    {
                        "registryType": "npm",
                        "identifier": "@example/a",
                        "version": "1.0.0",
                        "transport": {"type": "stdio"}
                    },
                    {
                        "registryType": "npm",
                        "identifier": "@example/b",
                        "version": "2.0.0",
                        "transport": {"type": "stdio"}
                    }
                ]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        // Both are npm → only one Node requirement.
        assert_eq!(entry.requirements.len(), 1);
        assert_eq!(entry.requirements[0].kind, RuntimeKind::Node);
    }

    #[test]
    fn mixed_package_runtimes() {
        let val = json!({
            "server": {
                "name": "ai.example/mixed",
                "description": "npm + pypi packages.",
                "packages": [
                    {
                        "registryType": "npm",
                        "identifier": "@example/js",
                        "version": "1.0.0",
                        "transport": {"type": "stdio"}
                    },
                    {
                        "registryType": "pypi",
                        "identifier": "py-part",
                        "version": "1.0.0",
                        "transport": {"type": "stdio"}
                    }
                ]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(entry.requirements.len(), 2);
        let kinds: Vec<_> = entry.requirements.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&RuntimeKind::Node));
        assert!(kinds.contains(&RuntimeKind::Python));
    }

    #[test]
    fn deduplicates_env_vars_from_multiple_packages() {
        let val = json!({
            "server": {
                "name": "ai.example/dup-env",
                "description": "Duplicate env vars.",
                "packages": [
                    {
                        "registryType": "npm",
                        "identifier": "@example/a",
                        "transport": {"type": "stdio"},
                        "environmentVariables": [
                            {"name": "API_KEY", "description": "key A", "isRequired": true}
                        ]
                    },
                    {
                        "registryType": "npm",
                        "identifier": "@example/b",
                        "transport": {"type": "stdio"},
                        "environmentVariables": [
                            {"name": "API_KEY", "description": "key B", "isRequired": false}
                        ]
                    }
                ]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        // API_KEY appears only once (first-package-wins).
        assert_eq!(entry.default_env.len(), 1);
        assert_eq!(entry.default_env[0].key, "API_KEY");
    }

    #[test]
    fn env_var_with_empty_name_skipped() {
        let val = json!({
            "server": {
                "name": "ai.example/empty-env",
                "description": "Env var with empty name.",
                "packages": [{
                    "registryType": "npm",
                    "identifier": "@example/x",
                    "transport": {"type": "stdio"},
                    "environmentVariables": [
                        {"name": "", "description": "should be skipped"}
                    ]
                }]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert!(entry.default_env.is_empty());
    }

    #[test]
    fn unknown_registry_type_no_runtime() {
        let val = json!({
            "server": {
                "name": "ai.example/unknown",
                "description": "Unknown registry type.",
                "packages": [{
                    "registryType": "cargo",
                    "identifier": "some-crate",
                    "transport": {"type": "stdio"}
                }]
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert!(entry.requirements.is_empty());
        // Falls back to raw identifier as command.
        match &entry.install {
            InstallSpec::Stdio { command, .. } => {
                assert_eq!(command, "some-crate");
            }
            _ => panic!("expected Stdio"),
        }
    }

    #[test]
    fn trust_ceiling_clamps_verified_down_to_community() {
        let val = sample_wrapper("com.example/server", true);
        let wrapper = parse_wrapper(&val);
        // Even with Verified ceiling, entry trust is clamped to Community.
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Verified).unwrap();
        assert_eq!(entry.trust, TrustLevel::Community);
    }

    #[test]
    fn trust_ceiling_unverified_clamps_down() {
        let val = sample_wrapper("com.example/server", true);
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Unverified).unwrap();
        assert_eq!(entry.trust, TrustLevel::Unverified);
    }

    #[test]
    fn first_sentence_cuts_at_period() {
        assert_eq!(first_sentence("Hello. World", 200), "Hello");
    }

    #[test]
    fn first_sentence_cuts_at_newline() {
        assert_eq!(first_sentence("Line one\nLine two", 200), "Line one");
    }

    #[test]
    fn first_sentence_truncates_long() {
        let long = "A".repeat(150);
        let result = first_sentence(&long, 50);
        assert!(result.ends_with('…'));
        assert!(result.chars().count() <= 51); // 50 chars + ellipsis
    }

    #[test]
    fn empty_description_produces_display_name_summary() {
        let val = json!({
            "server": {
                "name": "com.example/no-desc",
                "title": "No Desc",
                "version": "1.0.0"
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(entry.description, "");
        assert_eq!(entry.summary, "No Desc");
    }

    #[test]
    fn website_url_beats_repository_for_homepage() {
        let val = json!({
            "server": {
                "name": "com.example/srv",
                "version": "1.0.0",
                "websiteUrl": "https://example.com",
                "repository": {"url": "https://github.com/example/srv"}
            }
        });
        let wrapper = parse_wrapper(&val);
        let entry = map_mcp_to_entry("mcp-registry", &wrapper, TrustLevel::Community).unwrap();
        assert_eq!(entry.homepage.as_deref(), Some("https://example.com"));
    }
}
