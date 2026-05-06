# MCP Marketplace — Phase 2 (Remote Catalogs) Design

**Date:** 2026-05-06
**Status:** Draft for review
**Parent spec:** [`2026-05-06-mcp-marketplace-design.md`](./2026-05-06-mcp-marketplace-design.md)
**Phase 1 plan:** [`2026-05-06-mcp-marketplace.md`](../plans/2026-05-06-mcp-marketplace.md) (assumed complete)

## Goal

Extend the Phase 1 marketplace (which only browses a built-in JSON catalog of
~24 entries) so that users can also browse and install MCP servers from
**user-configurable remote catalog sources**, including the public
[Smithery Registry](https://smithery.ai/) and any self-hosted Kairox-format
JSON endpoint.

## Non-Goals

- Publishing servers to remote registries (read-only consumer in Phase 2).
- Authentication flows beyond a single bearer token / API key per source.
- Replacing the built-in catalog (it stays as the always-available offline
  fallback).
- Implementing a Kairox-hosted central registry (out of scope; users supply
  URLs).

## Locked Decisions (from brainstorming)

| Decision               | Choice                                                                                                                                       | Rationale                                                                                                                                                                            |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Remote protocol shape  | **Adapter pattern**: `KairoxJsonProvider` + `SmitheryProvider`                                                                               | Type-safe, easy to extend, each adapter independently testable with `wiremock`.                                                                                                      |
| HTTP cache reuse       | **New `HttpResponseCache` module — do NOT reuse `DiscoveryCache`**                                                                           | `DiscoveryCache` is MCP-protocol-specific (`tools/resources/prompts`); HTTP caching is semantically different. The parent spec's wording "DiscoveryCache re-used" is corrected here. |
| Cache strategy         | TTL=15min default + `ETag`/`If-None-Match` 304 short-circuit + on-disk persistence at `~/.kairox/cache/catalog/<source>.json`                | Lets the GUI render instantly on cold start; refresh is async.                                                                                                                       |
| Authentication         | Per-source `api_key_env` (env-var name) following `agent-config` convention                                                                  | No hard-coded secrets; consistent with existing model-provider keys.                                                                                                                 |
| Failure tolerance      | One failing source → `tracing::warn` + `CatalogSourceFailed` event; never fails the aggregate `list()`                                       | Builtin always works even if every remote is down.                                                                                                                                   |
| Concurrency            | `AggregateCatalogProvider` queries all sources in parallel via `futures::future::join_all`                                                   | Faster first paint.                                                                                                                                                                  |
| Source config location | Marketplace toml `~/.kairox/mcp_servers.toml` gains `[[catalog_sources]]` array                                                              | The marketplace toml is already owned by Kairox (Phase 1); avoids touching user-edited main config.                                                                                  |
| Trust ceiling          | Each source declares `default_trust`; effective trust per entry = `min(entry.trust, source.default_trust)`                                   | Prevents a remote source from claiming `Verified` for arbitrary entries.                                                                                                             |
| `Installer` changes    | **None** (Phase 1 installer is reused as-is)                                                                                                 | Pure additive change.                                                                                                                                                                |
| New Tauri commands     | 4: `list_catalog_sources`, `add_catalog_source`, `update_catalog_source`, `remove_catalog_source`. `refresh_catalog` from Phase 1 is reused. | Minimal IPC surface.                                                                                                                                                                 |
| New events             | 2: `CatalogSourceFailed`, `CatalogSourceAdded`. `CatalogRefreshed` from Phase 1 is reused.                                                   | Minimal event surface.                                                                                                                                                               |
| GUI surface            | One new component `CatalogSourcesSettings.vue` + extend `Marketplace.vue` source filter to multi-select chips                                | Smallest UI delta.                                                                                                                                                                   |
| Cargo feature          | New `remote-catalog` feature on `agent-mcp` (default-on, opt-out) gating `reqwest` import for the catalog module                             | Keeps the option of a `--no-default-features` lean build.                                                                                                                            |
| Argument substitution  | Reuse Phase 1 `${VAR}` expansion in `Installer` unchanged                                                                                    | Adapters write the same `InstallSpec` shape as builtin entries.                                                                                                                      |

## Architecture

```text
agent-mcp/src/catalog/
├── mod.rs                 # (unchanged) trait + types
├── builtin.rs             # (unchanged) BuiltinCatalogProvider
├── aggregate.rs           # (extended) priority + parallel + failure-tolerant
└── remote/                # (new)
    ├── mod.rs             # RemoteSourceConfig, RemoteSourceKind, common errors,
    │                      # construction helper: build_provider(cfg) → Arc<dyn CatalogProvider>
    ├── http_client.rs     # SharedHttpClient: reqwest wrapper with timeout, UA,
    │                      # auth header injection, single shared instance
    ├── http_cache.rs      # HttpResponseCache: in-memory LRU + on-disk JSON,
    │                      # TTL + ETag/If-None-Match support
    ├── kairox_json.rs     # KairoxJsonProvider: GET <url>, parse same shape as
    │                      # builtin-catalog.json
    └── smithery.rs        # SmitheryProvider: maps Smithery Registry API to
                           # ServerEntry
```

```text
agent-config/src/loader.rs
└── parse_catalog_sources()    # reads [[catalog_sources]] from marketplace toml,
                                # returns Vec<RemoteSourceConfig>

agent-runtime/src/facade_runtime.rs
└── on startup, build AggregateCatalogProvider from
    [BuiltinCatalogProvider] ++ map(parse_catalog_sources, build_provider)

apps/agent-gui/src/
├── components/CatalogSourcesSettings.vue   (new)
├── components/Marketplace.vue              (extended: multi-source chip filter)
└── stores/catalog.ts                       (extended: sources state + actions)
```

## End-to-end data flow

```text
GUI Marketplace mounted
  → invoke("list_catalog") → AppFacade::list_catalog
    → AggregateCatalogProvider::list(query)
      ├─ BuiltinCatalogProvider::list           (sync, in-memory)
      ├─ KairoxJsonProvider::list (parallel)    (HTTP via SharedHttpClient)
      │     └─ HttpResponseCache::get_or_fetch
      │           ├─ disk hit + fresh → return
      │           ├─ disk hit + stale → background refresh, return stale
      │           └─ miss → fetch, parse, persist, return
      └─ SmitheryProvider::list   (parallel)
            └─ HttpResponseCache::get_or_fetch + map_smithery_to_server_entries

GUI Settings → Add Source
  → invoke("add_catalog_source", RemoteSourceConfig)
    → loader appends to [[catalog_sources]] in mcp_servers.toml (atomic write)
    → AggregateCatalogProvider::reload_from_config()
    → emit DomainEvent::CatalogSourceAdded { source }

When a single source errors during list:
  → tracing::warn!(source=..., error=...);
  → emit DomainEvent::CatalogSourceFailed { source, error };
  → AggregateCatalogProvider returns the union of successful sources
```

## Data Model

All new types live in `crates/agent-mcp/src/catalog/remote/mod.rs` and gate
specta behind the existing `specta` feature. Naming follows the Phase 1
convention (no `Marketplace` prefix; `Catalog` is the namespace).

```rust
/// Configuration for one remote catalog source. Persisted in the
/// `[[catalog_sources]]` array of `~/.kairox/mcp_servers.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RemoteSourceConfig {
    /// Stable identifier, e.g. "smithery", "internal-registry".
    /// Acts as the `source` field on `ServerEntry` produced by this source.
    pub id: String,

    /// Human-readable label shown in the GUI source filter.
    pub display_name: String,

    /// Adapter to use.
    pub kind: RemoteSourceKind,

    /// Base URL. For Kairox JSON: full URL of the JSON document.
    /// For Smithery: base API URL (default https://registry.smithery.ai/).
    pub url: String,

    /// Optional name of an environment variable holding the API key /
    /// bearer token. The variable's value is sent as `Authorization: Bearer ...`.
    /// Following `agent-config`'s api_key_env convention.
    #[serde(default)]
    pub api_key_env: Option<String>,

    /// Lower order = higher priority in the aggregate result. Default 100.
    #[serde(default = "default_priority")]
    pub priority: u32,

    /// Trust ceiling. Effective trust per entry is
    /// `min(entry.trust, source.default_trust)`. Default `Community`.
    #[serde(default = "default_trust")]
    pub default_trust: TrustLevel,

    /// If `false`, the source is parsed but skipped during list/get.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Optional override of the default 15-minute cache TTL, in seconds.
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
    /// JSON document with the same shape as builtin-catalog.json.
    KairoxJson,
    /// Smithery Registry HTTP API.
    Smithery,
}

/// Errors emitted by remote catalog operations.
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

impl From<RemoteError> for CatalogError {
    fn from(e: RemoteError) -> Self {
        CatalogError::Provider(e.to_string())
    }
}
```

### Cache types

```rust
/// One cached HTTP response, persisted to
/// `~/.kairox/cache/catalog/<source_id>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedResponse {
    pub fetched_at_unix: u64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// The decoded body as `Vec<ServerEntry>` so each adapter does its own
    /// pre-mapping before persisting (avoids storing the raw upstream shape).
    pub entries: Vec<ServerEntry>,
}

pub(crate) struct HttpResponseCache {
    cache_dir: PathBuf,
    in_memory: Mutex<HashMap<String, CachedResponse>>,
}
```

### Aggregate extensions

`AggregateCatalogProvider` (extended in `aggregate.rs`):

```rust
pub struct AggregateCatalogProvider {
    /// Sorted by ascending priority at construction time.
    inner: Vec<Arc<dyn CatalogProvider>>,
    /// Sender for `CatalogSourceFailed` events. Optional so unit tests can
    /// construct the aggregate without an event bus.
    event_sink: Option<Arc<dyn DomainEventSink>>,
}

impl AggregateCatalogProvider {
    pub fn new_with_priority(
        providers: Vec<(u32, Arc<dyn CatalogProvider>)>,
        event_sink: Option<Arc<dyn DomainEventSink>>,
    ) -> Self { ... }

    /// Replaces all providers atomically, used after the user
    /// adds/removes/updates a source.
    pub fn reload(&mut self, providers: Vec<(u32, Arc<dyn CatalogProvider>)>);
}
```

`DomainEventSink` is a tiny trait that the runtime implements over its
existing event broadcaster. It avoids making `agent-mcp` depend on
`agent-runtime`.

```rust
// in agent-mcp/src/catalog/mod.rs
#[async_trait]
pub trait DomainEventSink: Send + Sync {
    async fn emit_source_failed(&self, source_id: &str, error: &str);
}
```

The `agent-runtime` crate provides the concrete impl that translates this into
`DomainEvent::CatalogSourceFailed`.

## Configuration File Layout

`~/.kairox/mcp_servers.toml` gains an optional top-level array. Existing
`[mcp_servers.<id>]` tables are unchanged.

```toml
# Managed by Kairox marketplace, schema=1
# Edit at your own risk; entries here may be rewritten by the marketplace UI.

[[catalog_sources]]
id          = "smithery"
display_name = "Smithery"
kind        = "smithery"
url         = "https://registry.smithery.ai"
api_key_env = "SMITHERY_API_KEY"
priority    = 50
default_trust = "community"
enabled     = true

[[catalog_sources]]
id          = "internal-registry"
display_name = "Internal MCP Registry"
kind        = "kairox_json"
url         = "https://mcp.internal.example.com/catalog.json"
priority    = 10                   # higher than Smithery
default_trust = "verified"

# (Existing entries below — installed servers; unchanged by Phase 2)
[mcp_servers.filesystem]
transport = "stdio"
command   = "npx"
args      = ["-y", "@modelcontextprotocol/server-filesystem", "/Users/me/work"]
```

`agent-config::loader` is extended:

```rust
pub fn parse_catalog_sources(toml_text: &str)
    -> Result<Vec<RemoteSourceConfig>, ConfigError>;

// Existing load_with_marketplace() additionally returns the parsed sources:
pub struct LoadedConfig {
    pub config: Config,                       // existing
    pub catalog_sources: Vec<RemoteSourceConfig>,  // new
}
```

## Smithery Adapter Mapping

Reference: <https://smithery.ai/docs/concepts/registry_search_servers>

`SmitheryProvider` translates Smithery's API into `ServerEntry`. The exact
endpoint shapes will be re-verified during T1 against fixtures captured by an
integration test, but the design is stable around these expected response
fields.

| Smithery field                            | `ServerEntry` field                                    | Notes                                                                                                                                                                                                                   |
| ----------------------------------------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `qualifiedName` (e.g. `@org/server`)      | `id`                                                   | Used verbatim; characters outside `[A-Za-z0-9._-]` get replaced with `-` for stable filesystem use.                                                                                                                     |
| `displayName`                             | `display_name`                                         | Falls back to `qualifiedName` if absent.                                                                                                                                                                                |
| `description` (markdown)                  | `description`                                          | Full markdown body.                                                                                                                                                                                                     |
| First sentence of description, ≤200 chars | `summary`                                              | Computed; never empty.                                                                                                                                                                                                  |
| `iconUrl`                                 | `icon`                                                 | URL preserved (GUI handles fetch).                                                                                                                                                                                      |
| `homepage`                                | `homepage`                                             |                                                                                                                                                                                                                         |
| `useCount`, `verified` (bool)             | `trust`                                                | `verified == true → Verified`, else `Community`. Then ceiling-clipped by `source.default_trust`.                                                                                                                        |
| `tags`                                    | `tags` + `categories`                                  | Smithery doesn't differentiate; same list flows into both.                                                                                                                                                              |
| `connection.type == "stdio"`              | `InstallSpec::Stdio { command, args, env, cwd: None }` | Mapped 1:1 from `connection.command/args/env`.                                                                                                                                                                          |
| `connection.type == "http"` or `"sse"`    | `InstallSpec::Sse { url, headers }`                    | `connection.connectionUrl` and `connection.headers`.                                                                                                                                                                    |
| `requirements.runtimes[]` (if present)    | `requirements: Vec<RuntimeRequirement>`                | Mapped via best-effort string match (`node`/`python`/`docker` → `RuntimeKind`). Unknown → `RuntimeKind::Other` with `install_hint` carrying the raw string.                                                             |
| `configSchema` (JSON Schema)              | `default_env: Vec<EnvVarSpec>`                         | Per-property mapping: `description` → `description`, `format == "password"` → `secret=true`, `required` array → `required`, `default` → `default`. Unsupported keywords (`oneOf`, `enum`) skipped with `tracing::warn`. |
| n/a                                       | `source`                                               | Hard-coded to the source id (e.g. `"smithery"`).                                                                                                                                                                        |
| n/a                                       | `version`                                              | Set to Smithery's `version` field if present, else `None`.                                                                                                                                                              |

The mapping function lives in `smithery.rs`:

```rust
fn map_smithery_to_entry(
    source_id: &str,
    raw: &SmitheryServer,
    trust_ceiling: TrustLevel,
) -> Result<ServerEntry, RemoteError>;
```

It is **pure** (no I/O), making unit tests trivial: feed in a JSON fixture,
assert the returned `ServerEntry`.

## AppFacade and Tauri Commands

`AppFacade` (in `crates/agent-core/src/facade.rs`) gains 4 methods:

```rust
async fn list_catalog_sources(&self) -> Result<Vec<RemoteSourceConfig>>;
async fn add_catalog_source(&self, cfg: RemoteSourceConfig) -> Result<()>;
async fn update_catalog_source(&self, cfg: RemoteSourceConfig) -> Result<()>;
async fn remove_catalog_source(&self, source_id: &str) -> Result<()>;
```

All four mutate `~/.kairox/mcp_servers.toml` atomically (temp-file + rename),
then call `AggregateCatalogProvider::reload(...)`. `add` and `update` validate
the URL is a parseable absolute http(s) URL before persisting and return
`CatalogError::InvalidData` otherwise.

`refresh_catalog(source: Option<&str>)` from Phase 1 is reused:

- `Some(source_id)` → calls `provider.refresh()` only on that source's
  underlying remote provider, which forces an `HttpResponseCache` invalidate +
  next-call refetch.
- `None` → invalidates cache for **all** remote providers (builtin is no-op).

`apps/agent-gui/src-tauri/src/commands.rs` gains four
`#[tauri::command] #[specta::specta]` wrappers with the same names. Each is
registered in **both** `generate_handler!` and `collect_commands!`.

After implementation, `just gen-types` regenerates
`apps/agent-gui/src/generated/{commands.ts,events.ts}` and `just check-types`
must pass.

## Events

Add to `EventPayload` in `crates/agent-core/src/events.rs` (with
corresponding match arms in `event_type()` and specta registration):

| Variant               | Fields                            | When emitted                                                                                                                                                                             |
| --------------------- | --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CatalogSourceAdded`  | `source: String`                  | After `add_catalog_source` succeeds.                                                                                                                                                     |
| `CatalogSourceFailed` | `source: String`, `error: String` | When `AggregateCatalogProvider::list` catches a per-source error. Rate-limited: same `(source, error)` pair within 60 seconds is suppressed to avoid event storms when a remote is down. |

`CatalogRefreshed` from Phase 1 is reused for refresh completion. There is no
`CatalogSourceRemoved` / `CatalogSourceUpdated` event in v1 — the GUI re-reads
the source list via `list_catalog_sources` after each mutation, which is
cheap. (We can add one later if a TUI ever needs to react.)

## UI Flow

### Marketplace.vue (extended)

The Browse tab's existing **Source** dropdown becomes a row of selectable
chips, one per source from `list_catalog_sources()` plus a permanent
`Built-in` chip. Multi-select with `OR` semantics. Default: all chips
selected.

A new ⚙️ icon next to the chips opens `CatalogSourcesSettings.vue` as a side
drawer (using the same drawer component as `CatalogDetail.vue`).

When `CatalogSourceFailed` arrives, the offending chip gets a small `⚠`
badge with a tooltip showing the error. Clicking the badge opens the same
settings drawer scrolled to that source.

### CatalogSourcesSettings.vue (new)

Single-column list of configured sources. Each row:

- Header: `display_name` + `id` (small monospace) + enabled toggle.
- Body: kind badge (`Smithery` / `Kairox JSON`), URL (copy button), priority
  number, default trust select, `api_key_env` text input.
- Footer: **Save**, **Remove** (confirm dialog), **Test connection** button
  (calls `refresh_catalog(Some(id))` and reports success / first error).

Bottom of the list: **Add source** button → expands an inline form with the
same fields. Save calls `add_catalog_source`.

### Coexistence with Phase 1 surface

- `Marketplace.vue` is unchanged in layout; only the source filter row gains
  chips and a ⚙️.
- `CatalogDetail.vue` already renders source badge — works unchanged because
  remote entries set `source` to the user-defined id.
- `InstallProgress.vue` is unchanged; remote entries flow through the same
  Phase 1 `Installer`.

## Testing Strategy

| Layer                                          | Tests                                                                                                                                                                                                                                                                                                | Command                      |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| `agent-mcp::catalog::remote::http_cache` unit  | TTL hit / TTL expired triggers refetch / ETag 304 returns cached / on-disk persistence round-trip / single-flight under concurrent calls                                                                                                                                                             | `cargo test -p agent-mcp`    |
| `agent-mcp::catalog::remote::kairox_json` unit | wiremock 200 with valid doc / 200 with bad schema (Decode error) / 304 returns cached / 5xx returns RemoteError::Http / connect timeout / unauthorized when api_key_env unset                                                                                                                        | `cargo test -p agent-mcp`    |
| `agent-mcp::catalog::remote::smithery` unit    | Map fixture stdio server / map fixture http server / verified→Verified, else→Community / trust ceiling clips Verified→Community / unknown runtime → Other with hint / configSchema → EnvVarSpec                                                                                                      | `cargo test -p agent-mcp`    |
| `agent-mcp::catalog::aggregate` extended unit  | Priority sorts deterministically / one source error doesn't fail others / parallel list ≈ slowest source latency (smoke) / reload swaps providers atomically / `CatalogSourceFailed` is rate-limited                                                                                                 | `cargo test -p agent-mcp`    |
| `agent-config::loader` unit                    | `[[catalog_sources]]` parses with all fields / minimal fields use defaults / invalid kind → ConfigError / preserves existing `[mcp_servers.*]` round-trip                                                                                                                                            | `cargo test -p agent-config` |
| `agent-runtime` integration                    | `tests/marketplace_remote.rs`: spin two wiremock servers (one Kairox, one Smithery-shaped); construct full `LocalRuntime` with marketplace toml referencing both; `list_catalog` returns merged + sorted set; install one entry from each; one source crashes → `CatalogSourceFailed` event observed | `just test-mcp`              |
| GUI Vitest                                     | `CatalogSourcesSettings.test.ts` (form add/edit/remove/validate) / `Marketplace.test.ts` extended (chip multi-select, ⚠ badge on `CatalogSourceFailed`) / `catalog.ts` store extended                                                                                                                | `pnpm vitest`                |
| Playwright E2E                                 | `marketplace.spec.ts` extended: open settings, add a source backed by tauri-mock, the new chip appears, install an entry from it, remove the source, chip disappears                                                                                                                                 | `just test-e2e`              |
| Tauri mock                                     | `tauri-mock.js` gains the 4 new commands and emits the 2 new events; mock keeps an in-memory `sources[]` array                                                                                                                                                                                       | as part of E2E               |
| Type sync                                      | `just check-types` after `just gen-types`                                                                                                                                                                                                                                                            | CI `type-sync` job           |

TDD is enforced per `test-driven-development` skill: each module starts with
failing tests; each Vue component starts with failing Vitest specs.

## Risks

| Risk                                                                | Impact | Mitigation                                                                                                                                                                                    |
| ------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Smithery API changes shape                                          | Med    | Adapter is pure mapping function; one fixture file per supported response shape; CI uses wiremock not the live API. Documented as "best-effort, may need tweaks per Smithery release."        |
| Misconfigured URL hangs requests                                    | Med    | `SharedHttpClient` enforces 5s connect + 10s total per request; aggregate timeout per source is 12s before the source is reported failed.                                                     |
| Disk cache corruption                                               | Low    | Cache reads tolerate decode errors by treating the cache as a miss + emitting `tracing::warn`. Cache files are best-effort, never authoritative.                                              |
| Concurrent identical fetches                                        | Low    | `HttpResponseCache::get_or_fetch` uses `tokio::sync::Mutex` per source key for single-flight.                                                                                                 |
| `[[catalog_sources]]` schema breaks across releases                 | Low    | Self-identifying header comment + `#[serde(deny_unknown_fields = false)]` so future fields are forward-compatible.                                                                            |
| Remote server claims `Verified` for malicious entries               | High   | Trust ceiling per source; UI badges show `effective trust` not `claimed trust`.                                                                                                               |
| `CatalogSourceFailed` event storm when a remote is permanently down | Low    | Rate-limit window (60s per `(source, error)` tuple) inside aggregate.                                                                                                                         |
| `reqwest` increases cold compile time                               | Low    | Already in workspace deps for `agent-mcp` (sse) and `agent-models`; no new top-level dep. The `remote-catalog` feature is default-on; users wanting a lean build use `--no-default-features`. |

## Phase Slicing & Out-of-Scope (for Phase 3+)

| Item                                                            | Why deferred                                                                                                                                            |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Publishing servers to remote registries (Smithery upload, etc.) | Out of Phase 2 goal; the UX is materially different (auth, validation, user-owned metadata).                                                            |
| Pagination of remote results                                    | Smithery + Kairox JSON both return ≤500 entries in practice; pagination adds complexity without current benefit. Will add when a real source forces it. |
| Server-side rating / install counts displayed in card           | Smithery exposes `useCount` but rendering it is purely presentational and not blocking.                                                                 |
| OAuth / device-code login flows                                 | YAGNI; bearer token via env var covers all surveyed sources today.                                                                                      |
| Source-specific health dashboard                                | The `CatalogSourceFailed` chip badge is enough; dashboard is gold-plating.                                                                              |

## Open Questions

None. All branching points have been pinned by the Locked Decisions table.
