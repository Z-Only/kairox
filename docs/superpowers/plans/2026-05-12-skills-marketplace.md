# Skills Marketplace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a multi-source skills marketplace system with built-in (skills.sh + SkillHub) and user-configurable sources, caching, and unified UI card styles.

**Architecture:** New `SkillCatalogProvider` trait in `agent-mcp::catalog::skills` mirroring MCP's `CatalogProvider`. Reuse `SharedHttpClient` + `HttpResponseCache` from the MCP catalog layer. Extend `AppFacade` with new methods, wire Tauri commands, and add Vue components with unified `.catalog-card` styles.

**Tech Stack:** Rust (tokio, reqwest, serde, toml_edit), TypeScript (Vue 3, Pinia), specta for type generation

---

## File Structure

| Layer               | Create                                                                                                            | Modify                                                                      |
| ------------------- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Core types          | —                                                                                                                 | `crates/agent-core/src/facade.rs` (add types after line 323)                |
| Catalog module      | `crates/agent-mcp/src/catalog/skills/mod.rs`                                                                      | `crates/agent-mcp/src/catalog/mod.rs` (add `pub mod skills;`)               |
| Aggregate           | `crates/agent-mcp/src/catalog/skills/aggregate.rs`                                                                | —                                                                           |
| Providers           | `crates/agent-mcp/src/catalog/skills/skills_sh.rs`, `skillhub.rs`, `remote.rs`                                    | —                                                                           |
| HTTP/Cache          | —                                                                                                                 | Reuse `crates/agent-mcp/src/catalog/remote/http_client.rs`, `http_cache.rs` |
| Configuration       | `crates/agent-runtime/src/skill_sources_toml.rs`                                                                  | —                                                                           |
| Facade trait        | —                                                                                                                 | `crates/agent-core/src/facade.rs` (add default method stubs after line 686) |
| Facade impl         | —                                                                                                                 | `crates/agent-runtime/src/facade_runtime.rs` (add impl after line 1235)     |
| Tauri commands      | —                                                                                                                 | `apps/agent-gui/src-tauri/src/commands.rs`, `lib.rs`                        |
| Frontend store      | —                                                                                                                 | `apps/agent-gui/src/stores/skills.ts`                                       |
| Frontend components | `apps/agent-gui/src/components/skills/SkillDiscoverCard.vue`, `SkillDiscoverList.vue`, `SkillSourcesSettings.vue` | `apps/agent-gui/src/components/SkillSettingsPane.vue`                       |
| Generated types     | —                                                                                                                 | `just gen-types` (auto)                                                     |

---

### Task 1: Add SkillCatalogEntry, SkillCatalogQuery, SkillSourceView DTOs to agent-core

**Files:**

- Modify: `crates/agent-core/src/facade.rs:323` (after `InstallGithubSkillRequest`)

- [ ] **Step 1: Add the new types after line 323**

```rust
// ── Skills catalog / marketplace ───────────────────────────────────────

/// A single skill entry returned by the skills catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillCatalogEntry {
    pub catalog_id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub source_url: String,
    pub install_count: Option<u64>,
    pub github_stars: Option<u64>,
    pub security_score: Option<u32>,
    pub rating: Option<f64>,
    pub package: String,
}

/// Query against the skills catalog.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillCatalogQuery {
    pub keyword: Option<String>,
    pub sources: Option<Vec<String>>,
    pub limit: Option<usize>,
}

/// JSON field mapping for a skill source API response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillFieldMappingView {
    pub name_path: String,
    pub description_path: String,
    pub install_count_path: Option<String>,
    pub github_stars_path: Option<String>,
    pub package_path: String,
    pub source_url_path: Option<String>,
}

impl Default for SkillFieldMappingView {
    fn default() -> Self {
        Self {
            name_path: "name".into(),
            description_path: "description".into(),
            install_count_path: Some("installs".into()),
            github_stars_path: None,
            package_path: "id".into(),
            source_url_path: None,
        }
    }
}

/// A configured skill catalog source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillSourceView {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub url: String,
    pub search_template: String,
    pub list_template: Option<String>,
    pub field_mapping: SkillFieldMappingView,
    pub enabled: bool,
    pub priority: u32,
    pub cache_ttl_seconds: u64,
    pub last_error: Option<String>,
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p agent-core 2>&1
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-core/src/facade.rs
git commit -m "feat(core): add skill catalog DTOs (SkillCatalogEntry, SkillCatalogQuery, SkillSourceView)"
```

---

### Task 2: Add AppFacade default method stubs for skill catalog

**Files:**

- Modify: `crates/agent-core/src/facade.rs` (after line 686, after `update_skill` default)

- [ ] **Step 1: Add 5 new default methods after the `update_skill` stub**

```rust
    // ── Skills catalog / marketplace ─────────────────────────────────

    /// List skill catalog entries, optionally filtered by query.
    async fn list_skill_catalog(
        &self,
        _query: SkillCatalogQuery,
    ) -> crate::Result<Vec<SkillCatalogEntry>> {
        Ok(Vec::new())
    }

    /// List configured skill catalog sources (includes builtins).
    async fn list_skill_sources(&self) -> crate::Result<Vec<SkillSourceView>> {
        Ok(Vec::new())
    }

    /// Add a new skill catalog source.
    async fn add_skill_source(&self, _config: SkillSourceView) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    /// Remove a skill catalog source.
    async fn remove_skill_source(&self, _id: String) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    /// Enable or disable a skill catalog source.
    async fn set_skill_source_enabled(
        &self,
        _id: String,
        _enabled: bool,
    ) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    /// Refresh skill catalog data from all sources.
    async fn refresh_skill_catalog(&self) -> crate::Result<()> {
        Ok(())
    }
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p agent-core 2>&1
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-core/src/facade.rs
git commit -m "feat(core): add AppFacade default stubs for skill catalog operations"
```

---

### Task 3: Create the SkillCatalogProvider trait and SkillCatalogEntry type in agent-mcp

**Files:**

- Create: `crates/agent-mcp/src/catalog/skills/mod.rs`
- Modify: `crates/agent-mcp/src/catalog/mod.rs` (add `pub mod skills;` after line 172)

- [ ] **Step 1: Create `crates/agent-mcp/src/catalog/skills/mod.rs`**

```rust
//! Skills catalog: trait + data types for browsing skill registries.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single skill entry returned by a [`SkillCatalogProvider`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillCatalogEntry {
    pub catalog_id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub source_url: String,
    pub install_count: Option<u64>,
    pub github_stars: Option<u64>,
    pub security_score: Option<u32>,
    pub rating: Option<f64>,
    pub package: String,
}

/// Query parameters for skill catalog searches.
#[derive(Debug, Clone, Default)]
pub struct SkillCatalogQuery {
    pub keyword: Option<String>,
    pub sources: Option<Vec<String>>,
    pub limit: Option<usize>,
}

/// Errors specific to skill catalog operations.
#[derive(Debug, thiserror::Error)]
pub enum SkillCatalogError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type SkillCatalogResult<T> = std::result::Result<T, SkillCatalogError>;

/// A source of [`SkillCatalogEntry`] data.
#[async_trait]
pub trait SkillCatalogProvider: Send + Sync {
    fn source_id(&self) -> &str;

    /// Search this source for skills matching the query keyword.
    async fn search(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>>;

    /// List entries from this source (no keyword filtering). Returns empty
    /// vec if the source does not support listing.
    async fn list(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let _ = query;
        Ok(Vec::new())
    }

    /// Force-refresh the source's cache.
    async fn refresh(&self) -> SkillCatalogResult<()> {
        Ok(())
    }
}

pub mod aggregate;
pub mod remote;
pub mod skills_sh;
pub mod skillhub;
```

- [ ] **Step 2: Add `pub mod skills;` to `crates/agent-mcp/src/catalog/mod.rs` after line 172**

```rust
pub mod skills; // added in Task 3
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p agent-mcp 2>&1
```

Expected: may fail because sub-modules don't exist yet — that's fine, we create them next.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-mcp/src/catalog/skills/mod.rs crates/agent-mcp/src/catalog/mod.rs
git commit -m "feat(mcp): add SkillCatalogProvider trait and types"
```

---

### Task 4: Create remote source config types

**Files:**

- Create: `crates/agent-mcp/src/catalog/skills/remote.rs`

- [ ] **Step 1: Create `crates/agent-mcp/src/catalog/skills/remote.rs`**

```rust
//! Remote skill catalog source configuration and provider construction.

use crate::catalog::skills::SkillCatalogProvider;
use crate::catalog::remote::http_cache::HttpResponseCache;
use crate::catalog::remote::http_client::SharedHttpClient;
use std::sync::Arc;

/// Which adapter implementation backs a skill catalog source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSourceKind {
    /// skills.sh (`https://skills.sh/api/search`)
    SkillsSh,
    /// SkillHub (`https://skills.palebluedot.live/api/skills`)
    SkillHub,
}

impl SkillSourceKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "skills-sh" | "skills_sh" => Some(Self::SkillsSh),
            "skillhub" => Some(Self::SkillHub),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SkillsSh => "skills-sh",
            Self::SkillHub => "skillhub",
        }
    }
}

/// A single remote skill catalog source configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSkillSourceConfig {
    pub id: String,
    pub display_name: String,
    pub kind: SkillSourceKind,
    pub url: String,
    /// URL template for search, e.g. `/api/search?q={{query}}&limit={{limit}}`
    pub search_template: String,
    /// URL template for list, e.g. `/api/skills?limit={{limit}}`. None if not supported.
    pub list_template: Option<String>,
    pub enabled: bool,
    pub priority: u32,
    pub cache_ttl_seconds: u64,
}

/// Construct the right [`SkillCatalogProvider`] implementation.
pub fn build_skill_provider(
    cfg: RemoteSkillSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
) -> Arc<dyn SkillCatalogProvider> {
    match cfg.kind {
        SkillSourceKind::SkillsSh => Arc::new(
            crate::catalog::skills::skills_sh::SkillsShProvider::new(cfg, http, cache),
        ),
        SkillSourceKind::SkillHub => Arc::new(
            crate::catalog::skills::skillhub::SkillHubProvider::new(cfg, http, cache),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_source_kind_from_str_round_trip() {
        assert_eq!(
            SkillSourceKind::from_str("skills-sh"),
            Some(SkillSourceKind::SkillsSh)
        );
        assert_eq!(
            SkillSourceKind::from_str("skillhub"),
            Some(SkillSourceKind::SkillHub)
        );
        assert_eq!(SkillSourceKind::from_str("unknown"), None);
    }

    #[test]
    fn skill_source_kind_as_str() {
        assert_eq!(SkillSourceKind::SkillsSh.as_str(), "skills-sh");
        assert_eq!(SkillSourceKind::SkillHub.as_str(), "skillhub");
    }

    #[test]
    fn build_skill_provider_returns_correct_kind() {
        let http = SharedHttpClient::new().unwrap();
        let cache = Arc::new(HttpResponseCache::new(
            std::env::temp_dir().join("kairox-test-skill-cache"),
        ));
        let provider = build_skill_provider(
            RemoteSkillSourceConfig {
                id: "skills-sh".into(),
                display_name: "skills.sh".into(),
                kind: SkillSourceKind::SkillsSh,
                url: "https://skills.sh".into(),
                search_template: "/api/search?q={{query}}&limit={{limit}}".into(),
                list_template: None,
                enabled: true,
                priority: 10,
                cache_ttl_seconds: 900,
            },
            http,
            cache,
        );
        assert_eq!(provider.source_id(), "skills-sh");
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-mcp/src/catalog/skills/remote.rs
git commit -m "feat(mcp): add skill remote source config and build_skill_provider"
```

---

### Task 5: Implement SkillsShProvider

**Files:**

- Create: `crates/agent-mcp/src/catalog/skills/skills_sh.rs`

- [ ] **Step 1: Create the provider file**

```rust
//! skills.sh catalog provider.
//!
//! Adapts the skills.sh API (`/api/search`) to [`SkillCatalogEntry`].

use crate::catalog::remote::http_cache::{CachedResponse, HttpResponseCache};
use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::skills::remote::RemoteSkillSourceConfig;
use crate::catalog::skills::{
    SkillCatalogEntry, SkillCatalogError, SkillCatalogProvider, SkillCatalogQuery,
    SkillCatalogResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_SECONDS: u64 = 900;

#[derive(Debug, Deserialize)]
struct SkillsShResponse {
    #[serde(default)]
    skills: Vec<SkillsShItem>,
}

#[derive(Debug, Deserialize)]
struct SkillsShItem {
    id: String,
    name: String,
    #[serde(default)]
    installs: Option<u64>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default, rename = "skillId")]
    skill_id: Option<String>,
}

pub struct SkillsShProvider {
    cfg: RemoteSkillSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
}

impl SkillsShProvider {
    pub fn new(
        cfg: RemoteSkillSourceConfig,
        http: SharedHttpClient,
        cache: Arc<HttpResponseCache>,
    ) -> Self {
        Self { cfg, http, cache }
    }

    fn ttl(&self) -> u64 {
        if self.cfg.cache_ttl_seconds > 0 {
            self.cfg.cache_ttl_seconds
        } else {
            DEFAULT_TTL_SECONDS
        }
    }

    fn build_search_url(&self, query: &str, limit: usize) -> String {
        let base = self.cfg.url.trim_end_matches('/');
        let encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        self.cfg
            .search_template
            .replace("{{query}}", &encoded)
            .replace("{{limit}}", &limit.to_string())
            .replacen("{{query}}", &encoded, 1)
            .replacen("{{limit}}", &limit.to_string(), 1)
    }

    async fn fetch_search(
        &self,
        keyword: &str,
        limit: usize,
    ) -> Result<Vec<SkillCatalogEntry>, SkillCatalogError> {
        let url = if self.cfg.search_template.contains("{{query}}") {
            self.build_search_url(keyword, limit)
        } else {
            format!(
                "{}{}",
                self.cfg.url.trim_end_matches('/'),
                self.cfg.search_template
            )
        };

        let response = self
            .http
            .get_json(
                &url,
                GetOpts {
                    api_key_env: None,
                    if_none_match: None,
                },
            )
            .await
            .map_err(|e| SkillCatalogError::Http(format!("skills.sh request failed: {e}")))?;

        if !(200..300).contains(&response.status) {
            return Err(SkillCatalogError::Http(format!(
                "skills.sh returned status {}",
                response.status
            )));
        }

        let parsed: SkillsShResponse = serde_json::from_slice(&response.body)
            .map_err(|e| SkillCatalogError::Decode(format!("skills.sh parse: {e}")))?;

        let entries: Vec<SkillCatalogEntry> = parsed
            .skills
            .into_iter()
            .map(|item| SkillCatalogEntry {
                catalog_id: item.id.clone(),
                name: item.name,
                description: String::new(),
                source: self.cfg.id.clone(),
                source_url: format!("https://skills.sh/skills/{}", item.id),
                install_count: item.installs,
                github_stars: None,
                security_score: None,
                rating: None,
                package: item.id,
            })
            .collect();

        // Cache the result under a search-scoped key.
        let cache_key = format!("{}:search:{}", self.cfg.id, keyword);
        let cached = CachedResponse {
            fetched_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            etag: None,
            last_modified: None,
            entries: entries.clone(),
        };
        let _ = self.cache.put(&cache_key, cached).await;

        Ok(entries)
    }

    async fn cached_search(
        &self,
        keyword: &str,
        limit: usize,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let cache_key = format!("{}:search:{}", self.cfg.id, keyword);
        let lock = self.cache.lock_for(&cache_key).await;
        let _guard = lock.lock().await;

        if let Some(cached) = self.cache.get(&cache_key).await {
            if HttpResponseCache::is_fresh(&cached, self.ttl()) {
                return Ok(cached.entries);
            }
            match self.fetch_search(keyword, limit).await {
                Ok(entries) => Ok(entries),
                Err(e) => {
                    tracing::warn!(error=%e, "skills.sh refetch failed, serving stale");
                    Ok(cached.entries)
                }
            }
        } else {
            self.fetch_search(keyword, limit).await
        }
    }
}

#[async_trait]
impl SkillCatalogProvider for SkillsShProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn search(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let keyword = query.keyword.as_deref().unwrap_or("");
        let limit = query.limit.unwrap_or(50);
        self.cached_search(keyword, limit).await
    }

    async fn refresh(&self) -> SkillCatalogResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::skills::remote::SkillSourceKind;

    #[test]
    fn build_search_url_substitutes_placeholders() {
        let cfg = RemoteSkillSourceConfig {
            id: "skills-sh".into(),
            display_name: "skills.sh".into(),
            kind: SkillSourceKind::SkillsSh,
            url: "https://skills.sh".into(),
            search_template: "/api/search?q={{query}}&limit={{limit}}".into(),
            list_template: None,
            enabled: true,
            priority: 10,
            cache_ttl_seconds: 900,
        };
        let http = SharedHttpClient::new().unwrap();
        let cache = Arc::new(HttpResponseCache::new(
            std::env::temp_dir().join("kairox-test-sh-cache"),
        ));
        let provider = SkillsShProvider::new(cfg, http, cache);
        let url = provider.build_search_url("code review", 10);
        assert!(url.contains("q=code+review"));
        assert!(url.contains("limit=10"));
    }

    #[test]
    fn skills_sh_response_parses_correctly() {
        let json = r#"{"query":"test","skills":[{"id":"test/skill","name":"test-skill","installs":100,"source":"test/repo","skillId":"test-skill"}]}"#;
        let parsed: SkillsShResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.skills.len(), 1);
        assert_eq!(parsed.skills[0].name, "test-skill");
        assert_eq!(parsed.skills[0].installs, Some(100));
    }

    #[test]
    fn skills_sh_response_missing_optional_fields() {
        let json = r#"{"query":"test","skills":[{"id":"test/skill","name":"test-skill"}]}"#;
        let parsed: SkillsShResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.skills[0].installs, None);
        assert_eq!(parsed.skills[0].source, None);
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p agent-mcp 2>&1
```

- [ ] **Step 3: Run the unit tests**

```bash
cargo test -p agent-mcp -- catalog::skills::skills_sh 2>&1
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-mcp/src/catalog/skills/skills_sh.rs
git commit -m "feat(mcp): implement SkillsShProvider for skills.sh API"
```

---

### Task 6: Implement SkillHubProvider

**Files:**

- Create: `crates/agent-mcp/src/catalog/skills/skillhub.rs`

- [ ] **Step 1: Create the provider file**

```rust
//! SkillHub catalog provider.
//!
//! Adapts the SkillHub API (`https://skills.palebluedot.live/api/skills`)
//! to [`SkillCatalogEntry`].

use crate::catalog::remote::http_cache::{CachedResponse, HttpResponseCache};
use crate::catalog::remote::http_client::{GetOpts, SharedHttpClient};
use crate::catalog::skills::remote::RemoteSkillSourceConfig;
use crate::catalog::skills::{
    SkillCatalogEntry, SkillCatalogError, SkillCatalogProvider, SkillCatalogQuery,
    SkillCatalogResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_SECONDS: u64 = 900;

#[derive(Debug, Deserialize)]
struct SkillHubResponse {
    #[serde(default)]
    skills: Vec<SkillHubItem>,
    #[serde(default)]
    pagination: Option<SkillHubPagination>,
}

#[derive(Debug, Deserialize)]
struct SkillHubPagination {
    #[serde(default)]
    total: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SkillHubItem {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    #[serde(rename = "githubOwner")]
    github_owner: Option<String>,
    #[serde(default)]
    #[serde(rename = "githubRepo")]
    github_repo: Option<String>,
    #[serde(default)]
    #[serde(rename = "githubStars")]
    github_stars: Option<u64>,
    #[serde(default)]
    #[serde(rename = "downloadCount")]
    download_count: Option<u64>,
    #[serde(default)]
    #[serde(rename = "securityScore")]
    security_score: Option<u32>,
    #[serde(default)]
    rating: Option<f64>,
}

pub struct SkillHubProvider {
    cfg: RemoteSkillSourceConfig,
    http: SharedHttpClient,
    cache: Arc<HttpResponseCache>,
}

impl SkillHubProvider {
    pub fn new(
        cfg: RemoteSkillSourceConfig,
        http: SharedHttpClient,
        cache: Arc<HttpResponseCache>,
    ) -> Self {
        Self { cfg, http, cache }
    }

    fn ttl(&self) -> u64 {
        if self.cfg.cache_ttl_seconds > 0 {
            self.cfg.cache_ttl_seconds
        } else {
            DEFAULT_TTL_SECONDS
        }
    }

    fn build_url(&self, keyword: Option<&str>, limit: usize) -> String {
        let base = self.cfg.url.trim_end_matches('/');
        let template = match (keyword, &self.cfg.list_template) {
            (Some(_), _) => &self.cfg.search_template,
            (None, Some(list_tmpl)) => list_tmpl,
            (None, None) => &self.cfg.search_template,
        };

        let mut url = template
            .replace("{{limit}}", &limit.to_string())
            .replacen("{{limit}}", &limit.to_string(), 1);

        if let Some(kw) = keyword {
            let encoded =
                url::form_urlencoded::byte_serialize(kw.as_bytes()).collect::<String>();
            url = url
                .replace("{{query}}", &encoded)
                .replacen("{{query}}", &encoded, 1);
        } else {
            // Remove the query param if no keyword — the API will return
            // full listing.
            url = url.replace("?q={{query}}&", "?").replace("&q={{query}}", "");
        }

        if url.starts_with('/') {
            format!("{base}{url}")
        } else {
            url
        }
    }

    async fn fetch(
        &self,
        keyword: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SkillCatalogEntry>, SkillCatalogError> {
        let url = self.build_url(keyword, limit);

        let response = self
            .http
            .get_json(
                &url,
                GetOpts {
                    api_key_env: None,
                    if_none_match: None,
                },
            )
            .await
            .map_err(|e| SkillCatalogError::Http(format!("SkillHub request failed: {e}")))?;

        if !(200..300).contains(&response.status) {
            return Err(SkillCatalogError::Http(format!(
                "SkillHub returned status {}",
                response.status
            )));
        }

        let parsed: SkillHubResponse = serde_json::from_slice(&response.body)
            .map_err(|e| SkillCatalogError::Decode(format!("SkillHub parse: {e}")))?;

        let entries: Vec<SkillCatalogEntry> = parsed
            .skills
            .into_iter()
            .map(|item| SkillCatalogEntry {
                catalog_id: item.id.clone(),
                name: item.name,
                description: item.description.unwrap_or_default(),
                source: self.cfg.id.clone(),
                source_url: format!("https://skills.palebluedot.live/skills/{}", item.id),
                install_count: item.download_count,
                github_stars: item.github_stars,
                security_score: item.security_score,
                rating: item.rating,
                package: item.id,
            })
            .collect();

        let cache_key = match keyword {
            Some(kw) => format!("{}:search:{}", self.cfg.id, kw),
            None => format!("{}:list:{}", self.cfg.id, limit),
        };
        let cached = CachedResponse {
            fetched_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            etag: None,
            last_modified: None,
            entries: entries.clone(),
        };
        let _ = self.cache.put(&cache_key, cached).await;

        Ok(entries)
    }

    async fn cached_fetch(
        &self,
        keyword: Option<&str>,
        limit: usize,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let cache_key = match keyword {
            Some(kw) => format!("{}:search:{}", self.cfg.id, kw),
            None => format!("{}:list:{}", self.cfg.id, limit),
        };
        let lock = self.cache.lock_for(&cache_key).await;
        let _guard = lock.lock().await;

        if let Some(cached) = self.cache.get(&cache_key).await {
            if HttpResponseCache::is_fresh(&cached, self.ttl()) {
                return Ok(cached.entries);
            }
            match self.fetch(keyword, limit).await {
                Ok(entries) => Ok(entries),
                Err(e) => {
                    tracing::warn!(error=%e, "SkillHub refetch failed, serving stale");
                    Ok(cached.entries)
                }
            }
        } else {
            self.fetch(keyword, limit).await
        }
    }
}

#[async_trait]
impl SkillCatalogProvider for SkillHubProvider {
    fn source_id(&self) -> &str {
        &self.cfg.id
    }

    async fn search(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let keyword = query.keyword.as_deref();
        let limit = query.limit.unwrap_or(50);
        self.cached_fetch(keyword, limit).await
    }

    async fn list(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let limit = query.limit.unwrap_or(50);
        self.cached_fetch(None, limit).await
    }

    async fn refresh(&self) -> SkillCatalogResult<()> {
        // Bust cache by re-fetching.
        let _ = self.fetch(None, 50).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::skills::remote::SkillSourceKind;

    fn test_cfg() -> RemoteSkillSourceConfig {
        RemoteSkillSourceConfig {
            id: "skillhub".into(),
            display_name: "SkillHub".into(),
            kind: SkillSourceKind::SkillHub,
            url: "https://skills.palebluedot.live".into(),
            search_template: "/api/skills?q={{query}}&limit={{limit}}".into(),
            list_template: Some("/api/skills?limit={{limit}}".into()),
            enabled: true,
            priority: 20,
            cache_ttl_seconds: 900,
        }
    }

    #[test]
    fn build_search_url_with_keyword() {
        let http = SharedHttpClient::new().unwrap();
        let cache = Arc::new(HttpResponseCache::new(
            std::env::temp_dir().join("kairox-test-hub-cache"),
        ));
        let provider = SkillHubProvider::new(test_cfg(), http, cache);
        let url = provider.build_url(Some("code review"), 10);
        assert!(url.contains("q=code+review"));
        assert!(url.contains("limit=10"));
        assert!(url.starts_with("https://skills.palebluedot.live"));
    }

    #[test]
    fn build_list_url_without_keyword() {
        let http = SharedHttpClient::new().unwrap();
        let cache = Arc::new(HttpResponseCache::new(
            std::env::temp_dir().join("kairox-test-hub-list-cache"),
        ));
        let provider = SkillHubProvider::new(test_cfg(), http, cache);
        let url = provider.build_url(None, 20);
        assert!(!url.contains("q="));
        assert!(url.contains("limit=20"));
        assert!(url.starts_with("https://skills.palebluedot.live"));
    }

    #[test]
    fn skillhub_response_parses_correctly() {
        let json = r#"{"skills":[{"id":"test/skill","name":"test-skill","description":"A test skill","githubStars":100,"downloadCount":50,"securityScore":95,"rating":4.5}]}"#;
        let parsed: SkillHubResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.skills.len(), 1);
        assert_eq!(parsed.skills[0].name, "test-skill");
        assert_eq!(parsed.skills[0].description.as_deref(), Some("A test skill"));
        assert_eq!(parsed.skills[0].github_stars, Some(100));
        assert_eq!(parsed.skills[0].download_count, Some(50));
    }
}
```

- [ ] **Step 2: Verify it compiles and tests pass**

```bash
cargo test -p agent-mcp -- catalog::skills::skillhub 2>&1
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/src/catalog/skills/skillhub.rs
git commit -m "feat(mcp): implement SkillHubProvider for skills.palebluedot.live API"
```

---

### Task 7: Create AggregateSkillCatalogProvider

**Files:**

- Create: `crates/agent-mcp/src/catalog/skills/aggregate.rs`

- [ ] **Step 1: Create the aggregate file**

```rust
//! Aggregates multiple [`SkillCatalogProvider`]s with priority ordering,
//! parallel querying, and failure isolation.

use crate::catalog::skills::{SkillCatalogEntry, SkillCatalogError, SkillCatalogProvider, SkillCatalogQuery, SkillCatalogResult};
use async_trait::async_trait;
use futures::future::join_all;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct PrioritisedProvider {
    priority: u32,
    inner: Arc<dyn SkillCatalogProvider>,
}

pub struct AggregateSkillCatalogProvider {
    inner: Mutex<Vec<PrioritisedProvider>>,
}

impl AggregateSkillCatalogProvider {
    pub fn new(providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)>) -> Self {
        let mut inner: Vec<PrioritisedProvider> = providers
            .into_iter()
            .map(|(priority, inner)| PrioritisedProvider { priority, inner })
            .collect();
        inner.sort_by_key(|p| p.priority);
        Self {
            inner: Mutex::new(inner),
        }
    }

    pub fn reload(&self, providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)>) {
        let mut inner: Vec<PrioritisedProvider> = providers
            .into_iter()
            .map(|(priority, inner)| PrioritisedProvider { priority, inner })
            .collect();
        inner.sort_by_key(|p| p.priority);
        match self.inner.try_lock() {
            Ok(mut guard) => *guard = inner,
            Err(_) => {
                let mut guard = self.inner.blocking_lock();
                *guard = inner;
            }
        }
    }

    async fn query(
        &self,
        query: &SkillCatalogQuery,
        use_search: bool,
    ) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        let active: Vec<PrioritisedProvider> = providers
            .into_iter()
            .filter(|p| {
                query
                    .sources
                    .as_ref()
                    .map(|srcs| srcs.contains(&p.inner.source_id().to_string()))
                    .unwrap_or(true)
            })
            .collect();

        let futures = active.iter().map(|p| {
            let q = query.clone();
            async move {
                let id = p.inner.source_id().to_string();
                let result = if use_search {
                    p.inner.search(&q).await
                } else {
                    p.inner.list(&q).await
                };
                (p.priority, id, result)
            }
        });
        let mut results = join_all(futures).await;
        results.sort_by_key(|(prio, _, _)| *prio);

        let mut all: Vec<SkillCatalogEntry> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (_, source_id, res) in results {
            match res {
                Ok(entries) => {
                    for entry in entries {
                        let key = format!("{}:{}", source_id, entry.catalog_id);
                        if seen.insert(key) {
                            all.push(entry);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(source=%source_id, error=%e, "skill catalog source failed");
                }
            }
        }

        if let Some(limit) = query.limit {
            all.truncate(limit);
        }
        Ok(all)
    }
}

#[async_trait]
impl SkillCatalogProvider for AggregateSkillCatalogProvider {
    fn source_id(&self) -> &str {
        "aggregate"
    }

    async fn search(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        self.query(query, true).await
    }

    async fn list(&self, query: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
        self.query(query, false).await
    }

    async fn refresh(&self) -> SkillCatalogResult<()> {
        let providers: Vec<PrioritisedProvider> = self.inner.lock().await.clone();
        let futures = providers.iter().map(|p| async move {
            let id = p.inner.source_id().to_string();
            (id, p.inner.refresh().await)
        });
        let results = join_all(futures).await;
        for (source_id, res) in results {
            if let Err(e) = res {
                tracing::warn!(source=%source_id, error=%e, "skill catalog refresh failed");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::skills::SkillCatalogResult;
    use async_trait::async_trait;

    struct StaticSkillProvider {
        id: &'static str,
        entries: Vec<SkillCatalogEntry>,
    }

    #[async_trait]
    impl SkillCatalogProvider for StaticSkillProvider {
        fn source_id(&self) -> &str {
            self.id
        }

        async fn search(&self, _q: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
            Ok(self.entries.clone())
        }

        async fn list(&self, _q: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
            Ok(self.entries.clone())
        }
    }

    fn make_entry(id: &str, source: &str) -> SkillCatalogEntry {
        SkillCatalogEntry {
            catalog_id: id.into(),
            name: id.into(),
            description: String::new(),
            source: source.into(),
            source_url: String::new(),
            install_count: None,
            github_stars: None,
            security_score: None,
            rating: None,
            package: id.into(),
        }
    }

    struct FailingSkillProvider { id: &'static str }

    #[async_trait]
    impl SkillCatalogProvider for FailingSkillProvider {
        fn source_id(&self) -> &str {
            self.id
        }
        async fn search(&self, _q: &SkillCatalogQuery) -> SkillCatalogResult<Vec<SkillCatalogEntry>> {
            Err(SkillCatalogError::Provider("boom".into()))
        }
    }

    #[tokio::test]
    async fn aggregates_multiple_sources() {
        let a = Arc::new(StaticSkillProvider {
            id: "a",
            entries: vec![make_entry("x", "a")],
        });
        let b = Arc::new(StaticSkillProvider {
            id: "b",
            entries: vec![make_entry("y", "b")],
        });
        let agg = AggregateSkillCatalogProvider::new(vec![(10, a), (20, b)]);
        let q = SkillCatalogQuery::default();
        let results = agg.search(&q).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn one_source_failure_does_not_fail_aggregate() {
        let ok = Arc::new(StaticSkillProvider {
            id: "ok",
            entries: vec![make_entry("x", "ok")],
        });
        let bad: Arc<dyn SkillCatalogProvider> = Arc::new(FailingSkillProvider { id: "bad" });
        let agg = AggregateSkillCatalogProvider::new(vec![(10, ok), (20, bad)]);
        let results = agg.search(&SkillCatalogQuery::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "ok");
    }

    #[tokio::test]
    async fn deduplicates_by_source_and_catalog_id() {
        let p1 = Arc::new(StaticSkillProvider {
            id: "src",
            entries: vec![make_entry("dup", "src")],
        });
        let p2 = Arc::new(StaticSkillProvider {
            id: "src",
            entries: vec![make_entry("dup", "src"), make_entry("uniq", "src")],
        });
        let agg = AggregateSkillCatalogProvider::new(vec![(10, p1), (20, p2)]);
        let results = agg.search(&SkillCatalogQuery::default()).await.unwrap();
        assert_eq!(results.len(), 2, "should dedup by (source, catalog_id)");
    }

    #[tokio::test]
    async fn respects_limit() {
        let p = Arc::new(StaticSkillProvider {
            id: "src",
            entries: vec![
                make_entry("a", "src"),
                make_entry("b", "src"),
                make_entry("c", "src"),
            ],
        });
        let agg = AggregateSkillCatalogProvider::new(vec![(10, p)]);
        let mut q = SkillCatalogQuery::default();
        q.limit = Some(2);
        let results = agg.search(&q).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn filters_by_source_ids() {
        let a = Arc::new(StaticSkillProvider {
            id: "a",
            entries: vec![make_entry("x", "a")],
        });
        let b = Arc::new(StaticSkillProvider {
            id: "b",
            entries: vec![make_entry("y", "b")],
        });
        let agg = AggregateSkillCatalogProvider::new(vec![(10, a), (20, b)]);
        let mut q = SkillCatalogQuery::default();
        q.sources = Some(vec!["a".into()]);
        let results = agg.search(&q).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "a");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p agent-mcp -- catalog::skills::aggregate 2>&1
```

Expected: all 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/src/catalog/skills/aggregate.rs
git commit -m "feat(mcp): add AggregateSkillCatalogProvider"
```

---

### Task 8: Create SkillSourcesToml for configuration persistence

**Files:**

- Create: `crates/agent-runtime/src/skill_sources_toml.rs`

- [ ] **Step 1: Create the TOML persistence module**

```rust
//! Read/write `~/.kairox/skill_sources.toml` for skill catalog source
//! configuration persistence.

use agent_core::facade::{SkillFieldMappingView, SkillSourceView};
use std::path::{Path, PathBuf};
use toml_edit::{value, DocumentMut, Item};

pub struct SkillSourcesToml {
    path: PathBuf,
}

impl SkillSourcesToml {
    pub fn new(dir: &Path) -> Self {
        std::fs::create_dir_all(dir).ok();
        Self {
            path: dir.join("skill_sources.toml"),
        }
    }

    /// Read all sources from disk. Returns an empty vec if the file doesn't
    /// exist yet.
    pub fn read(&self) -> Vec<SkillSourceView> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        let doc: DocumentMut = match text.parse() {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };

        let mut sources = Vec::new();
        let Some(sources_array) = doc.get_array_of_tables("skill_sources") else {
            return sources;
        };

        for item in sources_array.iter() {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() {
                continue;
            }
            sources.push(SkillSourceView {
                id: id.to_string(),
                display_name: item
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(id)
                    .to_string(),
                kind: item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("custom")
                    .to_string(),
                url: item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                search_template: item
                    .get("search_template")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/api/search?q={{query}}&limit={{limit}}")
                    .to_string(),
                list_template: item
                    .get("list_template")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                field_mapping: SkillFieldMappingView::default(),
                enabled: item
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                priority: item
                    .get("priority")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(100) as u32,
                cache_ttl_seconds: item
                    .get("cache_ttl_seconds")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(900) as u64,
                last_error: None,
            });
        }
        sources
    }

    /// Write sources to disk. Creates the file if it doesn't exist.
    pub fn write(&self, sources: &[SkillSourceView]) -> Result<(), std::io::Error> {
        let mut doc = DocumentMut::new();
        for src in sources {
            let mut tbl = toml_edit::Table::new();
            tbl.insert("id", value(&src.id));
            tbl.insert("display_name", value(&src.display_name));
            tbl.insert("kind", value(&src.kind));
            tbl.insert("url", value(&src.url));
            tbl.insert("search_template", value(&src.search_template));
            if let Some(ref lt) = src.list_template {
                tbl.insert("list_template", value(lt));
            }
            tbl.insert("enabled", value(src.enabled));
            tbl.insert("priority", value(src.priority as i64));
            tbl.insert("cache_ttl_seconds", value(src.cache_ttl_seconds as i64));
            doc["skill_sources"]
                .as_array_of_tables_mut()
                .unwrap_or_else(|| {
                    doc["skill_sources"] = Item::ArrayOfTables(Default::default());
                    doc["skill_sources"].as_array_of_tables_mut().unwrap()
                })
                .push(tbl);
        }

        let text = doc.to_string();
        std::fs::write(&self.path, text)
    }

    /// Merge user sources with built-in defaults. User sources win on
    /// conflict by id.
    pub fn merge_with_defaults(
        &self,
        user_sources: &[SkillSourceView],
    ) -> Vec<SkillSourceView> {
        let defaults = default_skill_sources();
        let mut merged: Vec<SkillSourceView> = Vec::new();
        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        // User sources first (win on conflict).
        for src in user_sources {
            seen_ids.insert(src.id.clone());
            merged.push(src.clone());
        }

        // Defaults fill in gaps.
        for src in &defaults {
            if !seen_ids.contains(&src.id) {
                merged.push(src.clone());
            }
        }

        merged.sort_by_key(|s| s.priority);
        merged
    }
}

/// Built-in skill catalog sources shipped with Kairox.
pub fn default_skill_sources() -> Vec<SkillSourceView> {
    vec![
        SkillSourceView {
            id: "skills-sh".into(),
            display_name: "skills.sh".into(),
            kind: "skills-sh".into(),
            url: "https://skills.sh".into(),
            search_template: "/api/search?q={{query}}&limit={{limit}}".into(),
            list_template: None,
            field_mapping: SkillFieldMappingView::default(),
            enabled: true,
            priority: 0,
            cache_ttl_seconds: 900,
            last_error: None,
        },
        SkillSourceView {
            id: "skillhub".into(),
            display_name: "SkillHub".into(),
            kind: "skillhub".into(),
            url: "https://skills.palebluedot.live".into(),
            search_template: "/api/skills?q={{query}}&limit={{limit}}".into(),
            list_template: Some("/api/skills?limit={{limit}}".into()),
            field_mapping: SkillFieldMappingView::default(),
            enabled: true,
            priority: 1,
            cache_ttl_seconds: 900,
            last_error: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_empty_when_file_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let toml = SkillSourcesToml::new(dir.path());
        assert!(toml.read().is_empty());
    }

    #[test]
    fn write_and_read_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let toml = SkillSourcesToml::new(dir.path());
        let sources = default_skill_sources();
        toml.write(&sources).unwrap();
        let read_back = toml.read();
        assert_eq!(read_back.len(), 2);
        assert_eq!(read_back[0].id, "skills-sh");
        assert_eq!(read_back[1].id, "skillhub");
    }

    #[test]
    fn merge_user_wins_over_default() {
        let dir = tempfile::tempdir().unwrap();
        let toml = SkillSourcesToml::new(dir.path());
        let user = vec![SkillSourceView {
            id: "skills-sh".into(),
            display_name: "Custom skills.sh".into(),
            kind: "skills-sh".into(),
            url: "https://custom.sh".into(),
            search_template: "/api/search?q={{query}}".into(),
            list_template: None,
            field_mapping: SkillFieldMappingView::default(),
            enabled: false,
            priority: 0,
            cache_ttl_seconds: 600,
            last_error: None,
        }];
        let merged = toml.merge_with_defaults(&user);
        let sh = merged.iter().find(|s| s.id == "skills-sh").unwrap();
        assert_eq!(sh.display_name, "Custom skills.sh");
        assert!(!sh.enabled);
        // skillhub should still be present from defaults.
        assert!(merged.iter().any(|s| s.id == "skillhub"));
    }
}
```

- [ ] **Step 2: Add `mod skill_sources_toml;` to `crates/agent-runtime/src/lib.rs`**

Find the module declarations in `crates/agent-runtime/src/lib.rs` and add:

```rust
mod skill_sources_toml;
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p agent-runtime -- skill_sources_toml 2>&1
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime/src/skill_sources_toml.rs crates/agent-runtime/src/lib.rs
git commit -m "feat(runtime): add SkillSourcesToml for skill source config persistence"
```

---

### Task 9: Wire skill catalog into LocalRuntime (AppFacade impl)

**Files:**

- Modify: `crates/agent-runtime/src/facade_runtime.rs`

- [ ] **Step 1: Add fields to `LocalRuntime` struct**

In `crates/agent-runtime/src/facade_runtime.rs`, find the `LocalRuntime` struct (around line 92) and add:

```rust
    // Skill catalog fields (add after existing skill-related fields)
    skill_catalog: Option<Arc<agent_mcp::catalog::skills::aggregate::AggregateSkillCatalogProvider>>,
    skill_sources_toml: Option<SkillSourcesToml>,
    skill_catalog_http: Option<agent_mcp::catalog::remote::http_client::SharedHttpClient>,
    skill_catalog_cache: Option<Arc<agent_mcp::catalog::remote::http_cache::HttpResponseCache>>,
```

Add `use crate::skill_sources_toml::SkillSourcesToml;` at the top.

- [ ] **Step 2: Add builder methods**

```rust
    pub fn with_skill_catalog(
        mut self,
        dir: Option<PathBuf>,
        http: agent_mcp::catalog::remote::http_client::SharedHttpClient,
        cache: Arc<agent_mcp::catalog::remote::http_cache::HttpResponseCache>,
    ) -> Self {
        if let Some(dir) = dir {
            self.skill_sources_toml = Some(SkillSourcesToml::new(&dir));
        }
        self.skill_catalog_http = Some(http);
        self.skill_catalog_cache = Some(cache);
        self
    }
```

- [ ] **Step 3: Add skill catalog init in the constructor or first-use**

In the `build_skill_catalog` helper:

```rust
    fn ensure_skill_catalog(&self) -> Option<Arc<AggregateSkillCatalogProvider>> {
        if let Some(ref catalog) = self.skill_catalog {
            return Some(catalog.clone());
        }
        // Build from disk on first use
        let http = self.skill_catalog_http.clone()?;
        let cache = self.skill_catalog_cache.clone()?;
        let toml = self.skill_sources_toml.as_ref()?;

        let user_sources = toml.read();
        let merged = toml.merge_with_defaults(&user_sources);

        let providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)> = merged
            .into_iter()
            .filter(|s| s.enabled)
            .filter_map(|s| {
                let kind = agent_mcp::catalog::skills::remote::SkillSourceKind::from_str(&s.kind)?;
                let cfg = agent_mcp::catalog::skills::remote::RemoteSkillSourceConfig {
                    id: s.id.clone(),
                    display_name: s.display_name.clone(),
                    kind,
                    url: s.url.clone(),
                    search_template: s.search_template.clone(),
                    list_template: s.list_template.clone(),
                    enabled: s.enabled,
                    priority: s.priority,
                    cache_ttl_seconds: s.cache_ttl_seconds,
                };
                Some((
                    s.priority,
                    agent_mcp::catalog::skills::remote::build_skill_provider(
                        cfg,
                        http.clone(),
                        cache.clone(),
                    ),
                ))
            })
            .collect();

        let agg = Arc::new(AggregateSkillCatalogProvider::new(providers));
        // Store for future use (we use unsafe to get around &self).
        // Actually better: use an ArcSwap or just return it.
        Some(agg)
    }
```

Note: Since `LocalRuntime` methods take `&self` and we can't mutate, use `std::sync::OnceLock` or initialize eagerly. For simplicity, we use `OnceLock` fields.

The actual fields should be:

```rust
    skill_catalog: OnceLock<Arc<AggregateSkillCatalogProvider>>,
    skill_sources_toml: Option<SkillSourcesToml>,
    skill_catalog_http: Option<SharedHttpClient>,
    skill_catalog_cache: Option<Arc<HttpResponseCache>>,
```

- [ ] **Step 4: Implement the AppFacade methods**

After the existing `refresh_catalog` impl (around line 1317), add:

```rust
    // ── Skill catalog ─────────────────────────────────────────────

    async fn list_skill_catalog(
        &self,
        query: CoreSkillCatalogQuery,
    ) -> agent_core::Result<Vec<CoreSkillCatalogEntry>> {
        let catalog = self.ensure_skill_catalog().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog not configured".into())
        })?;

        let inner_query = SkillCatalogQuery {
            keyword: query.keyword,
            sources: query.sources,
            limit: query.limit,
        };

        let entries = catalog
            .search(&inner_query)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("skill catalog: {e}")))?;

        Ok(entries
            .into_iter()
            .map(|e| CoreSkillCatalogEntry {
                catalog_id: e.catalog_id,
                name: e.name,
                description: e.description,
                source: e.source,
                source_url: e.source_url,
                install_count: e.install_count,
                github_stars: e.github_stars,
                security_score: e.security_score,
                rating: e.rating,
                package: e.package,
            })
            .collect())
    }

    async fn list_skill_sources(&self) -> agent_core::Result<Vec<CoreSkillSourceView>> {
        let sources = match &self.skill_sources_toml {
            Some(toml) => toml.merge_with_defaults(&toml.read()),
            None => default_skill_sources(),
        };
        Ok(sources)
    }

    async fn add_skill_source(&self, config: CoreSkillSourceView) -> agent_core::Result<()> {
        let toml = self.skill_sources_toml.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill sources not configured".into())
        })?;
        let mut sources = toml.read();
        sources.retain(|s| s.id != config.id);
        sources.push(config);
        toml.write(&sources)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("write: {e}")))?;
        // Rebuild the aggregate.
        self.rebuild_skill_aggregate()?;
        Ok(())
    }

    async fn remove_skill_source(&self, id: String) -> agent_core::Result<()> {
        let toml = self.skill_sources_toml.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill sources not configured".into())
        })?;
        let mut sources = toml.read();
        sources.retain(|s| s.id != id);
        toml.write(&sources)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("write: {e}")))?;
        self.rebuild_skill_aggregate()?;
        Ok(())
    }

    async fn set_skill_source_enabled(&self, id: String, enabled: bool) -> agent_core::Result<()> {
        let toml = self.skill_sources_toml.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill sources not configured".into())
        })?;
        let mut sources = toml.read();
        if let Some(s) = sources.iter_mut().find(|s| s.id == id) {
            s.enabled = enabled;
        }
        toml.write(&sources)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("write: {e}")))?;
        self.rebuild_skill_aggregate()?;
        Ok(())
    }

    async fn refresh_skill_catalog(&self) -> agent_core::Result<()> {
        let catalog = self.ensure_skill_catalog().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog not configured".into())
        })?;
        catalog
            .refresh()
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("skill catalog refresh: {e}")))?;
        Ok(())
    }
```

Also add the `rebuild_skill_aggregate` helper and `ensure_skill_catalog`:

```rust
    fn rebuild_skill_aggregate(&self) -> agent_core::Result<()> {
        let Some(toml) = &self.skill_sources_toml else {
            return Ok(());
        };
        let http = self.skill_catalog_http.clone().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog http not configured".into())
        })?;
        let cache = self.skill_catalog_cache.clone().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog cache not configured".into())
        })?;
        let sources = toml.merge_with_defaults(&toml.read());
        let providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)> = sources
            .into_iter()
            .filter(|s| s.enabled)
            .filter_map(|s| {
                let kind = agent_mcp::catalog::skills::remote::SkillSourceKind::from_str(&s.kind)?;
                let cfg = agent_mcp::catalog::skills::remote::RemoteSkillSourceConfig {
                    id: s.id.clone(),
                    display_name: s.display_name,
                    kind,
                    url: s.url,
                    search_template: s.search_template,
                    list_template: s.list_template,
                    enabled: s.enabled,
                    priority: s.priority,
                    cache_ttl_seconds: s.cache_ttl_seconds,
                };
                Some((s.priority, agent_mcp::catalog::skills::remote::build_skill_provider(cfg, http.clone(), cache.clone())))
            })
            .collect();
        if let Some(catalog) = self.skill_catalog.get() {
            catalog.reload(providers);
        } else {
            let agg = Arc::new(AggregateSkillCatalogProvider::new(providers));
            let _ = self.skill_catalog.set(agg);
        }
        Ok(())
    }

    fn ensure_skill_catalog(&self) -> Option<Arc<AggregateSkillCatalogProvider>> {
        if let Some(c) = self.skill_catalog.get() {
            return Some(c.clone());
        }
        let _ = self.rebuild_skill_aggregate();
        self.skill_catalog.get().cloned()
    }
```

Also add the necessary imports at the top of facade_runtime.rs.

- [ ] **Step 5: Verify it compiles**

```bash
cargo check -p agent-runtime 2>&1
```

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime/src/facade_runtime.rs
git commit -m "feat(runtime): wire skill catalog into LocalRuntime AppFacade impl"
```

---

### Task 10: Add Tauri commands for skill catalog

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`

- [ ] **Step 1: Add 6 new Tauri commands after the existing skill commands in commands.rs (after line 1254)**

```rust
// ── Skill catalog ────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
async fn list_skill_catalog(
    state: tauri::State<'_, AppState>,
    query: agent_core::facade::SkillCatalogQuery,
) -> Result<Vec<agent_core::facade::SkillCatalogEntry>, String> {
    state
        .runtime
        .list_skill_catalog(query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
async fn list_skill_sources(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<agent_core::facade::SkillSourceView>, String> {
    state
        .runtime
        .list_skill_sources()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
async fn add_skill_source(
    state: tauri::State<'_, AppState>,
    config: agent_core::facade::SkillSourceView,
) -> Result<(), String> {
    state
        .runtime
        .add_skill_source(config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
async fn remove_skill_source(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state
        .runtime
        .remove_skill_source(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
async fn set_skill_source_enabled(
    state: tauri::State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_skill_source_enabled(id, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
async fn refresh_skill_catalog(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .runtime
        .refresh_skill_catalog()
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register commands in lib.rs**

In `apps/agent-gui/src-tauri/src/lib.rs`, add the new commands to both macros:

- Add to `generate_handler!` list
- Add to `collect_commands!` list (in `src/specta.rs`)

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p agent-gui 2>&1
```

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/lib.rs
git commit -m "feat(gui): add Tauri commands for skill catalog operations"
```

---

### Task 11: Regenerate TypeScript types

**Files:**

- Auto-generated: `apps/agent-gui/src/generated/commands.ts`, `events.ts`

- [ ] **Step 1: Run type generation**

```bash
just gen-types
```

Expected: types regenerated without errors.

- [ ] **Step 2: Verify generated types include new types**

```bash
grep "SkillCatalogEntry\|SkillCatalogQuery\|SkillSourceView\|listSkillCatalog\|listSkillSources" apps/agent-gui/src/generated/commands.ts
```

Expected: TypeScript types and function signatures present.

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/generated/
git commit -m "chore(gui): regenerate TypeScript types for skill catalog"
```

---

### Task 12: Extend useSkillsStore with catalog state and actions

**Files:**

- Modify: `apps/agent-gui/src/stores/skills.ts`

- [ ] **Step 1: Add new imports and state**

After line 8, add the new type imports:

```typescript
import type { SkillCatalogEntry, SkillCatalogQuery, SkillSourceView } from "@/generated/commands";
```

After line 55 (`remoteLoading`), add new state:

```typescript
// Skill catalog / marketplace
const catalogEntries = ref<SkillCatalogEntry[]>([]);
const catalogSources = ref<SkillSourceView[]>([]);
const catalogLoading = ref(false);
const searchCache = ref<Map<string, { entries: SkillCatalogEntry[]; timestamp: number }>>(
  new Map()
);
const CACHE_TTL_MS = 5 * 60 * 1000; // 5 minutes
```

- [ ] **Step 2: Add new actions**

After the `updateSkill` function (line 238), add:

```typescript
function getCacheKey(query: SkillCatalogQuery): string {
  return JSON.stringify({
    keyword: query.keyword ?? "",
    sources: query.sources ?? [],
    limit: query.limit ?? 50
  });
}

async function searchCatalog(query: SkillCatalogQuery): Promise<void> {
  const cacheKey = getCacheKey(query);
  const cached = searchCache.value.get(cacheKey);
  if (cached && Date.now() - cached.timestamp < CACHE_TTL_MS) {
    catalogEntries.value = cached.entries;
    return;
  }

  catalogLoading.value = true;
  error.value = null;
  try {
    const result = await unwrapCommandResult(commands.listSkillCatalog(query));
    catalogEntries.value = result;
    searchCache.value.set(cacheKey, {
      entries: result,
      timestamp: Date.now()
    });
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    catalogLoading.value = false;
  }
}

async function loadCatalogSources(): Promise<void> {
  error.value = null;
  try {
    catalogSources.value = await unwrapCommandResult(commands.listSkillSources());
  } catch (caughtError) {
    error.value = formatError(caughtError);
  }
}

async function toggleCatalogSource(id: string, enabled: boolean): Promise<void> {
  error.value = null;
  try {
    await unwrapCommandResult(commands.setSkillSourceEnabled(id, enabled));
    catalogSources.value = catalogSources.value.map((s) => (s.id === id ? { ...s, enabled } : s));
  } catch (caughtError) {
    error.value = formatError(caughtError);
  }
}

async function addCatalogSource(config: SkillSourceView): Promise<void> {
  error.value = null;
  try {
    await unwrapCommandResult(commands.addSkillSource(config));
    await loadCatalogSources();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  }
}

async function removeCatalogSource(id: string): Promise<void> {
  error.value = null;
  try {
    await unwrapCommandResult(commands.removeSkillSource(id));
    catalogSources.value = catalogSources.value.filter((s) => s.id !== id);
  } catch (caughtError) {
    error.value = formatError(caughtError);
  }
}

async function refreshCatalog(): Promise<void> {
  catalogLoading.value = true;
  error.value = null;
  try {
    await unwrapCommandResult(commands.refreshSkillCatalog());
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    catalogLoading.value = false;
  }
}
```

- [ ] **Step 3: Export new state and actions**

In the return block (after line 265), add:

```typescript
    // catalog
    catalogEntries,
    catalogSources,
    catalogLoading,
    searchCatalog,
    loadCatalogSources,
    toggleCatalogSource,
    addCatalogSource,
    removeCatalogSource,
    refreshCatalog,
```

- [ ] **Step 4: Verify TypeScript compiles**

```bash
pnpm --filter agent-gui run typecheck 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/stores/skills.ts
git commit -m "feat(gui): extend useSkillsStore with catalog state and actions"
```

---

### Task 13: Create SkillDiscoverCard.vue

**Files:**

- Create: `apps/agent-gui/src/components/skills/SkillDiscoverCard.vue`

- [ ] **Step 1: Create the card component**

```vue
<script setup lang="ts">
import type { SkillCatalogEntry } from "@/generated/commands";

defineProps<{ entry: SkillCatalogEntry }>();
defineEmits<{ click: [] }>();

function formatCount(n: number | null | undefined): string {
  if (n == null) return "";
  if (n >= 1000000) return `${(n / 1000000).toFixed(1)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}K`;
  return n.toLocaleString();
}
</script>

<template>
  <button
    type="button"
    class="card-button"
    :data-test="`skill-card-${entry.catalog_id.replace(/[^a-z0-9]+/g, '-')}`"
    @click="$emit('click')"
  >
    <div class="card catalog-card">
      <div class="card-body">
        <div class="card-head">
          <span class="display-name">{{ entry.name }}</span>
          <span v-if="entry.install_count != null" class="tag tag-info"
            >{{ formatCount(entry.install_count) }} installs</span
          >
        </div>
        <span class="summary">{{ entry.description || "No description" }}</span>
        <div class="tags">
          <span v-if="entry.security_score != null" class="tag tag-success">
            Security: {{ entry.security_score }}
          </span>
          <span v-if="entry.rating != null" class="tag tag-warning">
            ★ {{ entry.rating.toFixed(1) }}
          </span>
          <span class="tag">{{ entry.source }}</span>
        </div>
      </div>
    </div>
  </button>
</template>

<style scoped>
.card-button {
  all: unset;
  display: block;
  width: 100%;
  cursor: pointer;
  text-align: left;
}

.card-button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
  border-radius: 6px;
}

.card-button:hover .catalog-card {
  border-color: var(--app-primary-color);
}

.card-head {
  display: flex;
  align-items: center;
  gap: 6px;
}

.display-name {
  font-weight: 600;
}

.summary {
  font-size: 13px;
  display: block;
  color: var(--app-text-color-2);
  margin-top: 4px;
}

.tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  margin-top: 8px;
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/skills/SkillDiscoverCard.vue
git commit -m "feat(gui): add SkillDiscoverCard component"
```

---

### Task 14: Create SkillDiscoverList.vue

**Files:**

- Create: `apps/agent-gui/src/components/skills/SkillDiscoverList.vue`

- [ ] **Step 1: Create the list component**

```vue
<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import SkillDiscoverCard from "./SkillDiscoverCard.vue";

const skillsStore = useSkillsStore();
const keyword = ref("");
const limit = ref(50);

async function doSearch(): Promise<void> {
  await skillsStore.searchCatalog({
    keyword: keyword.value.trim() || null,
    sources: null,
    limit: limit.value
  });
}

onMounted(async () => {
  await skillsStore.loadCatalogSources();
  // Load initial results from all enabled sources.
  await doSearch();
});
</script>

<template>
  <div class="skill-discover-list">
    <div class="filters">
      <input
        v-model="keyword"
        placeholder="Search skills…"
        data-test="skill-catalog-search"
        class="filter-keyword"
        @keyup.enter="doSearch()"
      />
      <button
        class="btn btn-primary btn-sm"
        data-test="skill-catalog-search-btn"
        :disabled="skillsStore.catalogLoading"
        @click="doSearch()"
      >
        Search
      </button>
      <button
        class="btn btn-sm"
        data-test="skill-catalog-refresh"
        @click="skillsStore.refreshCatalog()"
      >
        Refresh
      </button>
    </div>

    <div v-if="skillsStore.catalogLoading" class="loading">
      <span class="spinner" />
      <span class="text-secondary">Loading…</span>
    </div>
    <span v-else-if="skillsStore.error" class="text-error error">
      {{ skillsStore.error }}
    </span>
    <div v-else-if="skillsStore.catalogEntries.length === 0" class="empty-state">
      No skills found. Try a different search term.
    </div>
    <div v-else class="grid">
      <SkillDiscoverCard
        v-for="entry in skillsStore.catalogEntries"
        :key="`${entry.source}:${entry.catalog_id}`"
        :entry="entry"
      />
    </div>
  </div>
</template>

<style scoped>
.skill-discover-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.filters {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
  align-items: center;
}

.filter-keyword {
  flex: 1;
  max-width: 320px;
  min-height: 32px;
  padding: 4px 8px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  color: var(--app-text-color, #111827);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
}

.loading {
  display: flex;
  align-items: center;
  gap: 8px;
}

.text-secondary {
  color: var(--app-text-color-2);
}

.text-error {
  color: var(--app-error-color);
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/skills/SkillDiscoverList.vue
git commit -m "feat(gui): add SkillDiscoverList component"
```

---

### Task 15: Create SkillSourcesSettings.vue

**Files:**

- Create: `apps/agent-gui/src/components/skills/SkillSourcesSettings.vue`

- [ ] **Step 1: Create the sources settings panel**

```vue
<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import type { SkillSourceView } from "@/generated/commands";

const skillsStore = useSkillsStore();
const showAddForm = ref(false);
const formError = ref<string | null>(null);

const draft = ref<SkillSourceView>({
  id: "",
  display_name: "",
  kind: "custom",
  url: "",
  search_template: "/api/search?q={{query}}&limit={{limit}}",
  list_template: null,
  field_mapping: {
    name_path: "name",
    description_path: "description",
    install_count_path: "installs",
    github_stars_path: null,
    package_path: "id",
    source_url_path: null
  },
  enabled: true,
  priority: 100,
  cache_ttl_seconds: 900,
  last_error: null
});

onMounted(() => {
  void skillsStore.loadCatalogSources();
});

function isValidUrl(u: string): boolean {
  return u.startsWith("http://") || u.startsWith("https://");
}

async function save(): Promise<void> {
  formError.value = null;
  if (!draft.value.id || !draft.value.display_name) {
    formError.value = "ID and display name are required.";
    return;
  }
  if (!isValidUrl(draft.value.url)) {
    formError.value = "URL must start with http:// or https://";
    return;
  }
  await skillsStore.addCatalogSource({ ...draft.value });
  showAddForm.value = false;
}
</script>

<template>
  <div class="sources-settings">
    <h4>Skill Sources</h4>

    <div class="sources-list">
      <div v-for="src in skillsStore.catalogSources" :key="src.id" class="source-row">
        <div class="source-info">
          <span class="source-name">{{ src.display_name }}</span>
          <span class="tag">{{ src.kind }}</span>
          <span v-if="src.last_error" class="tag tag-error">Error</span>
        </div>
        <div class="source-actions">
          <label class="toggle-label">
            <input
              type="checkbox"
              :checked="src.enabled"
              @change="
                skillsStore.toggleCatalogSource(src.id, ($event.target as HTMLInputElement).checked)
              "
            />
            {{ src.enabled ? "Enabled" : "Disabled" }}
          </label>
          <button
            v-if="src.kind !== 'skills-sh' && src.kind !== 'skillhub'"
            class="btn btn-danger btn-sm"
            @click="skillsStore.removeCatalogSource(src.id)"
          >
            Remove
          </button>
        </div>
      </div>
    </div>

    <button class="btn btn-sm" data-test="skill-source-add-btn" @click="showAddForm = !showAddForm">
      {{ showAddForm ? "Cancel" : "Add Source" }}
    </button>

    <form
      v-if="showAddForm"
      class="source-form"
      data-test="skill-source-form"
      @submit.prevent="save()"
    >
      <p v-if="formError" class="alert alert-error">{{ formError }}</p>

      <label for="ss-id">ID</label>
      <input id="ss-id" v-model="draft.id" required />

      <label for="ss-name">Display Name</label>
      <input id="ss-name" v-model="draft.display_name" required />

      <label for="ss-url">Base URL</label>
      <input id="ss-url" v-model="draft.url" type="url" required />

      <label for="ss-search">Search Template</label>
      <input id="ss-search" v-model="draft.search_template" />

      <label for="ss-list">List Template (optional)</label>
      <input id="ss-list" v-model="draft.list_template" />

      <button class="btn btn-primary" type="submit">Save Source</button>
    </form>
  </div>
</template>

<style scoped>
.sources-settings {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 16px;
  padding-top: 16px;
  border-top: 1px solid var(--app-border-color);
}

.source-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
}

.source-info {
  display: flex;
  gap: 8px;
  align-items: center;
}

.source-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.toggle-label {
  display: flex;
  gap: 4px;
  align-items: center;
  font-size: 13px;
}

.source-form {
  display: grid;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
}

.source-form label {
  font-weight: 600;
}

.source-form input {
  min-height: 32px;
  padding: 4px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/skills/SkillSourcesSettings.vue
git commit -m "feat(gui): add SkillSourcesSettings component"
```

---

### Task 16: Update SkillSettingsPane.vue Discover tab

**Files:**

- Modify: `apps/agent-gui/src/components/SkillSettingsPane.vue`

- [ ] **Step 1: Replace the Discover sub-tab template**

Replace the current Discover section (lines 262-310) to use the new components:

```vue
<template v-if="activeSubTab === 'discover'">
  <SkillDiscoverList />
  <SkillSourcesSettings />
</template>
```

- [ ] **Step 2: Add imports at the top of the script**

```typescript
import SkillDiscoverList from "@/components/skills/SkillDiscoverList.vue";
import SkillSourcesSettings from "@/components/skills/SkillSourcesSettings.vue";
```

- [ ] **Step 3: Remove unused state and functions from Discover tab**

Remove: `discoverQuery`, `searchRemoteSkills` (these are now in SkillDiscoverList).

- [ ] **Step 4: Verify the frontend builds**

```bash
pnpm --filter agent-gui run build 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/components/SkillSettingsPane.vue
git commit -m "feat(gui): refactor Skills Discover tab with catalog components"
```

---

### Task 17: End-to-end verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test --workspace --all-targets 2>&1 | grep "test result"
```

Expected: only pre-existing lifecycle_integration flaky tests fail.

- [ ] **Step 2: Run format and lint**

```bash
pnpm run format:check && pnpm run lint
```

- [ ] **Step 3: Verify API accessibility matches research**

Open a browser or use curl to confirm both APIs still respond:

```bash
curl -s "https://skills.sh/api/search?q=test&limit=1" | python3 -m json.tool | head -5
curl -s "https://skills.palebluedot.live/api/skills?limit=1" | python3 -m json.tool | head -5
```

- [ ] **Step 4: Commit any remaining changes**

```bash
git add -A
git commit -m "chore: final cleanup for skills marketplace feature"
```
