//! Smithery Registry catalog provider.
//!
//! Adapts the Smithery Registry API (`/servers` endpoint) to our internal
//! [`ServerEntry`] schema. Mapping is performed by the pure function
//! [`map_smithery_to_entry`] so it can be unit-tested without any HTTP.
//!
//! The list API (`GET /servers?page=N&pageSize=M`) returns a paginated
//! response with lightweight server metadata (no `connection` / `tags` /
//! `version` fields). Full details including `connections` are available
//! via the detail API (`GET /servers/{qualifiedName}`), which is used at
//! install time rather than during catalog browsing.

use crate::catalog::remote::http_cache::{CachedResponse, HttpResponseCache};
use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::remote::{RemoteError, RemoteSourceConfig};
use crate::catalog::{
    CatalogProvider, CatalogQuery, CatalogResult, EnvVarSpec, InstallSpec, RuntimeKind,
    RuntimeRequirement, ServerEntry, TrustLevel,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_SECONDS: u64 = 900;

/// Maximum number of servers to fetch from the paginated list API.
/// Smithery hosts 5 000+ entries; we cap at 200 (top by popularity) to
/// keep memory and startup latency reasonable.
const MAX_SERVERS_TO_FETCH: usize = 200;

/// Page size used when fetching the server list.
const PAGE_SIZE: usize = 100;

#[derive(Debug, Deserialize)]
struct SmitheryListResponse {
    #[serde(default)]
    servers: Vec<SmitheryServer>,
    #[serde(default)]
    pagination: Option<SmitheryPagination>,
}

#[derive(Debug, Deserialize)]
struct SmitheryPagination {
    #[serde(default, rename = "totalPages")]
    total_pages: u32,
}

/// Represents a server entry from the Smithery list API.
///
/// The list endpoint returns lightweight metadata; `connection`, `tags`,
/// `version`, `configSchema`, and `requirements` are only available from
/// the detail endpoint and are therefore `Option` / `Default` here.
#[derive(Debug, Deserialize)]
pub(super) struct SmitheryServer {
    #[serde(rename = "qualifiedName")]
    pub qualified_name: String,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "iconUrl")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub version: Option<String>,
    /// Present in the detail API; absent from the list API.
    #[serde(default)]
    pub connection: Option<SmitheryConnection>,
    #[serde(default, rename = "configSchema")]
    pub config_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub requirements: Option<SmitheryRequirements>,
    /// Whether this server is remotely hosted on Smithery's infrastructure.
    #[serde(default)]
    pub remote: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(super) enum SmitheryConnection {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    #[serde(alias = "sse")]
    Http {
        #[serde(rename = "connectionUrl", alias = "url")]
        connection_url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Deserialize)]
pub(super) struct SmitheryRequirements {
    #[serde(default)]
    pub runtimes: Vec<String>,
}

fn sanitize_id(qn: &str) -> String {
    qn.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

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

fn map_runtime(raw: &str) -> RuntimeRequirement {
    let kind = match raw.to_ascii_lowercase().as_str() {
        "node" | "node.js" | "nodejs" => RuntimeKind::Node,
        "python" | "python3" => RuntimeKind::Python,
        "uvx" => RuntimeKind::Uvx,
        "docker" => RuntimeKind::Docker,
        "bun" => RuntimeKind::Bun,
        "deno" => RuntimeKind::Deno,
        _ => RuntimeKind::Other,
    };
    RuntimeRequirement {
        kind,
        min_version: None,
        install_hint: if matches!(kind, RuntimeKind::Other) {
            Some(format!("install {raw}"))
        } else {
            None
        },
    }
}

fn map_config_schema_to_env(schema: &serde_json::Value) -> Vec<EnvVarSpec> {
    let obj = match schema.get("properties").and_then(|v| v.as_object()) {
        Some(o) => o,
        None => return vec![],
    };
    let required: std::collections::HashSet<String> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let mut out = Vec::with_capacity(obj.len());
    for (key, val) in obj {
        if val.get("oneOf").is_some() || val.get("enum").is_some() {
            tracing::warn!(prop=%key, "smithery configSchema: skipping unsupported keyword");
            continue;
        }
        let description = val
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let secret = val
            .get("format")
            .and_then(|v| v.as_str())
            .map(|f| f == "password")
            .unwrap_or(false);
        let default = val.get("default").and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            _ => v.as_str().map(str::to_string),
        });
        out.push(EnvVarSpec {
            key: key.clone(),
            label: key.clone(),
            description,
            required: required.contains(key),
            secret,
            default,
        });
    }
    out
}

pub(super) fn map_smithery_to_entry(
    source_id: &str,
    raw: &SmitheryServer,
    trust_ceiling: TrustLevel,
) -> Result<ServerEntry, RemoteError> {
    let id = sanitize_id(&raw.qualified_name);
    let display_name = raw
        .display_name
        .clone()
        .unwrap_or_else(|| raw.qualified_name.clone());
    let description = raw.description.clone().unwrap_or_default();
    let summary = if description.is_empty() {
        display_name.clone()
    } else {
        first_sentence(&description, 200)
    };
    let install = match &raw.connection {
        Some(SmitheryConnection::Stdio { command, args, env }) => InstallSpec::Stdio {
            command: command.clone(),
            args: args.clone(),
            env: env.clone(),
            cwd: None,
        },
        Some(SmitheryConnection::Http {
            connection_url,
            headers,
        }) => InstallSpec::Sse {
            url: connection_url.clone(),
            headers: headers.clone(),
        },
        None => {
            // List API omits connection info.  For remotely-hosted servers
            // Smithery exposes an SSE endpoint derived from the qualified
            // name.  Non-remote servers will need the detail API at install
            // time, but we still need a placeholder here so the catalog
            // card can render.
            if raw.remote {
                let slug = raw.qualified_name.replace('/', "--");
                InstallSpec::Sse {
                    url: format!("https://{slug}.smithery.ai/sse"),
                    headers: BTreeMap::new(),
                }
            } else {
                // Non-remote server without connection info — use a
                // placeholder that the installer will resolve via the
                // detail API.
                InstallSpec::Stdio {
                    command: raw.qualified_name.clone(),
                    args: vec![],
                    env: BTreeMap::new(),
                    cwd: None,
                }
            }
        }
    };
    let claimed_trust = if raw.verified {
        TrustLevel::Verified
    } else {
        TrustLevel::Community
    };
    let trust = if claimed_trust > trust_ceiling {
        trust_ceiling
    } else {
        claimed_trust
    };
    let requirements = raw
        .requirements
        .as_ref()
        .map(|r| r.runtimes.iter().map(|s| map_runtime(s)).collect())
        .unwrap_or_default();
    let default_env = raw
        .config_schema
        .as_ref()
        .map(map_config_schema_to_env)
        .unwrap_or_default();

    Ok(ServerEntry {
        id,
        source: source_id.to_string(),
        display_name,
        summary,
        description,
        categories: raw.tags.clone(),
        tags: raw.tags.clone(),
        author: None,
        homepage: raw.homepage.clone(),
        version: raw.version.clone(),
        install,
        requirements,
        trust,
        default_env,
        icon: raw.icon_url.clone(),
    })
}

pub struct SmitheryProvider {
    cfg: RemoteSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
}

impl SmitheryProvider {
    pub fn new(
        cfg: RemoteSourceConfig,
        http: SharedHttpClient,
        cache: Arc<HttpResponseCache>,
    ) -> Self {
        Self { cfg, http, cache }
    }

    fn ttl(&self) -> u64 {
        self.cfg.cache_ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS)
    }

    async fn fetch(&self) -> Result<Vec<ServerEntry>, RemoteError> {
        let base = format!("{}/servers", self.cfg.url.trim_end_matches('/'));
        let ceiling = self.cfg.default_trust;
        let mut all_entries: Vec<ServerEntry> = Vec::new();
        let mut page: u32 = 1;

        loop {
            let url = format!("{base}?page={page}&pageSize={PAGE_SIZE}");
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
            let parsed: SmitheryListResponse = serde_json::from_slice(&resp.body)
                .map_err(|e| RemoteError::Decode(format!("smithery page {page}: {e}")))?;

            for srv in &parsed.servers {
                match map_smithery_to_entry(&self.cfg.id, srv, ceiling) {
                    Ok(e) => all_entries.push(e),
                    Err(e) => {
                        tracing::warn!(qn=%srv.qualified_name, error=%e, "skipping entry")
                    }
                }
            }

            let has_more = parsed
                .pagination
                .as_ref()
                .map(|p| page < p.total_pages)
                .unwrap_or(false);

            if !has_more || all_entries.len() >= MAX_SERVERS_TO_FETCH {
                break;
            }
            page += 1;
        }

        all_entries.truncate(MAX_SERVERS_TO_FETCH);

        let cached = CachedResponse {
            fetched_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            etag: None,
            last_modified: None,
            entries: all_entries.clone(),
        };
        self.cache.put(&self.cfg.id, cached).await?;
        Ok(all_entries)
    }

    async fn entries(&self) -> CatalogResult<Vec<ServerEntry>> {
        let lock = self.cache.lock_for(&self.cfg.id).await;
        let _g = lock.lock().await;
        if let Some(c) = self.cache.get(&self.cfg.id).await {
            if HttpResponseCache::is_fresh(&c, self.ttl()) {
                return Ok(c.entries);
            }
            match self.fetch().await {
                Ok(e) => Ok(e),
                Err(e) => {
                    tracing::warn!(error=%e, "smithery refetch failed, serving stale");
                    Ok(c.entries)
                }
            }
        } else {
            self.fetch().await.map_err(Into::into)
        }
    }
}

#[async_trait]
impl CatalogProvider for SmitheryProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let mut entries = self.entries().await?;
        if let Some(kw) = &query.keyword {
            let kw_lc = kw.to_lowercase();
            entries.retain(|e| {
                let hay = format!(
                    "{} {}",
                    e.display_name.to_lowercase(),
                    e.summary.to_lowercase()
                );
                hay.contains(&kw_lc)
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
        self.fetch().await?;
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

    fn raw_stdio() -> SmitheryServer {
        SmitheryServer {
            qualified_name: "@org/server".into(),
            display_name: Some("Server".into()),
            description: Some("First sentence. Second sentence is ignored.".into()),
            icon_url: Some("https://x/icon.png".into()),
            homepage: Some("https://x".into()),
            tags: vec!["dev".into()],
            verified: true,
            version: Some("0.1.0".into()),
            connection: Some(SmitheryConnection::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "@org/server".into()],
                env: BTreeMap::new(),
            }),
            config_schema: None,
            requirements: Some(SmitheryRequirements {
                runtimes: vec!["node".into()],
            }),
            remote: false,
        }
    }

    #[test]
    fn maps_stdio_server() {
        let entry = map_smithery_to_entry("smithery", &raw_stdio(), TrustLevel::Verified).unwrap();
        assert_eq!(entry.source, "smithery");
        assert_eq!(entry.id, "-org-server");
        assert_eq!(entry.summary, "First sentence");
        assert_eq!(entry.trust, TrustLevel::Verified);
        match entry.install {
            InstallSpec::Stdio { command, args, .. } => {
                assert_eq!(command, "npx");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected stdio"),
        }
        assert_eq!(entry.requirements.len(), 1);
        assert_eq!(entry.requirements[0].kind, RuntimeKind::Node);
    }

    #[test]
    fn maps_http_server() {
        let mut raw = raw_stdio();
        raw.connection = Some(SmitheryConnection::Http {
            connection_url: "https://api.example.com/mcp".into(),
            headers: BTreeMap::new(),
        });
        let entry = map_smithery_to_entry("smithery", &raw, TrustLevel::Verified).unwrap();
        match entry.install {
            InstallSpec::Sse { url, .. } => assert_eq!(url, "https://api.example.com/mcp"),
            _ => panic!("expected sse"),
        }
    }

    #[test]
    fn remote_server_without_connection_gets_smithery_sse_url() {
        let mut raw = raw_stdio();
        raw.connection = None;
        raw.remote = true;
        raw.qualified_name = "upstash/context7-mcp".into();
        let entry = map_smithery_to_entry("smithery", &raw, TrustLevel::Verified).unwrap();
        match entry.install {
            InstallSpec::Sse { url, .. } => {
                assert_eq!(url, "https://upstash--context7-mcp.smithery.ai/sse");
            }
            _ => panic!("expected sse for remote server without connection"),
        }
    }

    #[test]
    fn non_remote_server_without_connection_gets_placeholder_stdio() {
        let mut raw = raw_stdio();
        raw.connection = None;
        raw.remote = false;
        raw.qualified_name = "some/local-tool".into();
        let entry = map_smithery_to_entry("smithery", &raw, TrustLevel::Verified).unwrap();
        match &entry.install {
            InstallSpec::Stdio { command, .. } => {
                assert_eq!(command, "some/local-tool");
            }
            _ => panic!("expected stdio placeholder for non-remote server"),
        }
    }

    #[test]
    fn unverified_clips_to_community() {
        let mut raw = raw_stdio();
        raw.verified = false;
        let entry = map_smithery_to_entry("smithery", &raw, TrustLevel::Verified).unwrap();
        assert_eq!(entry.trust, TrustLevel::Community);
    }

    #[test]
    fn trust_ceiling_clips_verified_to_community() {
        let entry = map_smithery_to_entry("smithery", &raw_stdio(), TrustLevel::Community).unwrap();
        assert_eq!(entry.trust, TrustLevel::Community);
    }

    #[test]
    fn unknown_runtime_becomes_other_with_hint() {
        let mut raw = raw_stdio();
        raw.requirements = Some(SmitheryRequirements {
            runtimes: vec!["rust".into()],
        });
        let entry = map_smithery_to_entry("smithery", &raw, TrustLevel::Verified).unwrap();
        assert_eq!(entry.requirements[0].kind, RuntimeKind::Other);
        assert_eq!(
            entry.requirements[0].install_hint.as_deref(),
            Some("install rust")
        );
    }

    #[test]
    fn config_schema_maps_required_and_secret() {
        let mut raw = raw_stdio();
        raw.config_schema = Some(json!({
            "type": "object",
            "required": ["API_KEY"],
            "properties": {
                "API_KEY": { "description": "key", "format": "password" },
                "REGION":  { "description": "region", "default": "us-east-1" }
            }
        }));
        let entry = map_smithery_to_entry("smithery", &raw, TrustLevel::Verified).unwrap();
        let api_key = entry
            .default_env
            .iter()
            .find(|e| e.key == "API_KEY")
            .unwrap();
        assert!(api_key.required);
        assert!(api_key.secret);
        let region = entry
            .default_env
            .iter()
            .find(|e| e.key == "REGION")
            .unwrap();
        assert_eq!(region.default.as_deref(), Some("us-east-1"));
        assert!(!region.required);
        assert!(!region.secret);
    }

    #[test]
    fn config_schema_skips_oneof() {
        let mut raw = raw_stdio();
        raw.config_schema = Some(json!({
            "properties": {
                "MODE": { "oneOf": [{"const":"a"},{"const":"b"}] }
            }
        }));
        let entry = map_smithery_to_entry("smithery", &raw, TrustLevel::Verified).unwrap();
        assert!(entry.default_env.is_empty());
    }

    #[tokio::test]
    async fn end_to_end_list_fetches_and_maps() {
        let server = MockServer::start().await;
        let body = r#"{"servers":[{
            "qualifiedName":"@a/b","displayName":"Ab","description":"Hi.",
            "verified":true,"remote":false
        }],"pagination":{"currentPage":1,"pageSize":100,"totalPages":1,"totalCount":1}}"#;
        Mock::given(method("GET"))
            .and(path("/servers"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
        let p = SmitheryProvider::new(
            RemoteSourceConfig {
                id: "smithery".into(),
                display_name: "Smithery".into(),
                kind: RemoteSourceKind::Smithery,
                url: server.uri(),
                api_key_env: None,
                priority: 50,
                default_trust: TrustLevel::Verified,
                enabled: true,
                cache_ttl_seconds: None,
            },
            SharedHttpClient::new().unwrap(),
            cache,
        );
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "smithery");
        assert_eq!(entries[0].display_name, "Ab");
    }
}
