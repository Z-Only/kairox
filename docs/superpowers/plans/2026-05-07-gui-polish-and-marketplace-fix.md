# GUI Polish & Marketplace Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair six observable issues in the Tauri/Vue GUI (input area styling, permissions panel theme, broken marketplace, dark-mode mismatch, leaking status bar, incoherent status bar visuals) plus relocate the Marketplace under Settings as a 2nd-level page; harden the theme system so all surfaces follow `prefers-color-scheme`.

**Architecture:** Backend `agent-mcp::catalog` becomes resilient to missing `[mcp_marketplace]` config (always returns built-in source + optional remote sources with project-default URLs). Frontend `AppLayout` exposes more `--app-*` theme tokens, replaces hard-coded colors, and moves `<StatusBar />` from the layout into `WorkbenchView`. Routes nest Marketplace under Settings (`/settings/marketplace`); legacy `/marketplace` redirects.

**Tech Stack:** Rust (agent-mcp, agent-config, tauri commands), Vue 3 + TypeScript + Pinia + NaiveUI + vue-router + vue-i18n + Vitest + Playwright.

**Spec:** `docs/superpowers/specs/2026-05-07-gui-polish-and-marketplace-fix-design.md`

---

## Pre-Flight: Worktree & Baseline

### Task 0: Create isolated worktree

**Files:**

- No file edits — only worktree creation

- [ ] **Step 1: Create the worktree via just**

```bash
just worktree feat/gui-polish-and-marketplace-fix
```

Expected output: `.worktrees/feat-gui-polish-and-marketplace-fix` exists, `pnpm install` completes, husky hooks linked.

- [ ] **Step 2: cd into the worktree for all subsequent tasks**

```bash
cd .worktrees/feat-gui-polish-and-marketplace-fix
```

All subsequent paths in this plan are relative to this worktree.

- [ ] **Step 3: Run baseline test suite to confirm clean starting point**

```bash
cargo test --workspace --all-targets 2>&1 | tail -20
pnpm --filter agent-gui test 2>&1 | tail -20
```

Expected: all tests pass. If any fail, STOP and report to user before proceeding.

---

## Plan Outline (tasks expanded in subsequent sections)

The implementation is decomposed into 11 tasks grouped by layer:

**Backend (Rust):**

- Task 1 — `agent-mcp::catalog` resilience: never error on missing config
- Task 2 — `agent-config` default `[mcp_marketplace]` with 3 preconfigured remote sources
- Task 3 — Tauri commands: defensive wrapping in `commands.rs`

**Frontend foundations (theme + routing):**

- Task 4 — Theme token expansion (`naive-theme.ts` + `AppLayout.vue` `--app-*` vars)
- Task 5 — UI store synchronous dark-mode seed + `<html data-theme>` reflection
- Task 6 — Router: nest Marketplace under Settings, legacy redirect

**Frontend layout:**

- Task 7 — Move `<StatusBar />` from `AppLayout` into `WorkbenchView`
- Task 8 — Settings rewrite: NTabs host (General + Marketplace) + native `<select>` → `<NSelect>`
- Task 9 — Extract MarketplaceView body into `<MarketplacePane />` reusable component

**Frontend polish:**

- Task 10 — `StatusBar.vue`: rewrite to A-style (`label: value` + status dots)
- Task 11 — `ChatPanel.vue` & `PermissionCenter.vue`: hard-coded colors → `--app-*` vars

**Verification:**

- Task 12 — Full local verification (fmt + lint + cargo test + vitest + e2e)

Detailed task definitions follow.

---

## Task 1: Backend — `agent-mcp::catalog` resilience

**Files:**

- Modify: `crates/agent-mcp/src/catalog/aggregate.rs`
- Modify: `crates/agent-mcp/src/catalog/mod.rs` (only if it surfaces the "not configured" error)
- Test: `crates/agent-mcp/src/catalog/aggregate.rs` (`#[cfg(test)] mod tests`)

**Context:** The frontend currently sees `invalid state: marketplace not configured`. Find the originating `Err(...)` and replace it with a graceful fallback: when no remote sources are configured, return an empty list (the built-in source/entries are added by the aggregator).

- [ ] **Step 1: Locate the `not configured` error site**

```bash
grep -RIn "not configured\|MarketplaceNotConfigured" crates/agent-mcp/src crates/agent-config/src apps/agent-gui/src-tauri/src | cat
```

Expected: 1-3 matches. Note exact file/line of each.

- [ ] **Step 2: Read the surrounding context for each match**

For each match, `read_file` the function it lives in (~50 lines around the hit) so you understand whether the site is `list_sources()`, `list_entries()`, `refresh()`, or a constructor.

- [ ] **Step 3: Write a failing unit test**

Add to `crates/agent-mcp/src/catalog/aggregate.rs` under existing `mod tests`:

```rust
#[tokio::test]
async fn list_sources_returns_builtin_when_no_remote_configured() {
    let agg = CatalogAggregator::new(Vec::new());
    let sources = agg.list_sources().await.expect("list_sources must not error when empty");
    assert!(sources.iter().any(|s| s.id == "built-in"),
        "built-in source must always be present, got: {:?}", sources);
}

#[tokio::test]
async fn list_entries_returns_builtin_when_no_remote_configured() {
    let agg = CatalogAggregator::new(Vec::new());
    let entries = agg.list_entries(&Default::default()).await
        .expect("list_entries must not error when empty");
    assert!(!entries.is_empty(), "built-in entries must be present");
    assert!(entries.iter().all(|e| e.source == "built-in"));
}
```

(If `CatalogAggregator::new` has a different signature, adapt; the assertions are the contract.)

- [ ] **Step 4: Run tests to verify failure**

```bash
cargo test -p agent-mcp --lib catalog::aggregate::tests 2>&1 | tail -30
```

Expected: FAIL with the "not configured" error message reproduced.

- [ ] **Step 5: Fix each `not configured` site**

For each file/line from Step 1, replace `Err(McpError::InvalidState(...))` with a graceful path:

```rust
// Before:
return Err(McpError::InvalidState("marketplace not configured".into()));

// After:
// Missing config means "no remote sources" — built-in is always available
// via the BuiltinCatalog branch below; return an empty remote set.
return Ok(Vec::new());
```

For functions returning a single value (e.g., `get_entry`), prefer `Ok(None)` over a synthetic default.

- [ ] **Step 6: Re-run tests, expect PASS**

```bash
cargo test -p agent-mcp --lib catalog 2>&1 | tail -30
```

Expected: PASS for both new tests AND all pre-existing catalog tests (no regression).

- [ ] **Step 7: Run clippy**

```bash
cargo clippy -p agent-mcp --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: zero warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/agent-mcp/src/catalog/
git commit -m "fix(mcp): make catalog resilient to missing marketplace config

When [mcp_marketplace] is absent from kairox.toml, list_sources()
and list_entries() now return the built-in catalog instead of
erroring with 'marketplace not configured'. Remote sources are
optional; the GUI marketplace must work out of the box."
```

---

## Task 2: Backend — preconfigured remote catalog sources in `agent-config`

**Files:**

- Modify: `crates/agent-config/src/<marketplace_module>.rs` (locate first; may be `lib.rs`)
- Modify: `kairox.toml.example`
- Test: same module's existing `#[cfg(test)] mod tests`

**Context:** Spec D5 = ship 3 default remote sources: ① Kairox-official placeholder URL, ② Smithery (already has adapter `remote/smithery.rs`), ③ a `modelcontextprotocol/servers` JSON mirror. Baked into the default config; user can disable individually or set `enabled = false` to opt out.

- [ ] **Step 1: Locate marketplace config struct**

```bash
grep -RIn "mcp_marketplace\|MarketplaceConfig\|MarketplaceDef" crates/agent-config/src | cat
```

Note the struct name and file.

- [ ] **Step 2: Read the struct's current `Default` impl**

`read_file` the file. Confirm whether `Default` already exists; we will modify (or add) it to ship 3 sources.

- [ ] **Step 3: Write a failing unit test**

```rust
#[test]
fn default_marketplace_ships_three_remote_sources() {
    let cfg = MarketplaceConfig::default();
    assert!(cfg.enabled, "default must be enabled");
    let ids: Vec<&str> = cfg.sources.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"kairox-official"), "missing kairox-official, got: {:?}", ids);
    assert!(ids.contains(&"smithery"),        "missing smithery, got: {:?}", ids);
    assert!(ids.contains(&"mcp-servers"),     "missing mcp-servers, got: {:?}", ids);
    assert_eq!(cfg.sources.len(), 3, "expected exactly 3 default sources");
}
```

- [ ] **Step 4: Run, expect FAIL**

```bash
cargo test -p agent-config default_marketplace_ships_three 2>&1 | tail -20
```

- [ ] **Step 5: Implement the default**

```rust
impl Default for MarketplaceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sources: vec![
                CatalogSourceConfig {
                    id: "kairox-official".into(),
                    display_name: "Kairox Official".into(),
                    url: "https://kairox.dev/marketplace/index.json".into(),
                    enabled: true,
                    trust: TrustLevel::Verified,
                },
                CatalogSourceConfig {
                    id: "smithery".into(),
                    display_name: "Smithery".into(),
                    url: "https://smithery.ai/api/v1/registry".into(),
                    enabled: true,
                    trust: TrustLevel::Community,
                },
                CatalogSourceConfig {
                    id: "mcp-servers".into(),
                    display_name: "Model Context Protocol — Servers".into(),
                    url: "https://raw.githubusercontent.com/modelcontextprotocol/servers/main/registry.json".into(),
                    enabled: true,
                    trust: TrustLevel::Community,
                },
            ],
        }
    }
}
```

(Adapt field names / enum variants to whatever the existing struct uses. If a field is named differently, keep the spirit: id, display_name, url, enabled, trust.)

- [ ] **Step 6: Re-run test, expect PASS**

```bash
cargo test -p agent-config default_marketplace_ships_three 2>&1 | tail -20
```

- [ ] **Step 7: Update `kairox.toml.example`**

Append at the end of the file:

```toml
# [mcp_marketplace]
# enabled = true                          # default: true
#
# Three sources are preconfigured by default:
#   - kairox-official  (Verified)
#   - smithery         (Community)
#   - mcp-servers      (Community)
# To override or extend, define [[mcp_marketplace.sources]] entries here.
# Setting `enabled = false` disables ALL remote sources (built-in stays).
```

- [ ] **Step 8: Run all agent-config tests**

```bash
cargo test -p agent-config 2>&1 | tail -20
```

Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add crates/agent-config/ kairox.toml.example
git commit -m "feat(config): preconfigure 3 default marketplace sources

Ships kairox-official, smithery, and modelcontextprotocol/servers
as default remote catalog sources so a fresh install has a usable
marketplace without any manual configuration. Users can disable
individual sources or set enabled=false to opt out entirely."
```

---

## Task 3: Backend — defensive Tauri command wrappers

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs` (functions `list_catalog` @~909, `list_catalog_sources` @~1112, `refresh_catalog` if present)

**Context:** Even after Task 1, defense in depth is cheap: if any future code path bubbles a "not configured" error up to a Tauri command, we still want the GUI to render an empty (but functional) marketplace rather than a red toast.

- [ ] **Step 1: Read each command function**

```bash
grep -n "fn list_catalog\b\|fn list_catalog_sources\|fn refresh_catalog" apps/agent-gui/src-tauri/src/commands.rs | cat
```

Use the line numbers to `read_file` 30-line slices around each.

- [ ] **Step 2: Add a helper near the top of `commands.rs`**

After the existing imports, add:

```rust
/// Convert McpError::InvalidState("...not configured...") into Ok(default()).
/// Other errors propagate unchanged. Keeps the marketplace UI usable when the
/// user has no remote sources configured.
fn degrade_marketplace_not_configured<T: Default>(
    res: Result<T, agent_mcp::McpError>,
) -> Result<T, String> {
    match res {
        Ok(v) => Ok(v),
        Err(agent_mcp::McpError::InvalidState(msg)) if msg.contains("not configured") => {
            tracing::debug!("marketplace not configured; returning default: {}", msg);
            Ok(T::default())
        }
        Err(e) => Err(e.to_string()),
    }
}
```

(If imports differ — e.g., the crate is re-exported as `agent_mcp_crate` — adapt the path. If `tracing` isn't already in scope, add `use tracing;` or drop the log line.)

- [ ] **Step 3: Apply in `list_catalog`, `list_catalog_sources`, and `refresh_catalog`**

Wherever the current code does `.list_catalog(q).await.map_err(|e| e.to_string())`, replace with `degrade_marketplace_not_configured(.list_catalog(q).await)`. Same pattern for the other two functions. For `refresh_catalog` returning `()`, `()` already implements `Default`.

- [ ] **Step 4: cargo check**

```bash
cargo check -p agent-gui-tauri 2>&1 | tail -20
```

Expected: zero errors.

- [ ] **Step 5: cargo clippy**

```bash
cargo clippy -p agent-gui-tauri --all-targets -- -D warnings 2>&1 | tail -20
```

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs
git commit -m "fix(gui): degrade marketplace 'not configured' to empty result

Defense in depth on top of the agent-mcp catalog fix: if any future
code path still bubbles InvalidState('...not configured...') up to
the Tauri command boundary, we now return an empty default instead
of surfacing a red toast in the marketplace UI."
```

---

## Task 4: Theme token expansion

**Files:**

- Modify: `apps/agent-gui/src/styles/naive-theme.ts`
- Modify: `apps/agent-gui/src/layouts/AppLayout.vue`

**Context:** Many components hard-code colors (`#d7d7d7`, `#0077cc`, `#22a06b`, `#888`, `#f0f0f0`). To swap them all to theme-aware tokens we must first expose those tokens. Extend NaiveUI overrides for `successColor` / `warningColor` / `errorColor` / `infoColor`, then expose them on the shell as `--app-*` CSS vars.

- [ ] **Step 1: Extend `naive-theme.ts` light overrides**

In `lightThemeOverrides.common`, append:

```ts
successColor: "#22a06b",
successColorHover: "#34b87f",
warningColor: "#d97706",
warningColorHover: "#f59e0b",
errorColor: "#dc2626",
errorColorHover: "#ef4444",
infoColor: "#0077cc",
infoColorHover: "#1e90ff",
```

- [ ] **Step 2: Extend `naive-theme.ts` dark overrides**

In `darkThemeOverrides.common`, append (slightly desaturated for contrast against dark surfaces):

```ts
successColor: "#34b87f",
successColorHover: "#48cf94",
warningColor: "#f59e0b",
warningColorHover: "#fbbf24",
errorColor: "#ef4444",
errorColorHover: "#f87171",
infoColor: "#60a5fa",
infoColorHover: "#93c5fd",
```

- [ ] **Step 3: Expose them as `--app-*` vars in `AppLayout.vue`**

Replace the existing `:style="{ '--app-body-color': ... }"` block on `<div class="app-shell">` with:

```vue
:style="{ '--app-body-color': themeVars.bodyColor, '--app-card-color': themeVars.cardColor,
'--app-border-color': themeVars.borderColor, '--app-divider-color': themeVars.dividerColor,
'--app-text-color': themeVars.textColor1, '--app-text-color-2': themeVars.textColor2,
'--app-text-color-3': themeVars.textColor3, '--app-primary-color': themeVars.primaryColor,
'--app-success-color': themeVars.successColor, '--app-warning-color': themeVars.warningColor,
'--app-error-color': themeVars.errorColor, '--app-info-color': themeVars.infoColor,
'--app-hover-color': themeVars.hoverColor, '--app-code-bg': isDark ? '#2a2f36' : '#f5f5f5' }"
```

- [ ] **Step 4: Reflect theme on `<html>` for non-Naive surfaces**

In `<script setup>` of `AppLayout.vue`, add:

```ts
watchEffect(() => {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = isDark.value ? "dark" : "light";
  }
});
```

(`watchEffect` is auto-imported in `.vue` files per the auto-import whitelist.)

- [ ] **Step 5: Type-check + lint**

```bash
pnpm --filter agent-gui exec vue-tsc --noEmit 2>&1 | tail -10
pnpm --filter agent-gui run lint 2>&1 | tail -10
```

Expected: zero errors.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/styles/naive-theme.ts apps/agent-gui/src/layouts/AppLayout.vue
git commit -m "feat(gui): expand theme tokens (success/warning/error/info) and html data-theme

Adds light + dark overrides for the standard semantic colors and
exposes them on the shell root as --app-* CSS variables. Also writes
isDark to <html data-theme> so non-Naive surfaces (markdown highlight,
hand-rolled scoped CSS) can switch via CSS attribute selectors."
```

---

## Task 5: UI store synchronous dark-mode seed

**Files:**

- Modify: `apps/agent-gui/src/stores/ui.ts`
- Test: `apps/agent-gui/src/stores/ui.test.ts`

**Context:** `usePreferredDark()` from `@vueuse/core` lazily evaluates the media query, so when `colorMode === "auto"` the very first render of `AppLayout` resolves `isDark` to `false` (the SSR-style fallback) regardless of the system. Result: a flash of light theme even when the OS is dark. Seed it synchronously at store init.

- [ ] **Step 1: Add a failing test**

In `apps/agent-gui/src/stores/ui.test.ts`, add:

```ts
it("isDark reflects system preference on first read when colorMode=auto", () => {
  // Simulate dark system preference BEFORE the store mounts.
  vi.spyOn(window, "matchMedia").mockImplementation(
    (q) =>
      ({
        matches: q.includes("dark"),
        media: q,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn()
      }) as unknown as MediaQueryList
  );

  setActivePinia(createPinia());
  const ui = useUiStore();

  expect(ui.colorMode).toBe("auto");
  expect(ui.isDark).toBe(true); // ← currently false; we want true
});
```

(Imports: `setActivePinia`, `createPinia` from `pinia`; `vi`, `it`, `expect` from `vitest`.)

- [ ] **Step 2: Run, expect FAIL**

```bash
pnpm --filter agent-gui test ui.test 2>&1 | tail -20
```

- [ ] **Step 3: Implement the synchronous seed**

In `apps/agent-gui/src/stores/ui.ts`, replace the `preferredDark` line with:

```ts
// Seed synchronously from window.matchMedia so the very first paint already
// reflects the OS preference. usePreferredDark would otherwise evaluate
// lazily and flash the wrong theme for one frame.
const initialPrefersDark =
  typeof window !== "undefined" &&
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-color-scheme: dark)").matches;

const preferredDark = usePreferredDark();
// Reactive: usePreferredDark stays the source of truth after mount, but we
// override the initial value before any subscriber reads it.
if (preferredDark.value !== initialPrefersDark) {
  // @vueuse exposes a writable ref under the hood; assigning is safe and
  // keeps subsequent media-query change events flowing through.
  (preferredDark as { value: boolean }).value = initialPrefersDark;
}
```

If TypeScript objects to mutating `preferredDark`, replace the assignment branch with:

```ts
const isDark = computed(() => {
  if (colorMode.value !== "auto") return colorMode.value === "dark";
  // Prefer the live ref once it's mounted; fall back to the initial seed
  // for the first synchronous read.
  return preferredDark.value || initialPrefersDark;
});
```

(Pick whichever path the lint accepts; both make the test pass.)

- [ ] **Step 4: Re-run test, expect PASS**

```bash
pnpm --filter agent-gui test ui.test 2>&1 | tail -20
```

- [ ] **Step 5: Run all UI store tests + theme test**

```bash
pnpm --filter agent-gui test ui 2>&1 | tail -20
```

Expected: no regressions.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/stores/ui.ts apps/agent-gui/src/stores/ui.test.ts
git commit -m "fix(gui): seed isDark synchronously from prefers-color-scheme

usePreferredDark evaluates lazily, causing AppLayout to render
light-themed for one frame on first mount even when the OS is dark.
We now read window.matchMedia synchronously at store init so the
very first paint matches the system preference."
```

---

## Task 6: Router — nest Marketplace under Settings

**Files:**

- Modify: `apps/agent-gui/src/router/routes.ts`
- Modify: `apps/agent-gui/src/locales/en.json`, `apps/agent-gui/src/locales/zh-CN.json`

**Context:** Spec D3 = Marketplace becomes a sub-page of Settings. Route table changes; `/marketplace` keeps working as a redirect for back-compat.

- [ ] **Step 1: Read current routes**

```bash
cat apps/agent-gui/src/router/routes.ts | cat
```

- [ ] **Step 2: Rewrite the route table**

Replace the entire routes array with:

```ts
export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: { name: "workbench" } },
  {
    path: "/workbench/:sessionId?",
    name: "workbench",
    component: () => import("@/views/WorkbenchView.vue"),
    props: true
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("@/views/SettingsView.vue"),
    redirect: { name: "settings-general" },
    children: [
      {
        path: "general",
        name: "settings-general",
        component: () => import("@/views/settings/SettingsGeneral.vue")
      },
      {
        path: "marketplace",
        name: "settings-marketplace",
        component: () => import("@/views/settings/SettingsMarketplace.vue")
      }
    ]
  },
  // Back-compat: deep links to the old top-level marketplace still work.
  { path: "/marketplace", redirect: { name: "settings-marketplace" } },
  { path: "/:pathMatch(.*)*", redirect: { name: "workbench" } }
];
```

- [ ] **Step 3: Add the i18n keys**

In `apps/agent-gui/src/locales/en.json`, under the existing `settings` object, add:

```json
"tabGeneral": "General",
"tabMarketplace": "Marketplace"
```

In `apps/agent-gui/src/locales/zh-CN.json`, mirror:

```json
"tabGeneral": "通用",
"tabMarketplace": "市场"
```

- [ ] **Step 4: Remove the Marketplace nav link from `AppLayout.vue`**

In the `<nav class="app-nav">` block, delete the `<RouterLink :to="{ name: 'marketplace' }">` line entirely. Remove the corresponding `nav.marketplace` translation usage if it ends up unused (do NOT delete the i18n key itself — keep it for back-compat with any test that references it).

- [ ] **Step 5: Type-check**

The new component imports (`SettingsGeneral.vue`, `SettingsMarketplace.vue`) don't exist yet — this is intentional. They're created in Tasks 8 and 9. Skip vue-tsc here; we'll run it at the end of those tasks.

- [ ] **Step 6: Commit (incomplete state acknowledged)**

```bash
git add apps/agent-gui/src/router/routes.ts apps/agent-gui/src/locales/ apps/agent-gui/src/layouts/AppLayout.vue
git commit -m "refactor(gui): nest marketplace under settings, drop top-level nav link

Routes /settings/general and /settings/marketplace are added; legacy
/marketplace redirects to /settings/marketplace for back-compat.

NOTE: SettingsGeneral.vue and SettingsMarketplace.vue are introduced
in the next two commits; this commit intentionally does not type-check
on its own — the worktree compiles green again after Task 9."
```

---

## Task 7: Move `<StatusBar />` from AppLayout into WorkbenchView

**Files:**

- Modify: `apps/agent-gui/src/layouts/AppLayout.vue`
- Modify: `apps/agent-gui/src/views/WorkbenchView.vue`
- Test: `apps/agent-gui/src/views/WorkbenchView.test.ts`, `apps/agent-gui/src/views/SettingsView.test.ts` (the latter created in Task 8)

**Context:** Spec #5 = StatusBar shows workbench-only signals (profile, session count, streaming, MCP), so it should not render on Settings/Marketplace.

- [ ] **Step 1: Add a failing test in `WorkbenchView.test.ts`**

```ts
it("renders the StatusBar inside the workbench", () => {
  const wrapper = mountWithPlugins(WorkbenchView);
  expect(wrapper.find('[data-test="status-bar"]').exists()).toBe(true);
});
```

(Use `mountWithPlugins` from `@/test-utils/mount`; existing tests already follow this pattern.)

- [ ] **Step 2: Run, expect FAIL** (StatusBar isn't rendered in WorkbenchView yet)

```bash
pnpm --filter agent-gui test WorkbenchView 2>&1 | tail -20
```

- [ ] **Step 3: Move StatusBar in AppLayout.vue**

Delete the `<StatusBar />` line under `<RouterView />` and remove the `import StatusBar from "@/components/StatusBar.vue"` line.

- [ ] **Step 4: Add it to WorkbenchView.vue**

In the `<script setup>`, add:

```ts
import StatusBar from "@/components/StatusBar.vue";
```

In the template, after the `<aside class="right-sidebar">` block, before `</main>`, insert nothing — instead wrap `<main>` in a `<div class="workbench-shell">` and add the StatusBar at the bottom:

```vue
<template>
  <div class="workbench-shell" data-test="view-workbench">
    <main class="workbench">
      <SessionsSidebar />
      <ChatPanel />
      <aside class="right-sidebar">
        <TraceTimeline />
        <PermissionCenter />
      </aside>
    </main>
    <StatusBar />
  </div>
</template>
```

In the scoped `<style>`, add:

```css
.workbench-shell {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.workbench {
  flex: 1;
  min-height: 0;
}
```

Also fix the hard-coded border in `.right-sidebar`:

```css
.right-sidebar {
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--app-border-color, #d7d7d7);
  overflow: hidden;
}
```

(The `data-test="view-workbench"` attribute moved from `<main>` to the new wrapper — update existing tests if they query `main[data-test="view-workbench"]` specifically. Most use just the attribute selector and continue working.)

- [ ] **Step 5: Re-run WorkbenchView tests, expect PASS**

```bash
pnpm --filter agent-gui test WorkbenchView 2>&1 | tail -20
```

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/layouts/AppLayout.vue apps/agent-gui/src/views/WorkbenchView.vue apps/agent-gui/src/views/WorkbenchView.test.ts
git commit -m "refactor(gui): move StatusBar from AppLayout into WorkbenchView

StatusBar surfaces workbench-only signals (profile, session count,
streaming, MCP) and was leaking into Settings and Marketplace pages.
It now lives inside the workbench view, with the workbench layout
wrapped in a shell div so the status bar pins to the bottom.

The right sidebar border now uses --app-border-color so it adapts
to dark mode."
```

---

## Task 8: SettingsView rewrite — NTabs host (General + Marketplace)

**Files:**

- Modify: `apps/agent-gui/src/views/SettingsView.vue` (becomes a thin shell)
- Create: `apps/agent-gui/src/views/settings/SettingsGeneral.vue` (extracted from current SettingsView body)
- Create: `apps/agent-gui/src/views/settings/SettingsMarketplace.vue` (placeholder; populated in Task 9)
- Modify: `apps/agent-gui/src/views/SettingsView.test.ts`

**Context:** Spec D3 + #5: Settings becomes a NTabs host with two children rendered via `<RouterView>` (matches the nested route table from Task 6). Native `<select>` controls inside General are swapped for `<NSelect>` so they pick up theme automatically.

- [ ] **Step 1: Add a failing test in `SettingsView.test.ts`**

```ts
it("renders two tabs: General and Marketplace", () => {
  const router = createRouter({
    history: createMemoryHistory(),
    routes
  });
  const wrapper = mountWithPlugins(SettingsView, { router });
  const tabs = wrapper.findAll('[data-test^="settings-tab-"]');
  expect(tabs).toHaveLength(2);
  expect(tabs[0].text()).toMatch(/General|通用/);
  expect(tabs[1].text()).toMatch(/Marketplace|市场/);
});

it("does not render the StatusBar inside Settings", () => {
  const wrapper = mountWithPlugins(SettingsView);
  expect(wrapper.find('[data-test="status-bar"]').exists()).toBe(false);
});
```

(Imports: `createRouter`, `createMemoryHistory` from `vue-router`; `routes` from `@/router/routes`.)

- [ ] **Step 2: Run, expect FAIL**

```bash
pnpm --filter agent-gui test SettingsView 2>&1 | tail -20
```

- [ ] **Step 3: Create `apps/agent-gui/src/views/settings/SettingsGeneral.vue`**

```vue
<script setup lang="ts">
import { useUiStore, type ThemeMode, type SupportedLocale } from "@/stores/ui";

const themes = [
  { value: "auto", label: "settings.themeAuto" },
  { value: "light", label: "settings.themeLight" },
  { value: "dark", label: "settings.themeDark" }
] as const;

const locales = [
  { value: "en", label: "settings.localeEn" },
  { value: "zh-CN", label: "settings.localeZh" }
] as const;

const { t } = useI18n();
const ui = useUiStore();
const { locale, colorMode } = storeToRefs(ui);

const themeOptions = computed(() => themes.map((o) => ({ value: o.value, label: t(o.label) })));
const localeOptions = computed(() => locales.map((o) => ({ value: o.value, label: t(o.label) })));
</script>

<template>
  <NSpace vertical :size="16" data-test="settings-general">
    <NFormItem :label="t('settings.locale')" label-placement="left">
      <NSelect
        :value="locale"
        :options="localeOptions"
        data-test="settings-locale"
        style="max-width: 220px"
        @update:value="(v: SupportedLocale) => ui.setLocale(v)"
      />
    </NFormItem>

    <NFormItem :label="t('settings.theme')" label-placement="left">
      <NSelect
        :value="colorMode"
        :options="themeOptions"
        data-test="settings-theme"
        style="max-width: 220px"
        @update:value="(v: ThemeMode) => ui.setTheme(v)"
      />
    </NFormItem>
  </NSpace>
</template>
```

- [ ] **Step 4: Create `apps/agent-gui/src/views/settings/SettingsMarketplace.vue` (placeholder)**

```vue
<script setup lang="ts">
import MarketplacePane from "@/components/marketplace/MarketplacePane.vue";
</script>

<template>
  <div data-test="settings-marketplace">
    <MarketplacePane />
  </div>
</template>
```

(`MarketplacePane.vue` is created in Task 9 — file lint will complain until then. That's fine; we commit at the end of Task 9.)

- [ ] **Step 5: Rewrite `SettingsView.vue` as the tab host**

Replace the full file with:

```vue
<script setup lang="ts">
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const tabs = [
  { name: "settings-general", label: "settings.tabGeneral" },
  { name: "settings-marketplace", label: "settings.tabMarketplace" }
] as const;

const activeTab = computed(() => (route.name as string) ?? "settings-general");

function onTabChange(name: string) {
  router.push({ name });
}
</script>

<template>
  <section class="settings-shell" data-test="view-settings">
    <header class="settings-header">
      <h2>{{ t("settings.title") }}</h2>
    </header>

    <NTabs
      :value="activeTab"
      type="line"
      animated
      size="medium"
      class="settings-tabs"
      @update:value="onTabChange"
    >
      <NTabPane v-for="tab in tabs" :key="tab.name" :name="tab.name">
        <template #tab>
          <span :data-test="`settings-tab-${tab.name.replace('settings-', '')}`">
            {{ t(tab.label) }}
          </span>
        </template>
      </NTabPane>
    </NTabs>

    <div class="settings-content">
      <RouterView />
    </div>
  </section>
</template>

<style scoped>
.settings-shell {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  padding: 16px;
  overflow: hidden;
  background: var(--app-body-color);
  color: var(--app-text-color);
}
.settings-header h2 {
  margin: 0 0 12px;
  font-size: 20px;
}
.settings-content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  margin-top: 12px;
}
</style>
```

- [ ] **Step 6: Run all GUI tests**

```bash
pnpm --filter agent-gui test 2>&1 | tail -30
```

Expected: SettingsView tests PASS; ui.test.ts and WorkbenchView.test.ts still PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src/views/SettingsView.vue apps/agent-gui/src/views/SettingsView.test.ts apps/agent-gui/src/views/settings/
git commit -m "refactor(gui): rewrite SettingsView as NTabs host with nested router

Settings now hosts two tabs (General + Marketplace) backed by the
nested route table from the previous commit. The General tab uses
NSelect for Language and Theme so they pick up the active theme
automatically. The Marketplace tab is a placeholder pointing at
MarketplacePane (introduced in the next commit).

StatusBar is no longer rendered here; the workbench owns it now."
```

---

## Task 9: Extract MarketplacePane component

**Files:**

- Create: `apps/agent-gui/src/components/marketplace/MarketplacePane.vue` (extracted from `MarketplaceView.vue`)
- Modify: `apps/agent-gui/src/views/MarketplaceView.vue` (becomes a thin re-export wrapper for back-compat)

**Context:** The current `MarketplaceView.vue` body (header + Browse/Installed inner tabs + source filter + InstallProgress) needs to live inside `<SettingsMarketplace />`. We extract the body into a reusable `MarketplacePane` component so both the legacy view (kept as a redirect target) and the new Settings sub-page can render it.

- [ ] **Step 1: Read the current MarketplaceView body**

```bash
sed -n '1,120p' apps/agent-gui/src/views/MarketplaceView.vue | cat
```

- [ ] **Step 2: Create `MarketplacePane.vue` by moving the body verbatim**

Copy `<script setup>` + `<template>` + `<style scoped>` from `MarketplaceView.vue` into a new file `apps/agent-gui/src/components/marketplace/MarketplacePane.vue`. One template tweak: the outer `<NCard class="marketplace">` wrapper was forcing a card surface; inside Settings we want it to blend in. Replace the outer `<NCard>` with a plain `<div class="marketplace">` and drop the `:bordered` / `content-style` props. Keep the inner structure unchanged.

Also replace the title `<NText tag="h1">` with `<NText tag="h2">` since it's now nested under Settings's h2.

- [ ] **Step 3: Slim down `MarketplaceView.vue` to a thin wrapper**

Replace the full file with:

```vue
<script setup lang="ts">
// Kept for back-compat: hash-route deep links to #/marketplace redirect to
// #/settings/marketplace (see router/routes.ts). This view is no longer
// reachable through the nav, but if any external link or test resolves it
// directly we still render the same content.
import MarketplacePane from "@/components/marketplace/MarketplacePane.vue";
</script>

<template>
  <MarketplacePane />
</template>
```

- [ ] **Step 4: Update `Marketplace.test.ts` (under `components/marketplace/`)**

If the existing test imports `MarketplaceView`, switch the import to `MarketplacePane` (the assertions should still pass since the body is identical). If it asserts on the outer `<NCard>` styling, update those assertions to query the inner template structure instead.

- [ ] **Step 5: Type-check + lint + tests**

```bash
pnpm --filter agent-gui exec vue-tsc --noEmit 2>&1 | tail -10
pnpm --filter agent-gui run lint 2>&1 | tail -10
pnpm --filter agent-gui test 2>&1 | tail -30
```

Expected: zero errors; all tests green.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/components/marketplace/MarketplacePane.vue apps/agent-gui/src/views/MarketplaceView.vue apps/agent-gui/src/components/marketplace/Marketplace.test.ts
git commit -m "refactor(gui): extract MarketplacePane from MarketplaceView

The marketplace body (Browse/Installed tabs + source filter +
InstallProgress) is now a reusable MarketplacePane component
consumed by both SettingsMarketplace and the legacy
MarketplaceView (kept as a thin wrapper for back-compat with
deep links to #/marketplace)."
```

---

## Task 10: StatusBar — rewrite to A-style (label:value + status dots)

**Files:**

- Modify: `apps/agent-gui/src/components/StatusBar.vue`
- Modify: `apps/agent-gui/src/components/StatusBar.test.ts`
- Modify: `apps/agent-gui/src/locales/en.json`, `apps/agent-gui/src/locales/zh-CN.json`

**Context:** Spec D1 + #6: every status item renders as `label: value` text in `--app-text-color-3`, with a small `.status-dot` colored by state for binary/health signals (`connected`, `streaming`, `mcp`).

- [ ] **Step 1: Add i18n keys**

In `apps/agent-gui/src/locales/en.json` under the existing `statusBar`:

```json
"streamingValueYes": "yes",
"streamingValueNo": "no",
"connectedValueYes": "yes",
"connectedValueNo": "no",
"mcpValueOn": "on",
"mcpValueOff": "off"
```

Mirror in `zh-CN.json` with appropriate translations (`是`/`否`/`开`/`关`).

- [ ] **Step 2: Update `StatusBar.test.ts` with new assertions**

Replace the visual-style assertions with:

```ts
it("renders every status item as label: value text", () => {
  const wrapper = mountWithPlugins(StatusBar);
  const items = wrapper.findAll('[data-test^="status-"]');
  for (const item of items) {
    // Each must contain a `:` separator (the label/value pattern)
    expect(item.text()).toMatch(/:/);
  }
});

it("renders a status dot for binary signals", () => {
  const wrapper = mountWithPlugins(StatusBar);
  const dots = wrapper.findAll(".status-dot");
  // connected, streaming, mcp = 3 dots minimum
  expect(dots.length).toBeGreaterThanOrEqual(3);
});

it("dot color reflects connection state", async () => {
  const wrapper = mountWithPlugins(StatusBar, {
    storeOverrides: {
      session: { connected: false }
    }
  });
  const dot = wrapper.find('[data-test="status-connected"] .status-dot');
  expect(dot.classes()).toContain("status-dot--err");
});
```

(If `mountWithPlugins` doesn't already accept `storeOverrides`, simulate via `createTestingPinia` directly or set `session.connected = false` after mount.)

- [ ] **Step 3: Run, expect FAIL**

```bash
pnpm --filter agent-gui test StatusBar 2>&1 | tail -20
```

- [ ] **Step 4: Rewrite `StatusBar.vue`**

Replace the full file with:

```vue
<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useMcpStore } from "@/stores/mcp";
import McpStatusIndicator from "./McpStatusIndicator.vue";
import McpServerManager from "./McpServerManager.vue";

const { t } = useI18n();
const session = useSessionStore();
const mcp = useMcpStore();
const permissionMode = ref("interactive");
const showMcpManager = ref(false);

type DotState = "ok" | "warn" | "err" | "idle";
const connectedState = computed<DotState>(() => (session.connected ? "ok" : "err"));
const streamingState = computed<DotState>(() => (session.isStreaming ? "warn" : "idle"));
const mcpState = computed<DotState>(() => (mcp.servers && mcp.servers.length > 0 ? "ok" : "idle"));
const mcpValueText = computed(() =>
  mcp.servers && mcp.servers.length > 0 ? t("statusBar.mcpValueOn") : t("statusBar.mcpValueOff")
);

onMounted(async () => {
  try {
    const mode: string = await invoke("get_permission_mode");
    permissionMode.value = mode.toLowerCase();
  } catch {
    permissionMode.value = "interactive";
  }
  try {
    await mcp.fetchServers();
  } catch {
    /* non-critical */
  }
});
</script>

<template>
  <footer class="status-bar" data-test="status-bar">
    <NTooltip trigger="hover">
      <template #trigger>
        <span class="status-item" data-test="status-profile">
          {{ t("statusBar.labelProfile") }}: <strong>{{ session.currentProfile }}</strong>
        </span>
      </template>
      {{ t("status.activeProfile") }}
    </NTooltip>

    <span class="status-item" data-test="status-sessions">
      {{ t("statusBar.labelSessions") }}: <strong>{{ session.sessions.length }}</strong>
    </span>

    <span class="status-item" data-test="status-streaming">
      {{ t("statusBar.labelStreaming") }}:
      <span class="status-dot" :class="`status-dot--${streamingState}`" />
      <strong>{{
        session.isStreaming ? t("statusBar.streamingValueYes") : t("statusBar.streamingValueNo")
      }}</strong>
    </span>

    <span class="status-item" data-test="status-connected">
      {{ t("statusBar.labelConnected") }}:
      <span class="status-dot" :class="`status-dot--${connectedState}`" />
      <strong>{{
        session.connected ? t("statusBar.connectedValueYes") : t("statusBar.connectedValueNo")
      }}</strong>
    </span>

    <span class="status-item" data-test="status-mode">
      {{ t("statusBar.labelMode") }}: <strong>{{ permissionMode }}</strong>
    </span>

    <span
      class="status-item mcp-item"
      data-test="status-mcp"
      @click="showMcpManager = !showMcpManager"
    >
      {{ t("statusBar.labelMcp") }}:
      <span class="status-dot" :class="`status-dot--${mcpState}`" />
      <strong>{{ mcpValueText }}</strong>
      <McpStatusIndicator class="hidden-trigger" />
      <McpServerManager v-if="showMcpManager" @close="showMcpManager = false" />
    </span>
  </footer>
</template>

<style scoped>
.status-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  padding: 4px 16px;
  background: var(--app-card-color);
  border-top: 1px solid var(--app-border-color);
  font-size: 11px;
  color: var(--app-text-color-3);
}
.status-item {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}
.status-item strong {
  color: var(--app-text-color);
  font-weight: 500;
}
.mcp-item {
  cursor: pointer;
}
.hidden-trigger {
  display: none;
}
.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--app-text-color-3);
}
.status-dot--ok {
  background: var(--app-success-color);
}
.status-dot--warn {
  background: var(--app-warning-color);
}
.status-dot--err {
  background: var(--app-error-color);
}
.status-dot--idle {
  background: var(--app-text-color-3);
  opacity: 0.5;
}
</style>
```

- [ ] **Step 5: Add the new label-\* i18n keys referenced above**

In `en.json` under `statusBar`:

```json
"labelProfile": "profile",
"labelSessions": "sessions",
"labelStreaming": "streaming",
"labelConnected": "connected",
"labelMode": "mode",
"labelMcp": "MCP"
```

Mirror in `zh-CN.json` (`配置`/`会话`/`流式`/`连接`/`模式`/`MCP`).

- [ ] **Step 6: Re-run StatusBar tests**

```bash
pnpm --filter agent-gui test StatusBar 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src/components/StatusBar.vue apps/agent-gui/src/components/StatusBar.test.ts apps/agent-gui/src/locales/
git commit -m "feat(gui): unify StatusBar visuals — label:value text + status dots

Every status item now renders as 'label: value' text in --app-text-
color-3 with a small 8px round dot for binary signals (connected,
streaming, MCP). Replaces the previous mix of dark NTags, colored
NTags, and bare text with a single VSCode-style language."
```

---

## Task 11: ChatPanel & PermissionCenter — replace hard-coded colors

**Files:**

- Modify: `apps/agent-gui/src/components/ChatPanel.vue`
- Modify: `apps/agent-gui/src/components/PermissionCenter.vue`

**Context:** Spec #1 + #2 + #7: residual hard-coded colors (`#0077cc`, `#22a06b`, `#7c3aed`, `#888`, `#f0f0f0`) need to switch to `--app-*` vars. The chat input area also needs an explicit theme-aware background so it stops showing white-on-light in dark mode.

- [ ] **Step 1: Patch `ChatPanel.vue` scoped styles**

In the `<style scoped>` block, apply these replacements:

```css
/* role colors → semantic theme tokens */
.message-user .message-role {
  color: var(--app-info-color);
  font-weight: 600;
}
.message-assistant .message-role {
  color: var(--app-success-color);
  font-weight: 600;
}
.message-planner .message-role {
  color: var(--app-info-color);
  font-weight: 600;
}
.message-worker .message-role {
  color: var(--app-success-color);
  font-weight: 600;
}
.message-reviewer .message-role {
  color: var(--app-primary-color);
  font-weight: 600;
}
.message-system .message-role {
  color: var(--app-text-color-3);
  font-weight: 600;
  font-style: italic;
}
.message-system .message-content {
  color: var(--app-text-color-3);
  font-style: italic;
}

/* input area — give it an explicit surface that follows the theme */
.input-area {
  padding: 8px 16px;
  border-top: 1px solid var(--app-border-color);
  background: var(--app-card-color);
}

/* code background uses the new --app-code-bg var */
.markdown-body :deep(:not(pre) > code) {
  background: var(--app-code-bg);
  padding: 2px 4px;
  border-radius: 3px;
  font-size: 12px;
}
```

The `.chat-header` border already uses `var(--app-border-color, #d7d7d7)` — leave as is, but consider dropping the fallback once Task 4 is in place (the var is always defined now). Optional: drop fallbacks for consistency.

- [ ] **Step 2: Patch `PermissionCenter.vue`**

Replace the scoped style block with:

```css
.permission-center {
  border-top: 1px solid var(--app-border-color);
  max-height: 260px;
  overflow-y: auto;
  background: var(--app-card-color);
  color: var(--app-text-color);
}
.permission-center :deep(.n-card-header) {
  padding: 12px 12px 4px;
}
.permission-center :deep(.n-card__content) {
  padding: 4px 12px 12px;
}
.permission-center h2 {
  margin: 0;
  font-size: 14px;
  color: var(--app-text-color);
}
.empty-state {
  color: var(--app-text-color-3);
  font-size: 13px;
}
```

The `<NCard>` itself becomes theme-aware via NaiveUI's tokens; the explicit `background` keeps the panel coherent when the parent (right-sidebar) doesn't provide one.

- [ ] **Step 3: Run all GUI component tests**

```bash
pnpm --filter agent-gui test ChatPanel PermissionCenter 2>&1 | tail -20
```

Expected: no regressions.

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/components/ChatPanel.vue apps/agent-gui/src/components/PermissionCenter.vue
git commit -m "fix(gui): replace hard-coded colors with theme tokens

ChatPanel role colors now use --app-info/success/primary/text-color-*;
the input area gets an explicit --app-card-color background so it
no longer appears as white-on-light in dark mode. PermissionCenter
similarly switches to theme tokens, fixing the dark-block-on-light-
background issue from the screenshots."
```

---

## Task 12: Full local verification

**Files:**

- No code changes — verification only.

**Context:** The pre-PR gate per `AGENTS.md`: `pnpm run format:check`, `pnpm run lint`, `cargo test --workspace --all-targets`, plus the GUI-specific test layers (`vitest`, Playwright e2e, `gen-types`).

- [ ] **Step 1: Update e2e tauri mock for the new resilient marketplace behavior**

Edit `apps/agent-gui/e2e/tauri-mock.js`. Find the handlers for `list_catalog` and `list_catalog_sources`. Ensure they always return at least the built-in source/entries even when the test scenario has not configured remote sources. Concretely:

```js
case "list_catalog_sources":
  return [
    { id: "built-in", display_name: "Built-in", url: null, enabled: true, trust: "verified" },
    ...(state.remoteSources ?? [])
  ];

case "list_catalog":
  return [
    ...builtInCatalogEntries,
    ...(state.remoteEntries ?? [])
  ];
```

(Adapt to whatever shape the existing mock uses; the contract is "never throw, always include built-in".)

- [ ] **Step 2: Format check**

```bash
pnpm run format:check 2>&1 | tail -10
```

If any drift, run `pnpm run format` and stage.

- [ ] **Step 3: Lint**

```bash
pnpm run lint 2>&1 | tail -20
```

- [ ] **Step 4: Type sync (regenerates if needed; should be a no-op since we changed no command signatures or domain types)**

```bash
just check-types 2>&1 | tail -10
```

- [ ] **Step 5: Rust workspace tests**

```bash
cargo test --workspace --all-targets 2>&1 | tail -30
```

Expected: all green.

- [ ] **Step 6: Vitest**

```bash
pnpm --filter agent-gui test 2>&1 | tail -30
```

Expected: all green.

- [ ] **Step 7: Playwright e2e**

```bash
just test-e2e 2>&1 | tail -40
```

Expected: all green. If a previously-passing spec fails because of the marketplace move, update its navigation to go via `#/settings/marketplace` (or use the legacy redirect path).

- [ ] **Step 8: Manual smoke test in `tauri-dev` (if local env has Tauri toolchain)**

```bash
just tauri-dev
```

Verify in the running app:

1. System theme is dark → app is dark on first paint.
2. Chat input area background is theme-coherent (no white-on-light in dark).
3. PermissionCenter blends with right sidebar.
4. Status bar shows `label: value` items + dots; only present on Workbench.
5. Settings → 2 tabs visible; Marketplace tab loads with built-in entries (no red toast).
6. `#/marketplace` deep link redirects to `#/settings/marketplace`.

If any of the above fails, STOP and triage the offending task — do not proceed to commit/push.

- [ ] **Step 9: Final commit (if e2e mock or any drift)**

```bash
git status | cat
# Expected: clean working tree, OR only the e2e mock changes from Step 1.
git add apps/agent-gui/e2e/tauri-mock.js  # if applicable
git commit -m "test(gui): update tauri mock for resilient marketplace contract"
```

- [ ] **Step 10: Push the branch**

```bash
git push -u origin feat/gui-polish-and-marketplace-fix
```

- [ ] **Step 11: Hand off to the user with the PR creation link**

Capture the URL printed by `git push` (looks like `https://github.com/<owner>/kairox/pull/new/feat/gui-polish-and-marketplace-fix`) and surface it in the final message.

---

## Self-Review Notes

This plan covers spec sections 1–9 with the following mapping:

- **Spec §4.1 (frontend layout)** → Tasks 6, 7, 8, 9
- **Spec §4.2 (theme hardening)** → Tasks 4, 5, 11
- **Spec §4.3 (marketplace backend)** → Tasks 1, 2, 3
- **Spec §4.4 (status bar D1)** → Task 10
- **Spec §4.5 (settings page D3)** → Tasks 6, 8, 9
- **Spec §8 (testing strategy)** → Tasks 1, 5, 7, 8, 10, 12
- **Spec §9 (migration)** → Task 6 (legacy `/marketplace` redirect)

Out-of-scope items from spec §10 are explicitly excluded.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-07-gui-polish-and-marketplace-fix.md`. Two execution options:

1. **Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
