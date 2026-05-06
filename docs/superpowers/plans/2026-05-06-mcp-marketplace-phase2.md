# MCP Marketplace Phase 2 (Remote Catalogs) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add user-configurable remote MCP catalog sources (Smithery + custom Kairox-format JSON endpoints) to the Phase 1 marketplace, with HTTP caching, parallel aggregation, failure tolerance, trust ceiling, and a GUI settings panel — without breaking any Phase 1 behavior.

**Architecture:** New `crates/agent-mcp/src/catalog/remote/` submodule with adapter pattern (`KairoxJsonProvider`, `SmitheryProvider`). New `HttpResponseCache` (TTL + ETag, on-disk). `AggregateCatalogProvider` extended with priority + parallel + per-source failure isolation. `agent-config` parses `[[catalog_sources]]` from `~/.kairox/mcp_servers.toml`. Four new Tauri commands + two new `EventPayload` variants. One new Vue component + extended `Marketplace.vue`.

**Tech Stack:** Rust (tokio, async-trait, reqwest behind `remote-catalog` feature, wiremock for tests, futures::join_all), Vue 3 + Pinia + Vitest + Playwright, tauri-specta for IPC type generation.

**Spec:** [`docs/superpowers/specs/2026-05-06-mcp-marketplace-phase2-design.md`](../specs/2026-05-06-mcp-marketplace-phase2-design.md)

---

## Pre-flight

Before T1, the worktree `.worktrees/feat-mcp-marketplace-phase2` has been created from `main` via `just worktree feat/mcp-marketplace-phase2` (handled by `using-git-worktrees` skill, not this plan). All work happens inside that worktree.

## Task index

- **T1** — `agent-mcp`: Add `remote-catalog` Cargo feature; declare `RemoteSourceConfig`, `RemoteSourceKind`, `RemoteError`, `DomainEventSink` types
- **T2** — `agent-mcp`: Implement `HttpResponseCache` (TTL + ETag + on-disk persistence + single-flight)
- **T3** — `agent-mcp`: Implement `SharedHttpClient` (reqwest wrapper with timeout + auth + UA)
- **T4** — `agent-mcp`: Implement `KairoxJsonProvider` adapter
- **T5** — `agent-mcp`: Implement `SmitheryProvider` adapter (pure mapping function + provider impl)
- **T6** — `agent-mcp`: Extend `AggregateCatalogProvider` with priority, parallel listing, failure isolation, `reload`, rate-limited error events
- **T7** — `agent-mcp`: Wire `build_provider(cfg)` constructor + add `remote::*` re-exports to `lib.rs`
- **T8** — `agent-config`: Parse `[[catalog_sources]]` table; extend `LoadedConfig`
- **T9** — `agent-core`: Add `CatalogSourceFailed` and `CatalogSourceAdded` `EventPayload` variants + specta registration
- **T10** — `agent-core`: Add 4 new `AppFacade` methods (`list/add/update/remove_catalog_source`)
- **T11** — `agent-runtime`: Implement `DomainEventSink` over the broadcaster; build aggregate at startup; implement the 4 new facade methods atop `~/.kairox/mcp_servers.toml`
- **T12** — `agent-runtime` integration test: end-to-end remote catalog with two wiremock servers
- **T13** — Tauri: 4 new `#[tauri::command] #[specta::specta]` wrappers, registration, `tauri-mock.js` updates
- **T14** — `just gen-types` + commit regenerated `commands.ts` / `events.ts`
- **T15** — Vue: extend `stores/catalog.ts` with sources state + actions
- **T16** — Vue: build `CatalogSourcesSettings.vue` (Vitest first)
- **T17** — Vue: extend `Marketplace.vue` with multi-source chip filter + ⚠ badge on `CatalogSourceFailed`
- **T18** — E2E: extend `marketplace.spec.ts` with add/remove source + install-from-remote flow
- **T19** — Final verification gate: `just check`, `just test-mcp`, `just test-fullstack`, `just check-types`, `just test-e2e`

## Dependency graph

```text
T1 ─┬─ T2 ─┬─ T4 ─┬─ T6 ─ T7 ─ T11 ─ T12 ─ T13 ─ T14 ─ T15 ─ T16 ─ T17 ─ T18 ─ T19
    │      │      │              │
    │      └─ T3 ─┘              │
    │                            │
    ├─ T5 ───────────────────────┤
    │                            │
    └─ T8 ───────────────────────┤
                                 │
T9 ─ T10 ────────────────────────┘
```

T1 / T9 / T10 are independent of each other and can be parallelized when dispatched as subagents. T2/T3/T4/T5/T8 are independent once T1 lands.

---

(Detailed task steps follow below; each step is bite-sized.)

## Task 1: Add `remote-catalog` feature + base remote types

**Files:**

- Modify: `crates/agent-mcp/Cargo.toml`
- Create: `crates/agent-mcp/src/catalog/remote/mod.rs`
- Modify: `crates/agent-mcp/src/catalog/mod.rs` (add `pub mod remote;` + `DomainEventSink` trait + `From<RemoteError>` impl)
- Test: same file (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Add the `remote-catalog` Cargo feature**

In `crates/agent-mcp/Cargo.toml`, under `[features]`, add:

```toml
default = ["remote-catalog"]
remote-catalog = ["dep:reqwest", "dep:futures"]
```

And ensure `reqwest` is `optional = true` (already is) and add `futures = { version = "0.3", optional = true }` to `[dependencies]`. Add `futures.workspace = true` to root `Cargo.toml` `[workspace.dependencies]` if not present.

- [ ] **Step 2: Create `remote/mod.rs` with failing test**

```rust
// crates/agent-mcp/src/catalog/remote/mod.rs
use crate::catalog::TrustLevel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RemoteSourceConfig {
    pub id: String,
    pub display_name: String,
    pub kind: RemoteSourceKind,
    pub url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: u32,
    #[serde(default = "default_trust")]
    pub default_trust: TrustLevel,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub cache_ttl_seconds: Option<u64>,
}

fn default_priority() -> u32 { 100 }
fn default_trust() -> TrustLevel { TrustLevel::Community }
fn default_true() -> bool { true }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RemoteSourceKind {
    KairoxJson,
    Smithery,
}

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("http: {0}")]
    Http(String),
    #[error("decode: {0}")]
    Decode(String),
    #[error("auth missing: env var {0} not set")]
    AuthMissing(String),
    #[error("cache io: {0}")]
    CacheIo(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_through_toml_with_defaults() {
        let toml = r#"
            id = "smithery"
            display_name = "Smithery"
            kind = "smithery"
            url = "https://registry.smithery.ai"
        "#;
        let cfg: RemoteSourceConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.id, "smithery");
        assert_eq!(cfg.kind, RemoteSourceKind::Smithery);
        assert_eq!(cfg.priority, 100);
        assert_eq!(cfg.default_trust, TrustLevel::Community);
        assert!(cfg.enabled);
        assert!(cfg.api_key_env.is_none());
    }

    #[test]
    fn config_serializes_kind_snake_case() {
        let cfg = RemoteSourceConfig {
            id: "x".into(),
            display_name: "X".into(),
            kind: RemoteSourceKind::KairoxJson,
            url: "https://example.com/c.json".into(),
            api_key_env: None,
            priority: 100,
            default_trust: TrustLevel::Community,
            enabled: true,
            cache_ttl_seconds: None,
        };
        let s = toml::to_string(&cfg).unwrap();
        assert!(s.contains(r#"kind = "kairox_json""#), "got:\n{s}");
    }
}
```

In `crates/agent-mcp/src/catalog/mod.rs`, append at the bottom (next to existing `pub mod aggregate; pub mod builtin;`):

```rust
#[cfg(feature = "remote-catalog")]
pub mod remote;

#[cfg(feature = "remote-catalog")]
impl From<remote::RemoteError> for CatalogError {
    fn from(e: remote::RemoteError) -> Self {
        CatalogError::Provider(e.to_string())
    }
}

/// Sink for emitting per-source failure events out of the catalog layer.
/// Implemented by `agent-runtime` over its `DomainEvent` broadcaster so the
/// catalog crate stays event-bus agnostic.
#[async_trait::async_trait]
pub trait DomainEventSink: Send + Sync {
    async fn emit_source_failed(&self, source_id: &str, error: &str);
    async fn emit_source_added(&self, source_id: &str);
}
```

Add `toml = "0.8"` to `agent-mcp` `[dev-dependencies]` if not present.

- [ ] **Step 3: Run tests to verify they pass**

```bash
cargo test -p agent-mcp --features remote-catalog catalog::remote::tests
```

Expected: 2 PASSED.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-mcp/ Cargo.toml
git commit -m "feat(mcp): add remote-catalog feature and base remote source types"
```

---

## Task 2: HttpResponseCache (TTL + ETag + on-disk + single-flight)

**Files:**

- Create: `crates/agent-mcp/src/catalog/remote/http_cache.rs`
- Modify: `crates/agent-mcp/src/catalog/remote/mod.rs` (add `pub(crate) mod http_cache;`)

- [ ] **Step 1: Write the failing tests**

```rust
// crates/agent-mcp/src/catalog/remote/http_cache.rs
use crate::catalog::ServerEntry;
use crate::catalog::remote::RemoteError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedResponse {
    pub fetched_at_unix: u64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub entries: Vec<ServerEntry>,
}

pub(crate) struct HttpResponseCache {
    cache_dir: PathBuf,
    in_memory: Mutex<HashMap<String, CachedResponse>>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl HttpResponseCache {
    pub fn new(cache_dir: PathBuf) -> Self { todo!() }
    pub async fn get(&self, key: &str) -> Option<CachedResponse> { todo!() }
    pub async fn put(&self, key: &str, value: CachedResponse) -> Result<(), RemoteError> { todo!() }
    pub fn is_fresh(value: &CachedResponse, ttl_seconds: u64) -> bool { todo!() }
    pub async fn lock_for(&self, key: &str) -> Arc<Mutex<()>> { todo!() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{InstallSpec, TrustLevel};
    use std::collections::BTreeMap;

    fn sample_entries() -> Vec<ServerEntry> {
        vec![ServerEntry {
            id: "s".into(), source: "x".into(), display_name: "S".into(),
            summary: "".into(), description: "".into(),
            categories: vec![], tags: vec![], author: None, homepage: None,
            version: None,
            install: InstallSpec::Stdio { command: "echo".into(), args: vec![],
                env: BTreeMap::new(), cwd: None },
            requirements: vec![], trust: TrustLevel::Community,
            default_env: vec![], icon: None,
        }]
    }

    #[tokio::test]
    async fn put_then_get_round_trips_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let cache = HttpResponseCache::new(dir.path().to_path_buf());
        let v = CachedResponse {
            fetched_at_unix: 100, etag: Some("W/\"abc\"".into()),
            last_modified: None, entries: sample_entries(),
        };
        cache.put("src1", v.clone()).await.unwrap();
        let got = cache.get("src1").await.unwrap();
        assert_eq!(got.entries.len(), 1);
        assert_eq!(got.etag.as_deref(), Some("W/\"abc\""));
    }

    #[tokio::test]
    async fn put_persists_to_disk_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let cache1 = HttpResponseCache::new(path.clone());
        let v = CachedResponse {
            fetched_at_unix: 200, etag: None, last_modified: None,
            entries: sample_entries(),
        };
        cache1.put("src2", v).await.unwrap();
        // New instance — should read disk on first get()
        let cache2 = HttpResponseCache::new(path);
        let got = cache2.get("src2").await.unwrap();
        assert_eq!(got.fetched_at_unix, 200);
    }

    #[test]
    fn is_fresh_within_ttl() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let v = CachedResponse {
            fetched_at_unix: now, etag: None, last_modified: None,
            entries: vec![],
        };
        assert!(HttpResponseCache::is_fresh(&v, 60));
    }

    #[test]
    fn is_stale_after_ttl() {
        let v = CachedResponse {
            fetched_at_unix: 0, etag: None, last_modified: None,
            entries: vec![],
        };
        assert!(!HttpResponseCache::is_fresh(&v, 60));
    }

    #[tokio::test]
    async fn lock_for_returns_same_mutex_across_calls() {
        let dir = tempfile::tempdir().unwrap();
        let cache = HttpResponseCache::new(dir.path().to_path_buf());
        let l1 = cache.lock_for("k").await;
        let l2 = cache.lock_for("k").await;
        assert!(Arc::ptr_eq(&l1, &l2));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p agent-mcp --features remote-catalog catalog::remote::http_cache::tests
```

Expected: 5 FAILED with `unimplemented` from `todo!()`.

- [ ] **Step 3: Implement `HttpResponseCache`**

Replace each `todo!()` body:

```rust
impl HttpResponseCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            in_memory: Mutex::new(HashMap::new()),
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, key: &str) -> Option<CachedResponse> {
        if let Some(v) = self.in_memory.lock().await.get(key) {
            return Some(v.clone());
        }
        // Disk fallback — best effort, decode errors treated as miss.
        let path = self.cache_dir.join(format!("{key}.json"));
        let bytes = tokio::fs::read(&path).await.ok()?;
        let value: CachedResponse = serde_json::from_slice(&bytes).ok()?;
        self.in_memory.lock().await.insert(key.to_string(), value.clone());
        Some(value)
    }

    pub async fn put(&self, key: &str, value: CachedResponse) -> Result<(), RemoteError> {
        tokio::fs::create_dir_all(&self.cache_dir)
            .await
            .map_err(|e| RemoteError::CacheIo(e.to_string()))?;
        let bytes = serde_json::to_vec(&value)
            .map_err(|e| RemoteError::CacheIo(e.to_string()))?;
        let final_path = self.cache_dir.join(format!("{key}.json"));
        let tmp_path = self.cache_dir.join(format!("{key}.json.tmp"));
        tokio::fs::write(&tmp_path, &bytes)
            .await
            .map_err(|e| RemoteError::CacheIo(e.to_string()))?;
        tokio::fs::rename(&tmp_path, &final_path)
            .await
            .map_err(|e| RemoteError::CacheIo(e.to_string()))?;
        self.in_memory.lock().await.insert(key.to_string(), value);
        Ok(())
    }

    pub fn is_fresh(value: &CachedResponse, ttl_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now.saturating_sub(value.fetched_at_unix) < ttl_seconds
    }

    pub async fn lock_for(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().await;
        locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}
```

Wire the module: in `crates/agent-mcp/src/catalog/remote/mod.rs` add `pub(crate) mod http_cache;`. Add `tempfile = "3"` to `agent-mcp` `[dev-dependencies]` if not present.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p agent-mcp --features remote-catalog catalog::remote::http_cache::tests
```

Expected: 5 PASSED.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-mcp/
git commit -m "feat(mcp): add HttpResponseCache with TTL, on-disk persistence, single-flight locks"
```

---

## Task 3: SharedHttpClient (reqwest wrapper with timeout + auth + UA)

**Files:**

- Create: `crates/agent-mcp/src/catalog/remote/http_client.rs`
- Modify: `crates/agent-mcp/src/catalog/remote/mod.rs` (add `pub(crate) mod http_client;`)

- [ ] **Step 1: Write the failing test**

```rust
// crates/agent-mcp/src/catalog/remote/http_client.rs
use crate::catalog::remote::RemoteError;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, USER_AGENT};
use std::time::Duration;

const DEFAULT_USER_AGENT: &str = concat!("kairox-marketplace/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub(crate) struct SharedHttpClient {
    inner: reqwest::Client,
}

pub(crate) struct GetOpts<'a> {
    pub api_key_env: Option<&'a str>,
    pub if_none_match: Option<&'a str>,
}

pub(crate) struct GetResponse {
    pub status: u16,
    pub etag: Option<String>,
    pub body: Vec<u8>,
}

impl SharedHttpClient {
    pub fn new() -> Result<Self, RemoteError> {
        let inner = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map_err(|e| RemoteError::Http(e.to_string()))?;
        Ok(Self { inner })
    }

    pub async fn get_json(&self, url: &str, opts: GetOpts<'_>) -> Result<GetResponse, RemoteError> {
        let mut headers = HeaderMap::new();
        if let Some(env_key) = opts.api_key_env {
            let value = std::env::var(env_key)
                .map_err(|_| RemoteError::AuthMissing(env_key.to_string()))?;
            let header_val = HeaderValue::from_str(&format!("Bearer {value}"))
                .map_err(|e| RemoteError::Http(e.to_string()))?;
            headers.insert(AUTHORIZATION, header_val);
        }
        if let Some(etag) = opts.if_none_match {
            headers.insert(
                HeaderName::from_static("if-none-match"),
                HeaderValue::from_str(etag).map_err(|e| RemoteError::Http(e.to_string()))?,
            );
        }
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        let resp = self
            .inner
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| RemoteError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let body = resp
            .bytes()
            .await
            .map_err(|e| RemoteError::Http(e.to_string()))?
            .to_vec();
        Ok(GetResponse { status, etag, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, header_exists, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_json_sends_user_agent_and_returns_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .and(header_exists("user-agent"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "W/\"v1\"")
                    .set_body_string(r#"{"ok":true}"#),
            )
            .mount(&server)
            .await;
        let c = SharedHttpClient::new().unwrap();
        let resp = c
            .get_json(
                &format!("{}/c.json", server.uri()),
                GetOpts { api_key_env: None, if_none_match: None },
            )
            .await
            .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.etag.as_deref(), Some("W/\"v1\""));
        assert_eq!(resp.body, br#"{"ok":true}"#);
    }

    #[tokio::test]
    async fn get_json_attaches_bearer_when_env_set() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/x"))
            .and(header("authorization", "Bearer SECRET-VALUE"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .mount(&server)
            .await;
        // SAFETY: tests share env; use a unique key per test run to avoid collisions.
        std::env::set_var("KAIROX_TEST_BEARER_T3", "SECRET-VALUE");
        let c = SharedHttpClient::new().unwrap();
        let resp = c
            .get_json(
                &format!("{}/x", server.uri()),
                GetOpts {
                    api_key_env: Some("KAIROX_TEST_BEARER_T3"),
                    if_none_match: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(resp.status, 200);
        std::env::remove_var("KAIROX_TEST_BEARER_T3");
    }

    #[tokio::test]
    async fn get_json_returns_auth_missing_when_env_unset() {
        let c = SharedHttpClient::new().unwrap();
        std::env::remove_var("KAIROX_TEST_MISSING_T3");
        let err = c
            .get_json(
                "http://127.0.0.1:1/never",
                GetOpts {
                    api_key_env: Some("KAIROX_TEST_MISSING_T3"),
                    if_none_match: None,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, RemoteError::AuthMissing(ref k) if k == "KAIROX_TEST_MISSING_T3"));
    }

    #[tokio::test]
    async fn get_json_returns_304_when_etag_matches() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/304"))
            .and(header("if-none-match", "W/\"v1\""))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;
        let c = SharedHttpClient::new().unwrap();
        let resp = c
            .get_json(
                &format!("{}/304", server.uri()),
                GetOpts { api_key_env: None, if_none_match: Some("W/\"v1\"") },
            )
            .await
            .unwrap();
        assert_eq!(resp.status, 304);
    }
}
```

In `crates/agent-mcp/src/catalog/remote/mod.rs` add `pub(crate) mod http_client;`.

- [ ] **Step 2: Run tests to verify they pass**

The implementation is included alongside the test in Step 1 (this is a small wrapper, no separate fail-step needed). Run:

```bash
cargo test -p agent-mcp --features remote-catalog catalog::remote::http_client::tests
```

Expected: 4 PASSED.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/
git commit -m "feat(mcp): add SharedHttpClient wrapper with timeout, bearer auth, and ETag support"
```

---

## Task 4: KairoxJsonProvider adapter

**Files:**

- Create: `crates/agent-mcp/src/catalog/remote/kairox_json.rs`
- Modify: `crates/agent-mcp/src/catalog/remote/mod.rs` (add `pub mod kairox_json;`)

- [ ] **Step 1: Write the failing tests**

```rust
// crates/agent-mcp/src/catalog/remote/kairox_json.rs
use crate::catalog::remote::http_cache::{CachedResponse, HttpResponseCache};
use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::remote::{RemoteError, RemoteSourceConfig};
use crate::catalog::{
    CatalogError, CatalogProvider, CatalogQuery, CatalogResult, ServerEntry, TrustLevel,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_SECONDS: u64 = 900; // 15 minutes

#[derive(Debug, Deserialize)]
struct Doc {
    schema_version: String,
    #[serde(default)]
    #[allow(dead_code)]
    generated_at: Option<String>,
    entries: Vec<ServerEntry>,
}

pub struct KairoxJsonProvider {
    cfg: RemoteSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
}

impl KairoxJsonProvider {
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

    fn clip_trust(entry: &mut ServerEntry, ceiling: TrustLevel) {
        if entry.trust > ceiling {
            entry.trust = ceiling;
        }
    }

    async fn fetch_and_store(&self, etag: Option<&str>) -> Result<CachedResponse, RemoteError> {
        let resp = self
            .http
            .get_json(
                &self.cfg.url,
                GetOpts {
                    api_key_env: self.cfg.api_key_env.as_deref(),
                    if_none_match: etag,
                },
            )
            .await?;
        if resp.status == 304 {
            // 304 means "use cache". Caller decides what to do; we re-bubble
            // by returning a sentinel error type-tagged via a separate path.
            return Err(RemoteError::Http("304_not_modified".into()));
        }
        if !(200..300).contains(&resp.status) {
            return Err(RemoteError::Http(format!("status {}", resp.status)));
        }
        let doc: Doc = serde_json::from_slice(&resp.body)
            .map_err(|e| RemoteError::Decode(format!("body: {e}")))?;
        if doc.schema_version != "1" {
            return Err(RemoteError::Decode(format!(
                "unsupported schema_version: {}",
                doc.schema_version
            )));
        }
        let ceiling = self.cfg.default_trust;
        let mut entries = doc.entries;
        for entry in &mut entries {
            entry.source = self.cfg.id.clone();
            Self::clip_trust(entry, ceiling);
        }
        let cached = CachedResponse {
            fetched_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            etag: resp.etag,
            last_modified: None,
            entries,
        };
        self.cache.put(&self.cfg.id, cached.clone()).await?;
        Ok(cached)
    }

    async fn entries(&self) -> CatalogResult<Vec<ServerEntry>> {
        // Single-flight per source id.
        let lock = self.cache.lock_for(&self.cfg.id).await;
        let _guard = lock.lock().await;

        let cached = self.cache.get(&self.cfg.id).await;
        if let Some(c) = &cached {
            if HttpResponseCache::is_fresh(c, self.ttl()) {
                return Ok(c.entries.clone());
            }
        }
        // Stale or missing — try refetch with conditional GET.
        let etag = cached.as_ref().and_then(|c| c.etag.clone());
        match self.fetch_and_store(etag.as_deref()).await {
            Ok(c) => Ok(c.entries),
            Err(RemoteError::Http(ref s)) if s == "304_not_modified" => {
                // Refresh fetched_at by re-putting the same body.
                if let Some(mut c) = cached {
                    c.fetched_at_unix = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    self.cache.put(&self.cfg.id, c.clone()).await?;
                    Ok(c.entries)
                } else {
                    Err(CatalogError::Provider(
                        "304 with no cached body".to_string(),
                    ))
                }
            }
            Err(e) => {
                // Soft fallback: serve stale if we have it.
                if let Some(c) = cached {
                    tracing::warn!(source=%self.cfg.id, error=%e, "kairox_json refetch failed, serving stale");
                    Ok(c.entries)
                } else {
                    Err(e.into())
                }
            }
        }
    }
}

#[async_trait]
impl CatalogProvider for KairoxJsonProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let mut out: Vec<ServerEntry> = self
            .entries()
            .await?
            .into_iter()
            .filter(|e| {
                if let Some(kw) = &query.keyword {
                    let kw_lc = kw.to_lowercase();
                    let hay = format!(
                        "{} {} {}",
                        e.display_name.to_lowercase(),
                        e.summary.to_lowercase(),
                        e.tags.join(" ").to_lowercase()
                    );
                    if !hay.contains(&kw_lc) {
                        return false;
                    }
                }
                if let Some(cat) = &query.category {
                    if !e.categories.iter().any(|c| c == cat) {
                        return false;
                    }
                }
                if let Some(min) = query.trust_min {
                    if e.trust < min {
                        return false;
                    }
                }
                true
            })
            .collect();
        out.sort_by(|a, b| {
            b.trust
                .cmp(&a.trust)
                .then_with(|| a.display_name.cmp(&b.display_name))
        });
        if let Some(limit) = query.limit {
            out.truncate(limit);
        }
        Ok(out)
    }

    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
        Ok(self.entries().await?.into_iter().find(|e| e.id == id))
    }

    async fn refresh(&self) -> CatalogResult<()> {
        // Force refetch by zeroing the cache entry's fetched_at.
        if let Some(mut c) = self.cache.get(&self.cfg.id).await {
            c.fetched_at_unix = 0;
            self.cache.put(&self.cfg.id, c).await?;
        }
        let _ = self.entries().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::remote::RemoteSourceKind;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cfg(url: &str, ceiling: TrustLevel) -> RemoteSourceConfig {
        RemoteSourceConfig {
            id: "kx".into(),
            display_name: "kx".into(),
            kind: RemoteSourceKind::KairoxJson,
            url: url.to_string(),
            api_key_env: None,
            priority: 100,
            default_trust: ceiling,
            enabled: true,
            cache_ttl_seconds: None,
        }
    }

    fn body(trust: &str) -> String {
        format!(
            r#"{{
              "schema_version": "1",
              "entries": [{{
                "id": "x",
                "source": "ignored",
                "display_name": "X",
                "summary": "s",
                "description": "d",
                "categories": ["dev-tools"],
                "tags": ["t"],
                "install": {{"transport":"stdio","command":"echo","args":[],"env":{{}}}},
                "requirements": [],
                "trust": "{trust}",
                "default_env": []
              }}]
            }}"#
        )
    }

    async fn provider_for(server: &MockServer, ceiling: TrustLevel) -> KairoxJsonProvider {
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(HttpResponseCache::new(dir.path().to_path_buf()));
        let http = SharedHttpClient::new().unwrap();
        // tempdir leaks intentionally — short-lived test process
        std::mem::forget(dir);
        KairoxJsonProvider::new(cfg(&format!("{}/c.json", server.uri()), ceiling), http, cache)
    }

    #[tokio::test]
    async fn list_returns_entries_and_overwrites_source_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body("verified")))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified).await;
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "kx");
        assert_eq!(entries[0].trust, TrustLevel::Verified);
    }

    #[tokio::test]
    async fn list_clips_trust_to_ceiling() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body("verified")))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Community).await;
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries[0].trust, TrustLevel::Community);
    }

    #[tokio::test]
    async fn list_serves_stale_on_5xx_after_first_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body("community")))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified).await;
        // First call: succeeds.
        let _ = p.list(&CatalogQuery::default()).await.unwrap();
        // Force refresh path by calling refresh().
        p.refresh().await.unwrap_err(); // refetch fails with 503; refresh propagates if cache absent
        // List must still serve stale entries from cache.
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn list_returns_decode_error_on_bad_schema_version() {
        let server = MockServer::start().await;
        let bad = r#"{"schema_version":"99","entries":[]}"#;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(bad))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified).await;
        let err = p.list(&CatalogQuery::default()).await.unwrap_err();
        assert!(matches!(err, CatalogError::Provider(_)));
    }

    #[tokio::test]
    async fn second_call_within_ttl_uses_cache_and_does_not_hit_network() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body("community")))
            .expect(1)
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified).await;
        let _ = p.list(&CatalogQuery::default()).await.unwrap();
        let _ = p.list(&CatalogQuery::default()).await.unwrap();
        // .expect(1) above asserts via wiremock on drop.
    }

    #[tokio::test]
    async fn conditional_get_with_if_none_match_when_etag_known() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "W/\"v1\"")
                    .set_body_string(body("community")),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/c.json"))
            .and(header("if-none-match", "W/\"v1\""))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;
        let p = provider_for(&server, TrustLevel::Verified).await;
        let _ = p.list(&CatalogQuery::default()).await.unwrap();
        // refresh forces a refetch which should send If-None-Match and 304.
        p.refresh().await.unwrap();
        // Cache still serves entries.
        let entries = p.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
    }
}
```

Wire `pub mod kairox_json;` in `remote/mod.rs`, also add `pub use kairox_json::KairoxJsonProvider;`.

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test -p agent-mcp --features remote-catalog catalog::remote::kairox_json::tests
```

Expected: 6 PASSED. If clippy emits dead-code warnings on `Doc.generated_at`, the `#[allow(dead_code)]` attribute already covers it.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/
git commit -m "feat(mcp): add KairoxJsonProvider remote catalog adapter with TTL/ETag/stale-fallback"
```

---

## Task 5: SmitheryProvider adapter (mapping function + provider impl)

**Files:**

- Create: `crates/agent-mcp/src/catalog/remote/smithery.rs`
- Create: `crates/agent-mcp/src/catalog/remote/fixtures/smithery_servers.json` (test fixture)
- Modify: `crates/agent-mcp/src/catalog/remote/mod.rs` (add `pub mod smithery;`)

- [ ] **Step 1: Write the failing test for the pure mapping function first**

```rust
// crates/agent-mcp/src/catalog/remote/smithery.rs
use crate::catalog::remote::http_cache::{CachedResponse, HttpResponseCache};
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
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_SECONDS: u64 = 900;

#[derive(Debug, Deserialize)]
struct SmitheryListResponse {
    #[serde(default)]
    servers: Vec<SmitheryServer>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SmitheryServer {
    pub qualified_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub version: Option<String>,
    pub connection: SmitheryConnection,
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub requirements: Option<SmitheryRequirements>,
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
        .find(|c: char| c == '.' || c == '\n')
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
            tracing::warn!(prop=%key, "smithery configSchema: skipping unsupported keyword (oneOf/enum)");
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
        SmitheryConnection::Stdio { command, args, env } => InstallSpec::Stdio {
            command: command.clone(),
            args: args.clone(),
            env: env.clone(),
            cwd: None,
        },
        SmitheryConnection::Http {
            connection_url,
            headers,
        } => InstallSpec::Sse {
            url: connection_url.clone(),
            headers: headers.clone(),
        },
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
        .map(|s| map_config_schema_to_env(s))
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
    pub fn new(cfg: RemoteSourceConfig, http: SharedHttpClient, cache: Arc<HttpResponseCache>) -> Self {
        Self { cfg, http, cache }
    }

    fn ttl(&self) -> u64 {
        self.cfg.cache_ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS)
    }

    async fn fetch(&self) -> Result<Vec<ServerEntry>, RemoteError> {
        let url = format!("{}/servers", self.cfg.url.trim_end_matches('/'));
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
            .map_err(|e| RemoteError::Decode(format!("smithery: {e}")))?;
        let ceiling = self.cfg.default_trust;
        let mut entries = Vec::with_capacity(parsed.servers.len());
        for srv in &parsed.servers {
            match map_smithery_to_entry(&self.cfg.id, srv, ceiling) {
                Ok(e) => entries.push(e),
                Err(e) => tracing::warn!(qn=%srv.qualified_name, error=%e, "skipping entry"),
            }
        }
        let cached = CachedResponse {
            fetched_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            etag: resp.etag,
            last_modified: None,
            entries: entries.clone(),
        };
        self.cache.put(&self.cfg.id, cached).await?;
        Ok(entries)
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
                let hay = format!("{} {}", e.display_name.to_lowercase(), e.summary.to_lowercase());
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
            connection: SmitheryConnection::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "@org/server".into()],
                env: BTreeMap::new(),
            },
            config_schema: None,
            requirements: Some(SmitheryRequirements {
                runtimes: vec!["node".into()],
            }),
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
        raw.connection = SmitheryConnection::Http {
            connection_url: "https://api.example.com/mcp".into(),
            headers: BTreeMap::new(),
        };
        let entry = map_smithery_to_entry("smithery", &raw, TrustLevel::Verified).unwrap();
        match entry.install {
            InstallSpec::Sse { url, .. } => assert_eq!(url, "https://api.example.com/mcp"),
            _ => panic!("expected sse"),
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
        assert_eq!(entry.requirements[0].install_hint.as_deref(), Some("install rust"));
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
        let api_key = entry.default_env.iter().find(|e| e.key == "API_KEY").unwrap();
        assert!(api_key.required);
        assert!(api_key.secret);
        let region = entry.default_env.iter().find(|e| e.key == "REGION").unwrap();
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
            "qualifiedName":"@a/b","displayName":"Ab","description":"Hi.","tags":[],
            "verified":true,
            "connection":{"type":"stdio","command":"echo","args":[],"env":{}}
        }]}"#;
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
```

Wire `pub mod smithery;` and `pub use smithery::SmitheryProvider;` in `remote/mod.rs`.

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test -p agent-mcp --features remote-catalog catalog::remote::smithery::tests
```

Expected: 8 PASSED.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/
git commit -m "feat(mcp): add SmitheryProvider remote catalog adapter"
```

---

## Task 6: Extend AggregateCatalogProvider (priority + parallel + failure isolation + reload + rate limit)

**Files:**

- Modify: `crates/agent-mcp/src/catalog/aggregate.rs`

- [ ] **Step 1: Write the failing tests**

Append to `aggregate.rs` `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests_phase2 {
    use super::*;
    use crate::catalog::{
        CatalogError, CatalogProvider, CatalogQuery, CatalogResult, DomainEventSink, InstallSpec,
        ServerEntry, TrustLevel,
    };
    use async_trait::async_trait;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn make_entry(id: &str, source: &str, trust: TrustLevel) -> ServerEntry {
        ServerEntry {
            id: id.into(), source: source.into(), display_name: id.into(),
            summary: "".into(), description: "".into(),
            categories: vec![], tags: vec![], author: None, homepage: None,
            version: None,
            install: InstallSpec::Stdio { command: "x".into(), args: vec![],
                env: BTreeMap::new(), cwd: None },
            requirements: vec![], trust, default_env: vec![], icon: None,
        }
    }

    struct StaticProvider {
        id: &'static str,
        entries: Vec<ServerEntry>,
    }

    #[async_trait]
    impl CatalogProvider for StaticProvider {
        fn source_id(&self) -> &str { self.id }
        async fn list(&self, _q: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
            Ok(self.entries.clone())
        }
        async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
            Ok(self.entries.iter().find(|e| e.id == id).cloned())
        }
    }

    struct FailingProvider { id: &'static str }
    #[async_trait]
    impl CatalogProvider for FailingProvider {
        fn source_id(&self) -> &str { self.id }
        async fn list(&self, _q: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
            Err(CatalogError::Provider("boom".into()))
        }
        async fn get(&self, _id: &str) -> CatalogResult<Option<ServerEntry>> {
            Err(CatalogError::Provider("boom".into()))
        }
    }

    #[derive(Default)]
    struct RecordingSink {
        failed: Mutex<Vec<(String, String)>>,
        added: Mutex<Vec<String>>,
    }
    #[async_trait]
    impl DomainEventSink for RecordingSink {
        async fn emit_source_failed(&self, id: &str, err: &str) {
            self.failed.lock().unwrap().push((id.to_string(), err.to_string()));
        }
        async fn emit_source_added(&self, id: &str) {
            self.added.lock().unwrap().push(id.to_string());
        }
    }

    #[tokio::test]
    async fn higher_priority_source_first_in_aggregated_list() {
        let low = Arc::new(StaticProvider { id: "low",
            entries: vec![make_entry("a", "low", TrustLevel::Community)] });
        let high = Arc::new(StaticProvider { id: "high",
            entries: vec![make_entry("b", "high", TrustLevel::Community)] });
        let agg = AggregateCatalogProvider::new_with_priority(
            vec![(100, low), (10, high)],
            None,
        );
        let entries = agg.list(&CatalogQuery::default()).await.unwrap();
        // Both present; high-priority source's entry comes first when trust ties.
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].source, "high");
    }

    #[tokio::test]
    async fn one_source_failure_does_not_fail_aggregate() {
        let ok = Arc::new(StaticProvider { id: "ok",
            entries: vec![make_entry("a", "ok", TrustLevel::Community)] });
        let bad = Arc::new(FailingProvider { id: "bad" });
        let sink = Arc::new(RecordingSink::default());
        let agg = AggregateCatalogProvider::new_with_priority(
            vec![(10, ok), (20, bad)],
            Some(sink.clone() as Arc<dyn DomainEventSink>),
        );
        let entries = agg.list(&CatalogQuery::default()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "ok");
        let failed = sink.failed.lock().unwrap().clone();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].0, "bad");
    }

    #[tokio::test]
    async fn duplicate_failure_within_60s_is_rate_limited() {
        let bad = Arc::new(FailingProvider { id: "bad" });
        let sink = Arc::new(RecordingSink::default());
        let agg = AggregateCatalogProvider::new_with_priority(
            vec![(20, bad)],
            Some(sink.clone() as Arc<dyn DomainEventSink>),
        );
        for _ in 0..3 {
            let _ = agg.list(&CatalogQuery::default()).await;
        }
        let failed = sink.failed.lock().unwrap().clone();
        assert_eq!(failed.len(), 1, "duplicate (source, error) should rate-limit");
    }

    #[tokio::test]
    async fn reload_swaps_providers_atomically() {
        let v1 = Arc::new(StaticProvider { id: "v",
            entries: vec![make_entry("a", "v", TrustLevel::Community)] });
        let mut agg = AggregateCatalogProvider::new_with_priority(vec![(10, v1)], None);
        assert_eq!(agg.list(&CatalogQuery::default()).await.unwrap().len(), 1);
        let v2 = Arc::new(StaticProvider { id: "v",
            entries: vec![
                make_entry("a", "v", TrustLevel::Community),
                make_entry("b", "v", TrustLevel::Community),
            ] });
        agg.reload(vec![(10, v2)]);
        assert_eq!(agg.list(&CatalogQuery::default()).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn parallel_list_does_not_serialize_slow_sources() {
        struct Slow { id: &'static str, ms: u64, counter: Arc<AtomicUsize> }
        #[async_trait]
        impl CatalogProvider for Slow {
            fn source_id(&self) -> &str { self.id }
            async fn list(&self, _q: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
                self.counter.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(self.ms)).await;
                Ok(vec![])
            }
            async fn get(&self, _: &str) -> CatalogResult<Option<ServerEntry>> { Ok(None) }
        }
        let counter = Arc::new(AtomicUsize::new(0));
        let a = Arc::new(Slow { id: "a", ms: 100, counter: counter.clone() });
        let b = Arc::new(Slow { id: "b", ms: 100, counter: counter.clone() });
        let agg = AggregateCatalogProvider::new_with_priority(vec![(10, a), (20, b)], None);
        let start = std::time::Instant::now();
        let _ = agg.list(&CatalogQuery::default()).await.unwrap();
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(180),
            "expected parallel ~100ms, got {elapsed:?}"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p agent-mcp catalog::aggregate::tests_phase2
```

Expected: 5 FAILED (compile error: `new_with_priority`, `reload`, `DomainEventSink` not yet on the struct).

- [ ] **Step 3: Replace `aggregate.rs` body with the extended impl**

```rust
//! Aggregates multiple [`CatalogProvider`]s into one logical view, with
//! per-source priority, parallel querying, failure isolation, and rate-limited
//! per-source failure events.

use crate::catalog::{
    CatalogProvider, CatalogQuery, CatalogResult, DomainEventSink, ServerEntry,
};
use async_trait::async_trait;
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone)]
struct PrioritisedProvider {
    priority: u32,
    inner: Arc<dyn CatalogProvider>,
}

pub struct AggregateCatalogProvider {
    inner: Mutex<Vec<PrioritisedProvider>>,
    event_sink: Option<Arc<dyn DomainEventSink>>,
    /// Last time we emitted `(source_id, error_signature)` → for 60s rate limit.
    failure_emit_log: Mutex<HashMap<(String, String), Instant>>,
}

const FAILURE_RATE_LIMIT: Duration = Duration::from_secs(60);

impl AggregateCatalogProvider {
    pub fn new(inner: Vec<Arc<dyn CatalogProvider>>) -> Self {
        // Backward compatibility: equal priority = preserve insertion order via index.
        let providers = inner
            .into_iter()
            .enumerate()
            .map(|(i, p)| PrioritisedProvider { priority: 100 + i as u32, inner: p })
            .collect();
        Self {
            inner: Mutex::new(providers),
            event_sink: None,
            failure_emit_log: Mutex::new(HashMap::new()),
        }
    }

    pub fn new_with_priority(
        providers: Vec<(u32, Arc<dyn CatalogProvider>)>,
        event_sink: Option<Arc<dyn DomainEventSink>>,
    ) -> Self {
        let mut inner: Vec<PrioritisedProvider> = providers
            .into_iter()
            .map(|(priority, inner)| PrioritisedProvider { priority, inner })
            .collect();
        inner.sort_by_key(|p| p.priority);
        Self {
            inner: Mutex::new(inner),
            event_sink,
            failure_emit_log: Mutex::new(HashMap::new()),
        }
    }

    pub fn reload(&mut self, providers: Vec<(u32, Arc<dyn CatalogProvider>)>) {
        let mut inner: Vec<PrioritisedProvider> = providers
            .into_iter()
            .map(|(priority, inner)| PrioritisedProvider { priority, inner })
            .collect();
        inner.sort_by_key(|p| p.priority);
        *self.inner.try_lock().expect("reload: inner not contended") = inner;
    }

    pub fn add(&self, provider: Arc<dyn CatalogProvider>) {
        let mut guard = self.inner.try_lock().expect("add: inner not contended");
        let next_priority = guard.iter().map(|p| p.priority).max().unwrap_or(99) + 1;
        guard.push(PrioritisedProvider { priority: next_priority, inner: provider });
        guard.sort_by_key(|p| p.priority);
    }

    async fn maybe_emit_failure(&self, source_id: &str, err: &str) {
        let key = (source_id.to_string(), err.to_string());
        let mut log = self.failure_emit_log.lock().await;
        let now = Instant::now();
        if let Some(prev) = log.get(&key) {
            if now.duration_since(*prev) < FAILURE_RATE_LIMIT {
                return;
            }
        }
        log.insert(key, now);
        drop(log);
        if let Some(sink) = &self.event_sink {
            sink.emit_source_failed(source_id, err).await;
        }
    }
}

#[async_trait]
impl CatalogProvider for AggregateCatalogProvider {
    fn source_id(&self) -> &str { "aggregate" }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        // Filter early by source if requested.
        let active: Vec<PrioritisedProvider> = providers
            .into_iter()
            .filter(|p| {
                query
                    .source
                    .as_ref()
                    .map(|src| p.inner.source_id() == src)
                    .unwrap_or(true)
            })
            .collect();

        // Issue all `list` calls in parallel.
        let futures = active.iter().map(|p| {
            let q = query.clone();
            async move {
                let id = p.inner.source_id().to_string();
                let result = p.inner.list(&q).await;
                (p.priority, id, result)
            }
        });
        let results = join_all(futures).await;

        // Stable merge: collect successes in priority order, emit failures via sink.
        let mut sorted = results;
        sorted.sort_by_key(|(prio, _, _)| *prio);

        let mut all: Vec<ServerEntry> = Vec::new();
        let mut seen: std::collections::HashSet<(String, String)> = Default::default();
        for (_, source_id, res) in sorted {
            match res {
                Ok(entries) => {
                    for entry in entries {
                        let key = (entry.source.clone(), entry.id.clone());
                        if seen.insert(key) {
                            all.push(entry);
                        }
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    tracing::warn!(source=%source_id, error=%err_str, "catalog source failed");
                    self.maybe_emit_failure(&source_id, &err_str).await;
                }
            }
        }

        all.sort_by(|a, b| {
            b.trust
                .cmp(&a.trust)
                .then_with(|| a.source.cmp(&b.source))
                .then_with(|| a.display_name.cmp(&b.display_name))
        });
        if let Some(limit) = query.limit {
            all.truncate(limit);
        }
        Ok(all)
    }

    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        for p in providers {
            match p.inner.get(id).await {
                Ok(Some(e)) => return Ok(Some(e)),
                Ok(None) => continue,
                Err(e) => {
                    self.maybe_emit_failure(p.inner.source_id(), &e.to_string()).await;
                    continue;
                }
            }
        }
        Ok(None)
    }

    async fn refresh(&self) -> CatalogResult<()> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        let futures = providers.iter().map(|p| async move {
            let id = p.inner.source_id().to_string();
            (id, p.inner.refresh().await)
        });
        let results = join_all(futures).await;
        for (source_id, res) in results {
            if let Err(e) = res {
                self.maybe_emit_failure(&source_id, &e.to_string()).await;
            }
        }
        Ok(())
    }
}
```

The original `tests` module from Phase 1 still exists; ensure both `tests` and `tests_phase2` modules compile. The pre-existing test that constructs `AggregateCatalogProvider::new(...)` continues to pass thanks to the backward-compatible constructor.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p agent-mcp catalog::aggregate
```

Expected: original Phase 1 tests + 5 new = all PASSED.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-mcp/
git commit -m "feat(mcp): extend AggregateCatalogProvider with priority, parallel listing, and failure isolation"
```

---

## Task 7: Wire `build_provider(cfg)` constructor + crate re-exports

**Files:**

- Modify: `crates/agent-mcp/src/catalog/remote/mod.rs` (add `build_provider`)
- Modify: `crates/agent-mcp/src/lib.rs` (re-export remote types when feature enabled)

- [ ] **Step 1: Write the failing test**

Append to `crates/agent-mcp/src/catalog/remote/mod.rs`:

```rust
pub use http_cache::HttpResponseCache;
pub use http_client::SharedHttpClient;
pub use kairox_json::KairoxJsonProvider;
pub use smithery::SmitheryProvider;

use crate::catalog::CatalogProvider;
use std::sync::Arc;

/// Constructs the right provider based on `cfg.kind`.
pub fn build_provider(
    cfg: RemoteSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
) -> Arc<dyn CatalogProvider> {
    match cfg.kind {
        RemoteSourceKind::KairoxJson => Arc::new(KairoxJsonProvider::new(cfg, http, cache)),
        RemoteSourceKind::Smithery => Arc::new(SmitheryProvider::new(cfg, http, cache)),
    }
}

#[cfg(test)]
mod build_tests {
    use super::*;
    use crate::catalog::TrustLevel;

    #[test]
    fn build_provider_returns_kairox_or_smithery_per_kind() {
        let http = SharedHttpClient::new().unwrap();
        let cache = Arc::new(HttpResponseCache::new(std::env::temp_dir().join("kairox-test-cache")));
        let kj = build_provider(
            RemoteSourceConfig {
                id: "k".into(), display_name: "k".into(), kind: RemoteSourceKind::KairoxJson,
                url: "https://x/c.json".into(), api_key_env: None, priority: 100,
                default_trust: TrustLevel::Community, enabled: true, cache_ttl_seconds: None,
            },
            http.clone(),
            cache.clone(),
        );
        assert_eq!(kj.source_id(), "k");
        let sm = build_provider(
            RemoteSourceConfig {
                id: "s".into(), display_name: "s".into(), kind: RemoteSourceKind::Smithery,
                url: "https://reg".into(), api_key_env: None, priority: 100,
                default_trust: TrustLevel::Community, enabled: true, cache_ttl_seconds: None,
            },
            http,
            cache,
        );
        assert_eq!(sm.source_id(), "s");
    }
}
```

In `crates/agent-mcp/src/lib.rs` add at the bottom:

```rust
#[cfg(feature = "remote-catalog")]
pub use catalog::remote::{
    build_provider as build_remote_catalog_provider, HttpResponseCache, KairoxJsonProvider,
    RemoteError, RemoteSourceConfig, RemoteSourceKind, SharedHttpClient, SmitheryProvider,
};
pub use catalog::DomainEventSink;
```

- [ ] **Step 2: Run tests + clippy**

```bash
cargo test -p agent-mcp --features remote-catalog catalog::remote::build_tests
cargo clippy -p agent-mcp --all-targets --features remote-catalog -- -D warnings
```

Expected: PASSED, zero clippy warnings.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/
git commit -m "feat(mcp): expose build_provider constructor and re-export remote-catalog types"
```

---

## Task 8: agent-config — parse `[[catalog_sources]]`

**Files:**

- Modify: `crates/agent-config/src/loader.rs`
- Modify: `crates/agent-config/src/lib.rs` (re-export)

- [ ] **Step 1: Inspect the existing loader to see where to splice**

```bash
grep -n "load_with_marketplace\|MarketplaceTomlInner\|mcp_servers" crates/agent-config/src/loader.rs | head -30
```

The Phase 2 design adds an optional `catalog_sources` array to the same marketplace toml file. The existing `load_with_marketplace` is the splice point.

- [ ] **Step 2: Write the failing tests**

Append at the bottom of `crates/agent-config/src/loader.rs` `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod catalog_sources_tests {
    use super::*;

    #[test]
    fn parses_catalog_sources_with_defaults() {
        let main_toml = "";
        let market_toml = r#"
[[catalog_sources]]
id           = "smithery"
display_name = "Smithery"
kind         = "smithery"
url          = "https://registry.smithery.ai"
"#;
        let loaded = load_with_marketplace_str(main_toml, Some(market_toml)).unwrap();
        assert_eq!(loaded.catalog_sources.len(), 1);
        let s = &loaded.catalog_sources[0];
        assert_eq!(s.id, "smithery");
        assert_eq!(s.priority, 100);
        assert!(s.enabled);
    }

    #[test]
    fn parses_multiple_sources_with_full_fields() {
        let market_toml = r#"
[[catalog_sources]]
id            = "internal"
display_name  = "Internal"
kind          = "kairox_json"
url           = "https://mcp.example.com/c.json"
api_key_env   = "INTERNAL_KEY"
priority      = 10
default_trust = "verified"
enabled       = true
cache_ttl_seconds = 600

[[catalog_sources]]
id           = "smithery"
display_name = "Smithery"
kind         = "smithery"
url          = "https://registry.smithery.ai"
priority     = 50
enabled      = false
"#;
        let loaded = load_with_marketplace_str("", Some(market_toml)).unwrap();
        assert_eq!(loaded.catalog_sources.len(), 2);
        let internal = loaded.catalog_sources.iter().find(|s| s.id == "internal").unwrap();
        assert_eq!(internal.priority, 10);
        assert_eq!(internal.api_key_env.as_deref(), Some("INTERNAL_KEY"));
        assert_eq!(internal.cache_ttl_seconds, Some(600));
        let smithery = loaded.catalog_sources.iter().find(|s| s.id == "smithery").unwrap();
        assert!(!smithery.enabled);
    }

    #[test]
    fn rejects_unknown_kind() {
        let market_toml = r#"
[[catalog_sources]]
id           = "x"
display_name = "X"
kind         = "wat"
url          = "https://x"
"#;
        let err = load_with_marketplace_str("", Some(market_toml)).unwrap_err();
        assert!(format!("{err:?}").to_lowercase().contains("kind"));
    }

    #[test]
    fn missing_marketplace_yields_empty_sources() {
        let loaded = load_with_marketplace_str("", None).unwrap();
        assert!(loaded.catalog_sources.is_empty());
    }

    #[test]
    fn marketplace_with_only_mcp_servers_yields_empty_sources() {
        let market_toml = r#"
[mcp_servers.foo]
transport = "stdio"
command   = "echo"
args      = []
"#;
        let loaded = load_with_marketplace_str("", Some(market_toml)).unwrap();
        assert_eq!(loaded.mcp_servers.len(), 1);
        assert!(loaded.catalog_sources.is_empty());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test -p agent-config catalog_sources_tests
```

Expected: 5 FAILED (compile error: `LoadedConfig.catalog_sources`, `load_with_marketplace_str` not exposed).

- [ ] **Step 4: Implement**

In `crates/agent-config/src/loader.rs`:

1. Extend the marketplace inner struct (the one with `mcp_servers` field) to also accept `catalog_sources`:

```rust
#[derive(Debug, serde::Deserialize, Default)]
struct MarketplaceTomlInner {
    #[serde(default)]
    mcp_servers: toml::value::Table,
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
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    cache_ttl_seconds: Option<u64>,
}
fn default_priority() -> u32 { 100 }
fn default_trust_str() -> String { "community".into() }
fn default_true() -> bool { true }
```

2. Add a small POD struct mirroring `agent_mcp::RemoteSourceConfig` field-for-field — `agent-config` must NOT depend on `agent-mcp` (cycle). Define it locally and let the runtime crate convert:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSourceConfig {
    pub id: String,
    pub display_name: String,
    pub kind: CatalogSourceKind,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: u32,
    pub default_trust: String, // "verified" | "community" | "unverified"
    pub enabled: bool,
    pub cache_ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSourceKind { KairoxJson, Smithery }
```

3. Extend `LoadedConfig`:

```rust
pub struct LoadedConfig {
    pub config: Config,                              // existing
    pub catalog_sources: Vec<CatalogSourceConfig>,   // new
}
```

4. Convert `RawCatalogSource → CatalogSourceConfig` and validate:

```rust
fn raw_to_source(raw: RawCatalogSource) -> Result<CatalogSourceConfig, ConfigError> {
    let kind = match raw.kind.as_str() {
        "kairox_json" => CatalogSourceKind::KairoxJson,
        "smithery"    => CatalogSourceKind::Smithery,
        other => return Err(ConfigError::Invalid(format!(
            "catalog_sources[{}]: unsupported kind '{other}'", raw.id
        ))),
    };
    // Light URL sanity check.
    if !raw.url.starts_with("http://") && !raw.url.starts_with("https://") {
        return Err(ConfigError::Invalid(format!(
            "catalog_sources[{}]: url must be http(s)://", raw.id
        )));
    }
    Ok(CatalogSourceConfig {
        id: raw.id,
        display_name: raw.display_name,
        kind,
        url: raw.url,
        api_key_env: raw.api_key_env,
        priority: raw.priority,
        default_trust: raw.default_trust,
        enabled: raw.enabled,
        cache_ttl_seconds: raw.cache_ttl_seconds,
    })
}
```

5. Expose `pub fn load_with_marketplace_str(main: &str, market: Option<&str>) -> Result<LoadedConfig, ConfigError>` that mirrors the existing file-based variant but takes strings (used by the new tests). The file-based variant calls into it.

6. In `lib.rs` re-export `pub use loader::{CatalogSourceConfig, CatalogSourceKind, LoadedConfig};`.

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p agent-config catalog_sources_tests
cargo test -p agent-config       # ensure existing tests still pass
```

Expected: PASSED + 5 new + all existing.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-config/
git commit -m "feat(config): parse [[catalog_sources]] from marketplace toml"
```

---

## Task 9: agent-core — add 2 new EventPayload variants

**Files:**

- Modify: `crates/agent-core/src/events.rs`

- [ ] **Step 1: Inspect the existing `EventPayload` and `event_type()`**

```bash
grep -n "CatalogEntryInstalled\|CatalogRefreshed\|event_type" crates/agent-core/src/events.rs | head -30
```

Phase 1 already has `CatalogRefreshed`, `CatalogEntryInstalling`, `CatalogEntryInstalled`, `CatalogEntryUninstalled`, `CatalogRuntimeMissing`. Phase 2 adds **two more**.

- [ ] **Step 2: Write the failing test**

In `crates/agent-core/src/events.rs` `#[cfg(test)] mod tests` (or its companion file), add:

```rust
#[test]
fn catalog_source_added_event_round_trips() {
    let p = EventPayload::CatalogSourceAdded { source: "smithery".into() };
    assert_eq!(p.event_type(), "catalog.source_added");
    let s = serde_json::to_string(&p).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, EventPayload::CatalogSourceAdded { ref source } if source == "smithery"));
}

#[test]
fn catalog_source_failed_event_round_trips() {
    let p = EventPayload::CatalogSourceFailed {
        source: "smithery".into(),
        error:  "timeout".into(),
    };
    assert_eq!(p.event_type(), "catalog.source_failed");
    let s = serde_json::to_string(&p).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, EventPayload::CatalogSourceFailed { ref source, ref error }
        if source == "smithery" && error == "timeout"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test -p agent-core events::tests::catalog_source
```

Expected: 2 FAILED (variants don't exist).

- [ ] **Step 4: Add the variants**

In `crates/agent-core/src/events.rs`, add to `EventPayload`:

```rust
CatalogSourceAdded {
    source: String,
},
CatalogSourceFailed {
    source: String,
    error: String,
},
```

In `event_type()`:

```rust
EventPayload::CatalogSourceAdded { .. }  => "catalog.source_added",
EventPayload::CatalogSourceFailed { .. } => "catalog.source_failed",
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p agent-core events
```

Expected: PASSED + all existing.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core/
git commit -m "feat(core): add CatalogSourceAdded and CatalogSourceFailed event payloads"
```

---

## Task 10: agent-core — add 4 new AppFacade methods

**Files:**

- Modify: `crates/agent-core/src/facade.rs`

- [ ] **Step 1: Add the trait methods**

In the `AppFacade` trait, add (alphabetised next to existing `*_catalog_*` methods):

```rust
async fn list_catalog_sources(&self) -> Result<Vec<CatalogSourceView>>;
async fn add_catalog_source(&self, cfg: CatalogSourceView) -> Result<()>;
async fn update_catalog_source(&self, cfg: CatalogSourceView) -> Result<()>;
async fn remove_catalog_source(&self, source_id: String) -> Result<()>;
```

`agent-core` cannot depend on `agent-mcp` directly (it would invert the layer order). Define a thin `CatalogSourceView` struct in `agent-core` that mirrors `RemoteSourceConfig` 1:1 — both are simple POD types and the runtime layer translates between them. `CatalogSourceView` gets specta + serde derives (consumed by GUI).

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CatalogSourceView {
    pub id: String,
    pub display_name: String,
    pub kind: String,             // "kairox_json" | "smithery"
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: u32,
    pub default_trust: String,    // "verified" | "community" | "unverified"
    pub enabled: bool,
    pub cache_ttl_seconds: Option<u64>,
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo build -p agent-core
```

The trait body has new abstract methods; existing impls of `AppFacade` (in `agent-runtime`) won't compile yet — that is by design and will be fixed in T11.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-core/
git commit -m "feat(core): add catalog source AppFacade methods and CatalogSourceView type"
```

---

## Task 11: agent-runtime — wire it all together

**Files:**

- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/src/mcp_manager.rs` (or wherever `AggregateCatalogProvider` is stored — confirm during T11.S1)
- Modify: `crates/agent-runtime/Cargo.toml` (turn on `agent-mcp/remote-catalog` feature)

- [ ] **Step 1: Locate where `BuiltinCatalogProvider` is constructed today**

```bash
grep -rn "BuiltinCatalogProvider\|AggregateCatalogProvider\|InstallerCommandImpl" crates/agent-runtime/src | head -30
```

This locates the exact splice point for replacing `Builtin → Aggregate(Builtin + remotes)`.

- [ ] **Step 2: Implement `DomainEventSink` over the runtime's broadcaster**

In a new file `crates/agent-runtime/src/catalog_sink.rs`:

```rust
use agent_core::events::{DomainEvent, EventPayload};
use agent_core::ids::SessionId;
use agent_mcp::DomainEventSink;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use chrono::Utc;

pub(crate) struct CatalogEventSink {
    tx: Sender<DomainEvent>,
}

impl CatalogEventSink {
    pub fn new(tx: Sender<DomainEvent>) -> Arc<Self> { Arc::new(Self { tx }) }

    fn build(payload: EventPayload) -> DomainEvent {
        DomainEvent {
            session_id: SessionId::system(),  // or a sentinel "marketplace" id
            occurred_at: Utc::now(),
            payload,
        }
    }
}

#[async_trait]
impl DomainEventSink for CatalogEventSink {
    async fn emit_source_failed(&self, source_id: &str, error: &str) {
        let _ = self.tx.send(Self::build(EventPayload::CatalogSourceFailed {
            source: source_id.to_string(),
            error: error.to_string(),
        }));
    }
    async fn emit_source_added(&self, source_id: &str) {
        let _ = self.tx.send(Self::build(EventPayload::CatalogSourceAdded {
            source: source_id.to_string(),
        }));
    }
}
```

If `SessionId::system()` does not exist, use `SessionId::nil()` or define a marketplace-scoped sentinel — pick whatever the existing Phase 1 marketplace events used (`grep -n "CatalogEntryInstalled" crates/agent-runtime/src` to find precedent).

Wire `pub mod catalog_sink;` in `crates/agent-runtime/src/lib.rs` (`pub(crate)` is also fine).

- [ ] **Step 3: Build the aggregate provider at startup**

In `facade_runtime.rs`, replace the construction site found in S1:

```rust
use agent_mcp::{
    build_remote_catalog_provider, AggregateCatalogProvider, BuiltinCatalogProvider,
    HttpResponseCache, RemoteSourceConfig, RemoteSourceKind, SharedHttpClient, TrustLevel,
};
use crate::catalog_sink::CatalogEventSink;
use std::sync::Arc;

fn build_catalog_provider(
    sources: &[agent_config::CatalogSourceConfig],
    cache_dir: std::path::PathBuf,
    event_tx: tokio::sync::broadcast::Sender<agent_core::DomainEvent>,
) -> Result<AggregateCatalogProvider, anyhow::Error> {
    let http = SharedHttpClient::new()?;
    let cache = Arc::new(HttpResponseCache::new(cache_dir));
    let mut providers: Vec<(u32, Arc<dyn agent_mcp::CatalogProvider>)> = Vec::new();
    // Builtin always first at priority 0.
    providers.push((0, Arc::new(BuiltinCatalogProvider::new()?)));
    for s in sources.iter().filter(|s| s.enabled) {
        let cfg = RemoteSourceConfig {
            id: s.id.clone(),
            display_name: s.display_name.clone(),
            kind: match s.kind {
                agent_config::CatalogSourceKind::KairoxJson => RemoteSourceKind::KairoxJson,
                agent_config::CatalogSourceKind::Smithery   => RemoteSourceKind::Smithery,
            },
            url: s.url.clone(),
            api_key_env: s.api_key_env.clone(),
            priority: s.priority,
            default_trust: parse_trust(&s.default_trust),
            enabled: true,
            cache_ttl_seconds: s.cache_ttl_seconds,
        };
        providers.push((s.priority, build_remote_catalog_provider(cfg, http.clone(), cache.clone())));
    }
    let sink = CatalogEventSink::new(event_tx);
    Ok(AggregateCatalogProvider::new_with_priority(
        providers,
        Some(sink as Arc<dyn agent_mcp::DomainEventSink>),
    ))
}

fn parse_trust(s: &str) -> TrustLevel {
    match s {
        "verified" => TrustLevel::Verified,
        "unverified" => TrustLevel::Unverified,
        _ => TrustLevel::Community,
    }
}
```

Replace the existing `BuiltinCatalogProvider::new()` call in `LocalRuntime::new`/builder with `build_catalog_provider(...)` and store the resulting `Arc<AggregateCatalogProvider>` in the runtime state. Keep the field type as `Arc<dyn CatalogProvider>` for testability OR keep it concrete to allow `reload`. Recommended: store **both** — `Arc<dyn CatalogProvider>` for queries and a `Arc<Mutex<AggregateCatalogProvider>>` for `reload()`.

- [ ] **Step 4: Implement the 4 new facade methods**

In `facade_runtime.rs`, on the `LocalRuntime` `impl AppFacade` block:

```rust
async fn list_catalog_sources(&self) -> Result<Vec<CatalogSourceView>> {
    Ok(self
        .marketplace_toml
        .read_sources()
        .await?
        .into_iter()
        .map(to_view)
        .collect())
}

async fn add_catalog_source(&self, cfg: CatalogSourceView) -> Result<()> {
    self.marketplace_toml.add_source(from_view(cfg.clone())?).await?;
    self.rebuild_aggregate_from_disk().await?;
    self.event_sink.emit_source_added(&cfg.id).await;
    Ok(())
}

async fn update_catalog_source(&self, cfg: CatalogSourceView) -> Result<()> {
    self.marketplace_toml.update_source(from_view(cfg)?).await?;
    self.rebuild_aggregate_from_disk().await
}

async fn remove_catalog_source(&self, source_id: String) -> Result<()> {
    self.marketplace_toml.remove_source(&source_id).await?;
    self.rebuild_aggregate_from_disk().await
}
```

`marketplace_toml` is a small helper (already exists in Phase 1 as part of the installer's atomic-write logic; if not, add it as a sibling module) that:

- Reads `~/.kairox/mcp_servers.toml`
- Parses with `agent_config::parse_catalog_sources`
- Writes back via temp-file + rename, preserving any `[mcp_servers.*]` tables verbatim (use `toml_edit` if it isn't already a dep — Phase 1 already manipulates the same file, so reuse that codepath).

`rebuild_aggregate_from_disk` reloads from `~/.kairox/mcp_servers.toml` and calls `AggregateCatalogProvider::reload(...)`.

- [ ] **Step 5: Add a unit test for the runtime wiring**

```rust
// crates/agent-runtime/tests/marketplace_phase2_unit.rs
// Smoke test that LocalRuntime exposes the new facade methods and a fresh
// marketplace toml round-trips through add → list → remove.

use agent_core::AppFacade;
use agent_core::CatalogSourceView;

#[tokio::test]
async fn add_list_remove_catalog_source_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = crate::test_support::runtime_with_marketplace_dir(tmp.path()).await;

    assert!(runtime.list_catalog_sources().await.unwrap().is_empty());

    let cfg = CatalogSourceView {
        id: "smithery".into(),
        display_name: "Smithery".into(),
        kind: "smithery".into(),
        url: "https://registry.smithery.ai".into(),
        api_key_env: None,
        priority: 50,
        default_trust: "community".into(),
        enabled: true,
        cache_ttl_seconds: None,
    };
    runtime.add_catalog_source(cfg.clone()).await.unwrap();
    let listed = runtime.list_catalog_sources().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "smithery");

    runtime.remove_catalog_source("smithery".into()).await.unwrap();
    assert!(runtime.list_catalog_sources().await.unwrap().is_empty());
}
```

If a `test_support` helper for "runtime with custom marketplace dir" doesn't exist yet, add the smallest one needed (likely `crates/agent-runtime/tests/common/mod.rs`). Reuse Phase 1 testing helpers wherever possible.

- [ ] **Step 6: Run tests**

```bash
cargo test -p agent-runtime marketplace_phase2_unit
cargo clippy -p agent-runtime --all-targets -- -D warnings
```

- [ ] **Step 7: Commit**

```bash
git add crates/agent-runtime/ crates/agent-core/
git commit -m "feat(runtime): wire AggregateCatalogProvider with remote sources, atomic toml mutations, and event sink"
```

---

## Task 12: integration test — end-to-end remote catalog with two wiremock servers

**Files:**

- Create: `crates/agent-runtime/tests/marketplace_remote.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! End-to-end Phase 2 test: build a LocalRuntime configured with two remote
//! catalog sources (one Kairox JSON, one Smithery), verify list aggregation,
//! install one entry from each, observe failure events when one source dies.

use agent_core::AppFacade;
use agent_core::CatalogSourceView;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod common;

#[tokio::test]
async fn list_catalog_aggregates_builtin_and_two_remote_sources() {
    let kairox_server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/c.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(kairox_doc()))
        .mount(&kairox_server).await;

    let smithery_server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/servers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(smithery_doc()))
        .mount(&smithery_server).await;

    let tmp = tempfile::tempdir().unwrap();
    let runtime = common::runtime_with_marketplace_dir(tmp.path()).await;

    runtime.add_catalog_source(CatalogSourceView {
        id: "internal".into(), display_name: "Internal".into(),
        kind: "kairox_json".into(),
        url: format!("{}/c.json", kairox_server.uri()),
        api_key_env: None, priority: 10,
        default_trust: "verified".into(), enabled: true, cache_ttl_seconds: None,
    }).await.unwrap();
    runtime.add_catalog_source(CatalogSourceView {
        id: "smithery".into(), display_name: "Smithery".into(),
        kind: "smithery".into(), url: smithery_server.uri(),
        api_key_env: None, priority: 50,
        default_trust: "community".into(), enabled: true, cache_ttl_seconds: None,
    }).await.unwrap();

    let entries = runtime
        .list_catalog(agent_mcp::CatalogQuery::default())
        .await
        .unwrap();

    // Builtin (~24) + 1 internal + 1 smithery
    assert!(entries.len() >= 26, "got {}", entries.len());
    assert!(entries.iter().any(|e| e.source == "internal"));
    assert!(entries.iter().any(|e| e.source == "smithery"));
    assert!(entries.iter().any(|e| e.source == "builtin"));
}

#[tokio::test]
async fn failed_source_does_not_break_list_and_emits_event() {
    let dead_server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/c.json"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&dead_server).await;

    let tmp = tempfile::tempdir().unwrap();
    let runtime = common::runtime_with_marketplace_dir(tmp.path()).await;
    let mut events = runtime.subscribe_all();

    runtime.add_catalog_source(CatalogSourceView {
        id: "broken".into(), display_name: "Broken".into(),
        kind: "kairox_json".into(),
        url: format!("{}/c.json", dead_server.uri()),
        api_key_env: None, priority: 10, default_trust: "community".into(),
        enabled: true, cache_ttl_seconds: None,
    }).await.unwrap();

    let entries = runtime
        .list_catalog(agent_mcp::CatalogQuery::default())
        .await
        .unwrap();
    // Builtin still works; "broken" returns nothing.
    assert!(entries.iter().all(|e| e.source != "broken"));

    // Drain up to 3 events; one of them should be CatalogSourceFailed.
    let mut saw = false;
    for _ in 0..5 {
        match tokio::time::timeout(std::time::Duration::from_millis(200), events.recv()).await {
            Ok(Ok(ev)) => {
                if matches!(ev.payload, agent_core::EventPayload::CatalogSourceFailed { ref source, .. }
                    if source == "broken") { saw = true; break; }
            }
            _ => break,
        }
    }
    assert!(saw, "expected CatalogSourceFailed event for 'broken'");
}

fn kairox_doc() -> String {
    r#"{
      "schema_version": "1",
      "entries": [{
        "id": "k1",
        "source": "ignored",
        "display_name": "K1",
        "summary": "kairox sample",
        "description": "",
        "categories": ["dev-tools"],
        "tags": [],
        "install": {"transport":"stdio","command":"echo","args":[],"env":{}},
        "requirements": [],
        "trust": "verified",
        "default_env": []
      }]
    }"#.into()
}

fn smithery_doc() -> String {
    r#"{"servers":[{
      "qualifiedName":"@a/b","displayName":"Ab","description":"Hello.",
      "tags":[],"verified":true,
      "connection":{"type":"stdio","command":"echo","args":[],"env":{}}
    }]}"#.into()
}
```

If `subscribe_all` is private, expose a `pub fn subscribe_all` on `LocalRuntime` for tests (or use the existing public AppFacade subscription).

- [ ] **Step 2: Run the test**

```bash
just test-mcp           # runs MCP integration suites
# or directly:
cargo test -p agent-runtime --test marketplace_remote
```

Expected: 2 PASSED.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime/
git commit -m "test(runtime): integration test for remote catalog aggregation and failure handling"
```

---

## Task 13: Tauri commands + tauri-mock.js updates

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs` (registration in `generate_handler!`)
- Modify: `apps/agent-gui/src-tauri/src/specta.rs` (registration in `collect_commands!`)
- Modify: `apps/agent-gui/e2e/tauri-mock.js`

- [ ] **Step 1: Add the 4 Tauri commands**

In `apps/agent-gui/src-tauri/src/commands.rs`, near the existing Phase 1 catalog commands:

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_catalog_sources(
    state: tauri::State<'_, GuiState>,
) -> Result<Vec<CatalogSourceView>, String> {
    state.runtime.list_catalog_sources().await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn add_catalog_source(
    state: tauri::State<'_, GuiState>,
    cfg: CatalogSourceView,
) -> Result<(), String> {
    state.runtime.add_catalog_source(cfg).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_catalog_source(
    state: tauri::State<'_, GuiState>,
    cfg: CatalogSourceView,
) -> Result<(), String> {
    state.runtime.update_catalog_source(cfg).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_catalog_source(
    state: tauri::State<'_, GuiState>,
    source_id: String,
) -> Result<(), String> {
    state.runtime.remove_catalog_source(source_id).await.map_err(|e| e.to_string())
}
```

Add `use agent_core::CatalogSourceView;` at the top.

- [ ] **Step 2: Register in BOTH `generate_handler!` and `collect_commands!`**

`apps/agent-gui/src-tauri/src/lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    /* …existing… */,
    commands::list_catalog_sources,
    commands::add_catalog_source,
    commands::update_catalog_source,
    commands::remove_catalog_source,
])
```

`apps/agent-gui/src-tauri/src/specta.rs`:

```rust
collect_commands![
    /* …existing… */,
    commands::list_catalog_sources,
    commands::add_catalog_source,
    commands::update_catalog_source,
    commands::remove_catalog_source,
]
```

Per AGENTS.md: forgetting either causes runtime or type-gen failures.

- [ ] **Step 3: Verify the Rust side compiles**

```bash
cargo build -p agent-gui-tauri
```

Expected: clean build.

- [ ] **Step 4: Update `tauri-mock.js`**

Append handlers (the file is JavaScript, not TS, and already has Phase 1 catalog commands). Add a module-level `__catalogSources = []` array:

```js
// in apps/agent-gui/e2e/tauri-mock.js inside the dispatcher
case "list_catalog_sources":
    return [...window.__catalogSources];
case "add_catalog_source":
    window.__catalogSources.push(args.cfg);
    window.__emit("kairox:domain_event", {
        payload: { type: "catalog.source_added", source: args.cfg.id },
    });
    return null;
case "update_catalog_source":
    window.__catalogSources = window.__catalogSources.map((s) =>
        s.id === args.cfg.id ? args.cfg : s,
    );
    return null;
case "remove_catalog_source":
    window.__catalogSources = window.__catalogSources.filter(
        (s) => s.id !== args.source_id,
    );
    return null;
```

Use the same emission helper the Phase 1 mock already uses for `catalog.entry_installed` events.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src-tauri/ apps/agent-gui/e2e/tauri-mock.js
git commit -m "feat(gui): add catalog source Tauri commands and mock handlers"
```

---

## Task 14: regenerate TypeScript bindings

**Files:**

- Generated: `apps/agent-gui/src/generated/commands.ts`
- Generated: `apps/agent-gui/src/generated/events.ts`

- [ ] **Step 1: Regenerate**

```bash
just gen-types
```

This produces TS for the 4 new commands and the 2 new events.

- [ ] **Step 2: Sanity check**

```bash
grep -E "list_catalog_sources|add_catalog_source|update_catalog_source|remove_catalog_source" apps/agent-gui/src/generated/commands.ts | wc -l
grep -E "catalog\.source_added|catalog\.source_failed" apps/agent-gui/src/generated/events.ts | wc -l
```

Both should print at least `4` and `2` respectively.

- [ ] **Step 3: Run `check-types` to confirm sync**

```bash
just check-types
```

Expected: no diff after re-running gen-types (the CI gate this enforces).

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/generated/
git commit -m "chore(gui): regenerate TypeScript bindings for catalog source commands and events"
```

---

## Task 15: Pinia catalog store extension

**Files:**

- Modify: `apps/agent-gui/src/stores/catalog.ts` (existing Phase 1 store)
- Test: `apps/agent-gui/src/stores/__tests__/catalog.test.ts` (extend or create)

- [ ] **Step 1: Write the failing tests**

Append:

```ts
// apps/agent-gui/src/stores/__tests__/catalog.test.ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useCatalogStore } from "../catalog";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));
import { invoke } from "@tauri-apps/api/core";

const sampleSource = {
  id: "smithery",
  display_name: "Smithery",
  kind: "smithery",
  url: "https://registry.smithery.ai",
  api_key_env: null,
  priority: 50,
  default_trust: "community",
  enabled: true,
  cache_ttl_seconds: null
};

describe("catalog store — sources", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("loads sources via list_catalog_sources", async () => {
    (invoke as any).mockResolvedValueOnce([sampleSource]);
    const store = useCatalogStore();
    await store.loadSources();
    expect(invoke).toHaveBeenCalledWith("list_catalog_sources");
    expect(store.sources).toHaveLength(1);
    expect(store.sources[0].id).toBe("smithery");
  });

  it("adds a source and re-loads", async () => {
    (invoke as any)
      .mockResolvedValueOnce(undefined) // add
      .mockResolvedValueOnce([sampleSource]); // refresh list
    const store = useCatalogStore();
    await store.addSource(sampleSource);
    expect(invoke).toHaveBeenNthCalledWith(1, "add_catalog_source", {
      cfg: sampleSource
    });
    expect(store.sources).toHaveLength(1);
  });

  it("removes a source and re-loads", async () => {
    (invoke as any).mockResolvedValueOnce(undefined).mockResolvedValueOnce([]);
    const store = useCatalogStore();
    store.sources = [sampleSource];
    await store.removeSource("smithery");
    expect(invoke).toHaveBeenNthCalledWith(1, "remove_catalog_source", {
      sourceId: "smithery"
    });
    expect(store.sources).toHaveLength(0);
  });

  it("records sourceFailures from CatalogSourceFailed events", () => {
    const store = useCatalogStore();
    store.handleSourceFailed({ source: "smithery", error: "timeout" });
    expect(store.sourceFailures.smithery).toBe("timeout");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd apps/agent-gui && pnpm vitest --run src/stores/__tests__/catalog.test.ts
```

Expected: FAILED on missing `loadSources/addSource/removeSource/handleSourceFailed/sourceFailures/sources`.

- [ ] **Step 3: Extend the store**

In `apps/agent-gui/src/stores/catalog.ts`, add to the existing store:

```ts
import type { CatalogSourceView } from "@/generated/commands";

// inside defineStore("catalog", () => { ... })
const sources = ref<CatalogSourceView[]>([]);
const sourceFailures = reactive<Record<string, string>>({});

async function loadSources() {
    sources.value = await invoke<CatalogSourceView[]>("list_catalog_sources");
}

async function addSource(cfg: CatalogSourceView) {
    await invoke("add_catalog_source", { cfg });
    await loadSources();
}

async function updateSource(cfg: CatalogSourceView) {
    await invoke("update_catalog_source", { cfg });
    await loadSources();
}

async function removeSource(sourceId: string) {
    await invoke("remove_catalog_source", { sourceId });
    await loadSources();
}

function handleSourceFailed(p: { source: string; error: string }) {
    sourceFailures[p.source] = p.error;
}

return {
    /* …existing exports… */,
    sources, sourceFailures,
    loadSources, addSource, updateSource, removeSource, handleSourceFailed,
};
```

In `apps/agent-gui/src/composables/useTauriEvents.ts` add an event branch routing `"catalog.source_failed"` and `"catalog.source_added"` to `catalogStore.handleSourceFailed` / `catalogStore.loadSources`.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd apps/agent-gui && pnpm vitest --run src/stores/__tests__/catalog.test.ts
```

Expected: 4 PASSED.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/stores apps/agent-gui/src/composables
git commit -m "feat(gui): extend catalog Pinia store with source CRUD and failure tracking"
```

---

## Task 16: CatalogSourcesSettings.vue (TDD with Vitest)

**Files:**

- Create: `apps/agent-gui/src/components/CatalogSourcesSettings.vue`
- Create: `apps/agent-gui/src/components/__tests__/CatalogSourcesSettings.test.ts`

- [ ] **Step 1: Write the failing component test**

```ts
// apps/agent-gui/src/components/__tests__/CatalogSourcesSettings.test.ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import CatalogSourcesSettings from "../CatalogSourcesSettings.vue";
import { useCatalogStore } from "@/stores/catalog";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import { invoke } from "@tauri-apps/api/core";

describe("CatalogSourcesSettings.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("renders empty state when no sources configured", async () => {
    (invoke as any).mockResolvedValueOnce([]);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    expect(wrapper.text()).toContain("No remote catalog sources");
  });

  it("renders configured sources", async () => {
    (invoke as any).mockResolvedValueOnce([
      {
        id: "smithery",
        display_name: "Smithery",
        kind: "smithery",
        url: "https://registry.smithery.ai",
        api_key_env: null,
        priority: 50,
        default_trust: "community",
        enabled: true,
        cache_ttl_seconds: null
      }
    ]);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    expect(wrapper.text()).toContain("Smithery");
    expect(wrapper.text()).toContain("registry.smithery.ai");
  });

  it("validates url before calling addSource", async () => {
    (invoke as any).mockResolvedValueOnce([]); // initial load
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    await wrapper.find('[data-test="add-source-toggle"]').trigger("click");
    await wrapper.find('[data-test="src-id"]').setValue("x");
    await wrapper.find('[data-test="src-name"]').setValue("X");
    await wrapper.find('[data-test="src-url"]').setValue("not-a-url");
    await wrapper.find('[data-test="src-save"]').trigger("click");
    expect(wrapper.text()).toContain("URL must start with http");
    expect(invoke).not.toHaveBeenCalledWith(
      "add_catalog_source",
      expect.any(Object)
    );
  });

  it("calls addSource with the form payload on save", async () => {
    (invoke as any)
      .mockResolvedValueOnce([]) // initial load
      .mockResolvedValueOnce(undefined) // add
      .mockResolvedValueOnce([]); // reload
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    await wrapper.find('[data-test="add-source-toggle"]').trigger("click");
    await wrapper.find('[data-test="src-id"]').setValue("x");
    await wrapper.find('[data-test="src-name"]').setValue("X");
    await wrapper.find('[data-test="src-url"]').setValue("https://x/c.json");
    await wrapper.find('[data-test="src-save"]').trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith(
      "add_catalog_source",
      expect.objectContaining({
        cfg: expect.objectContaining({ id: "x", url: "https://x/c.json" })
      })
    );
  });

  it("emits remove confirmation flow", async () => {
    (invoke as any).mockResolvedValueOnce([
      {
        id: "x",
        display_name: "X",
        kind: "kairox_json",
        url: "https://x/c.json",
        api_key_env: null,
        priority: 100,
        default_trust: "community",
        enabled: true,
        cache_ttl_seconds: null
      }
    ]);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    await wrapper.find('[data-test="src-remove-x"]').trigger("click");
    // confirm dialog renders
    expect(wrapper.text()).toContain("Remove source");
    (invoke as any).mockResolvedValueOnce(undefined).mockResolvedValueOnce([]);
    await wrapper.find('[data-test="src-confirm-remove"]').trigger("click");
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("remove_catalog_source", {
      sourceId: "x"
    });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd apps/agent-gui && pnpm vitest --run src/components/__tests__/CatalogSourcesSettings.test.ts
```

Expected: FAILED (component doesn't exist).

- [ ] **Step 3: Implement the component**

```vue
<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useCatalogStore } from "@/stores/catalog";
import type { CatalogSourceView } from "@/generated/commands";

const store = useCatalogStore();

const showAddForm = ref(false);
const formError = ref<string | null>(null);
const draft = ref<CatalogSourceView>({
  id: "",
  display_name: "",
  kind: "kairox_json",
  url: "",
  api_key_env: null,
  priority: 100,
  default_trust: "community",
  enabled: true,
  cache_ttl_seconds: null
});

const removeTarget = ref<string | null>(null);

onMounted(() => store.loadSources());

function isValidUrl(u: string): boolean {
  return u.startsWith("http://") || u.startsWith("https://");
}

async function save() {
  formError.value = null;
  if (!isValidUrl(draft.value.url)) {
    formError.value = "URL must start with http:// or https://";
    return;
  }
  if (!draft.value.id || !draft.value.display_name) {
    formError.value = "id and display_name are required";
    return;
  }
  await store.addSource({ ...draft.value });
  showAddForm.value = false;
  draft.value = { ...draft.value, id: "", display_name: "", url: "" };
}

function startRemove(id: string) {
  removeTarget.value = id;
}
async function confirmRemove() {
  if (removeTarget.value) {
    await store.removeSource(removeTarget.value);
    removeTarget.value = null;
  }
}
function cancelRemove() {
  removeTarget.value = null;
}
</script>

<template>
  <div class="catalog-sources-settings">
    <h3>Remote Catalog Sources</h3>

    <p v-if="store.sources.length === 0" class="empty">
      No remote catalog sources configured.
    </p>

    <ul v-else class="src-list">
      <li v-for="src in store.sources" :key="src.id" class="src-row">
        <div class="src-meta">
          <strong>{{ src.display_name }}</strong>
          <code>{{ src.id }}</code>
          <span class="src-kind">{{ src.kind }}</span>
          <a :href="src.url" target="_blank" rel="noopener">{{ src.url }}</a>
        </div>
        <button
          :data-test="`src-remove-${src.id}`"
          @click="startRemove(src.id)"
        >
          Remove
        </button>
      </li>
    </ul>

    <button
      v-if="!showAddForm"
      data-test="add-source-toggle"
      @click="showAddForm = true"
    >
      + Add source
    </button>
    <form v-else class="add-form" @submit.prevent="save">
      <label>id <input data-test="src-id" v-model="draft.id" /></label>
      <label
        >display name <input data-test="src-name" v-model="draft.display_name"
      /></label>
      <label
        >kind
        <select v-model="draft.kind">
          <option value="kairox_json">Kairox JSON</option>
          <option value="smithery">Smithery</option>
        </select>
      </label>
      <label>url <input data-test="src-url" v-model="draft.url" /></label>
      <label>api_key_env <input v-model="draft.api_key_env" /></label>
      <p v-if="formError" class="error">{{ formError }}</p>
      <button data-test="src-save" type="submit">Save</button>
      <button type="button" @click="showAddForm = false">Cancel</button>
    </form>

    <div v-if="removeTarget" class="confirm">
      <p>
        Remove source <code>{{ removeTarget }}</code
        >?
      </p>
      <button data-test="src-confirm-remove" @click="confirmRemove">
        Yes, remove
      </button>
      <button @click="cancelRemove">Cancel</button>
    </div>
  </div>
</template>

<style scoped>
.empty {
  color: var(--muted, #888);
}
.src-list {
  list-style: none;
  padding: 0;
}
.src-row {
  display: flex;
  justify-content: space-between;
  padding: 8px;
  border-bottom: 1px solid #eee;
}
.src-meta {
  display: flex;
  gap: 8px;
  align-items: center;
}
.src-kind {
  font-size: 0.8em;
  padding: 2px 6px;
  border: 1px solid #ccc;
  border-radius: 3px;
}
.add-form {
  display: grid;
  gap: 6px;
  padding: 12px;
  background: #f7f7f7;
}
.error {
  color: #c00;
}
.confirm {
  padding: 12px;
  background: #fffbe6;
  border: 1px solid #f0d000;
}
</style>
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd apps/agent-gui && pnpm vitest --run src/components/__tests__/CatalogSourcesSettings.test.ts
```

Expected: 5 PASSED.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/components/
git commit -m "feat(gui): add CatalogSourcesSettings component for managing remote catalog sources"
```

---

## Task 17: Marketplace.vue — multi-source chip filter + ⚠ badge

**Files:**

- Modify: `apps/agent-gui/src/components/Marketplace.vue`
- Modify: `apps/agent-gui/src/components/__tests__/Marketplace.test.ts` (extend)

- [ ] **Step 1: Write failing tests for the new behavior**

Add to existing test file:

```ts
it("renders a chip per configured source plus a built-in chip", async () => {
  (invoke as any)
    .mockResolvedValueOnce([]) // list_catalog
    .mockResolvedValueOnce([
      // list_catalog_sources
      {
        id: "smithery",
        display_name: "Smithery",
        kind: "smithery",
        url: "https://x",
        api_key_env: null,
        priority: 50,
        default_trust: "community",
        enabled: true,
        cache_ttl_seconds: null
      }
    ]);
  const wrapper = mount(Marketplace);
  await flushPromises();
  const chips = wrapper.findAll('[data-test="source-chip"]');
  // Built-in + 1 remote = 2 chips
  expect(chips).toHaveLength(2);
  expect(wrapper.text()).toContain("Built-in");
  expect(wrapper.text()).toContain("Smithery");
});

it("shows ⚠ badge on chip when CatalogSourceFailed observed", async () => {
  (invoke as any)
    .mockResolvedValueOnce([])
    .mockResolvedValueOnce([
      {
        id: "smithery",
        display_name: "Smithery",
        kind: "smithery",
        url: "https://x",
        api_key_env: null,
        priority: 50,
        default_trust: "community",
        enabled: true,
        cache_ttl_seconds: null
      }
    ]);
  const wrapper = mount(Marketplace);
  await flushPromises();
  const store = useCatalogStore();
  store.handleSourceFailed({ source: "smithery", error: "timeout" });
  await wrapper.vm.$nextTick();
  expect(wrapper.find('[data-test="src-warn-smithery"]').exists()).toBe(true);
});

it("multi-select chip semantics: deselecting builtin filters its entries out", async () => {
  (invoke as any)
    .mockResolvedValueOnce([
      {
        id: "a",
        source: "builtin",
        display_name: "A",
        summary: "",
        description: "",
        categories: [],
        tags: [],
        install: { transport: "stdio", command: "x", args: [], env: {} },
        requirements: [],
        trust: "community",
        default_env: []
      },
      {
        id: "b",
        source: "smithery",
        display_name: "B",
        summary: "",
        description: "",
        categories: [],
        tags: [],
        install: { transport: "stdio", command: "x", args: [], env: {} },
        requirements: [],
        trust: "community",
        default_env: []
      }
    ])
    .mockResolvedValueOnce([
      {
        id: "smithery",
        display_name: "Smithery",
        kind: "smithery",
        url: "https://x",
        api_key_env: null,
        priority: 50,
        default_trust: "community",
        enabled: true,
        cache_ttl_seconds: null
      }
    ]);
  const wrapper = mount(Marketplace);
  await flushPromises();
  // Initially both visible
  expect(wrapper.text()).toContain("A");
  expect(wrapper.text()).toContain("B");
  // Click chip "Built-in" to deselect
  await wrapper.find('[data-test="source-chip-builtin"]').trigger("click");
  await flushPromises();
  expect(wrapper.text()).not.toContain("A");
  expect(wrapper.text()).toContain("B");
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd apps/agent-gui && pnpm vitest --run src/components/__tests__/Marketplace.test.ts
```

Expected: 3 FAILED (chips/badges/multi-select missing).

- [ ] **Step 3: Implement the changes in `Marketplace.vue`**

Inside `<script setup>`:

```ts
import { computed, onMounted, ref } from "vue";
import { useCatalogStore } from "@/stores/catalog";

const store = useCatalogStore();
const selectedSources = ref<Set<string>>(new Set(["builtin"]));
const settingsOpen = ref(false);

onMounted(async () => {
  await store.loadSources();
  // default: all sources selected
  selectedSources.value = new Set([
    "builtin",
    ...store.sources.map((s) => s.id)
  ]);
});

const allSourceChips = computed(() => [
  { id: "builtin", display_name: "Built-in" },
  ...store.sources.map((s) => ({ id: s.id, display_name: s.display_name }))
]);

function toggleSource(id: string) {
  const next = new Set(selectedSources.value);
  next.has(id) ? next.delete(id) : next.add(id);
  selectedSources.value = next;
}

const visibleEntries = computed(() =>
  entries.value.filter((e) => selectedSources.value.has(e.source))
);
```

In the template, replace the existing source dropdown with chips:

```vue
<div class="source-filter">
    <button
        v-for="chip in allSourceChips"
        :key="chip.id"
        :data-test="`source-chip-${chip.id}`"
        :class="{ chip: true, active: selectedSources.has(chip.id) }"
        @click="toggleSource(chip.id)"
    >
        {{ chip.display_name }}
        <span
            v-if="store.sourceFailures[chip.id]"
            :data-test="`src-warn-${chip.id}`"
            :title="store.sourceFailures[chip.id]"
            class="warn"
        >⚠</span>
    </button>
    <button class="settings-icon" @click="settingsOpen = true" aria-label="Catalog source settings">
        ⚙
    </button>
</div>

<CatalogSourcesSettings v-if="settingsOpen" @close="settingsOpen = false" />
```

Bind the existing card grid loop over `visibleEntries` instead of `entries`.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd apps/agent-gui && pnpm vitest --run src/components/__tests__/Marketplace.test.ts
```

Expected: full file PASSED (existing + 3 new).

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/components/
git commit -m "feat(gui): add multi-source chip filter and failure badge to Marketplace"
```

---

## Task 18: Playwright E2E for remote catalog flow

**Files:**

- Modify: `apps/agent-gui/e2e/marketplace.spec.ts` (extend)

- [ ] **Step 1: Add the new specs**

Append after the existing Phase 1 install-happy-path spec:

```ts
test("user can add and remove a remote catalog source", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Marketplace" }).click();
  await page.getByLabel("Catalog source settings").click();

  // Drawer opens
  await expect(page.getByText("Remote Catalog Sources")).toBeVisible();
  await expect(page.getByText("No remote catalog sources")).toBeVisible();

  await page.getByTestId("add-source-toggle").click();
  await page.getByTestId("src-id").fill("smithery");
  await page.getByTestId("src-name").fill("Smithery");
  await page.getByTestId("src-url").fill("https://registry.smithery.ai");
  await page.getByTestId("src-save").click();

  // New chip appears in the marketplace toolbar.
  await expect(page.getByTestId("source-chip-smithery")).toBeVisible();

  // Remove it.
  await page.getByLabel("Catalog source settings").click();
  await page.getByTestId("src-remove-smithery").click();
  await page.getByTestId("src-confirm-remove").click();
  await expect(page.getByTestId("source-chip-smithery")).toHaveCount(0);
});

test("toggling source chip filters card grid", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Marketplace" }).click();

  // Built-in chip is on by default; cards are visible.
  await expect(
    page.locator('[data-test="catalog-card"]').first()
  ).toBeVisible();

  // Toggle Built-in off → no cards.
  await page.getByTestId("source-chip-builtin").click();
  await expect(page.locator('[data-test="catalog-card"]')).toHaveCount(0);

  // Toggle back on.
  await page.getByTestId("source-chip-builtin").click();
  await expect(
    page.locator('[data-test="catalog-card"]').first()
  ).toBeVisible();
});
```

The mock from T13 already serves these commands; no further mock changes are required.

- [ ] **Step 2: Run E2E**

```bash
just test-e2e
```

Expected: existing E2E + 2 new = all PASSED.

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/e2e/
git commit -m "test(gui): e2e specs for catalog source CRUD and chip filtering"
```

---

## Task 19: Final verification gate

This task does not modify code; it runs the full verification suite per
`verification-before-completion`. **All commands must pass before merge.**

- [ ] **Step 1: Format check**

```bash
pnpm run format:check
```

Expected: no diff.

- [ ] **Step 2: Lint (Rust + JS)**

```bash
pnpm run lint
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 3: Type sync gate**

```bash
just check-types
```

Expected: no diff in `apps/agent-gui/src/generated/`.

- [ ] **Step 4: Rust tests (full workspace)**

```bash
cargo test --workspace --all-targets
```

Expected: all PASSED, including the Phase 1 marketplace tests (regression check).

- [ ] **Step 5: MCP integration suite (Phase 2 tests live here)**

```bash
just test-mcp
```

Expected: all PASSED, including `marketplace_remote.rs`.

- [ ] **Step 6: Full-stack runtime tests**

```bash
just test-fullstack
```

Expected: all PASSED.

- [ ] **Step 7: GUI Vitest**

```bash
just test-gui
```

Expected: all PASSED, including `catalog.test.ts`, `CatalogSourcesSettings.test.ts`, `Marketplace.test.ts`.

- [ ] **Step 8: Playwright E2E**

```bash
just test-e2e
```

Expected: all PASSED, including `marketplace.spec.ts` extensions.

- [ ] **Step 9: Sanity build of TUI + Tauri bundle compile**

```bash
cargo build -p agent-tui
cargo build -p agent-gui-tauri
```

Both should succeed.

- [ ] **Step 10: Hand off to `finishing-a-development-branch` skill**

Per `executing-plans` skill: at this point announce

> "I'm using the finishing-a-development-branch skill to complete this work."

Then follow that skill to present merge / PR / cleanup options to the human.

---

## Acceptance Criteria

- [ ] All Phase 1 tests still pass (zero regressions)
- [ ] `agent-mcp` exposes `RemoteSourceConfig`, `KairoxJsonProvider`, `SmitheryProvider`, `HttpResponseCache`, `SharedHttpClient`, `build_provider`, `DomainEventSink`
- [ ] `AggregateCatalogProvider` supports priority, parallel listing, failure isolation, `reload`, rate-limited error events
- [ ] `agent-config` parses `[[catalog_sources]]` and exposes `LoadedConfig.catalog_sources`
- [ ] `agent-core` has `CatalogSourceAdded` + `CatalogSourceFailed` `EventPayload` variants and `CatalogSourceView` type
- [ ] `AppFacade` has `list/add/update/remove_catalog_source`
- [ ] 4 new Tauri commands registered in **both** `generate_handler!` and `collect_commands!`
- [ ] `tauri-mock.js` handles all 4 new commands and emits both new events
- [ ] `apps/agent-gui/src/generated/{commands,events}.ts` regenerated and committed
- [ ] `CatalogSourcesSettings.vue` rendered, editable, validated
- [ ] `Marketplace.vue` shows multi-source chips + warn badge on failure
- [ ] `tests/marketplace_remote.rs` integration test passes
- [ ] All commands in T19 succeed
- [ ] Conventional Commits used throughout (scope: `mcp`, `config`, `core`, `runtime`, `gui`)
