# GUI Polish, Marketplace Fix & NaiveUI Removal — Design Spec

**Date:** 2026-05-07
**Branch:** `feat/gui-polish-and-marketplace-fix`
**Type:** Bug fix + UX polish + dependency removal + architectural simplification

## 1. Problem Statement

The Tauri/Vue GUI shipped in v0.16 has multiple observable issues across three categories.

### 1.1 Display & Theme Issues (from user screenshots)

1. **Chat input area renders inconsistently** — input row has no theme-aware surface; bottom border only.
2. **Permissions panel renders as a dark block on a light background** — visually jarring; the component does not follow the active theme.
3. **System color scheme is dark, GUI is light** — the app does not honor `prefers-color-scheme: dark` on first paint.
4. **Settings page shows the workbench status bar** — `profile / sessions / streaming / connected / mode / MCP` are workbench-only signals; they leak into Settings (and Marketplace).
5. **Status bar visual style is incoherent** — some items are dark `<NTag>`s, some are colored `<NTag>`s, some are plain `<NText>`. No single visual language.
6. **Hard-coded colors scattered across components** (`#d7d7d7`, `#0077cc`, `#22a06b`, `#888`, `#f0f0f0`) bypass theme tokens and break in dark mode.

### 1.2 Residual Display Issues (from phase-2/phase-3 investigation)

7. **Nav links are unstyled blue underlines** — `AppLayout.vue` renders `<RouterLink>` inside `<nav>` without scoped CSS.
8. **`main.css` hard-codes `background: #fff; color: #333`** — overrides theme tokens on `html, body, #app`.
9. **StatusBar floats above page bottom** — parent layout lacks full-height flex; child scoped CSS prevents `grid-column` from reaching StatusBar.
10. **Settings "General" tab shows raw i18n key** — `en.json` missing `settings.general`.
11. **StatusBar dot colors are wrong** — `.dot-success/.dot-error/.dot-warning` all reference `--app-primary-color` instead of their respective semantic vars.
12. **SessionsSidebar hard-coded colors** — 15+ hard-coded hex values break in dark mode.
13. **AppLayout missing extended `--app-*` vars** — only 5 of 13 CSS custom properties are exposed.
14. **Chat input textarea too narrow** — wrapping container constrains input; textarea does not fill available width.
15. **Marketplace tab completely blank** — tab activation doesn't navigate to the child route; `<RouterView />` inside tab pane renders nothing.
16. **Marketplace lacks catalog fetch on mount** — `MarketplacePane.onMounted` calls `fetchSources()` but never `fetchCatalog()`.
17. **`[cancelled]` marker too bulky** — oversized alert for an inline status indicator.

### 1.3 Marketplace Backend Issue

18. **Marketplace is non-functional** — `Failed to load catalog: invalid state: marketplace not configured` is shown even though the app ships with a built-in catalog.

### 1.4 Architectural Simplification

19. **NaiveUI dependency is excessive** — the app uses NaiveUI primarily for basic HTML elements (`<NButton>`, `<NCard>`, `<NTag>`, `<NText>`, `<NSpace>`) plus a few services (`useMessage`, `useDialog`). The library adds significant bundle weight and introduces a Provider-stack complexity that causes bugs (issues #3, #14, #15). Replacing NaiveUI with native HTML + CSS variables + lightweight self-built services eliminates an entire class of Provider-related bugs and reduces bundle size.

## 2. Goals & Non-Goals

### Goals

- **Remove NaiveUI entirely** — replace all NaiveUI components and services with native HTML elements, CSS variables, and lightweight self-built composables (`useToast`, `useConfirm`).
- Theme system works correctly on first paint, including `prefers-color-scheme: dark`, using `@vueuse/core`'s `useDark()`.
- All GUI surfaces consistently use `--app-*` CSS variables defined in a pure CSS file (`styles/theme.css`).
- Marketplace is usable out of the box: built-in entries always available, plus preconfigured remote catalog sources.
- Status bar is visually coherent (`label: value` text + colored status dot) and only renders inside the Workbench.
- Marketplace moves from a top-level nav item to a sub-page under Settings.
- All hard-coded colors replaced with CSS variable references.
- Zero remaining `naive-ui` imports in the codebase.

### Non-Goals

- No changes to the agent runtime, model providers, MCP transport, or memory protocol.
- No new MCP servers added — only curating which remote catalog sources are preconfigured.
- No i18n locale additions beyond the keys this change requires.
- No version bump or release in this PR.

## 3. Decisions Locked In With User

| #   | Decision                     | Choice                                                                                      |
| --- | ---------------------------- | ------------------------------------------------------------------------------------------- |
| D1  | Status bar visual style      | **`label: value` text + small status dot** (VSCode-style)                                   |
| D2  | Built-in marketplace content | **Preconfigured remote catalog sources** (a small curated list)                             |
| D3  | Marketplace placement        | **Move under Settings as a 2nd-level page**                                                 |
| D4  | Repair scope                 | **Full sweep**: fix all 19 listed issues + remove NaiveUI                                   |
| D5  | NaiveUI replacement          | **Native HTML + CSS variables** — no new UI framework                                       |
| D6  | Theme management             | **@vueuse/core `useDark()`** — toggles `html.dark` class, pure CSS variable switching       |
| D7  | Toast notification system    | **Self-built** — store-driven `ToastContainer.vue` + `useToast()` composable                |
| D8  | Confirm dialog               | **Self-built** — native `<dialog>` element + `useConfirm()` composable via provide/inject   |
| D9  | Migration strategy           | **Gradual bottom-up** — build infrastructure first, migrate components, then remove NaiveUI |

## 4. Architecture Overview

### 4.1 Theme System

**Before (NaiveUI-based):**

- `NConfigProvider` injects theme → `useThemeVars()` → inline style bindings → `--app-*` CSS vars
- `naive-theme.ts` defines `GlobalThemeOverrides` for light/dark
- Dark mode toggled via `useUiStore().setTheme()` → swaps NaiveUI `darkTheme` object

**After (pure CSS):**

- `styles/theme.css` defines all 13 `--app-*` CSS variables on `:root` (light) and `html.dark` (dark)
- `@vueuse/core`'s `useDark()` toggles the `html.dark` class based on system preference or user choice
- `stores/ui.ts` wraps `useDark()` for the existing `setTheme()` API
- No runtime JS needed for color token resolution — pure CSS cascade

```css
/* styles/theme.css */
:root {
  --app-body-color: #ffffff;
  --app-card-color: #f9fafb;
  --app-border-color: #e5e7eb;
  --app-text-color: #1f2937;
  --app-text-color-2: #6b7280;
  --app-text-color-3: #9ca3af;
  --app-primary-color: #3b82f6;
  --app-success-color: #22c55e;
  --app-warning-color: #f59e0b;
  --app-error-color: #ef4444;
  --app-info-color: #3b82f6;
  --app-hover-color: #f3f4f6;
  --app-code-bg: #f1f5f9;
}

html.dark {
  --app-body-color: #111827;
  --app-card-color: #1f2937;
  --app-border-color: #374151;
  --app-text-color: #f9fafb;
  --app-text-color-2: #9ca3af;
  --app-text-color-3: #6b7280;
  --app-primary-color: #60a5fa;
  --app-success-color: #4ade80;
  --app-warning-color: #fbbf24;
  --app-error-color: #f87171;
  --app-info-color: #60a5fa;
  --app-hover-color: #1f2937;
  --app-code-bg: #1e293b;
}
```

### 4.2 Frontend Layout (after change)

```
AppLayout.vue (simplified — no NaiveUI Provider stack)
├── <nav> (Workbench | Settings)
├── <RouterView />
│    ├── WorkbenchView
│    │    ├── SessionsSidebar
│    │    ├── ChatPanel
│    │    ├── right-sidebar (TraceTimeline + PermissionCenter)
│    │    └── <StatusBar />                  ← workbench-only
│    └── SettingsView
│         ├── tab "General"  (Language / Theme)
│         └── tab "Marketplace" (MarketplacePane inline)
├── <ToastContainer />                       ← replaces NMessageProvider
└── <ConfirmDialog />                        ← replaces NDialogProvider
```

Routing:

```ts
{ path: "/workbench/:sessionId?", name: "workbench", ... },
{ path: "/settings", name: "settings", component: SettingsView },
{ path: "/marketplace", redirect: { name: "settings" } },   // back-compat
```

Marketplace tab is rendered inline in `SettingsView` (not via `<RouterView />`), avoiding the router ↔ tab sync bug (#15).

### 4.3 Global Services (replacing NaiveUI Providers)

#### Toast Notification System (`useMessage()` replacement)

- `stores/ui.ts` — adds `toasts: ref<Toast[]>`, `addToast(message, type, duration?)`, `removeToast(id)`
- `composables/useToast.ts` — wraps `useUiStore().addToast()`, exposes `success()` / `error()` / `info()` / `warning()`
- `components/ToastContainer.vue` — fixed position top-right, `<TransitionGroup>` for enter/leave animation, auto-timeout removal
- Type: `{ id: string, message: string, type: 'success' | 'error' | 'info' | 'warning', duration: number }`

#### Confirm Dialog (`useDialog()` replacement)

- `components/ConfirmDialog.vue` — uses native `<dialog>` element with `showModal()` / `close()`
- `composables/useConfirm.ts` — `useConfirm().confirm({ title, message, confirmText?, cancelText?, type? }): Promise<boolean>`
- Injected via `provide/inject` from `AppLayout.vue`

### 4.4 Marketplace Backend Resilience

Fix policy (Rust side — unchanged from phase-1 design):

- **`list_catalog_sources()`** must always return at least the synthetic `built-in` source descriptor; never error on missing config.
- **`list_catalog(query)`** must always return the merge of built-in + whatever remote sources are configured (possibly zero); never error on missing config.
- **Preconfigured remote sources**: `agent-config` ships with a default `[mcp_marketplace.sources.*]` table containing 2–3 well-known URLs. These act as defaults that can be disabled or removed by the user.
- The user's `~/.config/kairox/kairox.toml` overrides the defaults. If the user has explicitly set `[mcp_marketplace] enabled = false`, we honor it (built-in only).
- `MarketplacePane.onMounted` calls both `fetchSources()` and `fetchCatalog()` (#16).

### 4.5 Status Bar (D1)

Single visual language: `<label>: <value>` text in `var(--app-text-color-3)`, with a 8×8 round dot before the value for binary/health signals:

```
profile: deep • sessions: 2 • streaming: no • connected ●yes • mode: interactive • mcp ●off
```

- All static items: `<span>` with `color: var(--app-text-color-3)`.
- Health items (`connected`, `streaming`, `mcp`): `.status-dot.status-dot--{ok|warn|err|idle}` using `--app-success-color`, `--app-warning-color`, `--app-error-color`.
- StatusBar spans full grid width via `:deep(.status-bar) { grid-column: 1 / -1; }` in `WorkbenchView` (#9).

### 4.6 Settings Page (D3)

`SettingsView.vue` becomes a native tab host (no NaiveUI `<NTabs>`):

- **General** — Language / Theme controls using native `<select>`.
- **Marketplace** — `<MarketplacePane />` rendered inline (not via `<RouterView />`).
- Tab implementation: `<div role="tablist">` + `<button role="tab">` + `<div role="tabpanel">` with `v-model` controlling active tab.

## 5. Component Migration Mapping

### 5.1 Simple Replacements (1:1)

| NaiveUI Component | Replacement                      | CSS class                                                         |
| ----------------- | -------------------------------- | ----------------------------------------------------------------- |
| `<NButton>`       | `<button>`                       | `.btn`, `.btn-primary`, `.btn-danger`, `.btn-ghost`, `.btn-sm`    |
| `<NTag>`          | `<span>`                         | `.tag`, `.tag-success`, `.tag-warning`, `.tag-error`, `.tag-info` |
| `<NText>`         | `<span>`                         | Use `color: var(--app-text-color-2)` / `var(--app-text-color-3)`  |
| `<NSpace>`        | `<div>`                          | `display: flex; gap: ...`                                         |
| `<NEmpty>`        | `<div>`                          | `.empty-state`                                                    |
| `<NDivider>`      | `<hr>`                           | `.divider`                                                        |
| `<NCheckbox>`     | `<label><input type="checkbox">` | native + CSS                                                      |
| `<NInput>`        | `<input>` / `<textarea>`         | native styling                                                    |
| `<NScrollbar>`    | CSS `overflow-y: auto`           | browser native                                                    |
| `<NEllipsis>`     | CSS `text-overflow: ellipsis`    | `.truncate`                                                       |
| `<NSpin>`         | `<span>`                         | `.spinner` (CSS `@keyframes spin`)                                |
| `<NAlert>`        | `<div>`                          | `.alert`, `.alert-warning`, `.alert-error`                        |

### 5.2 Medium Complexity Replacements

| NaiveUI Component         | Replacement                                               | Notes                                  |
| ------------------------- | --------------------------------------------------------- | -------------------------------------- |
| `<NCard>`                 | `<div class="card">`                                      | `.card-header` / `.card-body` children |
| `<NList>` / `<NListItem>` | `<ul class="list">` / `<li>`                              | native list + CSS                      |
| `<NTabs>` / `<NTabPane>`  | `<div role="tablist">` / `role="tab"` / `role="tabpanel"` | `v-model` controls active tab          |
| `<NSelect>`               | `<select>`                                                | native select                          |
| `<NModal>`                | `<dialog>`                                                | `showModal()` / `close()`              |
| `<NDrawer>`               | `<dialog class="drawer">`                                 | right-slide CSS animation              |
| `<NTooltip>`              | `title` attribute or CSS `::after`                        | simple cases use `title`               |
| `<NDescriptions>`         | `<dl>` / `<dt>` / `<dd>`                                  | `.desc-list`                           |

### 5.3 Base Styles File

`styles/components.css` — contains all public CSS classes (`.btn`, `.tag`, `.card`, `.alert`, `.empty-state`, `.spinner`, `.list`, `.desc-list`, `.drawer`, `.divider`, `.truncate`). Imported via `main.ts`. Target: ≤ 300 lines.

### 5.4 Component Difficulty & Migration Order

| Difficulty | Component                    | NaiveUI usage | Migration notes                                        |
| ---------- | ---------------------------- | ------------- | ------------------------------------------------------ |
| 🟢 Low     | `StatusBar.vue`              | 0             | Already native; fix dot colors only                    |
| 🟢 Low     | `WorkbenchView.vue`          | 0             | Already native; fix grid + border                      |
| 🟢 Low     | `McpStatusIndicator.vue`     | 1             | Remove `NTag`                                          |
| 🟢 Low     | `NotificationToast.vue`      | 1             | Replaced by `ToastContainer`                           |
| 🟢 Low     | `TraceEntry.vue`             | 3             | `NEllipsis` → CSS, `NTag` → span                       |
| 🟢 Low     | `TraceTimeline.vue`          | 3             | `NButton` → button, `NScrollbar` → CSS, `NEmpty` → div |
| 🟢 Low     | `TaskSteps.vue`              | 2             | `NTag` → span                                          |
| 🟢 Low     | `RuntimeMissingHint.vue`     | 3             | `NText` → span                                         |
| 🟡 Med     | `ChatPanel.vue`              | 6             | Replace input area + cancelled marker                  |
| 🟡 Med     | `PermissionCenter.vue`       | 4             | `NButton` → button, `NCard` → div                      |
| 🟡 Med     | `SettingsView.vue`           | 5             | Native tabs, `NSelect` → select                        |
| 🟡 Med     | `TaskNode.vue`               | 6             | `NCard`/`NSpace`/`NTag`/`NDivider` → native            |
| 🟡 Med     | `SessionsSidebar.vue`        | 6             | Replace + fix hard-coded colors (#12)                  |
| 🔴 High    | `PermissionPrompt.vue`       | 7             | Full native conversion                                 |
| 🔴 High    | `MemoryBrowser.vue`          | 9             | `NSelect` → select, `useDialog` → useConfirm           |
| 🔴 High    | `McpServerManager.vue`       | 8             | Full native conversion                                 |
| 🔴 High    | `MarketplacePane.vue`        | 9             | Full native conversion + fix #16                       |
| 🔴 High    | `CatalogList.vue`            | 8             | Full native conversion                                 |
| 🔴 High    | `CatalogCard.vue`            | 5             | Full native conversion                                 |
| 🔴 High    | `CatalogDetail.vue`          | 19            | `NDrawer` → dialog.drawer, highest usage               |
| 🔴 High    | `CatalogSourcesSettings.vue` | 28            | Highest NaiveUI usage in codebase                      |
| 🔴 High    | `InstallProgress.vue`        | 4             | `NModal` → dialog                                      |
| 🟡 Med     | `InstalledList.vue`          | 6             | Table already native                                   |

## 6. Migration Phases

### Phase 1 — Infrastructure (no existing component changes)

1. Create `styles/theme.css` (pure CSS variables, light/dark)
2. Create `styles/components.css` (`.btn`, `.tag`, `.card`, etc.)
3. Create `composables/useToast.ts` + `components/ToastContainer.vue`
4. Create `composables/useConfirm.ts` + `components/ConfirmDialog.vue`
5. Modify `stores/ui.ts`: `setTheme()` uses `useDark()`, add toast state
6. Modify `main.ts`: import `theme.css` + `components.css`
7. Fix `main.css`: remove hard-coded `color`/`background` (#8)

NaiveUI still works — new and old systems coexist.

### Phase 2 — Low-complexity component migration (🟢)

- `McpStatusIndicator.vue`, `TraceEntry.vue`, `TraceTimeline.vue`, `TaskSteps.vue`, `RuntimeMissingHint.vue`
- Delete `NotificationToast.vue` (replaced by `ToastContainer`)
- Each component gets its own commit

### Phase 3 — Medium/high-complexity component migration (🟡🔴)

Ordered by dependency:

1. `SessionsSidebar.vue` (uses `useConfirm`, fix #12)
2. `ChatPanel.vue` (fix #1, #14, #17)
3. `PermissionCenter.vue` + `PermissionPrompt.vue` (fix #2)
4. `SettingsView.vue` (native tabs, fix #10, #15)
5. `TaskNode.vue`
6. `MemoryBrowser.vue`
7. `McpServerManager.vue`
8. Marketplace components: `MarketplacePane` → `CatalogList` → `CatalogCard` → `CatalogDetail` → `InstalledList` → `InstallProgress` → `CatalogSourcesSettings`

### Phase 4 — Cleanup & removal

1. Simplify `AppLayout.vue`: remove entire NaiveUI Provider stack, add `<ToastContainer>` + `<ConfirmDialog>`, fix nav styling (#7), full-height flex (#9), extended `--app-*` vars (#13)
2. Delete `styles/naive-theme.ts`
3. Uninstall `naive-ui` from `package.json`
4. Remove `NaiveUiResolver` from `vite.config.ts`
5. Remove NaiveUI config from `vitest.config.ts`
6. Update `test-utils/mount.ts`: remove `withNaiveProviders`, add `confirmDialog` provide mock
7. Remove NaiveUI auto-import presets (`useMessage`, `useDialog`, `useNotification`, `useLoadingBar`)
8. Update `AGENTS.md` (see §8)
9. Update E2E selectors in `tauri-mock.js` if needed

## 7. Marketplace Backend Changes (Rust)

| File                                        | Change                                                                                             |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `crates/agent-mcp/src/catalog/mod.rs`       | `list_sources()` & `list_entries()` never error when no config                                     |
| `crates/agent-mcp/src/catalog/aggregate.rs` | "missing config" becomes "empty remote set"                                                        |
| `crates/agent-config/src/...` (marketplace) | Provide default sources when `[mcp_marketplace]` is missing                                        |
| `apps/agent-gui/src-tauri/src/commands.rs`  | `list_catalog`, `list_catalog_sources`, `refresh_catalog` swallow "not configured" → empty success |

## 8. Build Config & Documentation Updates

### 8.1 Vite Configuration

- Remove `NaiveUiResolver` from `unplugin-vue-components`
- Remove NaiveUI presets from `unplugin-auto-import`
- Keep all other auto-import config unchanged

### 8.2 Dependency Changes

- Remove `naive-ui` from `apps/agent-gui/package.json`
- `@vueuse/core` already present — no new dependencies

### 8.3 Test Utils

- `test-utils/mount.ts`: remove `withNaiveProviders` option, add `confirmDialog` provide mock

### 8.4 AGENTS.md Updates

| Section                | Change                                                                                                                                                                                      |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **UI library**         | "NaiveUI. Provider stack..." → "Native HTML + CSS. Public styles in `styles/components.css`. Theme via `styles/theme.css` CSS variables. Dark/light toggle via `@vueuse/core` `useDark()`." |
| **Auto-imports**       | Remove NaiveUI auto-register and `useMessage`/`useDialog` paragraphs                                                                                                                        |
| **When modifying GUI** | "Prefer NaiveUI components..." → "Use `styles/components.css` classes. Toast via `useToast()`, confirm via `useConfirm()`."                                                                 |
| **Theme**              | "extend naive-theme.ts" → "modify `styles/theme.css` CSS variables"                                                                                                                         |
| **Common pitfalls**    | Remove 5 NaiveUI pitfalls; add: "Import `useToast`/`useConfirm` in components needing notifications or confirmation"                                                                        |

## 9. Data Flow

### Marketplace (fixed)

```
User opens Settings → Marketplace tab
  └─ MarketplacePane mounted
      └─ catalog.fetchSources()     ─► invoke("list_catalog_sources")
                                        └─ Rust: [built-in, ...remote_or_empty]   (NEVER errors)
      └─ catalog.fetchCatalog()     ─► invoke("list_catalog")
                                        └─ Rust: built-in entries + (remote ∨ ∅)  (NEVER errors)
```

### Theme switching

```
User toggles theme / system preference changes
  └─ useDark() detects change
      └─ toggles html.dark class
          └─ CSS variables cascade instantly (no JS)
              └─ all components inherit new colors
```

## 10. Error Handling

- Backend: missing `[mcp_marketplace]` config is **not** an error. `disabled = true` means "skip remote, keep built-in".
- Remote source fetch failure surfaces in `sourceFailures[id]` (per-source warning chip). No global error.
- Toast notifications replace `useMessage()` — same UX, no Provider dependency.
- Confirm dialogs replace `useDialog()` — same Promise-based API.

## 11. Testing Strategy

### Rust

- `crates/agent-mcp/src/catalog/{aggregate,mod}.rs`: assert `list_sources()` returns built-in when no config; assert `list_entries()` likewise.

### Vitest

- `StatusBar.test.ts` — assert each status item renders as `label: value`; assert dot class uses correct semantic color var.
- `SettingsView.test.ts` — assert two tabs ("General", "Marketplace"); assert StatusBar absent.
- `WorkbenchView.test.ts` — assert StatusBar present.
- `ui.test.ts` — assert `isDark` true when system prefers dark on first read.
- `Marketplace.test.ts` — assert built-in source visible when no remote sources returned.
- `ToastContainer.test.ts` — assert toast renders, auto-removes after timeout.
- `ConfirmDialog.test.ts` — assert dialog opens on `confirm()`, resolves true/false.

### Playwright E2E

- `notifications.spec.ts` regression: `Failed to load catalog` toast no longer appears.
- Settings → Marketplace tab renders entries.
- All E2E selectors updated from `.n-button` → `.btn`, `.n-card` → `.card`, etc.

### Validation Checklist (Phase 4 completion)

- [ ] `pnpm ls naive-ui` returns empty
- [ ] `grep -rn "naive-ui" apps/agent-gui/src/` returns empty
- [ ] `grep -rn "NaiveUiResolver" apps/agent-gui/` returns empty
- [ ] `pnpm run lint` passes
- [ ] `just test-gui` passes
- [ ] `just test-e2e` passes
- [ ] `just gui-dev` — all pages visually correct
- [ ] Dark/light theme toggle works
- [ ] AGENTS.md updated

## 12. Migration & Compatibility

- Hash-route deep links to `#/marketplace` keep working via redirect → `#/settings`.
- Existing user `kairox.toml` files are unaffected. Missing `[mcp_marketplace]` section silently gets project-default remote sources.
- No event types or Tauri command signatures change → `just gen-types` is a no-op.
- CSS variable names (`--app-*`) are unchanged — any external CSS that references them continues to work.

## 13. Out-of-Scope Follow-ups

- Adding more built-in MCP server entries (orthogonal — backend `built-in` catalog not expanded here).
- Status bar per-MCP-server health detail (single dot for now; richer popover is future iteration).
- Accessibility audit of native HTML components (good foundation, but full a11y pass is separate work).
- Animation polish for drawer/dialog transitions (functional first, polish later).
  SPECEOF; **aone_exit=$?; pwd -P > '/var/folders/xc/gr5ljf8n5hv8ff1y7557hkj00000gp/T/aone-copilot-cwd-1778149164824-61qleyixhcv.txt' 2>/dev/null; exit $**aone_exit
