# Skills Marketplace Design

## Summary

Extend the Skills settings page with a multi-source marketplace system that mirrors the MCP catalog architecture. Add built-in sources (skills.sh, SkillHub) with search and list capabilities, support user-configurable sources with URL templates and field mappings, unify the UI card styles between Skills and MCP marketplace, and add frontend/backend caching.

## Goals

- Add a `SkillCatalogProvider` trait (mirrors MCP `CatalogProvider`) with `list()` and `search()`.
- Ship two built-in sources: **skills.sh** (search-only) and **SkillHub** (list + search, richer metadata).
- Support user-added sources via URL template + JSON field mapping.
- Unify CSS card styles between skills Discover results and MCP marketplace cards.
- Add sources management panel in Skills settings (enable/disable, add, remove).
- Add HTTP-level caching (reuse `HttpResponseCache`) and frontend in-memory caching.
- Verify Rust backend can reach target APIs (reqwest User-Agent, potential curl fallback).

## Non-goals

- Do not change MCP marketplace architecture.
- Do not add a standalone `/marketplace/skills` route.
- Do not implement skill install from the marketplace in this phase (search/browse only).
- Do not change the Installed skills tab.
- Do not add semantic/embedding-based search.

## Architecture

### Backend: `agent-mcp/src/catalog/skills/`

New module sibling to `agent-mcp/src/catalog/remote/`:

```
crates/agent-mcp/src/catalog/skills/
├── mod.rs              # SkillCatalogProvider trait, SkillCatalogEntry, SkillCatalogQuery
├── aggregate.rs        # AggregateSkillCatalogProvider (parallel query, dedup)
├── builtin.rs          # Static builtin source configs (skills.sh + SkillHub)
├── remote.rs           # RemoteSkillSourceConfig, SkillSourceKind, BuildProvider factory
├── skills_sh.rs        # SkillsShProvider: wraps skills.sh API
└── skillhub.rs         # SkillHubProvider: wraps skills.palebluedot.live API
```

### Core Types (`agent-core/src/facade.rs`)

```rust
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

pub struct SkillCatalogQuery {
    pub keyword: Option<String>,
    pub sources: Option<Vec<String>>,
    pub limit: usize,
}

pub struct SkillSourceView {
    pub id: String,
    pub display_name: String,
    pub kind: String,            // "skills-sh" | "skillhub" | "custom"
    pub url: String,
    pub search_template: String,   // "/api/search?q={{query}}&limit={{limit}}"
    pub list_template: Option<String>,
    pub field_mapping: SkillFieldMappingView,
    pub enabled: bool,
    pub priority: u32,
    pub cache_ttl_seconds: u64,
}
```

### SkillCatalogProvider Trait

```rust
#[async_trait::async_trait]
pub trait SkillCatalogProvider: Send + Sync {
    fn source_id(&self) -> &str;
    async fn search(&self, query: &SkillCatalogQuery) -> Result<Vec<SkillCatalogEntry>>;
    async fn list(&self, query: &SkillCatalogQuery) -> Result<Vec<SkillCatalogEntry>>;
    async fn refresh(&self) -> Result<()>;
}
```

### Built-in Sources

| Source    | ID          | List | Search | URL                                                                      |
| --------- | ----------- | ---- | ------ | ------------------------------------------------------------------------ |
| skills.sh | `skills-sh` | No   | Yes    | `https://skills.sh/api/search?q={{query}}&limit={{limit}}`               |
| SkillHub  | `skillhub`  | Yes  | Yes    | `https://skills.palebluedot.live/api/skills?q={{query}}&limit={{limit}}` |

SkillHub list endpoint: `GET /api/skills?limit={{limit}}` (no `q` param). Returns paginated results with total count.

### Configuration Persistence

Stored in `~/.kairox/skill_sources.toml` (sibling to `mcp_servers.toml`):

```toml
[[skill_sources]]
id = "skills-sh"
display_name = "skills.sh"
kind = "skills-sh"
url = "https://skills.sh"
search_template = "/api/search?q={{query}}&limit={{limit}}"
enabled = true
priority = 0
cache_ttl_seconds = 900

[[skill_sources]]
id = "skillhub"
display_name = "SkillHub"
kind = "skillhub"
url = "https://skills.palebluedot.live"
search_template = "/api/skills?q={{query}}&limit={{limit}}"
list_template = "/api/skills?limit={{limit}}"
enabled = true
priority = 1
cache_ttl_seconds = 900
```

### HTTP & Caching

- Reuse `SharedHttpClient` from `agent-mcp/src/catalog/remote/http_client.rs` (reqwest + curl fallback, User-Agent `kairox-marketplace/...`).
- Reuse `HttpResponseCache` for disk+memory caching (TTL per source, default 15 min).
- Frontend: in-memory `Map<string, {entries: SkillCatalogEntry[], timestamp: number}>` in Pinia store.

### AppFacade Methods (new)

```rust
async fn list_skill_catalog(&self, query: SkillCatalogQuery) -> Result<Vec<SkillCatalogEntry>>;
async fn list_skill_sources(&self) -> Result<Vec<SkillSourceView>>;
async fn add_skill_source(&self, config: SkillSourceView) -> Result<()>;
async fn remove_skill_source(&self, id: &str) -> Result<()>;
async fn set_skill_source_enabled(&self, id: &str, enabled: bool) -> Result<()>;
async fn refresh_skill_catalog(&self) -> Result<()>;
```

### Tauri Commands (new)

```rust
#[tauri::command]
async fn list_skill_catalog(state: AppState, query: SkillCatalogQuery) -> Result<Vec<SkillCatalogEntry>>;
#[tauri::command]
async fn list_skill_sources(state: AppState) -> Result<Vec<SkillSourceView>>;
#[tauri::command]
async fn add_skill_source(state: AppState, config: SkillSourceView) -> Result<()>;
#[tauri::command]
async fn remove_skill_source(state: AppState, id: String) -> Result<()>;
#[tauri::command]
async fn set_skill_source_enabled(state: AppState, id: String, enabled: bool) -> Result<()>;
#[tauri::command]
async fn refresh_skill_catalog(state: AppState) -> Result<()>;
```

### Frontend Components

```
apps/agent-gui/src/components/
├── SkillSettingsPane.vue          # refactor: Discover tab uses SkillDiscoverList
├── skills/
│   ├── SkillDiscoverList.vue      # NEW: grid of SkillDiscoverCard, search bar, source filter chips
│   ├── SkillDiscoverCard.vue      # NEW: card (reuses .catalog-card styles)
│   └── SkillSourcesSettings.vue   # NEW: source management panel (enable/disable, add, remove)
```

### Pinia Store Changes (`useSkillsStore`)

New state:

- `catalogEntries: SkillCatalogEntry[]`
- `catalogSources: SkillSourceView[]`
- `catalogLoading: boolean`
- `searchCache: Map<string, {entries, timestamp}>` (frontend cache, session lifetime)

New actions:

- `searchCatalog(query)` → checks cache → calls `listSkillCatalog` → caches result
- `loadCatalogSources()` → calls `listSkillSources`
- `toggleSource(id, enabled)` → calls `setSkillSourceEnabled`
- `addSource(config)` → calls `addSkillSource`
- `removeSource(id)` → calls `removeSkillSource`
- `refreshCatalog()` → calls `refreshSkillCatalog`

### Style Unification

- `.catalog-card` CSS class reused for both MCP `CatalogCard` and skills `SkillDiscoverCard`.
- Same grid layout (`minmax(240px, 1fr)`), same tag classes (`tag-success`, `tag-warning`).
- Skills cards add `tag-info` for install count display.
- Source filter chips bar reused from MCP marketplace.

## Implementation Order

1. **Core types** — `SkillCatalogEntry`, `SkillCatalogQuery`, `SkillSourceView` in `agent-core`
2. **SkillCatalogProvider trait + aggregate** — new module in `agent-mcp`
3. **Built-in providers** — `SkillsShProvider`, `SkillHubProvider`
4. **HTTP client reuse + caching** — wire `SharedHttpClient` + `HttpResponseCache`
5. **Configuration** — `SkillSourcesToml` for `~/.kairox/skill_sources.toml`
6. **AppFacade + Tauri commands** — wire through `facade_runtime.rs` and `commands.rs`
7. **Frontend store** — extend `useSkillsStore`
8. **Frontend components** — `SkillDiscoverList`, `SkillDiscoverCard`, `SkillSourcesSettings`
9. **Style unification** — align card CSS classes
10. **Type generation** — `just gen-types`
11. **Tests** — unit tests for providers, aggregate; component tests for new Vue components

## Testing

- Unit tests for `SkillsShProvider::search()`, `SkillHubProvider::list()`, `SkillHubProvider::search()` using mock HTTP.
- Unit tests for `AggregateSkillCatalogProvider` (parallel query, dedup, error isolation).
- Unit tests for `SkillSourcesToml` (read, write, enable/disable, merge with defaults).
- Component tests for `SkillDiscoverList`, `SkillDiscoverCard`, `SkillSourcesSettings`.
- Store tests for new `useSkillsStore` actions.
- API accessibility verified via curl (skills.sh: 200, SkillHub: 200).
