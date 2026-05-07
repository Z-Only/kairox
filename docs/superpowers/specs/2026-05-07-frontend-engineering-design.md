# Frontend Engineering Foundation for `agent-gui` — Design Spec

**Date:** 2026-05-07
**Author:** Brainstormed with Aone Copilot via `superpowers:brainstorming`
**Branch:** `feat/frontend-engineering`
**Scope:** `apps/agent-gui/` only (Vue frontend + Vite config + e2e mock). No Rust changes.

---

## 1. Goal

Introduce 7 frontend engineering dependencies into `apps/agent-gui` and complete a full migration of the existing UI:

1. **vue-router** — routing system
2. **pinia** — state management (replacing hand-rolled `reactive({...})` stores)
3. **@vueuse/core** — composable utilities
4. **vue-i18n** — internationalization (en + zh-CN)
5. **NaiveUI** — component library (full migration of all SFCs)
6. **unplugin-auto-import** — auto-import + tree-shaking
7. **unplugin-vue-components** — auto-register components (NaiveUI + own SFCs)

After this work, the GUI is fully aligned with the stack documented in `AGENTS.md` (which already says "Pinia stores" — currently aspirational, this work makes it real), and is ready for future feature development with proper routing, i18n, theming, and a unified component library.

## 2. Non-goals

- No new business features beyond a `Settings` view (locale + theme switcher).
- No SSR / SSG / pre-rendering.
- No alternative theming systems beyond NaiveUI's `themeOverrides` + dark mode.
- No rewrite of `useTauriEvents`, `useNotifications`, `useTraceStore`, `useMarketplace`, `useUpdater` — these have domain semantics worth preserving.
- No changes to `src/generated/` (specta-generated, off-limits).
- No changes to Rust crates (`agent-gui-tauri`, `agent-core`, etc.).

## 3. Decisions Locked from Brainstorming

| ID  | Decision                                                                                                                                                                                                                                                          |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1  | **Single big PR** on branch `feat/frontend-engineering`; branch lifetime ~2-3 weeks.                                                                                                                                                                              |
| Q2  | **Full NaiveUI migration**: all 14 top-level SFCs + 6 marketplace subcomponents + 23 vitest specs + 10 playwright e2e specs.                                                                                                                                      |
| Q3  | **Idiomatic Pinia setup-store** (`defineStore('x', () => { state, getters, actions })`). All components migrate to `useXxxStore() + storeToRefs()`.                                                                                                               |
| Q4  | **Nested routes with hash history**: `/workbench/:sessionId?`, `/marketplace`, `/settings`, fallback redirect to `/workbench`. `createWebHashHistory()` (Tauri-friendly).                                                                                         |
| Q5  | **vue-i18n: extract common copy** — ~30-50 keys under `common.*`, `nav.*`, `settings.*`, `notifications.*`. Default `en`, `zh-CN` available, persisted via `useStorage('kairox.locale', 'en')`. Business-specific copy stays English in this PR; later migration. |
| Q6  | **@vueuse/core**: use `useDark` + `useColorMode` (NaiveUI dark integration), `useStorage` (locale persistence), `useEventListener` (replace bare `listen()` cleanup in `App.vue`). Do NOT rewrite existing domain composables.                                    |
| Q7  | **unplugin-auto-import** allowlist: `vue`, `vue-router`, `pinia`, `@vueuse/core`, `vue-i18n` (`useI18n`), `naive-ui` (only `useDialog`/`useMessage`/`useNotification`/`useLoadingBar`). Business stores stay explicitly imported.                                 |
| Q8  | **unplugin-vue-components**: `NaiveUiResolver()` + `dirs: ['src/components']`. Generated `.d.ts` files are gitignored.                                                                                                                                            |

### Workspace decision

Work happens in the **main worktree** (`/Users/chanyu/AIProjects/kairox`) on a fresh branch `feat/frontend-engineering`. Rationale: opening a separate git worktree would force `src-tauri/target/` to rebuild from scratch (Tauri's Rust target dir is per-worktree), adding 30+ minutes per dev iteration. We accept the small isolation tradeoff for the 5-10x speedup on `tauri-dev` cycles.

---

## 4. Architecture

### 4.1 File structure (after the work)

```
apps/agent-gui/
├── vite.config.ts                  # +AutoImport + Components + NaiveUiResolver
├── package.json                    # +7 deps
├── eslint.config.js  (root)        # +load .eslintrc-auto-import.json globals
├── .gitignore  (root)              # +auto-imports.d.ts, +components.d.ts
├── playwright.config.ts            # baseURL/use.baseURL stays Vite default; specs use '#/...'
├── e2e/
│   ├── tauri-mock.js               # unchanged IPC shape; stays compatible
│   └── *.spec.ts                   # all 10 specs updated for new selectors + hash routes
└── src/
    ├── main.ts                     # createApp + Pinia + router + i18n + mount
    ├── App.vue                     # NConfigProvider + NMessageProvider + NDialogProvider + NNotificationProvider + AppLayout
    ├── env.d.ts                    # unchanged
    ├── auto-imports.d.ts           # generated, gitignored
    ├── components.d.ts             # generated, gitignored
    ├── router/
    │   ├── index.ts                # createRouter + createWebHashHistory
    │   └── routes.ts               # route table
    ├── stores/                     # 6 stores migrated + 1 new ui store
    │   ├── session.ts              # defineStore('session', setup)
    │   ├── taskGraph.ts
    │   ├── agents.ts
    │   ├── mcp.ts
    │   ├── memory.ts
    │   ├── catalog.ts
    │   ├── ui.ts                   # NEW: locale + theme + sidebar collapsed
    │   └── *.test.ts               # all rewritten for new store API
    ├── composables/                # unchanged purpose; internals updated
    │   ├── useTauriEvents.ts       # internally uses useEventListener; uses session store
    │   ├── useTraceStore.ts        # unchanged surface
    │   ├── useNotifications.ts     # backed by NaiveUI useNotification (legacy fn kept as adapter)
    │   ├── useMarketplace.ts       # unchanged surface
    │   └── useUpdater.ts           # unchanged surface
    ├── views/                      # route-level views
    │   ├── WorkbenchView.vue       # 3-pane workbench, reads route.params.sessionId
    │   ├── MarketplaceView.vue     # was components/Marketplace.vue (moved)
    │   └── SettingsView.vue        # NEW: locale + theme switcher + buildInfo
    ├── layouts/
    │   └── AppLayout.vue           # top nav (NMenu) + RouterView + StatusBar + global toasts
    ├── locales/
    │   ├── en.json
    │   ├── zh-CN.json
    │   └── index.ts                # createI18n + ts type augmentation
    ├── styles/
    │   ├── naive-theme.ts          # GlobalThemeOverrides (light & dark)
    │   └── tokens.ts               # color/spacing tokens shared with theme overrides
    ├── components/                 # all 14 SFCs + marketplace/* migrated to NaiveUI
    │   └── *.vue, *.test.ts
    ├── types/                      # unchanged surface
    └── generated/                  # untouched (specta)
```

### 4.2 Dependency direction

```
main.ts
  ↓ creates
[Pinia, Router, i18n, NaiveUI providers]
  ↓ mounts
App.vue (NConfigProvider + global providers)
  ↓ renders
AppLayout (top NMenu nav + RouterView)
  ↓ activates
views/{Workbench,Marketplace,Settings}View
  ↓ composes
components/* (NaiveUI-based SFCs)
  ↓ talks to
stores/* (Pinia setup-stores)
  ↓ wraps
composables/* + Tauri IPC
```

No reverse dependencies. Stores never import components. Components import stores via `useXxxStore()`.

---

## 5. Detailed designs

### 5.1 Pinia: setup-store style

**Why setup-store**: matches Vue 3 Composition API mental model, allows free use of `computed`/`watch` inside, and is what `AGENTS.md` implies.

**Migration pattern (uniform across all 6 stores)**:

- Replace top-level `export const xxxState = reactive({...})` with `export const useXxxStore = defineStore('xxx', () => { ... })`.
- State → `ref(...)` declarations.
- Derived data → `computed(...)`.
- Action functions → plain functions inside the setup, returned in the `return {}`.
- Cross-store dependencies (e.g. `session.ts` imports from `taskGraph.ts`/`agents.ts`) → call `useTaskGraphStore()` lazily inside actions to avoid circular init.
- Components migrate from `import { sessionState } from '../stores/session'` to:
  ```ts
  const session = useSessionStore();
  const { projection, isStreaming } = storeToRefs(session);
  // actions stay non-destructured: session.recoverSessions()
  ```

**Special cases**:

- `streamsByTask = reactive(new Map(...))` → `const streamsByTask = ref(new Map(...))`. Mutations via `streamsByTask.value.set(...)`.
- `setProjection(p)` and similar setters become plain actions.
- `addNotification` (currently a top-level function in `useNotifications.ts`) becomes both:
  1. an action on the new `ui` store (`useUiStore().pushNotification(...)`), and
  2. a thin re-export `export function addNotification(...)` that delegates to the store, so existing call sites in `App.vue`/`session.ts` can be migrated incrementally without breaking the build mid-commit.

**New `ui` store** (added in this work):

```ts
export const useUiStore = defineStore("ui", () => {
  // theme
  const colorMode = useColorMode({ storageKey: "kairox.color-mode" }); // 'auto' | 'light' | 'dark'
  const isDark = useDark();
  // locale
  const locale = useStorage<"en" | "zh-CN">("kairox.locale", "en");
  // sidebar collapse (future-proof)
  const sidebarCollapsed = useStorage("kairox.sidebar-collapsed", false);
  // notifications (replaces module-scope state in useNotifications.ts)
  const notifications = ref<NotificationItem[]>([]);
  function pushNotification(level: NotificationLevel, message: string) {
    /* ... */
  }
  function dismissNotification(id: string) {
    /* ... */
  }
  return {
    colorMode,
    isDark,
    locale,
    sidebarCollapsed,
    notifications,
    pushNotification,
    dismissNotification
  };
});
```

### 5.2 vue-router

**Route table** (`src/router/routes.ts`):

```ts
import type { RouteRecordRaw } from "vue-router";

export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: { name: "workbench" } },
  {
    path: "/workbench/:sessionId?",
    name: "workbench",
    component: () => import("@/views/WorkbenchView.vue"),
    props: true
  },
  {
    path: "/marketplace",
    name: "marketplace",
    component: () => import("@/views/MarketplaceView.vue")
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("@/views/SettingsView.vue")
  },
  { path: "/:pathMatch(.*)*", redirect: { name: "workbench" } }
];
```

**Router setup** (`src/router/index.ts`):

```ts
import { createRouter, createWebHashHistory } from "vue-router";
import { routes } from "./routes";

export const router = createRouter({
  history: createWebHashHistory(),
  routes
});
```

**Vite alias** (`vite.config.ts`): add `resolve.alias = { '@': fileURLToPath(new URL('./src', import.meta.url)) }` for `@/views/...` imports.

**Session ↔ route sync** (in `WorkbenchView.vue`):

- On mount and on `route.params.sessionId` change → call `session.switchSession(routeId)`.
- On `session.currentSessionId` change (when triggered by IPC events, e.g. session created) → `router.replace({ name: 'workbench', params: { sessionId: newId } })` (use `replace` not `push` to keep back/forward intuitive).
- If `routeId` is invalid → push notification "Session not found", redirect to `/workbench` with no param.

**No vue-router auth guards** in this work (no auth model). Future-proofed structure only.

### 5.3 @vueuse/core

Used in:

- `stores/ui.ts`: `useColorMode`, `useDark`, `useStorage`.
- `composables/useTauriEvents.ts`: replace bare `listen()` cleanup with `useEventListener`-style abstraction; or keep `listen()` and just ensure unmounting via `tryOnScopeDispose`. Prefer the latter — `listen()` is Tauri-specific, not a DOM event.
- Various small uses where natural (`useThrottleFn` if scrolling becomes a concern in `TraceTimeline`).

**Constraint**: do not refactor existing composables for the sake of using vueuse. Only adopt where it removes hand-rolled code.

### 5.4 vue-i18n

**Setup** (`src/locales/index.ts`):

```ts
import { createI18n } from "vue-i18n";
import en from "./en.json";
import zhCN from "./zh-CN.json";

type MessageSchema = typeof en;

export const i18n = createI18n<[MessageSchema], "en" | "zh-CN">({
  legacy: false,
  locale: localStorage.getItem("kairox.locale") ?? "en",
  fallbackLocale: "en",
  messages: { en, "zh-CN": zhCN }
});
```

**Locale change**: `useUiStore().locale` watcher → `i18n.global.locale.value = newLocale` (single source of truth = the store; i18n is a derived consumer).

**Key namespaces** (initial set, ~40 keys):

```
common: { send, cancel, confirm, delete, save, edit, retry, close, copy,
          loading, empty, error, ok, yes, no, search, refresh }
nav: { workbench, marketplace, settings }
settings: { title, locale, localeEn, localeZh, theme, themeAuto, themeLight,
            themeDark, build, buildVersion, buildCommit, buildBuiltAt }
notifications: { sessionError, copySuccess, copyFailed }
status: { ready, streaming, connecting, error }
```

**Type-safe `$t`**: `vue-i18n` v9 supports schema augmentation. Add a `vue-i18n.d.ts` (NOT in `generated/`):

```ts
import 'vue-i18n';
import type en from './locales/en.json';
declare module 'vue-i18n' {
  export interface DefineLocaleMessage extends typeof en {}
}
```

**Plurals / interpolation**: not needed for this initial set; use simple flat keys.

### 5.5 NaiveUI

**Provider hierarchy** (`App.vue`):

```vue
<n-config-provider
  :theme="ui.isDark ? darkTheme : null"
  :theme-overrides="themeOverrides"
  :locale="naiveLocale"
>
  <n-loading-bar-provider>
    <n-message-provider>
      <n-dialog-provider>
        <n-notification-provider>
          <AppLayout />
        </n-notification-provider>
      </n-dialog-provider>
    </n-message-provider>
  </n-loading-bar-provider>
</n-config-provider>
```

`naiveLocale` switches with `ui.locale` (e.g. `enUS` ↔ `zhCN` from `naive-ui/lib/locales`).

**Theme overrides** (`src/styles/naive-theme.ts`):

- Map current CSS variables (`--accent: #345566` etc.) into `GlobalThemeOverrides.common.{primaryColor, primaryColorHover, borderColor, cardColor, fontSize}`.
- Provide `lightOverrides` and `darkOverrides` with sensible contrast tweaks.
- Keep `assets/main.css` for non-NaiveUI scoped global rules (font, scrollbar, code blocks for highlight.js).

**Component migration map** (full list):

| Original SFC                         | NaiveUI replacement                                                                              |
| ------------------------------------ | ------------------------------------------------------------------------------------------------ |
| top nav buttons (`App.vue`)          | `NMenu mode="horizontal"`                                                                        |
| `ConfirmDialog.vue`                  | `useDialog().warning()` (singleton wrapper composable)                                           |
| `NotificationToast.vue`              | `useNotification()` driven by `ui.notifications` watch                                           |
| `PermissionPrompt.vue`               | `NModal preset="card"` + `NSpace` + `NButton`                                                    |
| `PermissionCenter.vue`               | `NCollapse` + `NCard`                                                                            |
| `SessionsSidebar.vue`                | `NLayoutSider` + `NScrollbar` + `NList`/`NListItem` + `NButton`                                  |
| `StatusBar.vue`                      | `NLayoutFooter` + `NSpace` + `NTag`                                                              |
| `TraceTimeline.vue`                  | `NScrollbar` + `NTimeline` + `NTimelineItem`                                                     |
| `TraceEntry.vue`                     | `NTimelineItem` content slot                                                                     |
| `TaskSteps.vue`                      | `NCard` + recursive `TaskNode`                                                                   |
| `TaskNode.vue`                       | `NCard size="small"` + `NTag` + `NProgress` (when running)                                       |
| `MemoryBrowser.vue`                  | `NTabs` (by scope) + `NList` + `NEmpty`                                                          |
| `McpServerManager.vue`               | `NDataTable` columns: name, status, transport, actions                                           |
| `McpStatusIndicator.vue`             | `NTag` + `NTooltip` + `NBadge`                                                                   |
| `CatalogSourcesSettings.vue`         | `NList` + `NButton` (add/remove)                                                                 |
| `ChatPanel.vue`                      | `NScrollbar` (history) + markdown-it render (kept) + `NInput type="textarea"` + `NButton` (Send) |
| `marketplace/CatalogList.vue`        | `NGrid` + `NCard`                                                                                |
| `marketplace/CatalogCard.vue`        | `NCard` + `NTag` + `NButton`                                                                     |
| `marketplace/CatalogDetail.vue`      | `NDescriptions` + `NCode`                                                                        |
| `marketplace/InstalledList.vue`      | `NList` + `NButton`                                                                              |
| `marketplace/InstallProgress.vue`    | `NProgress type="line"` + `NText`                                                                |
| `marketplace/RuntimeMissingHint.vue` | `NAlert type="warning"`                                                                          |

**Test-stable selectors**: every NaiveUI-wrapped node that an e2e or unit test needs to target gets a wrapping `<div :data-test="...">` (or `:data-test` directly on the NaiveUI component when it forwards attrs). E2E selectors NEVER depend on NaiveUI internal classes.

### 5.6 unplugin-auto-import

`vite.config.ts` block:

```ts
AutoImport({
  imports: [
    "vue",
    "vue-router",
    "pinia",
    "@vueuse/core",
    { "vue-i18n": ["useI18n"] },
    {
      "naive-ui": ["useDialog", "useMessage", "useNotification", "useLoadingBar", "useThemeVars"]
    }
  ],
  dirs: [], // explicitly empty: business stores stay explicit
  dts: "auto-imports.d.ts",
  vueTemplate: true,
  eslintrc: {
    enabled: true,
    filepath: "./.eslintrc-auto-import.json",
    globalsPropValue: true
  }
});
```

**ESLint integration**: the generated `.eslintrc-auto-import.json` (gitignored) contains a `globals` map. The repo's flat config (`eslint.config.js` at root) gets a small block that loads this JSON and merges its `globals` into the `apps/agent-gui/**` rule. If the file does not exist (fresh clone before `pnpm dev`), the block fallbacks to an empty object — non-fatal.

### 5.7 unplugin-vue-components

`vite.config.ts` block:

```ts
Components({
  resolvers: [NaiveUiResolver()],
  dirs: ["src/components"],
  extensions: ["vue"],
  dts: "components.d.ts",
  deep: true,
  directoryAsNamespace: false // marketplace/CatalogCard stays usable as <CatalogCard/>
});
```

**Naming collisions**: avoid by keeping our component PascalCase names distinct from any `N*` NaiveUI name. Current names (`ChatPanel`, `TaskSteps`, `CatalogCard`, etc.) have no collision.

**Manual import escape hatch**: explicit `import` in `<script setup>` always wins over auto-resolution; useful in tests where we mount without the resolver running.

### 5.8 Vitest harness updates

NaiveUI components require a config provider in tests. Add `src/test-utils/mount.ts`:

```ts
import { mount as baseMount } from "@vue/test-utils";
import { createTestingPinia } from "@pinia/testing"; // peer dep added
import { createI18n } from "vue-i18n";
import { createRouter, createMemoryHistory } from "vue-router";
import en from "@/locales/en.json";
import { routes } from "@/router/routes";

export function mount(comp, options = {}) {
  const pinia = createTestingPinia({ stubActions: false });
  const i18n = createI18n({ legacy: false, locale: "en", messages: { en } });
  const router = createRouter({ history: createMemoryHistory(), routes });
  return baseMount(comp, {
    ...options,
    global: {
      plugins: [pinia, i18n, router],
      stubs: {
        /* NaiveUI stubs only when needed */
      },
      ...(options.global ?? {})
    }
  });
}
```

Test files import `mount` from `@/test-utils/mount` instead of `@vue/test-utils` directly.

### 5.9 Playwright e2e updates

- Update `playwright.config.ts`'s `use.baseURL` if needed (Vite default `http://localhost:1420` stays). Goto patterns become `await page.goto('/#/workbench')`.
- All `data-test` selectors in specs that currently target hand-written elements get re-pointed at the new `<div data-test>` wrappers around NaiveUI components.
- `tauri-mock.js` does not need IPC changes (we did not add new commands), but a small block is added so hash navigation does not break the mock's `appReady` event timing.
- Add a tiny shared `e2e/helpers.ts` exposing `gotoWorkbench(page, sessionId?)`, `gotoMarketplace(page)`, `gotoSettings(page)`.

### 5.10 Vite config (full)

```ts
import { fileURLToPath, URL } from "node:url";
import vue from "@vitejs/plugin-vue";
import AutoImport from "unplugin-auto-import/vite";
import Components from "unplugin-vue-components/vite";
import { NaiveUiResolver } from "unplugin-vue-components/resolvers";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [
    vue(),
    AutoImport({
      /* see 5.6 */
    }),
    Components({
      /* see 5.7 */
    })
  ],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) }
  },
  clearScreen: false,
  server: { port: 1420, host: "0.0.0.0" } // matches existing dev script
});
```

### 5.11 .gitignore additions

At the repo root `.gitignore`:

```
# Auto-generated TypeScript declarations from unplugin-auto-import / unplugin-vue-components
apps/agent-gui/auto-imports.d.ts
apps/agent-gui/components.d.ts
apps/agent-gui/.eslintrc-auto-import.json
```

(These are regenerated at every Vite startup; treating them like `target/` keeps git history clean.)

---

## 6. Error handling & edge cases

| Surface                                    | Behavior                                                                                                                                                                                           |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Route param `:sessionId` does not exist    | `WorkbenchView` calls `session.switchSession(id)` → store catches, pushes notification "Session not found", clears current session, calls `router.replace({ name: 'workbench' })`.                 |
| i18n missing key                           | vue-i18n falls back to `en`; in dev, `console.warn` with the key; in prod, silent fallback.                                                                                                        |
| Locale = unknown value in localStorage     | `useStorage` initializer checks against allowlist `['en', 'zh-CN']`; resets to `'en'` on mismatch.                                                                                                 |
| NaiveUI provider missing                   | App.vue is the single source of providers; tests use `test-utils/mount.ts`. Standalone component import without provider is a programming error caught in code review.                             |
| Auto-imports `.d.ts` missing on cold clone | Vite generates on first `pnpm dev` / `pnpm build`. ESLint config tolerates missing `.eslintrc-auto-import.json`. CI runs `pnpm install` then a build before lint, so the files exist by lint time. |
| Tauri `listen()` listener leaks            | Each `useTauriEvents` registration uses `tryOnScopeDispose` (vueuse) to call the unlisten handle.                                                                                                  |

---

## 7. Testing strategy

**Vitest unit tests** (existing 23 + ~5 new for `ui` store):

- Per store: mount with `createTestingPinia({ stubActions: false })`, exercise actions, assert state/getters.
- Per component: mount via `test-utils/mount.ts` (provides Pinia + i18n + router + NaiveUI providers), drive via `data-test` selectors, assert emits and store mutations. Avoid asserting NaiveUI internal class names.

**Playwright e2e** (existing 10 specs):

- Update navigation calls to hash routes.
- Update selectors to wrapping `<div data-test>` elements.
- Add 1 new spec `settings.spec.ts` covering: open Settings → switch locale → assert nav menu text changes → switch theme → assert `<html>` theme attribute changes.

**Manual smoke** (before merge):

- `just gui-dev` → click through all 3 routes, both locales, both themes.
- `just tauri-dev` → real Tauri shell, verify `tauri-plugin-updater` and `tauri-plugin-process` still work (no regression from main.ts changes).

---

## 8. Commit plan (single branch, multiple commits)

Each commit is independently `pnpm lint` + `pnpm test` + `cargo test --workspace` clean. Branch is merged in one PR at the end.

| #   | Commit message                                                                                                                         | Touches                                                                                                                                  |
| --- | -------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `chore(deps): add pinia, vue-router, vue-i18n, naive-ui, @vueuse/core, unplugin-auto-import, unplugin-vue-components`                  | `package.json`, `pnpm-lock.yaml`                                                                                                         |
| 2   | `feat(gui): bootstrap pinia + vue-router (hash) + i18n + vite config`                                                                  | `main.ts`, `router/`, `locales/`, `vite.config.ts`, `eslint.config.js`, `.gitignore`. App.vue still uses old stores via re-export shims. |
| 3   | `refactor(gui): migrate 6 stores to Pinia setup-store style`                                                                           | `stores/*.ts`, `stores/*.test.ts`, all components' import lines updated. Re-export shims removed.                                        |
| 4   | `feat(gui): add ui store + integrate @vueuse/core (theme, locale, listener)`                                                           | `stores/ui.ts`, `composables/useTauriEvents.ts`, `composables/useNotifications.ts`.                                                      |
| 5   | `feat(gui): add Settings view + locale switcher + theme switcher`                                                                      | `views/SettingsView.vue`, `layouts/AppLayout.vue`, locale json files filled.                                                             |
| 6   | `feat(gui): integrate NaiveUI + theme overrides + provider hierarchy`                                                                  | `App.vue`, `styles/naive-theme.ts`, NaiveUI providers. Components still hand-CSS at this commit.                                         |
| 7   | `refactor(gui): migrate components to NaiveUI (chat, sessions, status, notifications, dialogs, permissions, memory, mcp, marketplace)` | All 14 + 6 SFCs + their test files. The biggest commit; can be split into 2-3 sub-commits if diff > 1500 LOC.                            |
| 8   | `test(gui): update playwright e2e for hash routes + new selectors + settings spec`                                                     | All specs in `e2e/`, new `e2e/helpers.ts`, `e2e/tauri-mock.js`, new `e2e/settings.spec.ts`.                                              |
| 9   | `feat(gui): enable unplugin-auto-import + unplugin-vue-components, clean redundant imports`                                            | `vite.config.ts`, `eslint.config.js`, all SFCs lose their `import { ref } from 'vue'` etc.                                               |
| 10  | `docs(gui): update AGENTS.md GUI section for new stack`                                                                                | `AGENTS.md` (router, ui store, locales, naive-ui, vueuse mentioned).                                                                     |

Conventional commit scope: `gui` for code, `deps` for dep-only commit, `docs` for doc commit. All on branch `feat/frontend-engineering`.

---

## 9. Definition of Done

- [ ] All 10 commits land on `feat/frontend-engineering`; branch fast-forwards onto `main` (or merges via PR).
- [ ] `pnpm install` clean (no peer warnings beyond what main has today).
- [ ] `pnpm run format:check` passes.
- [ ] `pnpm run lint` passes.
- [ ] `cargo test --workspace --all-targets` passes.
- [ ] `just check-types` passes (specta-generated files unchanged).
- [ ] `just test-gui` (vitest) passes; per-file count ≥ existing 23.
- [ ] `just test-e2e` (playwright) passes; per-file count = 11 (10 existing + 1 new settings spec).
- [ ] `just gui-dev` boots; manual smoke of all 3 routes × 2 locales × 2 themes passes.
- [ ] `just tauri-dev` boots; updater + process plugins still work.
- [ ] `apps/agent-gui/auto-imports.d.ts` and `components.d.ts` are gitignored.
- [ ] AGENTS.md updated.
- [ ] No new clippy warnings (`cargo clippy --workspace --all-targets --all-features -- -D warnings`).

---

## 10. Open follow-ups (NOT in this PR)

- Migrate business-specific copy (Trace entries, Task graph labels, Marketplace details) to i18n — separate `feat/i18n-business-copy` branch later.
- Add a richer theme palette and CSS-in-JS via NaiveUI's `useThemeVars()` once colors stabilize.
- Consider `vue-router` data fetching (`beforeRouteEnter`) for session prefetch — only if perceived perf issues arise.
- Move `tauri-mock.js` to TypeScript (`tauri-mock.ts`) once auto-import settles.

---

## 11. References

- Existing AGENTS.md GUI section (already references "Pinia stores" — this PR makes it real).
- specta type generation: `just gen-types` (untouched here; we never edit `src/generated/`).
- Conventional Commits scope `gui` is the only scope used (with one `deps` and one `docs` commit).
