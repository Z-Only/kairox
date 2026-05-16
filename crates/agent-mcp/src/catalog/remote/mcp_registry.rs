//! Official MCP Registry catalog provider.
//!
//! Adapts the Model Context Protocol Registry API
//! (`/v0.1/servers` endpoint) to our internal [`ServerEntry`] schema.
//!
//! The API returns a cursor-paginated list of server entries. Each entry
//! contains a `server` object (with `name`, `description`, `title`,
//! `version`, `remotes`, `packages`, `repository`) and `_meta` with
//! publish / latest-version metadata.
//!
//! Only entries whose `_meta` has `isLatest == true` are kept so the
//! catalog shows one entry per server rather than one per version.

use crate::catalog::remote::http_cache::HttpResponseCache;
use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::remote::{RemoteError, RemoteSourceConfig};
use crate::catalog::{
    CatalogError, CatalogProvider, CatalogQuery, CatalogResult, EnvVarSpec, InstallSpec,
    RuntimeKind, RuntimeRequirement, ServerEntry, TrustLevel,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Maximum number of servers to collect across all pages.
const MAX_SERVERS_TO_FETCH: usize = 500;

/// Page size requested per cursor-based page.
const PAGE_SIZE: usize = 100;

// ── API response types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct McpListResponse {
    #[serde(default)]
    servers: Vec<McpServerWrapper>,
    #[serde(default)]
    metadata: Option<McpMetadata>,
}

#[derive(Debug, Deserialize)]
struct McpMetadata {
    #[serde(default, rename = "nextCursor")]
    next_cursor: Option<String>,
}

/// Each item in `servers` wraps a `server` object and `_meta`.
#[derive(Debug, Deserialize)]
struct McpServerWrapper {
    server: McpServer,
    #[serde(default, rename = "_meta")]
    meta: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct McpServer {
    /// Scoped name like `com.example/my-server`.
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "websiteUrl")]
    website_url: Option<String>,
    #[serde(default)]
    remotes: Vec<McpRemote>,
    #[serde(default)]
    packages: Vec<McpPackage>,
    #[serde(default)]
    repository: Option<McpRepository>,
}

#[derive(Debug, Deserialize)]
struct McpRemote {
    #[serde(rename = "type")]
    transport_type: String,
    url: String,
    #[serde(default)]
    headers: Vec<McpRemoteHeader>,
}

#[derive(Debug, Deserialize)]
struct McpRemoteHeader {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "isRequired")]
    is_required: Option<bool>,
    #[serde(default, rename = "isSecret")]
    is_secret: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct McpPackage {
    #[serde(rename = "registryType")]
    registry_type: String,
    identifier: String,
    #[allow(dead_code)]
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    transport: Option<McpPackageTransport>,
    #[serde(default, rename = "environmentVariables")]
    environment_variables: Vec<McpEnvVar>,
}

#[derive(Debug, Deserialize)]
struct McpPackageTransport {
    #[serde(rename = "type")]
    transport_type: String,
}

#[derive(Debug, Deserialize)]
struct McpEnvVar {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "isRequired")]
    is_required: Option<bool>,
    #[serde(default, rename = "isSecret")]
    is_secret: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct McpRepository {
    #[serde(default)]
    url: Option<String>,
}

// ── Helpers ──────────────────────────────────────────────────────────

fn first_sentence(s: &str, max_chars: usize) -> String {
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
fn is_latest(meta: &Option<serde_json::Value>) -> bool {
    meta.as_ref()
        .and_then(|m| m.get("io.modelcontextprotocol.registry/official"))
        .and_then(|official| official.get("isLatest"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

fn infer_runtime_from_package(pkg: &McpPackage) -> Option<RuntimeRequirement> {
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

fn map_mcp_to_entry(
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

fn build_install_from_package(pkg: &McpPackage) -> InstallSpec {
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

// ── Provider ─────────────────────────────────────────────────────────

pub struct McpRegistryProvider {
    cfg: RemoteSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
    /// In-memory cache of fetched entries. Survives for the provider's
    /// lifetime (session-scoped). Never persisted to disk — app restart
    /// always starts cold and fetches fresh data.
    cached_entries: Mutex<Option<Vec<ServerEntry>>>,
}

impl McpRegistryProvider {
    pub fn new(
        cfg: RemoteSourceConfig,
        http: SharedHttpClient,
        cache: Arc<HttpResponseCache>,
    ) -> Self {
        Self {
            cfg,
            http,
            cache,
            cached_entries: Mutex::new(None),
        }
    }

    async fn fetch(&self) -> Result<Vec<ServerEntry>, RemoteError> {
        let base = format!("{}/v0.1/servers", self.cfg.url.trim_end_matches('/'));
        let ceiling = self.cfg.default_trust;
        let mut all_entries: Vec<ServerEntry> = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let url = match &cursor {
                Some(c) => format!("{base}?limit={PAGE_SIZE}&cursor={c}"),
                None => format!("{base}?limit={PAGE_SIZE}"),
            };

            let resp = self
                .http
                .get_json(
                    &url,
                    GetOpts {
                        api_key_env: self.cfg.api_key_env.as_deref(),
                        if_none_match: None,
                    },
                )
                .await?;
            if !(200..300).contains(&resp.status) {
                return Err(RemoteError::Http(format!("status {}", resp.status)));
            }

            let parsed: McpListResponse = serde_json::from_slice(&resp.body)
                .map_err(|e| RemoteError::Decode(format!("mcp registry: {e}")))?;

            for wrapper in &parsed.servers {
                // Only keep the latest version of each server.
                if !is_latest(&wrapper.meta) {
                    continue;
                }
                match map_mcp_to_entry(&self.cfg.id, wrapper, ceiling) {
                    Ok(entry) => all_entries.push(entry),
                    Err(e) => {
                        tracing::warn!(
                            name=%wrapper.server.name,
                            error=%e,
                            "skipping mcp registry entry"
                        );
                    }
                }
            }

            let next = parsed.metadata.as_ref().and_then(|m| m.next_cursor.clone());

            if next.is_none() || all_entries.len() >= MAX_SERVERS_TO_FETCH {
                break;
            }
            cursor = next;
        }

        all_entries.truncate(MAX_SERVERS_TO_FETCH);
        Ok(all_entries)
    }

    /// Serve from in-memory cache when warm. On cache miss, fetch from the
    /// network, store in cache, and return. The cache is session-scoped
    /// (never persisted to disk) so app restart always fetches fresh data.
    async fn entries(&self) -> CatalogResult<Vec<ServerEntry>> {
        // Fast path: in-memory cache hit.
        if let Some(cached) = self.cached_entries.lock().await.as_ref() {
            return Ok(cached.clone());
        }
        // Slow path: fetch from network with single-flight lock.
        let lock = self.cache.lock_for(&self.cfg.id).await;
        let _guard = lock.lock().await;
        // Double-check: another task may have populated the cache while we
        // waited for the lock.
        if let Some(cached) = self.cached_entries.lock().await.as_ref() {
            return Ok(cached.clone());
        }
        let entries = self.fetch().await.map_err(CatalogError::from)?;
        *self.cached_entries.lock().await = Some(entries.clone());
        Ok(entries)
    }
}

#[async_trait]
impl CatalogProvider for McpRegistryProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let mut entries = self.entries().await?;
        if let Some(kw) = &query.keyword {
            let kw_lower = kw.to_lowercase();
            entries.retain(|e| {
                let haystack = format!(
                    "{} {}",
                    e.display_name.to_lowercase(),
                    e.summary.to_lowercase()
                );
                haystack.contains(&kw_lower)
            });
        }
        if let Some(min) = query.trust_min {
            entries.retain(|e| e.trust >= min);
        }
        if let Some(limit) = query.limit {
            entries.truncate(limit);
        }
        Ok(entries)
    }

    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
        Ok(self.entries().await?.into_iter().find(|e| e.id == id))
    }

    async fn refresh(&self) -> CatalogResult<()> {
        let lock = self.cache.lock_for(&self.cfg.id).await;
        let _guard = lock.lock().await;
        // Clear the in-memory cache so the next read will see fresh data.
        self.cached_entries.lock().await.take();
        let entries = self.fetch().await?;
        *self.cached_entries.lock().await = Some(entries);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::remote::RemoteSourceKind;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
        // Header surfaced as configurable env var.
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

    #[tokio::test]
    async fn end_to_end_list_fetches_and_filters() {
        let server = MockServer::start().await;
        let body = json!({
            "servers": [
                sample_wrapper("com.example/a", true),
                sample_wrapper("com.example/b", false),
                sample_wrapper("com.example/c", true)
            ],
            "metadata": {"count": 3}
        });
        Mock::given(method("GET"))
            .and(path("/v0.1/servers"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
        let provider = McpRegistryProvider::new(
            RemoteSourceConfig {
                id: "mcp-registry".into(),
                display_name: "MCP Servers".into(),
                kind: RemoteSourceKind::McpRegistry,
                url: server.uri(),
                api_key_env: None,
                priority: 50,
                default_trust: TrustLevel::Community,
                enabled: true,
                cache_ttl_seconds: None,
            },
            SharedHttpClient::new().unwrap(),
            cache,
        );
        let entries = provider.list(&CatalogQuery::default()).await.unwrap();
        // Only isLatest==true entries are returned (a and c).
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "com.example/a");
        assert_eq!(entries[1].id, "com.example/c");
        assert_eq!(entries[0].source, "mcp-registry");
    }
}
