# GUI Polish & Marketplace Fix — Design Spec

**Date:** 2026-05-07
**Branch:** `feat/gui-polish-and-marketplace-fix`
**Type:** Bug fix + UX polish + small architectural change

## 1. Problem Statement

The Tauri/Vue GUI shipped in v0.16 has six observable issues that, taken together, signal an immature theme & layout system and a broken Marketplace experience. From the user's screenshots:

1. **Chat input area renders inconsistently** — input row has no theme-aware surface; bottom border only.
2. **Permissions panel renders as a dark block on a light background** — visually jarring; the component does not follow the active theme.
3. **Marketplace is non-functional** — `Failed to load catalog: invalid state: marketplace not configured` is shown even though the app is supposed to ship with a usable built-in catalog.
4. **System color scheme is dark, GUI is light** — the app does not honor `prefers-color-scheme: dark` on first paint.
5. **Settings page shows the workbench status bar** — `profile / sessions / streaming / connected / mode / MCP` are workbench-only signals; they leak into Settings (and Marketplace).
6. **Status bar visual style is incoherent** — some items are dark `<NTag>`s, some are colored `<NTag>`s, some are plain `<NText>`. No single visual language.

Plus a follow-up item discovered during code review:

7. **Hard-coded colors scattered across components** (`#d7d7d7`, `#0077cc`, `#22a06b`, `#888`, `#f0f0f0`) bypass NaiveUI theme tokens and break in dark mode.

## 2. Goals & Non-Goals

### Goals

- Theme system works correctly on first paint, including `prefers-color-scheme: dark`.
- All GUI surfaces (chat, permissions, status bar, marketplace, settings) consistently use NaiveUI theme tokens via `--app-*` CSS variables.
- Marketplace is usable out of the box: built-in entries always available, plus a small set of preconfigured remote catalog sources (curated by the project) that the user can disable but never has to manually add.
- Status bar is visually coherent (single style: `label: value` text with a small colored status dot for binary signals) and only renders inside the Workbench.
- Marketplace moves from a top-level nav item to a sub-page under Settings (Settings → Marketplace), reflecting that catalog management is configuration, not a daily-use surface.

### Non-Goals

- No changes to the agent runtime, model providers, MCP transport, or memory protocol.
- No new MCP servers added — only curating which remote catalog sources are preconfigured.
- No CSS framework swap; we keep NaiveUI.
- No i18n locale additions beyond the keys this change requires.
- No version bump or release in this PR.

## 3. Decisions Locked In With User

| #   | Decision                     | Choice                                                                           |
| --- | ---------------------------- | -------------------------------------------------------------------------------- |
| D1  | Status bar visual style      | **A. `label: value` text + small status dot** (VSCode-style)                     |
| D2  | Built-in marketplace content | **Preconfigured remote catalog sources** (a small curated list)                  |
| D3  | Marketplace placement        | **Move under Settings as a 2nd-level page**                                      |
| D4  | Repair scope                 | **Full sweep**: fix all 6 listed issues + actively repair hard-coded color leaks |

## 4. Architecture Overview

### 4.1 Frontend layout (after change)

```
AppLayout.vue
├── NConfigProvider(theme, themeOverrides)         ← unchanged shell
├── injects MORE themeVars → --app-*               ← extended (success, warning, info, code-bg)
├── <html data-theme="light|dark">                 ← NEW: top-level marker for non-Naive surfaces
├── nav (Workbench | Settings)                     ← Marketplace removed from top nav
├── <RouterView />
│    ├── WorkbenchView
│    │    ├── SessionsSidebar
│    │    ├── ChatPanel
│    │    ├── right-sidebar(TraceTimeline + PermissionCenter)
│    │    └── <StatusBar />                        ← MOVED here from AppLayout
│    └── SettingsView
│         ├── tab "General"  (Language / Theme)
│         └── tab "Marketplace" (was MarketplaceView contents)
└── NotificationToast
```

Routing change:

```ts
// router/routes.ts
{ path: "/workbench/:sessionId?", name: "workbench", ... },
{ path: "/settings", name: "settings", redirect: { name: "settings-general" }, children: [
    { path: "general",     name: "settings-general",     component: SettingsGeneral },
    { path: "marketplace", name: "settings-marketplace", component: SettingsMarketplace },
]},
// /marketplace stays as a redirect to /settings/marketplace for back-compat with deep links.
{ path: "/marketplace", redirect: { name: "settings-marketplace" } },
```

### 4.2 Theme hardening

- `useUiStore.ts` synchronously seeds `preferredDark` from `window.matchMedia(...)?.matches` at module init, so the first paint in `colorMode = "auto"` already matches the system.
- `AppLayout.vue` writes `document.documentElement.dataset.theme = isDark ? 'dark' : 'light'` so non-Naive surfaces (markdown highlight, code blocks, scrollbars) can switch themes via CSS attribute selectors.
- `--app-*` variables expanded in `AppLayout`:
  ```
  --app-body-color, --app-card-color, --app-border-color,
  --app-text-color, --app-text-color-2, --app-text-color-3,
  --app-primary-color, --app-success-color, --app-warning-color, --app-error-color,
  --app-info-color, --app-hover-color, --app-code-bg
  ```
- All hard-coded colors in `ChatPanel.vue`, `WorkbenchView.vue`, `PermissionCenter.vue`, `StatusBar.vue` switch to these vars (or to NaiveUI components, e.g., `<NTag :type="success">`).
- `naive-theme.ts` gains `successColor`, `warningColor`, `errorColor`, `infoColor` overrides (light + dark) so `themeVars` exposes them.

### 4.3 Marketplace backend resilience

Today the catalog stack reads as: `built-in (always)` + `remote sources (config-driven)` aggregated via `aggregate.rs`. The current frontend errors because some upstream code path returns `InvalidState("marketplace not configured")` when no `[mcp_marketplace]` config exists, instead of falling back to "built-in only".

Fix policy:

- **`list_catalog_sources()`** must always return at least the synthetic `built-in` source descriptor; never error on missing config.
- **`list_catalog(query)`** must always return the merge of built-in + whatever remote sources are configured (possibly zero); never error on missing config.
- **Preconfigured remote sources**: `agent-config` ships with a default `[mcp_marketplace.sources.*]` table containing 2–3 well-known kairox-flavored / MCP-flavored URLs. These act as defaults that can be disabled or removed by the user via `CatalogSourcesSettings.vue`. Concrete URLs are picked in the implementation plan from sources Kairox already cites in code comments (e.g., `modelcontextprotocol/servers` registry mirror).
- The user's `~/.config/kairox/kairox.toml` overrides the defaults. If the user has explicitly set `[mcp_marketplace] enabled = false`, we honor it (built-in only).

### 4.4 Status bar (D1)

Single visual language for every item: `<label>: <value>` text in `--app-text-color-3`, with a 8×8 round dot before the value when the item is a binary/health signal:

```
profile: deep • sessions: 2 • streaming: no • connected ●yes • mode: interactive • mcp ●off
```

- All static items: `<NText depth="3">` wrapping `label: value`.
- Health items (`connected`, `streaming`, `mcp`): same label + a `.status-dot.status-dot--{ok|warn|err|idle}` span before value.
- Removed: dark NTag for `profile`/`streaming`, colored NTag for `connected`. NaiveUI tooltips on hover for context (kept).

### 4.5 Settings page (D3 + #5)

`SettingsView.vue` becomes a `<NTabs>` host:

- **General** — current Language / Theme controls, but native `<select>` swapped for `<NSelect>` so they pick up theme automatically.
- **Marketplace** — entire current `MarketplaceView.vue` content rendered inline. The "Browse / Installed" sub-tabs become an inner `<NTabs>` (already are), so the visual hierarchy is `Settings tabs → Marketplace inner tabs`.

`<StatusBar />` is **not** rendered in Settings.

## 5. Component / File Inventory

### Backend (Rust)

| File                                                | Change                                                                                             |
| --------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `crates/agent-mcp/src/catalog/mod.rs`               | Ensure `list_sources()` & `list_entries()` never error when no config                              |
| `crates/agent-mcp/src/catalog/aggregate.rs`         | Same — "missing config" becomes "empty remote set"                                                 |
| `crates/agent-config/src/...` (marketplace section) | Provide default sources when `[mcp_marketplace]` is missing                                        |
| `apps/agent-gui/src-tauri/src/commands.rs`          | `list_catalog`, `list_catalog_sources`, `refresh_catalog` swallow "not configured" → empty success |

### Frontend (Vue / TS)

| File                                                 | Change                                                                                                                                                 |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `apps/agent-gui/src/main.ts`                         | seed `usePreferredDark` synchronously                                                                                                                  |
| `apps/agent-gui/src/stores/ui.ts`                    | (no API change) ensure `isDark` reflects on first paint                                                                                                |
| `apps/agent-gui/src/styles/naive-theme.ts`           | add success/warning/error/info colors, light + dark                                                                                                    |
| `apps/agent-gui/src/layouts/AppLayout.vue`           | extended `--app-*` vars, `<html data-theme>`, drop StatusBar, drop Marketplace nav link                                                                |
| `apps/agent-gui/src/router/routes.ts`                | nested settings routes, `/marketplace` → redirect                                                                                                      |
| `apps/agent-gui/src/views/WorkbenchView.vue`         | render `<StatusBar />`, replace `border-left: #d7d7d7` w/ var                                                                                          |
| `apps/agent-gui/src/views/SettingsView.vue`          | rewrite as tab host (General + Marketplace)                                                                                                            |
| `apps/agent-gui/src/views/MarketplaceView.vue`       | becomes a thin re-export wrapper for `<MarketplacePane />` reused inside Settings (or move contents into `components/marketplace/MarketplacePane.vue`) |
| `apps/agent-gui/src/components/StatusBar.vue`        | rewrite to A-style; new `.status-dot`                                                                                                                  |
| `apps/agent-gui/src/components/ChatPanel.vue`        | replace hard-coded colors w/ vars; ensure input area surface uses `--app-card-color`                                                                   |
| `apps/agent-gui/src/components/PermissionCenter.vue` | use `var(--app-*)`, remove dark hard-coding, fix layout (no fixed bottom-right)                                                                        |
| `apps/agent-gui/src/locales/{en,zh-CN}.json`         | new keys: `settings.tabGeneral`, `settings.tabMarketplace`, `statusBar.dotOn/dotOff/...`                                                               |
| `apps/agent-gui/e2e/tauri-mock.js`                   | `list_catalog_sources` / `list_catalog` return built-in source even when no remote sources are configured (no longer throws)                           |
| Component tests (`*.test.ts`)                        | add or adjust tests per file change                                                                                                                    |

## 6. Data Flow

For the marketplace fix specifically:

```
User opens Settings → Marketplace tab
  └─ MarketplacePane mounted
      └─ catalog.fetchSources()         ─► invoke("list_catalog_sources")
                                            └─ Rust: returns [built-in, ...remote_or_empty]   (NEVER errors)
      └─ catalog.fetchCatalog()         ─► invoke("list_catalog")
                                            └─ Rust: returns built-in entries + (remote ∨ ∅)  (NEVER errors)
```

Failure mode for individual remote sources is unchanged — they still surface a per-source warning chip, but they no longer collapse the whole panel.

## 7. Error Handling

- Backend: missing `[mcp_marketplace]` config is **not** an error. `disabled = true` is honored as "skip remote, keep built-in".
- A remote source that fails to fetch surfaces in `sourceFailures[id]` (already implemented). No global error.
- The `pushNotification("error", ...)` calls in `catalog.ts` remain for _unexpected_ errors from `invoke`, but the success path no longer triggers them.

## 8. Testing Strategy

- **Rust unit tests** in `crates/agent-mcp/src/catalog/{aggregate,mod}.rs`: assert `list_sources()` returns built-in when no config; assert `list_entries()` likewise.
- **Vitest** for the Vue layer:
  - `StatusBar.test.ts` — assert each status item renders as `label: value`; assert dot class corresponds to state.
  - `SettingsView.test.ts` — assert two tabs ("General", "Marketplace"); assert StatusBar absent.
  - `WorkbenchView.test.ts` — assert StatusBar present.
  - `ui.test.ts` — assert `isDark` true when system prefers dark on first read.
  - `Marketplace.test.ts` — assert built-in source visible when no remote sources are returned.
- **Playwright e2e**:
  - `notifications.spec.ts` regression: `Failed to load catalog` toast no longer appears.
  - new `settings-marketplace.spec.ts`: Settings → Marketplace tab renders entries.
- **Manual smoke** in `tauri-dev`: switch system theme, confirm GUI follows.

## 9. Migration & Compatibility

- Hash-route deep links to `#/marketplace` keep working via redirect → `#/settings/marketplace`.
- Existing user `kairox.toml` files are unaffected. If they had no `[mcp_marketplace]` section, they now silently get the project-default remote sources; the user can disable any of them in `CatalogSourcesSettings`.
- No event types, no Tauri command signatures change → `just gen-types` is a no-op.

## 10. Out-of-Scope Follow-ups

- Adding more built-in MCP server entries (orthogonal — backend `built-in` catalog itself isn't being expanded here).
- Status bar showing per-MCP-server health detail (currently a single dot; richer popover is a future iteration).
- Migrating other hard-coded colors in `TraceTimeline`, `TaskNode`, `MemoryBrowser` — reasonable to include opportunistically if the same component is touched, but not a blocker.
