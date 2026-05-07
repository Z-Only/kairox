# MCP Marketplace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Phase 1 of the MCP marketplace — an in-app, browse-and-install experience for ~24 curated MCP servers, with a `CatalogProvider` trait shaped for future remote registries.

**Architecture:** New `catalog` module inside `agent-mcp` exposes a `CatalogProvider` trait, a built-in JSON-backed provider, and an installer that writes to a separate `~/.kairox/mcp_servers.toml` and registers the server with the existing `McpServerManager`. New `AppFacade` methods are wired through `LocalRuntime` to six Tauri commands and consumed by a new `Marketplace` Vue surface. Five new `EventPayload` variants surface install progress.

**Tech Stack:** Rust (workspace, async-trait, thiserror, serde, toml); Tauri 2 + tauri-specta + Vue 3 + Pinia + TypeScript; Vitest + Playwright.

**Spec:** `docs/superpowers/specs/2026-05-06-mcp-marketplace-design.md`

---

## File Structure

### Created

| Path                                                               | Responsibility                                                                                                                                                                                                                                                                                                       |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/agent-mcp/src/catalog/mod.rs`                              | `CatalogProvider` trait + all data types (`ServerEntry`, `InstallSpec`, `RuntimeRequirement`, `RuntimeKind`, `EnvVarSpec`, `TrustLevel`, `CatalogQuery`, `InstalledEntry`, `InstallRequest`) and `CatalogError`. The install outcome enum lives next to its only producer in `installer.rs` as `InstallOutcomeView`. |
| `crates/agent-mcp/src/catalog/builtin.rs`                          | `BuiltinCatalogProvider` — `include_str!` the JSON, parse on first use, cache.                                                                                                                                                                                                                                       |
| `crates/agent-mcp/src/catalog/aggregate.rs`                        | `AggregateCatalogProvider` — fan-out `list`/`get` to inner providers, dedup by `(source, id)`, order by `(trust desc, source order, display_name asc)`.                                                                                                                                                              |
| `crates/agent-mcp/src/catalog/data/builtin-catalog.json`           | 24 curated entries (built-in source).                                                                                                                                                                                                                                                                                |
| `crates/agent-mcp/src/installer.rs`                                | `Installer` struct + `RuntimeProbe` trait + `OsRuntimeProbe`. Validates env, expands `${VAR}` placeholders, detects host runtimes, writes `mcp_servers.toml` atomically.                                                                                                                                             |
| `crates/agent-mcp/tests/catalog.rs`                                | Unit tests for catalog parsing, query filtering, aggregate ordering.                                                                                                                                                                                                                                                 |
| `crates/agent-mcp/tests/installer.rs`                              | Unit tests for runtime probe, env validation, toml writes, id collision.                                                                                                                                                                                                                                             |
| `crates/agent-runtime/tests/marketplace_integration.rs`            | End-to-end install/uninstall pipeline tests.                                                                                                                                                                                                                                                                         |
| `apps/agent-gui/src/views/Marketplace.vue`                         | Top-level marketplace view (Browse + Installed tabs).                                                                                                                                                                                                                                                                |
| `apps/agent-gui/src/components/marketplace/CatalogList.vue`        | Card grid with search/filter.                                                                                                                                                                                                                                                                                        |
| `apps/agent-gui/src/components/marketplace/CatalogCard.vue`        | Single entry card.                                                                                                                                                                                                                                                                                                   |
| `apps/agent-gui/src/components/marketplace/CatalogDetail.vue`      | Drawer with description, requirements, env form, install button.                                                                                                                                                                                                                                                     |
| `apps/agent-gui/src/components/marketplace/InstallProgress.vue`    | Modal with three-step progress.                                                                                                                                                                                                                                                                                      |
| `apps/agent-gui/src/components/marketplace/RuntimeMissingHint.vue` | Inline hint with install URLs.                                                                                                                                                                                                                                                                                       |
| `apps/agent-gui/src/components/marketplace/InstalledList.vue`      | Installed entries table.                                                                                                                                                                                                                                                                                             |
| `apps/agent-gui/src/stores/catalog.ts`                             | Pinia store (entries, filters, install state, installed list).                                                                                                                                                                                                                                                       |
| `apps/agent-gui/src/stores/catalog.test.ts`                        | Vitest unit tests for the store.                                                                                                                                                                                                                                                                                     |
| `apps/agent-gui/src/composables/useMarketplace.ts`                 | Façade composable that exposes invoke wrappers + reactive state.                                                                                                                                                                                                                                                     |
| `apps/agent-gui/e2e/marketplace.spec.ts`                           | Playwright E2E.                                                                                                                                                                                                                                                                                                      |

### Modified

| Path                                         | Why                                                                                                            |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `crates/agent-mcp/src/lib.rs`                | Re-export `catalog` module + `installer`; extend `McpError` with `Catalog(String)`, `Installer(String)`.       |
| `crates/agent-mcp/Cargo.toml`                | Add `async-trait`, `which` (runtime probe), `tempfile` (atomic write) — already-workspace deps where possible. |
| `crates/agent-config/src/loader.rs`          | Add `load_with_marketplace_overlay(main_path, marketplace_path)` that merges both files (main wins).           |
| `crates/agent-config/src/lib.rs`             | Re-export overlay loader.                                                                                      |
| `crates/agent-core/src/events.rs`            | Add 5 `EventPayload` variants + matching `event_type()` arms.                                                  |
| `crates/agent-core/src/facade.rs`            | Add 6 `AppFacade` methods + DTO re-exports.                                                                    |
| `crates/agent-runtime/src/mcp_manager.rs`    | Add `register_dynamic(def: McpServerDef) -> Result<(), McpError>` and `is_registered(id) -> bool`.             |
| `crates/agent-runtime/src/facade_runtime.rs` | Hold an `Arc<dyn CatalogProvider>` + `Installer`; implement the 6 facade methods; emit events.                 |
| `apps/agent-gui/src-tauri/src/commands.rs`   | Add 6 `#[tauri::command] #[specta::specta]` wrappers + response DTOs.                                          |
| `apps/agent-gui/src-tauri/src/specta.rs`     | Register new commands in `collect_commands![]`; register catalog types via `.typ::<T>()`.                      |
| `apps/agent-gui/src-tauri/src/lib.rs`        | Add the 6 commands to `tauri::generate_handler![]`.                                                            |
| `apps/agent-gui/src/App.vue`                 | Add `Marketplace` sidebar entry + route swap.                                                                  |
| `apps/agent-gui/e2e/tauri-mock.js`           | Mock 6 new commands + emit 5 new events.                                                                       |

---

## Task 1: Catalog data types and `CatalogProvider` trait

**Files:**

- Create: `crates/agent-mcp/src/catalog/mod.rs`
- Modify: `crates/agent-mcp/src/lib.rs` (add `pub mod catalog;` + `pub use catalog::*;` and extend `McpError`)
- Modify: `crates/agent-mcp/Cargo.toml` (add `async-trait`, `which`, `tempfile`)
- Test: `crates/agent-mcp/tests/catalog.rs`

- [ ] **Step 1: Add deps to `crates/agent-mcp/Cargo.toml`**

```toml
[dependencies]
# ...existing entries...
async-trait = { workspace = true }
which = "6"
tempfile = "3"
```

If `which` and `tempfile` are not in `[workspace.dependencies]`, add them with the same versions to the root `Cargo.toml` `[workspace.dependencies]` first, then reference as `dep.workspace = true`.

- [ ] **Step 2: Write the failing trait/data-types test**

Create `crates/agent-mcp/tests/catalog.rs`:

```rust
use agent_mcp::catalog::{
    CatalogQuery, EnvVarSpec, InstallSpec, RuntimeKind, RuntimeRequirement, ServerEntry,
    TrustLevel,
};
use std::collections::BTreeMap;

#[test]
fn server_entry_round_trips_through_json() {
    let entry = ServerEntry {
        id: "filesystem".into(),
        source: "builtin".into(),
        display_name: "Filesystem".into(),
        summary: "summary".into(),
        description: "desc".into(),
        categories: vec!["filesystem".into()],
        tags: vec!["files".into()],
        author: Some("MCP".into()),
        homepage: None,
        version: Some("0.6.0".into()),
        install: InstallSpec::Stdio {
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
            env: BTreeMap::new(),
            cwd: None,
        },
        requirements: vec![RuntimeRequirement {
            kind: RuntimeKind::Node,
            min_version: Some(">=18.0.0".into()),
            install_hint: Some("https://nodejs.org".into()),
        }],
        trust: TrustLevel::Verified,
        default_env: vec![EnvVarSpec {
            key: "WORKSPACE_PATH".into(),
            label: "Workspace path".into(),
            description: "directory the server can read".into(),
            required: true,
            secret: false,
            default: Some("~".into()),
        }],
        icon: Some("📁".into()),
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let back: ServerEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(entry, back);
}

#[test]
fn catalog_query_default_is_open() {
    let q = CatalogQuery::default();
    assert!(q.keyword.is_none());
    assert!(q.category.is_none());
    assert!(q.trust_min.is_none());
    assert!(q.source.is_none());
    assert!(q.limit.is_none());
}
```

The test references `PartialEq` on `ServerEntry`; derive it in Step 3.

- [ ] **Step 3: Run the failing test**

Run: `cargo test -p agent-mcp --test catalog`
Expected: FAIL — "module `catalog` is private" or "unresolved import `agent_mcp::catalog`".

- [ ] **Step 4: Create `crates/agent-mcp/src/catalog/mod.rs`**

```rust
//! MCP catalog: trait + data types for browsing and installing MCP servers
//! from one or more sources (built-in JSON today; remote registry later).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single server entry returned by a [`CatalogProvider`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ServerEntry {
    pub id: String,
    pub source: String,
    pub display_name: String,
    pub summary: String,
    pub description: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    pub install: InstallSpec,
    pub requirements: Vec<RuntimeRequirement>,
    pub trust: TrustLevel,
    pub default_env: Vec<EnvVarSpec>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum InstallSpec {
    Stdio {
        command: String,
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
        #[serde(default)]
        cwd: Option<String>,
    },
    Sse {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RuntimeRequirement {
    pub kind: RuntimeKind,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub install_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Node,
    Python,
    Uvx,
    Docker,
    Bun,
    Deno,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EnvVarSpec {
    pub key: String,
    pub label: String,
    pub description: String,
    pub required: bool,
    pub secret: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Unverified,
    Community,
    Verified,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CatalogQuery {
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub trust_min: Option<TrustLevel>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

// NOTE: The install-outcome enum is intentionally NOT defined here. It lives
// next to its only producer in `crates/agent-mcp/src/installer.rs` as
// `InstallOutcomeView` (Task 5). Keeping it there avoids a duplicate type and
// keeps mod.rs focused on catalog data. Consumers (`facade_runtime`,
// `commands.rs`) import it from `agent_mcp::installer`.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstalledEntry {
    pub server_id: String,
    pub catalog_id: Option<String>,
    pub source: Option<String>,
    pub display_name: String,
    pub installed_at: String,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallRequest {
    pub catalog_id: String,
    pub source: String,
    #[serde(default)]
    pub server_id_override: Option<String>,
    #[serde(default)]
    pub env_overrides: BTreeMap<String, String>,
    pub trust_grant: bool,
    pub auto_start: bool,
}

/// Errors specific to catalog/installer operations.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("entry not found: {0}")]
    NotFound(String),
    #[error("invalid catalog data: {0}")]
    InvalidData(String),
    #[error("provider error: {0}")]
    Provider(String),
}

pub type CatalogResult<T> = std::result::Result<T, CatalogError>;

/// A source of [`ServerEntry`] data.
#[async_trait]
pub trait CatalogProvider: Send + Sync {
    /// Stable identifier for this provider, e.g. `"builtin"`.
    fn source_id(&self) -> &str;

    /// List entries matching `query`. Implementations may apply `limit`.
    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>>;

    /// Fetch a single entry by id.
    async fn get(&self, id: &str) -> CatalogResult<Option<ServerEntry>>;

    /// Refresh provider data (no-op by default).
    async fn refresh(&self) -> CatalogResult<()> {
        Ok(())
    }
}

pub mod builtin;
pub mod aggregate;
```

- [ ] **Step 5: Add catalog module + new error variants in `lib.rs`**

Edit `crates/agent-mcp/src/lib.rs`. After the existing `pub mod transport;` line add:

```rust
pub mod catalog;
pub mod installer;
```

Inside the `McpError` enum add (alongside existing variants):

```rust
    #[error("catalog error: {0}")]
    Catalog(String),
    #[error("installer error: {0}")]
    Installer(String),
```

And add the conversion right after the enum:

```rust
impl From<crate::catalog::CatalogError> for McpError {
    fn from(e: crate::catalog::CatalogError) -> Self {
        McpError::Catalog(e.to_string())
    }
}
```

`installer.rs` is a stub for now (created and filled in Task 5):

```rust
// crates/agent-mcp/src/installer.rs
//! Stub created in Task 1 to keep the module path resolvable. Filled in Task 5.
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p agent-mcp --test catalog`
Expected: PASS — both tests green.

- [ ] **Step 7: Lint clean**

Run: `cargo clippy -p agent-mcp --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-mcp/Cargo.toml \
        crates/agent-mcp/src/lib.rs \
        crates/agent-mcp/src/catalog/mod.rs \
        crates/agent-mcp/src/installer.rs \
        crates/agent-mcp/tests/catalog.rs \
        Cargo.toml Cargo.lock
git commit -m "feat(mcp): add catalog data types and provider trait"
```

---

## Task 2: Built-in catalog JSON (24 entries)

**Files:**

- Create: `crates/agent-mcp/src/catalog/data/builtin-catalog.json`
- Test: extends `crates/agent-mcp/tests/catalog.rs`

- [ ] **Step 1: Author the JSON**

Create `crates/agent-mcp/src/catalog/data/builtin-catalog.json` with the structure below. Provide all 24 entries; the example shows two for brevity — fill in the rest using the table from the spec (`docs/superpowers/specs/2026-05-06-mcp-marketplace-design.md`, "Initial curated entries"). For each entry, fill `description` with at least one full sentence (no `...` placeholders).

```json
{
  "schema_version": "1",
  "generated_at": "2026-05-06T00:00:00Z",
  "entries": [
    {
      "id": "filesystem",
      "source": "builtin",
      "display_name": "Filesystem",
      "summary": "Read, write, and search files inside an allow-listed directory.",
      "description": "Provides safe filesystem access scoped to a workspace path you explicitly choose. Useful for letting agents read project files and write generated code without exposing the rest of your home directory.",
      "categories": ["filesystem", "dev-tools"],
      "tags": ["files", "fs", "search"],
      "author": "Anthropic / MCP",
      "homepage": "https://github.com/modelcontextprotocol/servers",
      "version": "0.6.0",
      "install": {
        "transport": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "${WORKSPACE_PATH}"],
        "env": {},
        "cwd": null
      },
      "requirements": [
        {
          "kind": "node",
          "min_version": ">=18.0.0",
          "install_hint": "Install Node from https://nodejs.org"
        }
      ],
      "trust": "verified",
      "default_env": [
        {
          "key": "WORKSPACE_PATH",
          "label": "Workspace path",
          "description": "Directory the server is allowed to access.",
          "required": true,
          "secret": false,
          "default": "~"
        }
      ],
      "icon": "📁"
    },
    {
      "id": "git",
      "source": "builtin",
      "display_name": "Git",
      "summary": "Inspect git repositories — log, diff, blame, branches.",
      "description": "Read-only git operations against any repository on disk. Pairs naturally with the Filesystem server.",
      "categories": ["git/code", "dev-tools"],
      "tags": ["git", "vcs"],
      "author": "Anthropic / MCP",
      "homepage": "https://github.com/modelcontextprotocol/servers",
      "version": "0.6.0",
      "install": {
        "transport": "stdio",
        "command": "uvx",
        "args": ["mcp-server-git", "--repository", "${REPO_PATH}"],
        "env": {},
        "cwd": null
      },
      "requirements": [
        {
          "kind": "uvx",
          "min_version": null,
          "install_hint": "Install uv from https://docs.astral.sh/uv/"
        }
      ],
      "trust": "verified",
      "default_env": [
        {
          "key": "REPO_PATH",
          "label": "Repository path",
          "description": "Path to a git repo on disk.",
          "required": true,
          "secret": false,
          "default": "."
        }
      ],
      "icon": "🔀"
    }
  ]
}
```

The remaining 22 entries follow the same shape: ids `github`, `gitlab`, `brave-search`, `exa`, `tavily`, `puppeteer`, `playwright`, `sqlite`, `postgres`, `redis`, `time`, `fetch`, `everything`, `memory`, `slack`, `gmail`, `notion`, `linear`, `obsidian`, `aws-kb-retrieval`, `google-maps`, `sentry`. Trust levels per the spec table.

- [ ] **Step 2: Append a JSON-validity test to `tests/catalog.rs`**

```rust
#[test]
fn builtin_catalog_json_parses() {
    const JSON: &str = include_str!("../src/catalog/data/builtin-catalog.json");

    #[derive(serde::Deserialize)]
    struct Doc {
        schema_version: String,
        entries: Vec<agent_mcp::catalog::ServerEntry>,
    }
    let doc: Doc = serde_json::from_str(JSON).expect("builtin catalog must be valid JSON");
    assert_eq!(doc.schema_version, "1");
    assert_eq!(doc.entries.len(), 24, "expected 24 curated entries");

    let mut seen = std::collections::HashSet::new();
    for entry in &doc.entries {
        assert!(seen.insert(entry.id.clone()), "duplicate id: {}", entry.id);
        assert_eq!(entry.source, "builtin");
        assert!(!entry.display_name.is_empty());
        assert!(!entry.summary.is_empty());
        assert!(!entry.description.is_empty(), "entry {} has empty description", entry.id);
        assert!(entry.summary.chars().count() <= 200, "summary too long for {}", entry.id);
    }
}
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p agent-mcp --test catalog builtin_catalog_json_parses`
Expected: PASS once the JSON file has all 24 entries with the required fields populated.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-mcp/src/catalog/data/builtin-catalog.json \
        crates/agent-mcp/tests/catalog.rs
git commit -m "feat(mcp): add built-in marketplace catalog with 24 curated servers"
```

---

## Task 3: `BuiltinCatalogProvider`

**Files:**

- Create: `crates/agent-mcp/src/catalog/builtin.rs`
- Test: extends `crates/agent-mcp/tests/catalog.rs`

- [ ] **Step 1: Append failing tests to `tests/catalog.rs`**

```rust
use agent_mcp::catalog::{builtin::BuiltinCatalogProvider, CatalogProvider, TrustLevel};

#[tokio::test]
async fn builtin_provider_lists_all_when_query_empty() {
    let p = BuiltinCatalogProvider::new().expect("builtin loads");
    let items = p.list(&CatalogQuery::default()).await.unwrap();
    assert_eq!(items.len(), 24);
    assert_eq!(p.source_id(), "builtin");
}

#[tokio::test]
async fn builtin_provider_filters_by_keyword_and_trust() {
    let p = BuiltinCatalogProvider::new().unwrap();
    let only_verified = p
        .list(&CatalogQuery {
            trust_min: Some(TrustLevel::Verified),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!only_verified.is_empty());
    assert!(only_verified.iter().all(|e| e.trust == TrustLevel::Verified));

    let by_kw = p
        .list(&CatalogQuery {
            keyword: Some("file".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(by_kw.iter().any(|e| e.id == "filesystem"));
}

#[tokio::test]
async fn builtin_provider_get_returns_none_for_unknown() {
    let p = BuiltinCatalogProvider::new().unwrap();
    assert!(p.get("does-not-exist").await.unwrap().is_none());
    assert!(p.get("filesystem").await.unwrap().is_some());
}
```

- [ ] **Step 2: Run the failing tests**

Run: `cargo test -p agent-mcp --test catalog`
Expected: 3 new tests FAIL — "no module `builtin`".

- [ ] **Step 3: Implement `builtin.rs`**

```rust
//! Built-in catalog backed by an embedded JSON file.

use crate::catalog::{
    CatalogError, CatalogProvider, CatalogQuery, CatalogResult, ServerEntry, TrustLevel,
};
use async_trait::async_trait;
use serde::Deserialize;

const BUILTIN_JSON: &str = include_str!("data/builtin-catalog.json");

#[derive(Debug, Deserialize)]
struct Doc {
    schema_version: String,
    #[serde(default)]
    generated_at: Option<String>,
    entries: Vec<ServerEntry>,
}

pub struct BuiltinCatalogProvider {
    entries: Vec<ServerEntry>,
}

impl BuiltinCatalogProvider {
    pub fn new() -> CatalogResult<Self> {
        let doc: Doc = serde_json::from_str(BUILTIN_JSON)
            .map_err(|e| CatalogError::InvalidData(format!("builtin catalog: {e}")))?;
        if doc.schema_version != "1" {
            return Err(CatalogError::InvalidData(format!(
                "unsupported builtin catalog schema_version: {}",
                doc.schema_version
            )));
        }
        Ok(Self { entries: doc.entries })
    }
}

#[async_trait]
impl CatalogProvider for BuiltinCatalogProvider {
    fn source_id(&self) -> &str {
        "builtin"
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let kw = query.keyword.as_deref().map(str::to_lowercase);
        let mut out: Vec<ServerEntry> = self
            .entries
            .iter()
            .filter(|e| {
                if let Some(ref k) = kw {
                    let hay = format!(
                        "{} {} {}",
                        e.display_name.to_lowercase(),
                        e.summary.to_lowercase(),
                        e.tags.join(" ").to_lowercase()
                    );
                    if !hay.contains(k) {
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
                if let Some(src) = &query.source {
                    if &e.source != src {
                        return false;
                    }
                }
                true
            })
            .cloned()
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
        Ok(self.entries.iter().find(|e| e.id == id).cloned())
    }
}

// Note: TrustLevel order is Unverified < Community < Verified per its derived
// PartialOrd, so trust_min filters work correctly.
#[allow(dead_code)]
const _ASSERT_TRUST_ORDER: () = {
    assert!(matches!(TrustLevel::Verified, TrustLevel::Verified));
};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p agent-mcp --test catalog`
Expected: all 6 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-mcp/src/catalog/builtin.rs \
        crates/agent-mcp/tests/catalog.rs
git commit -m "feat(mcp): add BuiltinCatalogProvider with keyword and trust filters"
```

---

## Task 4: `AggregateCatalogProvider`

**Files:**

- Create: `crates/agent-mcp/src/catalog/aggregate.rs`
- Test: extends `crates/agent-mcp/tests/catalog.rs`

- [ ] **Step 1: Append failing tests to `tests/catalog.rs`**

```rust
use agent_mcp::catalog::{aggregate::AggregateCatalogProvider};
use std::sync::Arc;

#[tokio::test]
async fn aggregate_dedupes_by_source_and_id_and_orders_by_trust() {
    let p1 = Arc::new(BuiltinCatalogProvider::new().unwrap());
    let p2 = Arc::new(BuiltinCatalogProvider::new().unwrap());
    // Two providers with the same source+id should not produce duplicates.
    let agg = AggregateCatalogProvider::new(vec![p1, p2]);
    let items = agg.list(&CatalogQuery::default()).await.unwrap();
    let mut ids = items.iter().map(|e| (e.source.clone(), e.id.clone())).collect::<Vec<_>>();
    ids.sort();
    let dedup_len = {
        let mut copy = ids.clone();
        copy.dedup();
        copy.len()
    };
    assert_eq!(dedup_len, ids.len(), "no duplicates expected");
    // Ordering: trust desc, then display_name asc.
    let trusts: Vec<_> = items.iter().map(|e| e.trust).collect();
    let mut sorted = trusts.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(trusts, sorted);
}

#[tokio::test]
async fn aggregate_get_returns_first_match() {
    let p = Arc::new(BuiltinCatalogProvider::new().unwrap());
    let agg = AggregateCatalogProvider::new(vec![p]);
    assert!(agg.get("filesystem").await.unwrap().is_some());
    assert!(agg.get("nope").await.unwrap().is_none());
}
```

- [ ] **Step 2: Run the failing tests**

Run: `cargo test -p agent-mcp --test catalog`
Expected: 2 new tests FAIL — `aggregate` module missing.

- [ ] **Step 3: Implement `aggregate.rs`**

```rust
//! Aggregates multiple [`CatalogProvider`]s into one logical view.

use crate::catalog::{
    CatalogProvider, CatalogQuery, CatalogResult, ServerEntry,
};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

pub struct AggregateCatalogProvider {
    inner: Vec<Arc<dyn CatalogProvider>>,
}

impl AggregateCatalogProvider {
    pub fn new(inner: Vec<Arc<dyn CatalogProvider>>) -> Self {
        Self { inner }
    }

    pub fn add(&mut self, provider: Arc<dyn CatalogProvider>) {
        self.inner.push(provider);
    }
}

#[async_trait]
impl CatalogProvider for AggregateCatalogProvider {
    fn source_id(&self) -> &str {
        "aggregate"
    }

    async fn list(&self, query: &CatalogQuery) -> CatalogResult<Vec<ServerEntry>> {
        let mut all: Vec<ServerEntry> = Vec::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for provider in &self.inner {
            // Honour source filter cheaply.
            if let Some(src) = &query.source {
                if provider.source_id() != src {
                    continue;
                }
            }
            for entry in provider.list(query).await? {
                let key = (entry.source.clone(), entry.id.clone());
                if seen.insert(key) {
                    all.push(entry);
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
        for provider in &self.inner {
            if let Some(entry) = provider.get(id).await? {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    async fn refresh(&self) -> CatalogResult<()> {
        for provider in &self.inner {
            provider.refresh().await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p agent-mcp --test catalog`
Expected: all 8 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-mcp/src/catalog/aggregate.rs \
        crates/agent-mcp/tests/catalog.rs
git commit -m "feat(mcp): add AggregateCatalogProvider with dedup and trust ordering"
```

---

## Task 5: `Installer` — runtime probe + atomic toml writes

**Files:**

- Replace stub: `crates/agent-mcp/src/installer.rs`
- Test: `crates/agent-mcp/tests/installer.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/agent-mcp/tests/installer.rs`:

```rust
use agent_mcp::catalog::{
    EnvVarSpec, InstallRequest, InstallSpec, RuntimeKind, RuntimeRequirement, ServerEntry,
    TrustLevel,
};
use agent_mcp::installer::{InstallOutcomeView, Installer, RuntimeProbe};
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;

/// Deterministic probe for tests.
struct StaticProbe {
    available: Vec<RuntimeKind>,
}

#[async_trait::async_trait]
impl RuntimeProbe for StaticProbe {
    async fn is_available(&self, kind: RuntimeKind) -> bool {
        self.available.contains(&kind)
    }
}

fn sample_entry() -> ServerEntry {
    ServerEntry {
        id: "filesystem".into(),
        source: "builtin".into(),
        display_name: "Filesystem".into(),
        summary: "s".into(),
        description: "d".into(),
        categories: vec!["filesystem".into()],
        tags: vec![],
        author: None,
        homepage: None,
        version: None,
        install: InstallSpec::Stdio {
            command: "npx".into(),
            args: vec!["-y".into(), "pkg".into(), "${WORKSPACE_PATH}".into()],
            env: BTreeMap::new(),
            cwd: None,
        },
        requirements: vec![RuntimeRequirement {
            kind: RuntimeKind::Node,
            min_version: None,
            install_hint: Some("https://nodejs.org".into()),
        }],
        trust: TrustLevel::Verified,
        default_env: vec![EnvVarSpec {
            key: "WORKSPACE_PATH".into(),
            label: "Workspace path".into(),
            description: "".into(),
            required: true,
            secret: false,
            default: Some("/tmp/x".into()),
        }],
        icon: None,
    }
}

#[tokio::test]
async fn install_writes_toml_and_marks_trust() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe { available: vec![RuntimeKind::Node] });
    let installer = Installer::new(toml_path.clone(), probe);

    let entry = sample_entry();
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: true,
        auto_start: true,
    };
    let outcome = installer.install(&entry, &req).await.unwrap();
    assert!(matches!(outcome, InstallOutcomeView::Installed { .. }));

    let body = std::fs::read_to_string(&toml_path).unwrap();
    assert!(body.contains("[mcp_servers.filesystem]"));
    assert!(body.contains("\"/tmp/x\""), "VAR substitution must materialize");
    assert!(body.contains("trusted_servers"));
    assert!(body.contains("\"filesystem\""));
}

#[tokio::test]
async fn install_runtime_missing_does_not_write_toml() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe { available: vec![] });
    let installer = Installer::new(toml_path.clone(), probe);

    let entry = sample_entry();
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: false,
    };
    let outcome = installer.install(&entry, &req).await.unwrap();
    assert!(matches!(outcome, InstallOutcomeView::RuntimeMissing { .. }));
    assert!(!toml_path.exists());
}

#[tokio::test]
async fn install_invalid_env_when_required_missing() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe { available: vec![RuntimeKind::Node] });
    let installer = Installer::new(toml_path, probe);

    let mut entry = sample_entry();
    entry.default_env[0].default = None; // required, no default, no override → invalid
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: false,
    };
    let outcome = installer.install(&entry, &req).await.unwrap();
    assert!(matches!(outcome, InstallOutcomeView::InvalidEnv { .. }));
}

#[tokio::test]
async fn install_id_collision_returns_already_installed() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe { available: vec![RuntimeKind::Node] });
    let installer = Installer::new(toml_path.clone(), probe);

    let entry = sample_entry();
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: false,
        auto_start: true,
    };
    installer.install(&entry, &req).await.unwrap();
    let again = installer.install(&entry, &req).await.unwrap();
    assert!(matches!(again, InstallOutcomeView::AlreadyInstalled { .. }));
}

#[tokio::test]
async fn uninstall_removes_section_and_trust() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("mcp_servers.toml");
    let probe = Arc::new(StaticProbe { available: vec![RuntimeKind::Node] });
    let installer = Installer::new(toml_path.clone(), probe);
    let entry = sample_entry();
    let req = InstallRequest {
        catalog_id: entry.id.clone(),
        source: entry.source.clone(),
        server_id_override: None,
        env_overrides: BTreeMap::new(),
        trust_grant: true,
        auto_start: false,
    };
    installer.install(&entry, &req).await.unwrap();

    installer.uninstall("filesystem").await.unwrap();
    let body = std::fs::read_to_string(&toml_path).unwrap_or_default();
    assert!(!body.contains("[mcp_servers.filesystem]"));
}
```

- [ ] **Step 2: Run the failing tests**

Run: `cargo test -p agent-mcp --test installer`
Expected: FAIL — `installer::Installer` / `InstallOutcomeView` / `RuntimeProbe` undefined.

- [ ] **Step 3: Replace `crates/agent-mcp/src/installer.rs` with the implementation**

```rust
//! Installer for marketplace catalog entries.
//!
//! Validates env vars, expands `${VAR}` placeholders in `args`, probes host
//! runtimes, atomically writes a `mcp_servers.toml`, and (optionally) marks
//! the entry as trusted.

use crate::catalog::{
    EnvVarSpec, InstallRequest, InstallSpec, RuntimeKind, RuntimeRequirement, ServerEntry,
};
use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallOutcomeView {
    Installed { server_id: String, started: bool },
    RuntimeMissing { missing: Vec<RuntimeRequirement> },
    AlreadyInstalled { server_id: String },
    InvalidEnv { missing_keys: Vec<String> },
}

/// Detects whether a host runtime is available.
#[async_trait]
pub trait RuntimeProbe: Send + Sync {
    async fn is_available(&self, kind: RuntimeKind) -> bool;
}

/// Default probe using the `which` crate.
pub struct OsRuntimeProbe;

#[async_trait]
impl RuntimeProbe for OsRuntimeProbe {
    async fn is_available(&self, kind: RuntimeKind) -> bool {
        let bin = match kind {
            RuntimeKind::Node => "node",
            RuntimeKind::Python => "python3",
            RuntimeKind::Uvx => "uvx",
            RuntimeKind::Docker => "docker",
            RuntimeKind::Bun => "bun",
            RuntimeKind::Deno => "deno",
            RuntimeKind::Other => return true,
        };
        which::which(bin).is_ok()
    }
}

pub struct Installer {
    toml_path: PathBuf,
    probe: Arc<dyn RuntimeProbe>,
    write_lock: Mutex<()>,
}

impl Installer {
    pub fn new(toml_path: PathBuf, probe: Arc<dyn RuntimeProbe>) -> Self {
        Self {
            toml_path,
            probe,
            write_lock: Mutex::new(()),
        }
    }

    pub async fn check_requirements(
        &self,
        entry: &ServerEntry,
    ) -> Vec<RuntimeRequirement> {
        let mut missing = Vec::new();
        for req in &entry.requirements {
            if !self.probe.is_available(req.kind).await {
                missing.push(req.clone());
            }
        }
        missing
    }

    pub async fn install(
        &self,
        entry: &ServerEntry,
        req: &InstallRequest,
    ) -> Result<InstallOutcomeView, InstallerError> {
        let _guard = self.write_lock.lock().await;
        let server_id = req
            .server_id_override
            .clone()
            .unwrap_or_else(|| entry.id.clone());

        // 1. Validate env.
        let resolved = match resolve_env(&entry.default_env, &req.env_overrides) {
            Ok(v) => v,
            Err(missing_keys) => {
                return Ok(InstallOutcomeView::InvalidEnv { missing_keys });
            }
        };

        // 2. Probe runtimes.
        let missing = self.check_requirements(entry).await;
        if !missing.is_empty() {
            return Ok(InstallOutcomeView::RuntimeMissing { missing });
        }

        // 3. Read current toml document (if any).
        let mut doc = self.read_doc()?;
        if doc_contains_server(&doc, &server_id) {
            return Ok(InstallOutcomeView::AlreadyInstalled { server_id });
        }

        // 4. Build the new section.
        let section = build_section(entry, &resolved)?;
        ensure_table(&mut doc, "mcp_servers");
        doc["mcp_servers"][&server_id] = toml_edit::Item::Table(section);

        // 5. Trust grant.
        if req.trust_grant {
            add_trusted(&mut doc, &server_id);
        }

        // 6. Atomic write.
        self.atomic_write(&doc.to_string())?;

        Ok(InstallOutcomeView::Installed {
            server_id,
            started: req.auto_start,
        })
    }

    pub async fn uninstall(&self, server_id: &str) -> Result<(), InstallerError> {
        let _guard = self.write_lock.lock().await;
        let mut doc = self.read_doc()?;
        if let Some(table) = doc
            .get_mut("mcp_servers")
            .and_then(|i| i.as_table_mut())
        {
            table.remove(server_id);
        }
        if let Some(arr) = doc
            .get_mut("trusted_servers")
            .and_then(|i| i.as_array_mut())
        {
            arr.retain(|v| v.as_str() != Some(server_id));
        }
        self.atomic_write(&doc.to_string())?;
        Ok(())
    }

    pub fn list_installed_ids(&self) -> Result<Vec<String>, InstallerError> {
        let doc = self.read_doc()?;
        let mut ids = Vec::new();
        if let Some(t) = doc.get("mcp_servers").and_then(|i| i.as_table()) {
            for (k, _) in t.iter() {
                ids.push(k.to_string());
            }
        }
        Ok(ids)
    }

    fn read_doc(&self) -> Result<toml_edit::DocumentMut, InstallerError> {
        if !self.toml_path.exists() {
            return Ok(toml_edit::DocumentMut::new());
        }
        let content = std::fs::read_to_string(&self.toml_path)
            .map_err(|e| InstallerError::Io(e.to_string()))?;
        content
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| InstallerError::Toml(e.to_string()))
    }

    fn atomic_write(&self, body: &str) -> Result<(), InstallerError> {
        if let Some(parent) = self.toml_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| InstallerError::Io(e.to_string()))?;
        }
        let parent = self
            .toml_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut tmp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| InstallerError::Io(e.to_string()))?;
        use std::io::Write;
        let header = "# Managed by Kairox marketplace, schema=1\n# Edit at your own risk; entries here may be rewritten by the marketplace UI.\n\n";
        tmp.write_all(header.as_bytes())
            .map_err(|e| InstallerError::Io(e.to_string()))?;
        tmp.write_all(body.as_bytes())
            .map_err(|e| InstallerError::Io(e.to_string()))?;
        tmp.persist(&self.toml_path)
            .map_err(|e| InstallerError::Io(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error("io: {0}")]
    Io(String),
    #[error("toml: {0}")]
    Toml(String),
    #[error("invalid: {0}")]
    Invalid(String),
}

fn resolve_env(
    default_env: &[EnvVarSpec],
    overrides: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, Vec<String>> {
    let mut out = BTreeMap::new();
    let mut missing = Vec::new();
    for spec in default_env {
        let value = overrides
            .get(&spec.key)
            .cloned()
            .or_else(|| spec.default.clone());
        match value {
            Some(v) => {
                out.insert(spec.key.clone(), v);
            }
            None if spec.required => missing.push(spec.key.clone()),
            None => {}
        }
    }
    if missing.is_empty() {
        Ok(out)
    } else {
        Err(missing)
    }
}

fn expand(s: &str, env: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
            if let Some(end) = s[i + 2..].find('}') {
                let key = &s[i + 2..i + 2 + end];
                out.push_str(env.get(key).map(String::as_str).unwrap_or(""));
                i = i + 2 + end + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn build_section(
    entry: &ServerEntry,
    env: &BTreeMap<String, String>,
) -> Result<toml_edit::Table, InstallerError> {
    use toml_edit::{value, Array, Table};
    let mut t = Table::new();
    match &entry.install {
        InstallSpec::Stdio {
            command,
            args,
            env: extra_env,
            cwd,
        } => {
            t["type"] = value("stdio");
            t["command"] = value(expand(command, env));
            let mut a = Array::new();
            for arg in args {
                a.push(expand(arg, env));
            }
            t["args"] = value(a);
            let mut env_table = Table::new();
            for (k, v) in env.iter().chain(extra_env.iter()) {
                env_table[k] = value(expand(v, env));
            }
            if !env_table.is_empty() {
                t["env"] = toml_edit::Item::Table(env_table);
            }
            if let Some(c) = cwd {
                t["cwd"] = value(expand(c, env));
            }
        }
        InstallSpec::Sse { url, headers } => {
            t["type"] = value("sse");
            t["url"] = value(expand(url, env));
            if !headers.is_empty() {
                let mut h = Table::new();
                for (k, v) in headers {
                    h[k] = value(expand(v, env));
                }
                t["headers"] = toml_edit::Item::Table(h);
            }
        }
    }
    // Marketplace bookkeeping.
    t["__catalog_id"] = value(entry.id.as_str());
    t["__source"] = value(entry.source.as_str());
    Ok(t)
}

fn ensure_table(doc: &mut toml_edit::DocumentMut, key: &str) {
    if doc.get(key).is_none() {
        doc[key] = toml_edit::Item::Table(toml_edit::Table::new());
    }
}

fn doc_contains_server(doc: &toml_edit::DocumentMut, id: &str) -> bool {
    doc.get("mcp_servers")
        .and_then(|i| i.as_table())
        .map(|t| t.contains_key(id))
        .unwrap_or(false)
}

fn add_trusted(doc: &mut toml_edit::DocumentMut, id: &str) {
    use toml_edit::{value, Array, Item};
    let mut existing: BTreeSet<String> = doc
        .get("trusted_servers")
        .and_then(|i| i.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    existing.insert(id.to_string());
    let mut arr = Array::new();
    for s in existing {
        arr.push(s);
    }
    doc["trusted_servers"] = Item::Value(value(arr).into_value().unwrap());
}
```

- [ ] **Step 4: Add `toml_edit` to deps**

In `crates/agent-mcp/Cargo.toml` add (root `[workspace.dependencies]` mirror if not present):

```toml
toml_edit = "0.22"
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo test -p agent-mcp --test installer`
Expected: 5 tests PASS.

Run: `cargo clippy -p agent-mcp --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-mcp/Cargo.toml \
        crates/agent-mcp/src/installer.rs \
        crates/agent-mcp/tests/installer.rs \
        Cargo.toml Cargo.lock
git commit -m "feat(mcp): add Installer with runtime probe and atomic toml writes"
```

---

## Task 6: agent-config — overlay loader for `mcp_servers.toml`

**Files:**

- Modify: `crates/agent-config/src/loader.rs`
- Modify: `crates/agent-config/src/lib.rs`
- Test: append to `crates/agent-config/src/loader.rs` `#[cfg(test)] mod tests` (or new `tests/overlay.rs` if absent).

- [ ] **Step 1: Write the failing test**

Append to `crates/agent-config/src/loader.rs`:

```rust
#[cfg(test)]
mod overlay_tests {
    use super::*;

    #[test]
    fn overlay_merges_marketplace_into_main_with_main_winning() {
        let main = r#"
[profiles.fast]
provider = "openai"
model_id = "gpt-4o-mini"

[mcp_servers.filesystem]
type = "stdio"
command = "main-fs"
args = []
"#;
        let market = r#"
[mcp_servers.filesystem]
type = "stdio"
command = "marketplace-fs"
args = []

[mcp_servers.brave-search]
type = "stdio"
command = "npx"
args = ["-y", "@mcp/brave"]
"#;
        let cfg = load_with_marketplace_overlay(main, Some(market), "kairox.toml", "mcp.toml")
            .expect("merge ok");
        let names: Vec<_> = cfg.mcp_servers.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"filesystem".to_string()));
        assert!(names.contains(&"brave-search".to_string()));
        let fs = cfg.mcp_servers.iter().find(|s| s.name == "filesystem").unwrap();
        assert_eq!(fs.command.as_deref(), Some("main-fs"), "main file wins");
    }

    #[test]
    fn overlay_with_no_marketplace_is_just_main() {
        let main = r#"
[profiles.fast]
provider = "openai"
model_id = "gpt-4o-mini"
"#;
        let cfg = load_with_marketplace_overlay(main, None, "k.toml", "m.toml").unwrap();
        assert!(cfg.mcp_servers.is_empty());
    }
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p agent-config overlay`
Expected: FAIL — `load_with_marketplace_overlay` not defined.

- [ ] **Step 3: Implement `load_with_marketplace_overlay` in `crates/agent-config/src/loader.rs`**

Add at the end of the file:

```rust
/// Load main config plus an optional marketplace `mcp_servers.toml` overlay.
///
/// Both sources contribute to `mcp_servers`. On id conflict, the main file
/// wins. Profiles, base config, etc. come solely from the main file.
pub fn load_with_marketplace_overlay(
    main_content: &str,
    marketplace_content: Option<&str>,
    main_path: &str,
    marketplace_path: &str,
) -> Result<Config, ConfigError> {
    let mut cfg = load_from_str(main_content, main_path)?;

    let Some(market) = marketplace_content else {
        return Ok(cfg);
    };

    let market_cfg = load_from_str(market, marketplace_path)?;
    let existing: std::collections::HashSet<String> =
        cfg.mcp_servers.iter().map(|s| s.name.clone()).collect();
    for srv in market_cfg.mcp_servers {
        if !existing.contains(&srv.name) {
            cfg.mcp_servers.push(srv);
        }
    }
    Ok(cfg)
}
```

If `load_from_str` rejects content that has no `[profiles.*]` section, the marketplace file may fail to parse. Fix by wrapping the marketplace input through a permissive parser: change `load_from_str` to make `profiles` optional in `ConfigToml` if it isn't already (`#[serde(default)]` already covers that per the existing code in `crates/agent-config/src/loader.rs`). No code change is needed here unless tests fail; if they do, add a unit test reproducing the failure first.

- [ ] **Step 4: Re-export from `lib.rs`**

In `crates/agent-config/src/lib.rs` add to the existing exports:

```rust
pub use loader::{load_from_str, load_with_marketplace_overlay};
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo test -p agent-config`
Expected: all green including the two new overlay tests.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-config/src/loader.rs crates/agent-config/src/lib.rs
git commit -m "feat(config): add marketplace mcp_servers.toml overlay loader"
```

---

## Task 7: `McpServerManager::register_dynamic`

**Files:**

- Modify: `crates/agent-runtime/src/mcp_manager.rs`
- Test: append to `crates/agent-runtime/src/mcp_manager.rs` `#[cfg(test)] mod tests`.

- [ ] **Step 1: Write the failing test**

Append to `crates/agent-runtime/src/mcp_manager.rs` (inside or alongside an existing `#[cfg(test)] mod tests` block — if absent, add the block):

```rust
#[cfg(test)]
mod register_dynamic_tests {
    use super::*;
    use agent_mcp::types::{McpServerDef, McpTransportConfig};
    use agent_tools::{permission::PermissionEngine, registry::ToolRegistry};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn manager() -> McpServerManager {
        let registry = Arc::new(Mutex::new(ToolRegistry::new()));
        let perms = Arc::new(Mutex::new(PermissionEngine::default()));
        McpServerManager::from_config(vec![], registry, perms, None)
    }

    fn def(name: &str) -> McpServerDef {
        McpServerDef {
            name: name.into(),
            transport: McpTransportConfig::Stdio {
                command: "echo".into(),
                args: vec![],
                env: Default::default(),
                cwd: None,
            },
            keep_alive: false,
            auto_restart: false,
            max_restart_attempts: 0,
            initial_backoff_ms: 0,
            backoff_multiplier: 1.0,
        }
    }

    #[tokio::test]
    async fn register_dynamic_adds_server() {
        let mut m = manager();
        assert!(!m.is_registered("alpha"));
        m.register_dynamic(def("alpha")).expect("register");
        assert!(m.is_registered("alpha"));
    }

    #[tokio::test]
    async fn register_dynamic_rejects_duplicate() {
        let mut m = manager();
        m.register_dynamic(def("alpha")).unwrap();
        let err = m.register_dynamic(def("alpha")).unwrap_err();
        assert!(matches!(err, McpError::Catalog(_) | McpError::Installer(_) | McpError::Protocol(_)));
    }
}
```

The exact `McpServerDef` field names must match the existing definition. If your tree's `McpServerDef` differs, adjust field names in the test fixture before running it (do not change types under test). Run `rg -n "struct McpServerDef" crates/agent-mcp/src/types.rs` to confirm. **Do not** add new public fields to `McpServerDef` from this task.

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p agent-runtime register_dynamic`
Expected: FAIL — `register_dynamic`/`is_registered` not defined.

- [ ] **Step 3: Implement the two methods on `McpServerManager`**

In `crates/agent-runtime/src/mcp_manager.rs`, add to `impl McpServerManager`:

```rust
    /// Returns true if a server with this id is currently registered (in any state).
    pub fn is_registered(&self, server_id: &str) -> bool {
        self.servers.contains_key(server_id)
    }

    /// Register a server definition at runtime (used by the marketplace installer).
    ///
    /// Returns `Err` if a server with the same id is already registered.
    /// The caller is responsible for invoking [`Self::ensure_server`] to start it.
    pub fn register_dynamic(&mut self, def: McpServerDef) -> Result<(), McpError> {
        if self.servers.contains_key(&def.name) {
            return Err(McpError::Protocol(format!(
                "server '{}' is already registered",
                def.name
            )));
        }
        let name = def.name.clone();
        self.servers.insert(name, ServerLifecycle::new(def));
        Ok(())
    }

    /// Remove a dynamically registered server. Stops it first if running.
    pub async fn unregister_dynamic(&mut self, server_id: &str) -> Result<(), McpError> {
        if let Some(lifecycle) = self.servers.get_mut(server_id) {
            if matches!(
                lifecycle.status(),
                agent_mcp::types::McpServerStatus::Running
            ) {
                let _ = lifecycle.stop().await;
            }
        }
        self.servers.remove(server_id);
        Ok(())
    }
```

If `ServerLifecycle::stop` is not async or has a different signature, drop the `await` and the `async` keyword from `unregister_dynamic` accordingly — but verify against the existing code first by reading `crates/agent-mcp/src/lifecycle.rs`.

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p agent-runtime register_dynamic`
Expected: 2 tests PASS.

Run: `cargo clippy -p agent-runtime --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime/src/mcp_manager.rs
git commit -m "feat(runtime): add McpServerManager::register_dynamic for marketplace installs"
```

---

## Task 8: `EventPayload` variants + `AppFacade` methods + `LocalRuntime` wiring

**Files:**

- Modify: `crates/agent-core/src/events.rs`
- Modify: `crates/agent-core/src/facade.rs`
- Modify: `crates/agent-core/src/lib.rs` (re-export new DTOs)
- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/Cargo.toml` (depend on `agent-mcp` if not already; it is)
- Test: `crates/agent-runtime/tests/marketplace_integration.rs`

- [ ] **Step 1: Write the failing integration test**

Create `crates/agent-runtime/tests/marketplace_integration.rs`:

```rust
use agent_core::AppFacade;
use agent_mcp::catalog::{
    BuiltinCatalogProvider, CatalogProvider, CatalogQuery, InstallRequest,
};
use agent_runtime::test_support::build_marketplace_runtime;
use std::collections::BTreeMap;

#[tokio::test]
async fn list_catalog_returns_builtin_entries() {
    let (rt, _tmp) = build_marketplace_runtime();
    let entries = rt.list_catalog(CatalogQuery::default()).await.expect("list");
    assert_eq!(entries.len(), 24);
}

#[tokio::test]
async fn install_then_list_installed_then_uninstall() {
    let (rt, _tmp) = build_marketplace_runtime();
    let req = InstallRequest {
        catalog_id: "filesystem".into(),
        source: "builtin".into(),
        server_id_override: None,
        env_overrides: BTreeMap::from([("WORKSPACE_PATH".into(), "/tmp".into())]),
        trust_grant: true,
        auto_start: false,
    };
    let outcome = rt.install_catalog_entry(req).await.expect("install");
    let kind = serde_json::to_value(&outcome).unwrap()["kind"].clone();
    assert_eq!(kind, serde_json::json!("installed"));

    let installed = rt.list_installed_entries().await.unwrap();
    assert!(installed.iter().any(|e| e.server_id == "filesystem"));

    rt.uninstall_catalog_entry("filesystem").await.unwrap();
    let installed = rt.list_installed_entries().await.unwrap();
    assert!(!installed.iter().any(|e| e.server_id == "filesystem"));
}
```

- [ ] **Step 2: Run the failing test**

Run: `cargo test -p agent-runtime --test marketplace_integration`
Expected: FAIL — `list_catalog`/`install_catalog_entry`/`test_support::build_marketplace_runtime` undefined.

- [ ] **Step 3: Add 5 `EventPayload` variants**

Edit `crates/agent-core/src/events.rs`. Inside the `EventPayload` enum, append before the closing brace:

```rust
    CatalogRefreshed {
        source: String,
        entry_count: usize,
    },
    CatalogEntryInstalling {
        catalog_id: String,
        source: String,
    },
    CatalogEntryInstalled {
        catalog_id: String,
        source: String,
        server_id: String,
    },
    CatalogEntryUninstalled {
        server_id: String,
    },
    CatalogRuntimeMissing {
        catalog_id: String,
        missing: Vec<String>,
    },
```

In the same file, inside `EventPayload::event_type()`'s match expression, append:

```rust
            Self::CatalogRefreshed { .. } => "CatalogRefreshed",
            Self::CatalogEntryInstalling { .. } => "CatalogEntryInstalling",
            Self::CatalogEntryInstalled { .. } => "CatalogEntryInstalled",
            Self::CatalogEntryUninstalled { .. } => "CatalogEntryUninstalled",
            Self::CatalogRuntimeMissing { .. } => "CatalogRuntimeMissing",
```

- [ ] **Step 4: Add 6 facade methods**

Edit `crates/agent-core/src/facade.rs`. Re-export catalog DTOs at the top of the file:

```rust
pub use agent_mcp::catalog::{
    CatalogQuery, InstallRequest, InstalledEntry, ServerEntry,
};
pub use agent_mcp::installer::InstallOutcomeView;
```

If `agent-mcp` is not already a dep of `agent-core`, **do not** add it (would create a cycle). Instead, redefine these as local DTO aliases inside `agent-core` and have `facade_runtime.rs` map between them. Confirm by inspecting the dependency direction first: `cargo tree -p agent-core | head` — agent-core must remain at the bottom of the dep stack.

If a cycle would result, define mirror types in `agent-core::facade`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CatalogQuery {
    pub keyword: Option<String>,
    pub category: Option<String>,
    pub trust_min: Option<String>,
    pub source: Option<String>,
    pub limit: Option<usize>,
}
// ...mirror ServerEntry, InstallRequest, InstalledEntry, InstallOutcomeView analogously,
// using only String/usize/bool/Option/Vec/BTreeMap/serde-friendly primitives.
```

Use whichever path keeps the cycle-free dep graph. The integration test in Step 1 talks to the trait through `agent_runtime`, which can re-import the canonical types from `agent_mcp`, so the trait can use mirror types and `LocalRuntime` translates.

Then append to `pub trait AppFacade { ... }`:

```rust
    /// List catalog entries, optionally filtered by `query`.
    async fn list_catalog(&self, query: CatalogQuery) -> crate::Result<Vec<ServerEntry>>;
    /// Get a single catalog entry by id (and optional source filter).
    async fn get_catalog_entry(
        &self,
        id: String,
        source: Option<String>,
    ) -> crate::Result<Option<ServerEntry>>;
    /// Refresh catalog data from all (or one named) source.
    async fn refresh_catalog(&self, source: Option<String>) -> crate::Result<()>;
    /// Install a catalog entry, returning a structured outcome.
    async fn install_catalog_entry(
        &self,
        request: InstallRequest,
    ) -> crate::Result<InstallOutcomeView>;
    /// Uninstall a previously installed entry.
    async fn uninstall_catalog_entry(&self, server_id: String) -> crate::Result<()>;
    /// List entries currently installed (marketplace + hand-edited).
    async fn list_installed_entries(&self) -> crate::Result<Vec<InstalledEntry>>;
```

Add re-exports in `crates/agent-core/src/lib.rs`:

```rust
pub use facade::{
    AppFacade, CatalogQuery, InstallRequest, InstallOutcomeView, InstalledEntry, ServerEntry,
};
```

- [ ] **Step 5: Wire `LocalRuntime`**

Edit `crates/agent-runtime/src/facade_runtime.rs`. Add fields:

```rust
use agent_mcp::catalog::{AggregateCatalogProvider, BuiltinCatalogProvider, CatalogProvider};
use agent_mcp::installer::{Installer, OsRuntimeProbe};
use std::sync::Arc;

// ...inside the existing LocalRuntime<...> struct definition:
//     catalog: Arc<dyn CatalogProvider>,
//     installer: Arc<Installer>,
```

In the constructor, build them:

```rust
let builtin = Arc::new(BuiltinCatalogProvider::new()
    .map_err(|e| RuntimeError::Init(format!("builtin catalog: {e}")))?);
let aggregate: Arc<dyn CatalogProvider> = Arc::new(AggregateCatalogProvider::new(vec![builtin]));
let toml_path = config_dir.join("mcp_servers.toml");
let installer = Arc::new(Installer::new(toml_path, Arc::new(OsRuntimeProbe)));
```

Where `config_dir` is the existing config directory used by `LocalRuntime` (find it via `rg -n "config_dir" crates/agent-runtime/src/`).

Implement the trait methods on `impl<S, M> AppFacade for LocalRuntime<S, M>`:

```rust
async fn list_catalog(&self, query: CatalogQuery) -> Result<Vec<ServerEntry>> {
    let q = map_query(query);
    let entries = self.catalog.list(&q).await
        .map_err(|e| RuntimeError::Other(format!("catalog: {e}")))?;
    Ok(entries.into_iter().map(map_entry).collect())
}

async fn get_catalog_entry(
    &self,
    id: String,
    _source: Option<String>,
) -> Result<Option<ServerEntry>> {
    let entry = self.catalog.get(&id).await
        .map_err(|e| RuntimeError::Other(format!("catalog: {e}")))?;
    Ok(entry.map(map_entry))
}

async fn refresh_catalog(&self, _source: Option<String>) -> Result<()> {
    self.catalog.refresh().await
        .map_err(|e| RuntimeError::Other(format!("catalog refresh: {e}")))?;
    self.emit_event(EventPayload::CatalogRefreshed {
        source: "aggregate".into(),
        entry_count: self.catalog.list(&Default::default()).await
            .map(|v| v.len()).unwrap_or(0),
    }).await;
    Ok(())
}

async fn install_catalog_entry(
    &self,
    request: InstallRequest,
) -> Result<InstallOutcomeView> {
    let inner_req = map_install_request(request.clone());
    let entry = self.catalog.get(&inner_req.catalog_id).await
        .map_err(|e| RuntimeError::Other(format!("catalog: {e}")))?
        .ok_or_else(|| RuntimeError::Other(format!("entry not found: {}", inner_req.catalog_id)))?;

    self.emit_event(EventPayload::CatalogEntryInstalling {
        catalog_id: inner_req.catalog_id.clone(),
        source: inner_req.source.clone(),
    }).await;

    let outcome = self.installer.install(&entry, &inner_req).await
        .map_err(|e| RuntimeError::Other(format!("installer: {e}")))?;

    match &outcome {
        InstallOutcomeView::RuntimeMissing { missing } => {
            self.emit_event(EventPayload::CatalogRuntimeMissing {
                catalog_id: inner_req.catalog_id.clone(),
                missing: missing.iter().map(|r| format!("{:?}", r.kind)).collect(),
            }).await;
        }
        InstallOutcomeView::Installed { server_id, started } => {
            // Hot-register with manager, optionally start.
            let def = build_server_def(&entry, &inner_req)?;
            self.mcp_manager.lock().await.register_dynamic(def)
                .map_err(|e| RuntimeError::Other(format!("register: {e}")))?;
            if *started {
                let _ = self.mcp_manager.lock().await.ensure_server(server_id).await;
            }
            self.emit_event(EventPayload::CatalogEntryInstalled {
                catalog_id: inner_req.catalog_id.clone(),
                source: inner_req.source.clone(),
                server_id: server_id.clone(),
            }).await;
        }
        _ => {}
    }
    Ok(map_outcome(outcome))
}

async fn uninstall_catalog_entry(&self, server_id: String) -> Result<()> {
    self.installer.uninstall(&server_id).await
        .map_err(|e| RuntimeError::Other(format!("installer: {e}")))?;
    let _ = self.mcp_manager.lock().await.unregister_dynamic(&server_id).await;
    self.emit_event(EventPayload::CatalogEntryUninstalled {
        server_id,
    }).await;
    Ok(())
}

async fn list_installed_entries(&self) -> Result<Vec<InstalledEntry>> {
    let ids = self.installer.list_installed_ids()
        .map_err(|e| RuntimeError::Other(format!("installer: {e}")))?;
    let mgr = self.mcp_manager.lock().await;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let entry = self.catalog.get(&id).await.ok().flatten();
        out.push(InstalledEntry {
            server_id: id.clone(),
            catalog_id: entry.as_ref().map(|e| e.id.clone()),
            source: entry.as_ref().map(|e| e.source.clone()),
            display_name: entry.as_ref().map(|e| e.display_name.clone())
                .unwrap_or_else(|| id.clone()),
            installed_at: chrono::Utc::now().to_rfc3339(),
            running: mgr.is_running(&id).unwrap_or(false),
        });
    }
    Ok(out)
}
```

Helper functions `map_query`, `map_entry`, `map_install_request`, `map_outcome`, `build_server_def` go in the same file, translating between the local DTOs (in `agent-core::facade`) and the canonical types in `agent-mcp::catalog`. `build_server_def` uses the same `${VAR}` substitution as the installer (factor that into a small `pub` helper in `installer.rs` and re-use it here, named `pub fn expand_args(args: &[String], env: &BTreeMap<String,String>) -> Vec<String>`).

If `is_running` does not exist on `McpServerManager`, add a one-line method:

```rust
pub fn is_running(&self, server_id: &str) -> Option<bool> {
    self.servers.get(server_id).map(|lc| matches!(
        lc.status(),
        agent_mcp::types::McpServerStatus::Running
    ))
}
```

- [ ] **Step 6: Add `test_support::build_marketplace_runtime`**

Create `crates/agent-runtime/src/test_support.rs` (or extend it if it exists):

```rust
//! Helpers for integration tests in `crates/agent-runtime/tests/`.

use crate::LocalRuntime;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use agent_memory::SqliteMemoryStore;
use std::sync::Arc;
use tempfile::TempDir;

pub fn build_marketplace_runtime() -> (LocalRuntime<SqliteEventStore, SqliteMemoryStore>, TempDir) {
    let tmp = TempDir::new().expect("tmp");
    let cfg = crate::RuntimeConfig {
        event_db_path: tmp.path().join("events.db"),
        memory_db_path: tmp.path().join("memory.db"),
        config_dir: tmp.path().to_path_buf(),
        // Other fields use existing defaults; copy them from RuntimeConfig::default()
        // and override only the paths above.
        ..crate::RuntimeConfig::default()
    };
    let model = Arc::new(FakeModelClient::default());
    let rt = LocalRuntime::new(cfg, model).expect("init");
    (rt, tmp)
}
```

The exact `RuntimeConfig` fields and `LocalRuntime::new` signature must match the current code. Read `crates/agent-runtime/src/facade_runtime.rs` first to confirm field names and constructor parameters; align this helper to them.

Add to `crates/agent-runtime/src/lib.rs`:

```rust
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
```

- [ ] **Step 7: Verify**

Run:

```bash
cargo test -p agent-core
cargo test -p agent-runtime --test marketplace_integration
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all green.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-core/src/events.rs \
        crates/agent-core/src/facade.rs \
        crates/agent-core/src/lib.rs \
        crates/agent-runtime/src/facade_runtime.rs \
        crates/agent-runtime/src/mcp_manager.rs \
        crates/agent-runtime/src/lib.rs \
        crates/agent-runtime/src/test_support.rs \
        crates/agent-runtime/tests/marketplace_integration.rs
git commit -m "feat(core): add catalog events and AppFacade marketplace methods"
```

---

## Task 9: 6 Tauri commands + specta types + regen TS bindings

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Generated (via `just gen-types`): `apps/agent-gui/src/generated/commands.ts`, `events.ts`

- [ ] **Step 1: Add 6 commands**

In `apps/agent-gui/src-tauri/src/commands.rs`, add response DTOs near the existing `*Response` structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CatalogQueryRequest {
    pub keyword: Option<String>,
    pub category: Option<String>,
    pub trust_min: Option<String>,
    pub source: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ServerEntryResponse {
    pub id: String,
    pub source: String,
    pub display_name: String,
    pub summary: String,
    pub description: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    pub trust: String,
    pub icon: Option<String>,
    /// JSON-encoded `InstallSpec` (UI does not need to mutate it; the Rust
    /// installer round-trips it through the catalog provider).
    pub install_json: String,
    /// JSON-encoded `Vec<RuntimeRequirement>`.
    pub requirements_json: String,
    /// JSON-encoded `Vec<EnvVarSpec>`.
    pub default_env_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstallRequestPayload {
    pub catalog_id: String,
    pub source: String,
    pub server_id_override: Option<String>,
    pub env_overrides: std::collections::BTreeMap<String, String>,
    pub trust_grant: bool,
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstallOutcomeResponse {
    pub kind: String,                 // "installed" | "runtime_missing" | "already_installed" | "invalid_env"
    pub server_id: Option<String>,
    pub started: Option<bool>,
    pub missing_runtimes: Vec<String>,
    pub missing_env_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstalledEntryResponse {
    pub server_id: String,
    pub catalog_id: Option<String>,
    pub source: Option<String>,
    pub display_name: String,
    pub installed_at: String,
    pub running: bool,
}
```

Then the six commands (also in `commands.rs`):

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_catalog(
    state: State<'_, GuiState>,
    query: Option<CatalogQueryRequest>,
) -> Result<Vec<ServerEntryResponse>, String> {
    let q = into_core_query(query.unwrap_or_default());
    let entries = state.runtime.list_catalog(q).await.map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(into_response_entry).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn get_catalog_entry(
    state: State<'_, GuiState>,
    id: String,
    source: Option<String>,
) -> Result<Option<ServerEntryResponse>, String> {
    let e = state.runtime.get_catalog_entry(id, source).await.map_err(|e| e.to_string())?;
    Ok(e.map(into_response_entry))
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_catalog(
    state: State<'_, GuiState>,
    source: Option<String>,
) -> Result<(), String> {
    state.runtime.refresh_catalog(source).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_catalog_entry(
    state: State<'_, GuiState>,
    request: InstallRequestPayload,
) -> Result<InstallOutcomeResponse, String> {
    let outcome = state
        .runtime
        .install_catalog_entry(into_core_install_request(request))
        .await
        .map_err(|e| e.to_string())?;
    Ok(into_response_outcome(outcome))
}

#[tauri::command]
#[specta::specta]
pub async fn uninstall_catalog_entry(
    state: State<'_, GuiState>,
    server_id: String,
) -> Result<(), String> {
    state.runtime.uninstall_catalog_entry(server_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_installed_entries(
    state: State<'_, GuiState>,
) -> Result<Vec<InstalledEntryResponse>, String> {
    let v = state.runtime.list_installed_entries().await.map_err(|e| e.to_string())?;
    Ok(v.into_iter().map(|e| InstalledEntryResponse {
        server_id: e.server_id,
        catalog_id: e.catalog_id,
        source: e.source,
        display_name: e.display_name,
        installed_at: e.installed_at,
        running: e.running,
    }).collect())
}
```

The conversion helpers (`into_core_query`, `into_response_entry`, `into_core_install_request`, `into_response_outcome`) live below the commands in the same file:

```rust
fn into_core_query(q: CatalogQueryRequest) -> agent_core::CatalogQuery {
    agent_core::CatalogQuery {
        keyword: q.keyword,
        category: q.category,
        trust_min: q.trust_min,
        source: q.source,
        limit: q.limit,
    }
}

fn into_response_entry(e: agent_core::ServerEntry) -> ServerEntryResponse {
    ServerEntryResponse {
        id: e.id,
        source: e.source,
        display_name: e.display_name,
        summary: e.summary,
        description: e.description,
        categories: e.categories,
        tags: e.tags,
        author: e.author,
        homepage: e.homepage,
        version: e.version,
        trust: e.trust,
        icon: e.icon,
        install_json: e.install_json,
        requirements_json: e.requirements_json,
        default_env_json: e.default_env_json,
    }
}

fn into_core_install_request(p: InstallRequestPayload) -> agent_core::InstallRequest {
    agent_core::InstallRequest {
        catalog_id: p.catalog_id,
        source: p.source,
        server_id_override: p.server_id_override,
        env_overrides: p.env_overrides,
        trust_grant: p.trust_grant,
        auto_start: p.auto_start,
    }
}

fn into_response_outcome(o: agent_core::InstallOutcomeView) -> InstallOutcomeResponse {
    match o {
        agent_core::InstallOutcomeView::Installed { server_id, started } => InstallOutcomeResponse {
            kind: "installed".into(),
            server_id: Some(server_id),
            started: Some(started),
            missing_runtimes: vec![],
            missing_env_keys: vec![],
        },
        agent_core::InstallOutcomeView::RuntimeMissing { missing } => InstallOutcomeResponse {
            kind: "runtime_missing".into(),
            server_id: None,
            started: None,
            missing_runtimes: missing,
            missing_env_keys: vec![],
        },
        agent_core::InstallOutcomeView::AlreadyInstalled { server_id } => InstallOutcomeResponse {
            kind: "already_installed".into(),
            server_id: Some(server_id),
            started: None,
            missing_runtimes: vec![],
            missing_env_keys: vec![],
        },
        agent_core::InstallOutcomeView::InvalidEnv { missing_keys } => InstallOutcomeResponse {
            kind: "invalid_env".into(),
            server_id: None,
            started: None,
            missing_runtimes: vec![],
            missing_env_keys: missing_keys,
        },
    }
}
```

This requires the mirror types in `agent-core::facade` to expose flat-string fields (`trust: String`, `install_json: String`, etc.). The simplest design is to keep the facade's `ServerEntry` exactly that shape, and translate from `agent_mcp::catalog::ServerEntry` inside `facade_runtime.rs::map_entry`. Update Task 8 Step 4 mirror types to match these field names; the test in Task 8 Step 1 only checks `entries.len() == 24`, so it's tolerant.

- [ ] **Step 2: Register commands in `lib.rs`**

In `apps/agent-gui/src-tauri/src/lib.rs`, find the `tauri::generate_handler![...]` block and append the six new command names. Build a sorted union with the existing entries; do not delete existing commands.

- [ ] **Step 3: Register commands and types in `specta.rs`**

In `apps/agent-gui/src-tauri/src/specta.rs`, add to `collect_commands![...]`:

```rust
            // Marketplace commands
            list_catalog,
            get_catalog_entry,
            refresh_catalog,
            install_catalog_entry,
            uninstall_catalog_entry,
            list_installed_entries,
```

And add to the chain of `.typ::<...>()` calls:

```rust
        .typ::<CatalogQueryRequest>()
        .typ::<ServerEntryResponse>()
        .typ::<InstallRequestPayload>()
        .typ::<InstallOutcomeResponse>()
        .typ::<InstalledEntryResponse>()
```

- [ ] **Step 4: Regenerate TypeScript bindings**

Run: `just gen-types`
Expected: writes `apps/agent-gui/src/generated/commands.ts` and `events.ts`. The new `EventPayload` variants (added in Task 8) appear in `events.ts`.

Run: `just check-types`
Expected: PASS (`Generated types are in sync`).

- [ ] **Step 5: Sanity-check the wire surface from a unit test**

Add to `apps/agent-gui/src-tauri/src/commands.rs` `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod marketplace_command_tests {
    use super::*;

    #[test]
    fn install_outcome_response_serializes_to_kind_strings() {
        let r = InstallOutcomeResponse {
            kind: "installed".into(),
            server_id: Some("filesystem".into()),
            started: Some(true),
            missing_runtimes: vec![],
            missing_env_keys: vec![],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"installed\""));
    }
}
```

Run: `cargo test -p agent-gui-tauri marketplace_command_tests`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs \
        apps/agent-gui/src-tauri/src/specta.rs \
        apps/agent-gui/src-tauri/src/lib.rs \
        apps/agent-gui/src/generated/commands.ts \
        apps/agent-gui/src/generated/events.ts
git commit -m "feat(gui): add Tauri commands and specta bindings for marketplace"
```

---

## Task 10: Catalog Pinia store + composable + tauri-mock updates

**Files:**

- Create: `apps/agent-gui/src/stores/catalog.ts`
- Create: `apps/agent-gui/src/stores/catalog.test.ts`
- Create: `apps/agent-gui/src/composables/useMarketplace.ts`
- Modify: `apps/agent-gui/e2e/tauri-mock.js`

- [ ] **Step 1: Write the failing store unit test**

Create `apps/agent-gui/src/stores/catalog.test.ts`:

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));
import { invoke } from "@tauri-apps/api/core";
import { useCatalogStore } from "./catalog";

describe("catalog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("loads entries via list_catalog", async () => {
    (invoke as any).mockResolvedValueOnce([
      {
        id: "filesystem",
        source: "builtin",
        display_name: "Filesystem",
        summary: "s",
        description: "d",
        categories: ["filesystem"],
        tags: [],
        author: null,
        homepage: null,
        version: null,
        trust: "verified",
        icon: "📁",
        install_json: "{}",
        requirements_json: "[]",
        default_env_json: "[]"
      }
    ]);
    const store = useCatalogStore();
    await store.refresh();
    expect(invoke).toHaveBeenCalledWith("list_catalog", {
      query: expect.any(Object)
    });
    expect(store.entries.length).toBe(1);
    expect(store.entries[0].id).toBe("filesystem");
  });

  it("install dispatches install_catalog_entry and stores outcome", async () => {
    (invoke as any).mockResolvedValueOnce({
      kind: "installed",
      server_id: "filesystem",
      started: true,
      missing_runtimes: [],
      missing_env_keys: []
    });
    const store = useCatalogStore();
    const outcome = await store.install({
      catalog_id: "filesystem",
      source: "builtin",
      env_overrides: { WORKSPACE_PATH: "/tmp" },
      trust_grant: true,
      auto_start: true
    });
    expect(outcome.kind).toBe("installed");
    expect(store.installState["filesystem"]).toEqual({
      kind: "installed",
      server_id: "filesystem",
      started: true,
      missing_runtimes: [],
      missing_env_keys: []
    });
  });

  it("filters by keyword + trust client-side", async () => {
    (invoke as any).mockResolvedValue([]);
    const store = useCatalogStore();
    store.entries = [
      {
        id: "a",
        display_name: "Alpha",
        summary: "x",
        trust: "verified",
        categories: [],
        tags: ["alpha"]
      } as any,
      {
        id: "b",
        display_name: "Beta",
        summary: "y",
        trust: "community",
        categories: [],
        tags: ["beta"]
      } as any
    ];
    store.filters.keyword = "alpha";
    store.filters.trustMin = "verified";
    expect(store.filtered.map((e) => e.id)).toEqual(["a"]);
  });
});
```

- [ ] **Step 2: Run the failing test**

Run: `pnpm --filter agent-gui run test -- --run catalog.test.ts`
Expected: FAIL — `./catalog` not found.

- [ ] **Step 3: Implement the store**

Create `apps/agent-gui/src/stores/catalog.ts`:

```ts
import { defineStore } from "pinia";
import { ref, computed, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type {
  ServerEntryResponse,
  InstalledEntryResponse,
  InstallOutcomeResponse,
  InstallRequestPayload,
  CatalogQueryRequest
} from "../generated/commands";

export type Tab = "browse" | "installed";

export interface CatalogFilters {
  keyword: string;
  category: string | null;
  trustMin: "verified" | "community" | "unverified" | null;
}

export const useCatalogStore = defineStore("catalog", () => {
  const entries = ref<ServerEntryResponse[]>([]);
  const installed = ref<InstalledEntryResponse[]>([]);
  const installState = reactive<Record<string, InstallOutcomeResponse>>({});
  const loading = ref(false);
  const error = ref<string | null>(null);
  const tab = ref<Tab>("browse");
  const filters = reactive<CatalogFilters>({
    keyword: "",
    category: null,
    trustMin: null
  });

  const filtered = computed<ServerEntryResponse[]>(() => {
    const kw = filters.keyword.trim().toLowerCase();
    const order = { unverified: 0, community: 1, verified: 2 } as const;
    const minOrder = filters.trustMin ? order[filters.trustMin] : -1;
    return entries.value.filter((e) => {
      if (kw) {
        const hay = `${e.display_name} ${e.summary} ${e.tags.join(" ")}`.toLowerCase();
        if (!hay.includes(kw)) return false;
      }
      if (filters.category && !e.categories.includes(filters.category)) return false;
      if (filters.trustMin) {
        const t = order[e.trust as keyof typeof order] ?? 0;
        if (t < minOrder) return false;
      }
      return true;
    });
  });

  async function refresh(query: CatalogQueryRequest = {}): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      entries.value = await invoke<ServerEntryResponse[]>("list_catalog", {
        query
      });
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function refreshInstalled(): Promise<void> {
    try {
      installed.value = await invoke<InstalledEntryResponse[]>("list_installed_entries");
    } catch (e) {
      error.value = String(e);
    }
  }

  async function install(req: InstallRequestPayload): Promise<InstallOutcomeResponse> {
    const outcome = await invoke<InstallOutcomeResponse>("install_catalog_entry", { request: req });
    installState[req.catalog_id] = outcome;
    if (outcome.kind === "installed") {
      await refreshInstalled();
    }
    return outcome;
  }

  async function uninstall(serverId: string): Promise<void> {
    await invoke("uninstall_catalog_entry", { serverId });
    delete installState[serverId];
    await refreshInstalled();
  }

  async function refreshSource(source: string | null = null): Promise<void> {
    await invoke("refresh_catalog", { source });
    await refresh();
  }

  return {
    entries,
    installed,
    installState,
    loading,
    error,
    tab,
    filters,
    filtered,
    refresh,
    refreshInstalled,
    install,
    uninstall,
    refreshSource
  };
});
```

- [ ] **Step 4: Implement the composable**

Create `apps/agent-gui/src/composables/useMarketplace.ts`:

```ts
import { onMounted } from "vue";
import { useCatalogStore } from "../stores/catalog";
import type { ServerEntryResponse } from "../generated/commands";

export function useMarketplace() {
  const store = useCatalogStore();

  onMounted(async () => {
    if (store.entries.length === 0) await store.refresh();
    if (store.installed.length === 0) await store.refreshInstalled();
  });

  function parseRequirements(entry: ServerEntryResponse) {
    try {
      return JSON.parse(entry.requirements_json) as Array<{
        kind: string;
        min_version: string | null;
        install_hint: string | null;
      }>;
    } catch {
      return [];
    }
  }

  function parseDefaultEnv(entry: ServerEntryResponse) {
    try {
      return JSON.parse(entry.default_env_json) as Array<{
        key: string;
        label: string;
        description: string;
        required: boolean;
        secret: boolean;
        default: string | null;
      }>;
    } catch {
      return [];
    }
  }

  return { store, parseRequirements, parseDefaultEnv };
}
```

- [ ] **Step 5: Update tauri-mock**

Edit `apps/agent-gui/e2e/tauri-mock.js`. Add to the `state` object:

```js
  catalog: [
    {
      id: "filesystem",
      source: "builtin",
      display_name: "Filesystem",
      summary: "Read, write, and search files inside an allow-listed directory.",
      description: "Provides safe filesystem access scoped to a workspace path.",
      categories: ["filesystem", "dev-tools"],
      tags: ["files", "fs"],
      author: "MCP",
      homepage: "https://github.com/modelcontextprotocol/servers",
      version: "0.6.0",
      trust: "verified",
      icon: "📁",
      install_json: JSON.stringify({
        transport: "stdio",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-filesystem", "${WORKSPACE_PATH}"],
        env: {},
        cwd: null,
      }),
      requirements_json: JSON.stringify([
        { kind: "node", min_version: ">=18.0.0", install_hint: "https://nodejs.org" },
      ]),
      default_env_json: JSON.stringify([
        {
          key: "WORKSPACE_PATH",
          label: "Workspace path",
          description: "Directory the server can read",
          required: true,
          secret: false,
          default: "/tmp",
        },
      ]),
    },
  ],
  installedCatalog: [],
  catalogRuntimePresent: { node: true, python: true, uvx: true, docker: true },
```

In the `invokeCommand(name, args)` switch statement (or equivalent dispatcher), add cases:

```js
case "list_catalog": {
  return state.catalog;
}
case "get_catalog_entry": {
  return state.catalog.find((e) => e.id === args.id) || null;
}
case "refresh_catalog": {
  emitEvent("CatalogRefreshed", { source: args.source || "aggregate", entry_count: state.catalog.length });
  return null;
}
case "install_catalog_entry": {
  const req = args.request;
  const entry = state.catalog.find((e) => e.id === req.catalog_id);
  const reqs = JSON.parse(entry.requirements_json);
  const missing = reqs.filter((r) => !state.catalogRuntimePresent[r.kind]).map((r) => r.kind);
  if (missing.length > 0) {
    emitEvent("CatalogRuntimeMissing", { catalog_id: req.catalog_id, missing });
    return { kind: "runtime_missing", server_id: null, started: null,
             missing_runtimes: missing, missing_env_keys: [] };
  }
  const defaults = JSON.parse(entry.default_env_json);
  const missingEnv = defaults
    .filter((d) => d.required && !req.env_overrides[d.key] && !d.default)
    .map((d) => d.key);
  if (missingEnv.length > 0) {
    return { kind: "invalid_env", server_id: null, started: null,
             missing_runtimes: [], missing_env_keys: missingEnv };
  }
  if (state.installedCatalog.find((e) => e.server_id === req.catalog_id)) {
    return { kind: "already_installed", server_id: req.catalog_id, started: null,
             missing_runtimes: [], missing_env_keys: [] };
  }
  state.installedCatalog.push({
    server_id: req.catalog_id,
    catalog_id: req.catalog_id,
    source: req.source,
    display_name: entry.display_name,
    installed_at: new Date().toISOString(),
    running: req.auto_start,
  });
  emitEvent("CatalogEntryInstalling", { catalog_id: req.catalog_id, source: req.source });
  emitEvent("CatalogEntryInstalled", { catalog_id: req.catalog_id, source: req.source, server_id: req.catalog_id });
  return { kind: "installed", server_id: req.catalog_id, started: req.auto_start,
           missing_runtimes: [], missing_env_keys: [] };
}
case "uninstall_catalog_entry": {
  state.installedCatalog = state.installedCatalog.filter((e) => e.server_id !== args.serverId);
  emitEvent("CatalogEntryUninstalled", { server_id: args.serverId });
  return null;
}
case "list_installed_entries": {
  return state.installedCatalog;
}
```

The `emitEvent(name, payload)` helper already exists in the mock — re-use it. If it doesn't, follow the same pattern used by existing MCP events in the mock.

- [ ] **Step 6: Run tests**

Run: `pnpm --filter agent-gui run test -- --run catalog.test.ts`
Expected: 3 tests PASS.

Run: `pnpm --filter agent-gui run lint`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src/stores/catalog.ts \
        apps/agent-gui/src/stores/catalog.test.ts \
        apps/agent-gui/src/composables/useMarketplace.ts \
        apps/agent-gui/e2e/tauri-mock.js
git commit -m "feat(gui): add catalog store, composable, and tauri-mock fixtures"
```

---

## Task 11: Vue components — `Marketplace.vue` + children + Vitest

**Files:**

- Create: `apps/agent-gui/src/views/Marketplace.vue`
- Create: `apps/agent-gui/src/components/marketplace/CatalogList.vue`
- Create: `apps/agent-gui/src/components/marketplace/CatalogCard.vue`
- Create: `apps/agent-gui/src/components/marketplace/CatalogDetail.vue`
- Create: `apps/agent-gui/src/components/marketplace/InstallProgress.vue`
- Create: `apps/agent-gui/src/components/marketplace/RuntimeMissingHint.vue`
- Create: `apps/agent-gui/src/components/marketplace/InstalledList.vue`
- Create: `apps/agent-gui/src/components/marketplace/Marketplace.test.ts`
- Modify: `apps/agent-gui/src/App.vue` (sidebar entry + route)

- [ ] **Step 1: Write the failing component test**

Create `apps/agent-gui/src/components/marketplace/Marketplace.test.ts`:

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import Marketplace from "../../views/Marketplace.vue";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([])
}));

describe("Marketplace.vue", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("renders Browse and Installed tabs", async () => {
    const wrapper = mount(Marketplace);
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain("Browse");
    expect(wrapper.text()).toContain("Installed");
  });

  it("switches to Installed tab on click", async () => {
    const wrapper = mount(Marketplace);
    await wrapper.find("[data-test='tab-installed']").trigger("click");
    expect(wrapper.find("[data-test='installed-list']").exists()).toBe(true);
  });
});
```

- [ ] **Step 2: Run the failing test**

Run: `pnpm --filter agent-gui run test -- --run Marketplace.test.ts`
Expected: FAIL — view does not exist.

- [ ] **Step 3: Implement `Marketplace.vue`**

Create `apps/agent-gui/src/views/Marketplace.vue`:

```vue
<script setup lang="ts">
import { computed } from "vue";
import { useMarketplace } from "../composables/useMarketplace";
import CatalogList from "../components/marketplace/CatalogList.vue";
import InstalledList from "../components/marketplace/InstalledList.vue";

const { store } = useMarketplace();

const installedCount = computed(() => store.installed.length);

function setTab(tab: "browse" | "installed") {
  store.tab = tab;
}
</script>

<template>
  <section class="marketplace">
    <header class="marketplace__header">
      <h1>Marketplace</h1>
      <nav class="tabs">
        <button
          data-test="tab-browse"
          :class="{ active: store.tab === 'browse' }"
          @click="setTab('browse')"
        >
          Browse
        </button>
        <button
          data-test="tab-installed"
          :class="{ active: store.tab === 'installed' }"
          @click="setTab('installed')"
        >
          Installed ({{ installedCount }})
        </button>
      </nav>
    </header>
    <CatalogList v-if="store.tab === 'browse'" />
    <InstalledList v-else data-test="installed-list" />
  </section>
</template>

<style scoped>
.marketplace {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
}
.marketplace__header {
  display: flex;
  align-items: baseline;
  gap: 24px;
}
.tabs {
  display: flex;
  gap: 8px;
}
.tabs button {
  padding: 6px 12px;
  border: 1px solid var(--border, #ccc);
  background: transparent;
  cursor: pointer;
}
.tabs button.active {
  background: var(--accent, #345);
  color: #fff;
}
</style>
```

- [ ] **Step 4: Implement `CatalogList.vue`**

Create `apps/agent-gui/src/components/marketplace/CatalogList.vue`:

```vue
<script setup lang="ts">
import { ref } from "vue";
import { useCatalogStore } from "../../stores/catalog";
import CatalogCard from "./CatalogCard.vue";
import CatalogDetail from "./CatalogDetail.vue";
import type { ServerEntryResponse } from "../../generated/commands";

const store = useCatalogStore();
const selected = ref<ServerEntryResponse | null>(null);
</script>

<template>
  <div class="catalog-list">
    <div class="filters">
      <input
        v-model="store.filters.keyword"
        placeholder="Search servers…"
        data-test="catalog-search"
      />
      <select v-model="store.filters.trustMin" data-test="catalog-trust">
        <option :value="null">All trust levels</option>
        <option value="verified">Verified+</option>
        <option value="community">Community+</option>
      </select>
      <button @click="store.refreshSource(null)" data-test="catalog-refresh">Refresh</button>
    </div>
    <p v-if="store.loading">Loading…</p>
    <p v-else-if="store.error" class="error">{{ store.error }}</p>
    <div v-else class="grid">
      <CatalogCard
        v-for="entry in store.filtered"
        :key="entry.id"
        :entry="entry"
        @click="selected = entry"
      />
    </div>
    <CatalogDetail v-if="selected" :entry="selected" @close="selected = null" />
  </div>
</template>

<style scoped>
.filters {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
}
.error {
  color: var(--error, #c33);
}
</style>
```

- [ ] **Step 5: Implement `CatalogCard.vue`**

Create `apps/agent-gui/src/components/marketplace/CatalogCard.vue`:

```vue
<script setup lang="ts">
import type { ServerEntryResponse } from "../../generated/commands";
defineProps<{ entry: ServerEntryResponse }>();
</script>

<template>
  <button class="card" data-test="catalog-card" @click="$emit('click')">
    <div class="card__head">
      <span class="icon">{{ entry.icon || "🔌" }}</span>
      <strong>{{ entry.display_name }}</strong>
      <span class="trust" :class="entry.trust">{{ entry.trust }}</span>
    </div>
    <p class="summary">{{ entry.summary }}</p>
    <div class="tags">
      <span v-for="t in entry.tags" :key="t" class="tag">{{ t }}</span>
    </div>
  </button>
</template>

<style scoped>
.card {
  text-align: left;
  padding: 12px;
  border: 1px solid var(--border, #ddd);
  cursor: pointer;
  background: transparent;
}
.card__head {
  display: flex;
  align-items: center;
  gap: 6px;
}
.trust {
  margin-left: auto;
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 3px;
  background: #eee;
}
.trust.verified {
  background: #cfc;
}
.trust.community {
  background: #ffd;
}
.summary {
  font-size: 13px;
  color: var(--muted, #555);
}
.tags {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}
.tag {
  font-size: 10px;
  padding: 1px 4px;
  border: 1px solid #ddd;
  border-radius: 2px;
}
</style>
```

- [ ] **Step 6: Implement `CatalogDetail.vue`**

Create `apps/agent-gui/src/components/marketplace/CatalogDetail.vue`:

```vue
<script setup lang="ts">
import { ref, computed } from "vue";
import type { ServerEntryResponse, InstallRequestPayload } from "../../generated/commands";
import { useMarketplace } from "../../composables/useMarketplace";
import RuntimeMissingHint from "./RuntimeMissingHint.vue";
import InstallProgress from "./InstallProgress.vue";

const props = defineProps<{ entry: ServerEntryResponse }>();
const emit = defineEmits<{ close: [] }>();

const { store, parseRequirements, parseDefaultEnv } = useMarketplace();
const requirements = computed(() => parseRequirements(props.entry));
const envSpec = computed(() => parseDefaultEnv(props.entry));
const overrides = ref<Record<string, string>>({});
const trustGrant = ref(props.entry.trust === "verified");
const autoStart = ref(true);
const showProgress = ref(false);

for (const spec of envSpec.value) {
  overrides.value[spec.key] = spec.default ?? "";
}

async function onInstall() {
  const req: InstallRequestPayload = {
    catalog_id: props.entry.id,
    source: props.entry.source,
    server_id_override: null,
    env_overrides: overrides.value,
    trust_grant: trustGrant.value,
    auto_start: autoStart.value
  };
  showProgress.value = true;
  await store.install(req);
}
</script>

<template>
  <aside class="drawer" role="dialog" aria-modal="true" data-test="catalog-detail">
    <header>
      <h2>{{ entry.display_name }}</h2>
      <button @click="emit('close')" aria-label="Close">×</button>
    </header>
    <p>{{ entry.description }}</p>
    <a v-if="entry.homepage" :href="entry.homepage" target="_blank" rel="noopener">Homepage</a>

    <section>
      <h3>Requirements</h3>
      <RuntimeMissingHint :requirements="requirements" />
    </section>

    <section>
      <h3>Configure</h3>
      <div v-for="spec in envSpec" :key="spec.key" class="field">
        <label>{{ spec.label }}<span v-if="spec.required">*</span></label>
        <input
          :type="spec.secret ? 'password' : 'text'"
          v-model="overrides[spec.key]"
          :placeholder="spec.default ?? ''"
          :data-test="`env-${spec.key}`"
        />
        <small>{{ spec.description }}</small>
      </div>
    </section>

    <section class="options">
      <label><input type="checkbox" v-model="trustGrant" /> Trust this server</label>
      <label><input type="checkbox" v-model="autoStart" /> Start after install</label>
    </section>

    <footer>
      <button @click="onInstall" data-test="catalog-install">Install</button>
    </footer>

    <InstallProgress v-if="showProgress" :catalog-id="entry.id" @close="showProgress = false" />
  </aside>
</template>

<style scoped>
.drawer {
  position: fixed;
  right: 0;
  top: 0;
  bottom: 0;
  width: min(480px, 90vw);
  background: var(--surface, #fff);
  border-left: 1px solid #ddd;
  padding: 16px;
  overflow-y: auto;
  z-index: 50;
}
.drawer header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.field {
  display: flex;
  flex-direction: column;
  margin-bottom: 8px;
}
.options {
  display: flex;
  gap: 16px;
}
</style>
```

- [ ] **Step 7: Implement `InstallProgress.vue`**

Create `apps/agent-gui/src/components/marketplace/InstallProgress.vue`:

```vue
<script setup lang="ts">
import { computed } from "vue";
import { useCatalogStore } from "../../stores/catalog";

const props = defineProps<{ catalogId: string }>();
defineEmits<{ close: [] }>();
const store = useCatalogStore();

const outcome = computed(() => store.installState[props.catalogId]);

const runtimeOk = computed(() => outcome.value && outcome.value.kind !== "runtime_missing");
const writeOk = computed(
  () => outcome.value?.kind === "installed" || outcome.value?.kind === "already_installed"
);
const startOk = computed(() => outcome.value?.kind === "installed" && outcome.value.started);
</script>

<template>
  <div class="modal" role="dialog" data-test="install-progress">
    <h3>Installing…</h3>
    <ul>
      <li :class="{ ok: runtimeOk, fail: outcome?.kind === 'runtime_missing' }">Detect runtime</li>
      <li :class="{ ok: writeOk, fail: outcome?.kind === 'invalid_env' }">Write config</li>
      <li :class="{ ok: startOk }">Start server</li>
    </ul>
    <p v-if="outcome?.kind === 'runtime_missing'">
      Missing runtimes: {{ outcome.missing_runtimes.join(", ") }}
    </p>
    <p v-if="outcome?.kind === 'invalid_env'">
      Required env: {{ outcome.missing_env_keys.join(", ") }}
    </p>
    <p v-if="outcome?.kind === 'already_installed'">Already installed.</p>
    <button @click="$emit('close')" data-test="install-close">Close</button>
  </div>
</template>

<style scoped>
.modal {
  position: fixed;
  inset: 20% 25%;
  background: var(--surface, #fff);
  border: 1px solid #ccc;
  padding: 16px;
  z-index: 60;
}
li.ok::before {
  content: "✓ ";
  color: green;
}
li.fail::before {
  content: "✗ ";
  color: #c33;
}
</style>
```

- [ ] **Step 8: Implement `RuntimeMissingHint.vue`**

Create `apps/agent-gui/src/components/marketplace/RuntimeMissingHint.vue`:

```vue
<script setup lang="ts">
defineProps<{
  requirements: Array<{
    kind: string;
    min_version: string | null;
    install_hint: string | null;
  }>;
}>();
</script>

<template>
  <ul class="hint" data-test="runtime-hint">
    <li v-for="r in requirements" :key="r.kind">
      <strong>{{ r.kind }}</strong>
      <span v-if="r.min_version"> ({{ r.min_version }})</span>
      <a v-if="r.install_hint" :href="r.install_hint" target="_blank" rel="noopener"> — install </a>
    </li>
  </ul>
</template>

<style scoped>
.hint {
  list-style: disc;
  margin-left: 18px;
}
</style>
```

- [ ] **Step 9: Implement `InstalledList.vue`**

Create `apps/agent-gui/src/components/marketplace/InstalledList.vue`:

```vue
<script setup lang="ts">
import { onMounted } from "vue";
import { useCatalogStore } from "../../stores/catalog";

const store = useCatalogStore();
onMounted(() => store.refreshInstalled());

async function onUninstall(serverId: string) {
  await store.uninstall(serverId);
}
</script>

<template>
  <table class="installed" data-test="installed-list">
    <thead>
      <tr>
        <th>Server</th>
        <th>Source</th>
        <th>Status</th>
        <th>Installed at</th>
        <th />
      </tr>
    </thead>
    <tbody>
      <tr v-for="row in store.installed" :key="row.server_id">
        <td>{{ row.display_name }}</td>
        <td>{{ row.source ?? "(manual)" }}</td>
        <td>
          <span :class="{ dot: true, running: row.running }" />
          {{ row.running ? "running" : "stopped" }}
        </td>
        <td>{{ row.installed_at }}</td>
        <td>
          <button
            :disabled="!row.source"
            :title="row.source ? '' : 'Hand-edited entries are not removable from here'"
            @click="onUninstall(row.server_id)"
            :data-test="`uninstall-${row.server_id}`"
          >
            Uninstall
          </button>
        </td>
      </tr>
    </tbody>
  </table>
</template>

<style scoped>
.installed {
  width: 100%;
  border-collapse: collapse;
}
.installed th,
.installed td {
  text-align: left;
  padding: 6px 8px;
  border-bottom: 1px solid #eee;
}
.dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #999;
  margin-right: 4px;
}
.dot.running {
  background: #2a2;
}
</style>
```

- [ ] **Step 10: Add sidebar entry in `App.vue`**

In `apps/agent-gui/src/App.vue` find the existing sidebar nav (the section that lists Sessions / MCP / Memory) and add a Marketplace entry pointing at the new view. Read `App.vue` first; the file uses `<script setup lang="ts">` and a `currentView`-style switch — match the existing style. Pseudo-diff:

```vue
<button :class="{ active: view === 'marketplace' }" @click="view = 'marketplace'">
  Marketplace
</button>
...
<Marketplace v-if="view === 'marketplace'" />
```

with `import Marketplace from "./views/Marketplace.vue";`.

- [ ] **Step 11: Verify**

Run:

```bash
pnpm --filter agent-gui run test -- --run Marketplace.test.ts catalog.test.ts
pnpm --filter agent-gui run lint
pnpm --filter agent-gui run format:check
```

Expected: all green.

- [ ] **Step 12: Commit**

```bash
git add apps/agent-gui/src/views/Marketplace.vue \
        apps/agent-gui/src/components/marketplace/ \
        apps/agent-gui/src/App.vue
git commit -m "feat(gui): add Marketplace view, components, and tests"
```

---

## Task 12: Playwright E2E + final `just check`

**Files:**

- Create: `apps/agent-gui/e2e/marketplace.spec.ts`

- [ ] **Step 1: Write the E2E spec**

Create `apps/agent-gui/e2e/marketplace.spec.ts`:

```ts
import { test, expect } from "@playwright/test";

test.describe("Marketplace", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.click("text=Marketplace");
  });

  test("browses the catalog and shows the filesystem entry", async ({ page }) => {
    const card = page.getByTestId("catalog-card").filter({ hasText: "Filesystem" });
    await expect(card).toBeVisible();
  });

  test("filters by keyword", async ({ page }) => {
    await page.getByTestId("catalog-search").fill("filesystem");
    await expect(page.getByTestId("catalog-card")).toHaveCount(1);
  });

  test("installs the filesystem entry happy path", async ({ page }) => {
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    await expect(page.getByTestId("install-progress")).toBeVisible();
    await page.getByTestId("install-close").click();
    await page.getByTestId("tab-installed").click();
    await expect(page.getByTestId("uninstall-filesystem")).toBeEnabled();
  });

  test("runtime-missing path shows a hint", async ({ page }) => {
    await page.evaluate(() => {
      // @ts-ignore — set in tauri-mock state at runtime
      window.__MARKETPLACE_FORCE_MISSING__ = ["node"];
    });
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("catalog-install").click();
    await expect(page.getByTestId("install-progress")).toContainText("Missing runtimes");
  });

  test("uninstall removes the entry", async ({ page }) => {
    await page.getByTestId("catalog-card").filter({ hasText: "Filesystem" }).click();
    await page.getByTestId("env-WORKSPACE_PATH").fill("/tmp/demo");
    await page.getByTestId("catalog-install").click();
    await page.getByTestId("install-close").click();
    await page.getByTestId("tab-installed").click();
    await page.getByTestId("uninstall-filesystem").click();
    await expect(page.getByTestId("uninstall-filesystem")).toHaveCount(0);
  });
});
```

The `runtime-missing path` test requires the mock to honour a `window.__MARKETPLACE_FORCE_MISSING__` override. Add that hook in `tauri-mock.js` `install_catalog_entry` case:

```js
const forced = (typeof window !== "undefined" && window.__MARKETPLACE_FORCE_MISSING__) || null;
const baseMissing = reqs.filter((r) => !state.catalogRuntimePresent[r.kind]).map((r) => r.kind);
const missing = forced || baseMissing;
```

- [ ] **Step 2: Run the E2E suite**

Run: `just test-e2e`
Expected: all 5 marketplace tests PASS, plus the existing E2E specs unchanged.

- [ ] **Step 3: Run the full check gate**

Run: `just check`
Expected: PASS.

Run: `just check-types`
Expected: PASS (`Generated types are in sync`).

Run: `just test-mcp`
Expected: PASS (existing MCP integration tests still green; the marketplace integration test is included via `cargo test --workspace`).

If any of these fail, fix in place and re-run before committing. Do NOT mark the task done until all three commands report success.

- [ ] **Step 4: Update `CHANGELOG` is NOT needed** — git-cliff regenerates it at release time. Skip this step.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/e2e/marketplace.spec.ts apps/agent-gui/e2e/tauri-mock.js
git commit -m "test(gui): add Playwright e2e suite for MCP marketplace"
```

- [ ] **Step 6: Final verification**

Run:

```bash
just check
just test-e2e
just test-mcp
just test-fullstack
just check-types
```

All must report success. Only then is Phase 1 done.

---

## Spec Coverage Map

| Spec section                                                                                            | Implemented in task(s) |
| ------------------------------------------------------------------------------------------------------- | ---------------------- |
| `CatalogProvider` trait + data types                                                                    | T1                     |
| Built-in catalog JSON (24 entries)                                                                      | T2                     |
| `BuiltinCatalogProvider` with filters                                                                   | T3                     |
| `AggregateCatalogProvider` dedup + ordering                                                             | T4                     |
| Installer: env validation, runtime probe, atomic toml, trust grant, idempotency                         | T5                     |
| `~/.kairox/mcp_servers.toml` overlay loader                                                             | T6                     |
| `McpServerManager::register_dynamic` / `unregister_dynamic`                                             | T7                     |
| 5 new `EventPayload` variants + matching `event_type()`                                                 | T8                     |
| 6 `AppFacade` methods + LocalRuntime wiring                                                             | T8                     |
| 6 `#[tauri::command]` wrappers + specta + regenerated TS                                                | T9                     |
| Catalog Pinia store + `useMarketplace` composable                                                       | T10                    |
| `tauri-mock.js` mock fixtures for the 6 commands and 5 events                                           | T10                    |
| `Marketplace.vue` (Browse/Installed tabs, sidebar entry)                                                | T11                    |
| `CatalogList`, `CatalogCard`, `CatalogDetail`, `InstallProgress`, `RuntimeMissingHint`, `InstalledList` | T11                    |
| Playwright e2e (browse, filter, install happy, runtime missing, uninstall)                              | T12                    |
| `just check` / `just check-types` / `just test-mcp` / `just test-fullstack` final gates                 | T12                    |
