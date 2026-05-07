# MCP Marketplace — Design Spec

**Date:** 2026-05-06
**Status:** Draft
**Approach:** Hybrid catalog (built-in curated + remote registry) with `CatalogProvider` trait abstraction; lightweight "registration" install semantics that reuse the existing `McpServerManager`. Phase 1 ships the built-in catalog only; Phase 2 adds remote registries incrementally.

## Summary

Add an in-app MCP server marketplace to Kairox so users can discover and install MCP servers without hand-editing TOML. The marketplace draws from a built-in curated catalog of ~24 high-quality servers (shipped inside the binary) and is designed to later layer remote registries (e.g. Smithery, mcp.so) on top via the same trait. Installing a server means writing a config entry and starting the existing `McpServerManager` lifecycle — Kairox does not bundle Node/Python/Docker runtimes.

This work targets the ROADMAP "Expand MCP ecosystem coverage (more transports, richer discovery, server marketplace UX)" item.

## Goals

- Users can browse a curated set of MCP servers in the GUI without leaving the app.
- Users can install a server in three steps (browse → configure env → install) with zero TOML editing.
- Detect missing host runtimes (`node`, `python`, `uvx`, `docker`, …) before install and present an actionable hint instead of a confusing crash.
- Keep marketplace-installed servers physically separated from hand-edited config so the two never fight.
- Trait surface (`CatalogProvider`) is shaped for both built-in and remote sources from day one, even though Phase 1 only ships built-in.
- Reuse the existing `McpServerManager`, `McpServerDef`, and `DiscoveryCache` rather than re-implementing lifecycle or caching.

## Non-Goals

- Bundling host runtimes (no embedded `bun`/`uv`/Node).
- Auto-upgrading installed servers when catalog metadata changes.
- Social features (ratings, reviews, download counts).
- Per-tool granular install (a server is the install unit).
- Offline npm/pip dependency packaging.
- Phase 1 does **not** ship `RemoteCatalogProvider`; only the trait surface and built-in implementation land in Phase 1.

## Architecture

### Module map

```text
crates/agent-mcp/                        (extended)
  src/
    catalog/
      mod.rs              # CatalogProvider trait + ServerEntry, InstallSpec,
                          # RuntimeRequirement, TrustLevel, EnvVarSpec,
                          # CatalogQuery, InstallOutcome, InstalledEntry
      builtin.rs          # BuiltinCatalogProvider — include_str! the JSON file
      aggregate.rs        # AggregateCatalogProvider — merges providers, dedupes
                          # (Phase 1 wraps a single BuiltinCatalogProvider; trait
                          #  shape lets Phase 2 add RemoteCatalogProvider without
                          #  changing call sites. remote.rs is NOT created in
                          #  Phase 1 to avoid dead code.)
      data/
        builtin-catalog.json   # ~24 curated entries; embedded at compile time
    installer.rs          # Validates entries, detects host runtimes, writes
                          # ~/.kairox/mcp_servers.toml, returns InstallOutcome
  tests/
    catalog.rs            # Built-in JSON parses; aggregate ordering rules
    installer.rs          # Runtime detection, idempotent toml writes,
                          # id-collision handling

crates/agent-config/                     (extended)
  src/loader.rs           # Read main config.toml AND ~/.kairox/mcp_servers.toml,
                          # merge with main file taking precedence on conflicts.

crates/agent-core/                       (extended)
  src/events.rs           # +5 EventPayload variants (Section: Events)
  src/facade.rs           # AppFacade gains 6 marketplace methods

crates/agent-runtime/                    (extended)
  src/facade_runtime.rs   # Wire AppFacade methods to catalog + installer +
                          # McpServerManager
  src/mcp_manager.rs      # No public API change; gains an internal
                          # `register_dynamic` helper so installer can
                          # add servers without restart.

apps/agent-gui/src-tauri/src/
  commands.rs             # +6 #[tauri::command] functions
  specta.rs               # Register new types in collect_types![]
                          # Register new commands in collect_commands![]

apps/agent-gui/src/
  views/Marketplace.vue
  components/marketplace/
    CatalogList.vue
    CatalogCard.vue
    CatalogDetail.vue
    InstallProgress.vue
    RuntimeMissingHint.vue
    InstalledList.vue
  stores/catalog.ts
  composables/useMarketplace.ts
  e2e/marketplace.spec.ts
```

### Locked decisions

| Decision                 | Choice                                                                        | Rationale                                                                                                     |
| ------------------------ | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Catalog source strategy  | Hybrid: built-in (Phase 1) + remote registry (Phase 2)                        | Local-first identity preserved; remote layered later without redesign                                         |
| Install semantics        | Lightweight registration (write toml + start)                                 | Aligns with current `McpServerManager`; matches Cursor/Claude Desktop conventions; no embedded runtime burden |
| Provider abstraction     | `CatalogProvider` trait                                                       | Built-in and future remote/registry/private sources share UI and command surface                              |
| Install file location    | `~/.kairox/mcp_servers.toml` (separate from main `config.toml`)               | Eliminates merge conflicts between marketplace writes and user hand-edits                                     |
| Trust default            | `Verified` auto-trusts; `Community`/`Unverified` require explicit user opt-in | Safer default; avoids silent privilege escalation                                                             |
| Phase 1 scope            | Built-in catalog only; trait + UI complete                                    | YAGNI — remote provider is purely additive in Phase 2                                                         |
| Auto-start after install | Default `true`, user-overridable per install                                  | Matches user mental model: clicking Install means "make it work now"                                          |

### End-to-end install data flow

```text
GUI Marketplace.vue
  → invoke("install_catalog_entry", InstallRequest)
    → Tauri commands.rs → AppFacade::install_catalog_entry
      → CatalogProvider::get(id) → ServerEntry
      → Installer::check_requirements(&entry)
        ├─ ok → continue
        └─ missing → InstallOutcome::RuntimeMissing (no toml write)
      → Installer::write_to_config(&entry, env_overrides)
        → ~/.kairox/mcp_servers.toml gains [mcp_servers.<id>]
      → McpServerManager::register_dynamic(McpServerDef)
      → if auto_start: McpServerManager::start(id)
      → emit DomainEvent::CatalogEntryInstalled { server_id, source, catalog_id }
GUI catalog store updates installState[id]; existing mcp store sees server_registered.
```

## Data Model

All catalog types live in `crates/agent-mcp/src/catalog/mod.rs` and gate specta behind the existing `specta` feature.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ServerEntry {
    pub id: String,                 // stable, e.g. "filesystem"
    pub source: String,             // "builtin" | "smithery" | ...
    pub display_name: String,
    pub summary: String,            // ≤ 200 chars, used on cards
    pub description: String,        // markdown; used in detail drawer
    pub categories: Vec<String>,    // free-form strings; canonical set listed in
                                    // "Initial curated entries" table below
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    pub install: InstallSpec,
    pub requirements: Vec<RuntimeRequirement>,
    pub trust: TrustLevel,
    pub default_env: Vec<EnvVarSpec>,
    pub icon: Option<String>,       // emoji or asset URL
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum InstallSpec {
    Stdio {
        command: String,            // "npx" | "uvx" | "python" | "docker" | absolute path
        args: Vec<String>,
        env: BTreeMap<String, String>,
        cwd: Option<String>,
    },
    Sse {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RuntimeRequirement {
    pub kind: RuntimeKind,
    pub min_version: Option<String>,    // semver expression, e.g. ">=18.0.0"
    pub install_hint: Option<String>,   // user-facing instruction or URL
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind { Node, Python, Uvx, Docker, Bun, Deno, Other }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EnvVarSpec {
    pub key: String,
    pub label: String,
    pub description: String,
    pub required: bool,
    pub secret: bool,
    pub default: Option<String>,        // pre-fills the form input. For secret
                                        // entries the GUI displays a password
                                        // field but still pre-fills (use only
                                        // for non-sensitive defaults like
                                        // "localhost"). Required-but-empty
                                        // fields cause InstallOutcome::InvalidEnv.
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel { Verified, Community, Unverified }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CatalogQuery {
    pub keyword: Option<String>,
    pub category: Option<String>,
    pub trust_min: Option<TrustLevel>,
    pub source: Option<String>,
    pub limit: Option<usize>,           // default 100
}

#[async_trait]
pub trait CatalogProvider: Send + Sync {
    fn source_id(&self) -> &str;
    async fn list(&self, query: &CatalogQuery) -> Result<Vec<ServerEntry>>;
    async fn get(&self, id: &str) -> Result<Option<ServerEntry>>;
    async fn refresh(&self) -> Result<()> { Ok(()) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallOutcome {
    Installed { server_id: String, started: bool },
    RuntimeMissing { missing: Vec<RuntimeRequirement> },
    AlreadyInstalled { server_id: String },
    InvalidEnv { missing_keys: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstalledEntry {
    pub server_id: String,
    pub catalog_id: Option<String>,     // None for hand-edited entries from main config.toml
    pub source: Option<String>,         // None for hand-edited entries
    pub display_name: String,
    pub installed_at: String,           // RFC3339; for hand-edited entries: file mtime
    pub running: bool,                  // resolved at query time from McpServerManager state
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallRequest {
    pub catalog_id: String,
    pub source: String,
    pub server_id_override: Option<String>,
    pub env_overrides: BTreeMap<String, String>,
    pub trust_grant: bool,
    pub auto_start: bool,               // default true at the GUI layer
}
```

## Built-in Catalog JSON

File: `crates/agent-mcp/src/catalog/data/builtin-catalog.json`, embedded at compile time via `include_str!`.

```jsonc
{
  "schema_version": "1",
  "generated_at": "<RFC3339 timestamp; informational only>",
  "entries": [
    {
      "id": "filesystem",
      "source": "builtin",
      "display_name": "Filesystem",
      "summary": "Read, write, and search files inside an allow-listed directory.",
      "description": "<markdown body, ≥ 1 paragraph; truncated in this example>",
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
    }
    // ... ~23 more entries
  ]
}
```

### Initial curated entries (~24)

| Category      | Entry IDs                                   | Trust                            |
| ------------- | ------------------------------------------- | -------------------------------- |
| filesystem    | `filesystem`                                | Verified                         |
| git/code      | `git`, `github`, `gitlab`                   | Verified / Community / Community |
| search        | `brave-search`, `exa`, `tavily`             | Community                        |
| browser       | `puppeteer`, `playwright`                   | Community                        |
| data          | `sqlite`, `postgres`, `redis`               | Community                        |
| dev-tools     | `time`, `fetch`, `everything`, `memory`     | Verified                         |
| communication | `slack`, `gmail`                            | Community                        |
| productivity  | `notion`, `linear`, `obsidian`              | Community                        |
| misc          | `aws-kb-retrieval`, `google-maps`, `sentry` | Community                        |

The exact list is finalised during implementation; the schema is the contract.

## Events

Add to `EventPayload` in `crates/agent-core/src/events.rs` (with corresponding match arms in `event_type()` and specta registration):

| Variant                   | Fields                                                      | When emitted                                                       |
| ------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------ |
| `CatalogRefreshed`        | `source: String`, `entry_count: usize`                      | After a `CatalogProvider::refresh()` succeeds                      |
| `CatalogEntryInstalling`  | `catalog_id: String`, `source: String`                      | When install pipeline starts (after runtime check passes)          |
| `CatalogEntryInstalled`   | `catalog_id: String`, `source: String`, `server_id: String` | After toml write + manager registration succeed                    |
| `CatalogEntryUninstalled` | `server_id: String`                                         | After uninstall removes the entry from the marketplace toml file   |
| `CatalogRuntimeMissing`   | `catalog_id: String`, `missing: Vec<String>`                | When `Installer::check_requirements` reports missing host runtimes |

These flow through the existing `event_forwarder` pipeline to the GUI catalog store.

## AppFacade and Tauri Commands

`AppFacade` (in `crates/agent-core/src/facade.rs`) gains 6 methods:

```rust
async fn list_catalog(&self, query: CatalogQuery) -> Result<Vec<ServerEntry>>;
async fn get_catalog_entry(&self, id: &str, source: Option<&str>) -> Result<Option<ServerEntry>>;
async fn refresh_catalog(&self, source: Option<&str>) -> Result<()>;
async fn install_catalog_entry(&self, req: InstallRequest) -> Result<InstallOutcome>;
async fn uninstall_catalog_entry(&self, server_id: &str) -> Result<()>;
async fn list_installed_entries(&self) -> Result<Vec<InstalledEntry>>;
```

`apps/agent-gui/src-tauri/src/commands.rs` gains six `#[tauri::command] #[specta::specta]` wrappers with the same names. Each is registered in **both** `generate_handler!` and `collect_commands!` (per AGENTS.md).

After implementation, `just gen-types` regenerates `apps/agent-gui/src/generated/{commands.ts,events.ts}` and `just check-types` must pass.

## UI Flow

### Navigation

A new sidebar entry **Marketplace** is added next to Sessions / MCP / Memory.

### `Marketplace.vue` (top-level)

Two tabs: **Browse** (default) and **Installed (n)**.

Browse tab:

- Search box (matches `display_name`, `summary`, `tags`).
- Category dropdown (sourced from a static category list mirroring `categories`).
- Trust filter (`All` / `Verified+` / `Community+`).
- Refresh button (calls `refresh_catalog`; in Phase 1 against the built-in source it is a no-op but visually consistent).
- Card grid via `CatalogList.vue` → `CatalogCard.vue`.

Installed tab:

- Lists `InstalledEntry` rows.
- Per-row status dot reads from the existing `mcp` Pinia store via `server_id`.
- Uninstall button is **disabled** for entries whose `source = None` (hand-edited in main config.toml); tooltip explains why.

### `CatalogDetail.vue` (drawer)

Opens when a card is clicked.

- Header: name, trust badge, source badge, homepage link.
- Description (markdown).
- **Requirements** block: each `RuntimeRequirement` is rendered with a "detected ✓ / missing ✗" indicator. Detection is invoked once when the drawer opens (a lightweight Tauri command call) and re-checked at install time.
- **Configure** block: form generated from `default_env`. `required` items are marked, `secret` items use a password input, descriptions are inline help.
- Footer: `Trust this server` checkbox (auto-checked when `trust == Verified`); primary `Install` button.

### `InstallProgress.vue`

Modal that opens on install. Three rows:

1. **Detect runtime** — passes / fails based on `RuntimeMissing`.
2. **Write config** — passes / fails based on toml write.
3. **Start server** — only runs if `auto_start`.

Failure variants:

- `RuntimeMissing` → red row, lists missing runtimes with `install_hint` links; primary button switches to `Open install instructions`.
- `AlreadyInstalled` → amber row; offers `Open settings` or `Replace existing`.
- Server crash post-start → reuses the existing `McpServerManager` retry; if `MaxRestartsExceeded` is observed within the modal's window, the modal shows a stderr tail and `Retry / Edit config / Uninstall`.

### Coexistence with `McpServerManager.vue`

`McpServerManager.vue` continues to be the operations view (start/stop/restart, log inspection). `Marketplace.vue` is the discovery+install view. They share state through `mcp` and the new `catalog` store keyed on `server_id`.

## Permission and Trust Behavior

The catalog entry's `trust` level only controls the **default value** of the
GUI's "Trust this server" checkbox; the **final** decision is whatever
`InstallRequest.trust_grant` carries when the install command is invoked.
The installer never silently grants trust on its own.

- GUI default: `Verified` → checkbox pre-checked; `Community` / `Unverified` → checkbox unchecked.
- Installer behavior: if `trust_grant == true`, append `server_id` to `trusted_servers` in `mcp_servers.toml`; otherwise leave it out.
- Trust only governs whether tool calls skip the per-call permission prompt; it does **not** bypass `PermissionMode::Interactive`.

## Configuration File Layout

- Main `~/.kairox/config.toml` (or project-local `kairox.toml`) — user-edited, untouched by marketplace.
- New `~/.kairox/mcp_servers.toml` — owned by marketplace; written by installer; safe to delete to "factory reset" marketplace state.

`agent-config::loader` is extended to load both files and merge: main file entries with the same id take precedence (so a user can override a marketplace entry without losing the marketplace toml).

Top of marketplace toml carries a self-describing comment:

```toml
# Managed by Kairox marketplace, schema=1
# Edit at your own risk; entries here may be rewritten by the marketplace UI.
```

## Testing Strategy

| Layer                       | Tests                                                                                                                                                                                   | Command                      |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| `agent-mcp::catalog` unit   | Built-in JSON parses; AggregateProvider de-duplicates and orders by `(trust desc, source order, display_name)`; `CatalogQuery` filters                                                  | `cargo test -p agent-mcp`    |
| `agent-mcp::installer` unit | Runtime detection mocked through a trait, idempotent toml writes, `id` collision returns `AlreadyInstalled`, env-var override merges with defaults                                      | `cargo test -p agent-mcp`    |
| `agent-config::loader` unit | Two-file merge precedence; missing marketplace file is non-fatal                                                                                                                        | `cargo test -p agent-config` |
| `agent-runtime` integration | New `tests/marketplace_integration.rs`: install full pipeline (built-in → toml write → `McpServerManager::register_dynamic` → start), uninstall full pipeline, runtime-missing pipeline | `just test-mcp`              |
| GUI Vitest                  | `catalog.test.ts` (store), `Marketplace.test.ts`, `CatalogDetail.test.ts`, `InstallProgress.test.ts`, `RuntimeMissingHint.test.ts`                                                      | `pnpm vitest`                |
| Playwright E2E              | `e2e/marketplace.spec.ts` covering: browse + filter + search; install happy path; runtime-missing flow; uninstall; trust default checkbox behavior                                      | `just test-e2e`              |
| Tauri mock                  | `tauri-mock.js` gains the 6 new commands and emits the 5 new events                                                                                                                     | as part of E2E               |
| Type sync                   | `just check-types` must pass after `just gen-types` regenerates `commands.ts` + `events.ts`                                                                                             | CI `type-sync` job           |

TDD per `test-driven-development` skill: each Rust module starts with failing unit tests; each Vue component starts with failing Vitest specs.

## Risks

| Risk                                             | Impact | Mitigation                                                                                                             |
| ------------------------------------------------ | ------ | ---------------------------------------------------------------------------------------------------------------------- |
| Built-in catalog goes stale                      | Medium | Catalog data lives in source; reviewed each release. `schema_version` allows future hot updates without code change.   |
| User host lacks Node/Python/etc.                 | Medium | Detection at drawer-open and at install; OS-aware `install_hint` URLs.                                                 |
| `mcp_servers.toml` schema breaks across releases | Low    | Self-identifying header comment; loader is permissive on unknown fields; major schema bumps emit a one-time migration. |
| Trust grant misuse                               | Medium | Default trust only for `Verified`; explicit checkbox elsewhere; tooltip documents the implication.                     |
| Server crashes on start                          | Low    | Existing `auto_restart=true, max_restart_attempts=3`; modal surfaces failure after the cap, never silently.            |
| Concurrent installs race on toml                 | Low    | Installer uses an in-process `Mutex` around the toml file; on-disk write is atomic via temp-file rename.               |
| Catalog id collides with hand-edited entry       | Medium | Installer detects collision, returns `AlreadyInstalled`; UI prompts `Replace` (rewrites toml) or `Cancel`.             |

## Phase Slicing

| Phase               | Scope                                                                                                                                                                                                                                                                                     | Estimate  | Outcome                                                                                               |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- | ----------------------------------------------------------------------------------------------------- |
| Phase 1 (this spec) | `CatalogProvider` trait + `BuiltinCatalogProvider` + ~24 curated entries + `Installer` + 6 AppFacade methods + 6 Tauri commands + 5 events + full Vue surface (Marketplace, CatalogList, CatalogCard, CatalogDetail, InstallProgress, RuntimeMissingHint, InstalledList) + complete tests | ~1 week   | Users can browse and install 24 servers from inside the GUI; missing runtimes produce friendly hints. |
| Phase 2 (future)    | `RemoteCatalogProvider` + `AggregateCatalogProvider` wired into `LocalRuntime` + remote-source preference UI + `DiscoveryCache` re-used for HTTP responses                                                                                                                                | follow-up | Users can opt into a remote registry to discover community servers beyond the built-in list.          |

## Argument Substitution in `InstallSpec.args`

`args` strings may contain `${VAR}` placeholders. These are expanded by the
**installer** (Rust side) before the command is passed to `McpServerDef`, using
values from (in order of precedence):

1. `InstallRequest.env_overrides` (user-supplied via the configure form)
2. `EnvVarSpec.default` for that key
3. Empty string (with a warning event) if the variable is not declared in
   `default_env`

This means `${VAR}` is **not** shell expansion at server start time; it is a
deterministic, OS-independent template substitution at install time. The
resulting concrete `args` and `env` are written to `mcp_servers.toml`.

## Open Questions

None requiring user input before implementation begins. All ambiguities listed
in the brainstorming `<HARD-GATE>` have been pinned by Locked Decisions above.
Implementation will proceed straight into the writing-plans skill on Phase 1
scope.
