# Frontend Engineering Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce 7 frontend engineering deps (vue-router, pinia, @vueuse/core, vue-i18n, naive-ui, unplugin-auto-import, unplugin-vue-components) into `apps/agent-gui`, fully migrating stores to Pinia setup-store style and all SFCs to NaiveUI.

**Architecture:** Single feature branch `feat/frontend-engineering` with ~10 sequential commits, each independently lint/test-clean. Each task corresponds to one commit. The branch is merged in a single PR after all tasks complete.

**Tech Stack:** Vue 3.5 + TS 6 + Vite 8 + Tauri 2 + Vitest 4 + Playwright 1.59 + pnpm 10. New: pinia 2, vue-router 4, vue-i18n 9, naive-ui 2, @vueuse/core 11, unplugin-auto-import 0.18+, unplugin-vue-components 0.27+.

**Spec:** `docs/superpowers/specs/2026-05-07-frontend-engineering-design.md`

**Linked decisions (from brainstorming):** All 14 + 6 SFCs migrate to NaiveUI; setup-store Pinia; nested hash routes; common-copy i18n with en + zh-CN; auto-import allowlist `vue, vue-router, pinia, @vueuse/core, vue-i18n, naive-ui (selected hooks)`.

**Commit message constraints (verified against repo `commitlint.config.js` which extends `@commitlint/config-conventional`):**

- **header-max-length: 100 characters** (the entire first line `<type>(<scope>): <subject>` must be ≤100 chars)
- **type-enum:** `build, chore, ci, docs, feat, fix, perf, refactor, revert, style, test`
- **scope-enum (allowed):** `core, runtime, models, tools, memory, store, config, tui, gui, deps, mcp, ci`
- **subject-case:** lower-case
- All commit messages in this plan have been pre-verified ≤100 chars. If you hit a `commitlint` failure, do NOT use `--no-verify`; report back NEEDS_CONTEXT instead.

---

## Pre-flight (do this once, not a numbered task)

- [ ] **PF-1: Verify clean baseline on `main`**

  ```bash
  cd /Users/chanyu/AIProjects/kairox
  git status
  ```

  Expected: working tree clean (or only the two committed brainstorming/plan markdown files staged).

- [ ] **PF-2: Create feature branch from `main`**

  ```bash
  git checkout -b feat/frontend-engineering main
  ```

  Expected: `Switched to a new branch 'feat/frontend-engineering'`. Confirm with `git branch --show-current` → `feat/frontend-engineering`.

- [ ] **PF-3: Run baseline tests to confirm green starting point**

  ```bash
  pnpm install
  pnpm --filter agent-gui run test
  cargo test --workspace --all-targets 2>&1 | tail -20
  ```

  Expected: vitest reports current pass count (~23 specs) all green; cargo reports all tests passing.

  **If anything fails:** stop. Investigate before starting Task 1. Report failures as a separate issue.

---

## Task 1: Add 7 dependencies (commit 1)

**Branch:** `feat/frontend-engineering`
**Commit message header (≤100 chars, verified 64):** `chore(deps): add frontend foundation deps for naive-ui migration`
**Commit message body (provide via `-m` second arg):**

```
- runtime: pinia, vue-router, vue-i18n, naive-ui, @vueuse/core
- dev: @pinia/testing, unplugin-auto-import, unplugin-vue-components
```

**Why first:** every later task depends on these being present in `node_modules` and `pnpm-lock.yaml`. Add them all in one commit so reviewers see the dep delta in one place.

**Files:**

- Modify: `apps/agent-gui/package.json`
- Modify: `pnpm-lock.yaml` (auto-updated by `pnpm install`)

- [ ] **Step 1: Add runtime deps to `apps/agent-gui/package.json`**

  Open `apps/agent-gui/package.json`. Find the `"dependencies"` block. Add these entries (alphabetical), keep existing ones:

  ```json
  "dependencies": {
    "@tauri-apps/api": "^2.11.0",
    "@tauri-apps/plugin-process": "^2.3.1",
    "@tauri-apps/plugin-updater": "^2.10.1",
    "@vueuse/core": "^11.3.0",
    "highlight.js": "^11.11.1",
    "markdown-it": "^14.1.1",
    "naive-ui": "^2.40.4",
    "pinia": "^2.3.0",
    "vue": "^3.5.33",
    "vue-i18n": "^9.14.2",
    "vue-router": "^4.5.0"
  }
  ```

- [ ] **Step 2: Add devDeps for unplugin + Pinia testing helper**

  In the same file, find `"devDependencies"` and add (keep existing entries):

  ```json
  "devDependencies": {
    "@pinia/testing": "^0.1.7",
    "@playwright/test": "^1.59.1",
    "@tauri-apps/cli": "^2.11.0",
    "@types/markdown-it": "^14.1.2",
    "@vitejs/plugin-vue": "^6.0.6",
    "@vitest/coverage-v8": "^4.1.5",
    "@vue/test-utils": "^2.4.10",
    "jsdom": "^29.1.1",
    "typescript": "^6.0.3",
    "unplugin-auto-import": "^0.18.6",
    "unplugin-vue-components": "^0.27.5",
    "vite": "^8.0.10",
    "vitest": "^4.1.5",
    "vue-tsc": "^3.2.7"
  }
  ```

- [ ] **Step 3: Install and update lockfile**

  ```bash
  cd /Users/chanyu/AIProjects/kairox
  pnpm install
  ```

  Expected: pnpm resolves all new deps, updates `pnpm-lock.yaml`. No peer-dep ERRORS (warnings about optional/legacy deps are fine — same noise level as current `main`).

  **If a peer-dep error blocks install:** check the version against the official npm page (`pnpm view <pkg> peerDependencies`) and bump if necessary. The constraint is "no errors above what `main` already has".

- [ ] **Step 4: Verify base build still passes (sanity check, no code changes yet)**

  ```bash
  pnpm --filter agent-gui run test
  pnpm --filter agent-gui run build
  ```

  Expected: vitest still passes (no source changed); `vite build` produces `apps/agent-gui/dist/` without errors. The new deps are in the dependency graph but not yet used, so build size grows slightly (NaiveUI is tree-shaken away by lack of imports).

- [ ] **Step 5: Commit**

  ```bash
  git add apps/agent-gui/package.json pnpm-lock.yaml
  git commit \
    -m "chore(deps): add frontend foundation deps for naive-ui migration" \
    -m "- runtime: pinia, vue-router, vue-i18n, naive-ui, @vueuse/core" \
    -m "- dev: @pinia/testing, unplugin-auto-import, unplugin-vue-components"
  ```

  Expected: husky pre-commit fires `prettier --write` on `package.json`, commit-msg hook accepts (header is 64 chars, well under the 100-char limit), commit succeeds. `git log --oneline -1 | cat` shows the new commit.

---

## Task 2: Bootstrap Pinia + vue-router + i18n in main.ts (commit 2)

**Branch:** `feat/frontend-engineering`
**Commit message:** `feat(gui): bootstrap pinia, vue-router (hash), and i18n; add @ alias`
**Why second:** wires the plugins so subsequent commits have somewhere to register stores/routes/locales. Old hand-written stores keep working unchanged; we only add new infrastructure.

**Files:**

- Create: `apps/agent-gui/src/router/index.ts`
- Create: `apps/agent-gui/src/router/routes.ts`
- Create: `apps/agent-gui/src/locales/en.json`
- Create: `apps/agent-gui/src/locales/zh-CN.json`
- Create: `apps/agent-gui/src/locales/index.ts`
- Create: `apps/agent-gui/src/locales/vue-i18n.d.ts`
- Modify: `apps/agent-gui/src/main.ts`
- Modify: `apps/agent-gui/vite.config.ts`
- Modify: `apps/agent-gui/tsconfig.json` (add `paths` for `@/*`)
- Modify: `apps/agent-gui/src/env.d.ts` (declare `*.json` if needed)

- [ ] **Step 1: Add Vite alias `@ → src`**

  Replace the entire content of `apps/agent-gui/vite.config.ts` with:

  ```ts
  import { fileURLToPath, URL } from "node:url";
  import vue from "@vitejs/plugin-vue";
  import { defineConfig } from "vite";

  export default defineConfig({
    plugins: [vue()],
    resolve: {
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url))
      }
    },
    clearScreen: false,
    server: { port: 1420, host: "0.0.0.0" }
  });
  ```

  (unplugin plugins are added in Task 9. We isolate config edits per concern.)

- [ ] **Step 2: Add TS path alias**

  Edit `apps/agent-gui/tsconfig.json`. Add a `paths` entry under `compilerOptions`:

  ```json
  {
    "compilerOptions": {
      "target": "ES2022",
      "module": "ESNext",
      "moduleResolution": "Bundler",
      "strict": true,
      "jsx": "preserve",
      "sourceMap": true,
      "resolveJsonModule": true,
      "isolatedModules": true,
      "lib": ["ES2022", "DOM", "DOM.Iterable"],
      "types": ["vitest/globals"],
      "baseUrl": ".",
      "paths": {
        "@/*": ["src/*"]
      }
    },
    "include": ["src/**/*.ts", "src/**/*.vue", "src/**/*.json"]
  }
  ```

  (Adding `src/**/*.json` so locale files type-check cleanly.)

- [ ] **Step 3: Create the route table**

  Create `apps/agent-gui/src/router/routes.ts`:

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

  Note: `WorkbenchView`, `MarketplaceView`, `SettingsView` do NOT exist yet — Vite/TS only complains at runtime navigation, not at build of router itself. We create them in Task 5/6/7.

- [ ] **Step 4: Create the router instance**

  Create `apps/agent-gui/src/router/index.ts`:

  ```ts
  import { createRouter, createWebHashHistory } from "vue-router";
  import { routes } from "./routes";

  export const router = createRouter({
    history: createWebHashHistory(),
    routes
  });
  ```

- [ ] **Step 5: Create the locale files (full common-copy set; consumed by Task 5 + Task 7)**

  Create `apps/agent-gui/src/locales/en.json`:

  ```json
  {
    "common": {
      "send": "Send",
      "cancel": "Cancel",
      "confirm": "Confirm",
      "delete": "Delete",
      "save": "Save",
      "edit": "Edit",
      "retry": "Retry",
      "close": "Close",
      "copy": "Copy",
      "loading": "Loading…",
      "empty": "Nothing here yet",
      "error": "Error",
      "ok": "OK",
      "yes": "Yes",
      "no": "No",
      "search": "Search",
      "refresh": "Refresh"
    },
    "nav": {
      "workbench": "Workbench",
      "marketplace": "Marketplace",
      "settings": "Settings"
    },
    "settings": {
      "title": "Settings",
      "locale": "Language",
      "localeEn": "English",
      "localeZh": "中文（简体）",
      "theme": "Theme",
      "themeAuto": "System",
      "themeLight": "Light",
      "themeDark": "Dark",
      "build": "Build",
      "buildVersion": "Version",
      "buildCommit": "Commit",
      "buildBuiltAt": "Built at"
    },
    "notifications": {
      "sessionError": "Session error",
      "copySuccess": "Copied to clipboard",
      "copyFailed": "Copy failed",
      "sessionNotFound": "Session not found, redirected"
    },
    "status": {
      "ready": "Ready",
      "streaming": "Streaming",
      "connecting": "Connecting",
      "error": "Error"
    }
  }
  ```

  Create `apps/agent-gui/src/locales/zh-CN.json`:

  ```json
  {
    "common": {
      "send": "发送",
      "cancel": "取消",
      "confirm": "确认",
      "delete": "删除",
      "save": "保存",
      "edit": "编辑",
      "retry": "重试",
      "close": "关闭",
      "copy": "复制",
      "loading": "加载中…",
      "empty": "暂无内容",
      "error": "错误",
      "ok": "好",
      "yes": "是",
      "no": "否",
      "search": "搜索",
      "refresh": "刷新"
    },
    "nav": {
      "workbench": "工作台",
      "marketplace": "应用市场",
      "settings": "设置"
    },
    "settings": {
      "title": "设置",
      "locale": "语言",
      "localeEn": "English",
      "localeZh": "中文（简体）",
      "theme": "主题",
      "themeAuto": "跟随系统",
      "themeLight": "浅色",
      "themeDark": "深色",
      "build": "构建信息",
      "buildVersion": "版本",
      "buildCommit": "提交",
      "buildBuiltAt": "构建时间"
    },
    "notifications": {
      "sessionError": "会话错误",
      "copySuccess": "已复制到剪贴板",
      "copyFailed": "复制失败",
      "sessionNotFound": "会话不存在，已跳转"
    },
    "status": {
      "ready": "就绪",
      "streaming": "传输中",
      "connecting": "连接中",
      "error": "错误"
    }
  }
  ```

- [ ] **Step 6: Create the i18n instance**

  Create `apps/agent-gui/src/locales/index.ts`:

  ```ts
  import { createI18n } from "vue-i18n";
  import en from "./en.json";
  import zhCN from "./zh-CN.json";

  export type SupportedLocale = "en" | "zh-CN";

  const STORAGE_KEY = "kairox.locale";

  function detectInitialLocale(): SupportedLocale {
    if (typeof window === "undefined") return "en";
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return stored === "zh-CN" || stored === "en" ? stored : "en";
  }

  export const i18n = createI18n({
    legacy: false,
    locale: detectInitialLocale(),
    fallbackLocale: "en",
    messages: { en, "zh-CN": zhCN }
  });
  ```

- [ ] **Step 7: Add type-safe i18n schema augmentation**

  Create `apps/agent-gui/src/locales/vue-i18n.d.ts`:

  ```ts
  import "vue-i18n";
  import type en from "./en.json";

  declare module "vue-i18n" {
    // eslint-disable-next-line @typescript-eslint/no-empty-object-type
    export interface DefineLocaleMessage extends Record<string, never> {}
  }

  declare module "vue-i18n" {
    export interface DefineLocaleMessage extends Omit<typeof en, never> {}
  }
  ```

  (Two declare blocks: first satisfies the TS lint about empty interfaces; second installs the schema.)

- [ ] **Step 8: Wire Pinia + router + i18n into main.ts**

  Replace `apps/agent-gui/src/main.ts` entirely with:

  ```ts
  import { createApp } from "vue";
  import { createPinia } from "pinia";
  import App from "./App.vue";
  import { router } from "./router";
  import { i18n } from "./locales";
  import "./assets/main.css";
  import "highlight.js/styles/github-dark.css";

  const app = createApp(App);
  app.use(createPinia());
  app.use(router);
  app.use(i18n);
  app.mount("#app");
  ```

- [ ] **Step 9: Build and run unit tests to confirm nothing is broken**

  ```bash
  pnpm --filter agent-gui run build
  pnpm --filter agent-gui run test
  ```

  Expected: `vite build` succeeds (the dynamic `import("@/views/...")` chunks emit lazy chunks but referenced views don't exist yet — Vite only warns at runtime navigation; build still passes); vitest still ≥23 specs pass (no source files modified).

  **If `vite build` fails** with "Cannot find module '@/views/WorkbenchView.vue'": that's a runtime-import error — should not break build. If it does, change router/routes.ts dynamic imports to `() => import(/* @vite-ignore */ "@/views/WorkbenchView.vue")` temporarily.

- [ ] **Step 10: Lint & format**

  ```bash
  pnpm --filter agent-gui exec prettier --write src/router src/locales src/main.ts vite.config.ts tsconfig.json
  pnpm run lint:eslint -- apps/agent-gui/src/router apps/agent-gui/src/locales apps/agent-gui/src/main.ts
  ```

  Expected: prettier writes formatted files; eslint reports no errors on the new files.

- [ ] **Step 11: Commit**

  ```bash
  git add apps/agent-gui/src/router apps/agent-gui/src/locales apps/agent-gui/src/main.ts \
          apps/agent-gui/vite.config.ts apps/agent-gui/tsconfig.json
  git commit -m "feat(gui): bootstrap pinia, vue-router (hash), and i18n; add @ alias"
  ```

  Expected: husky runs lint-staged on the new files; commit succeeds.

---

## Task 3: Migrate 6 stores to Pinia setup-store style (commit 3)

**Branch:** `feat/frontend-engineering`
**Commit message:** `refactor(gui): migrate stores to pinia setup-store style`
**Why third:** stores have the most consumers (every component) — getting them stable early means later commits have a stable substrate. We do all 6 stores + their `*.test.ts` + `App.vue`'s store imports in one commit so the codebase is never half-migrated.

**Files (all under `apps/agent-gui/src/`):**

- Modify: `stores/session.ts` (298 lines)
- Modify: `stores/taskGraph.ts` (114 lines)
- Modify: `stores/agents.ts` (147 lines)
- Modify: `stores/mcp.ts` (150 lines)
- Modify: `stores/memory.ts` (53 lines)
- Modify: `stores/catalog.ts` (273 lines)
- Modify: `stores/session.test.ts`, `taskGraph.test.ts`, `agents.test.ts`, `mcp.test.ts`, `memory.test.ts`, `catalog.test.ts`, `session-ipc.test.ts`
- Modify: `composables/useTraceStore.ts` (consumer of `sessionState` if any)
- Modify: `composables/useTauriEvents.ts` (consumer of `sessionState`)
- Modify: `composables/useNotifications.ts` (currently module-scope state; this commit wraps it in a thin store)
- Modify: `App.vue`
- Modify: `components/SessionsSidebar.vue`, `ChatPanel.vue`, `TaskSteps.vue`, `TaskNode.vue`, `TraceTimeline.vue`, `PermissionPrompt.vue`, `PermissionCenter.vue`, `MemoryBrowser.vue`, `McpServerManager.vue`, `McpStatusIndicator.vue`, `StatusBar.vue`, `NotificationToast.vue`, `ConfirmDialog.vue`, `CatalogSourcesSettings.vue`, `marketplace/*.vue`
- Create: `apps/agent-gui/src/test-utils/mount.ts`

**Existing store inventory (verified by `grep -rn` against `apps/agent-gui/src` + `e2e/` on the baseline commit):**

| Old export                                                                                                                                                                               | Current shape                 | Consumers (must all be migrated in this commit)                                                                             |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `sessionState` (reactive)                                                                                                                                                                | `stores/session.ts:31`        | `App.vue`, `composables/useTauriEvents.ts`, `stores/session-ipc.test.ts`, plus internal use in `stores/session.ts` itself   |
| `recoverSessions`, `setProjection`, `resetProjection`, `applyEvent`, `reportSendError`, `deleteSession`, `renameSession`, `streamsByTask`                                                | `stores/session.ts` top-level | `App.vue`, `useTauriEvents.ts`, `session-ipc.test.ts`, `ChatPanel.vue` (cancel/send), `SessionsSidebar.vue` (rename/delete) |
| `taskGraphState` (reactive)                                                                                                                                                              | `stores/taskGraph.ts`         | `useTauriEvents.ts`, `taskGraph.test.ts`, `session-ipc.test.ts:23` (mock object)                                            |
| `agentState`, `clearAgents`, `applyAgentEvent`                                                                                                                                           | `stores/agents.ts`            | `useTauriEvents.ts`, `agents.test.ts`, `taskGraph.test.ts:8`                                                                |
| `mcpState`, `runningServers`, `failedServers`, `runningCount`, `hasServers`, `fetchServers`, `startServer`, `stopServer`, `trustServer`, `revokeTrust`, `refreshTools`, `handleMcpEvent` | `stores/mcp.ts`               | `useTauriEvents.ts`, `mcp.test.ts`, `McpServerManager.vue`, `McpStatusIndicator.vue`                                        |
| `memoryState`, `loadMemories`, `deleteMemoryItem`, `setMemoryFilter`                                                                                                                     | `stores/memory.ts`            | `MemoryBrowser.vue`, `memory.test.ts` (no other consumers)                                                                  |
| `catalogState`, `fetchSources`, `handleSourceFailed`, `isSourceSelected`, `toggleSource`, `fetchCatalog`, `fetchInstalled`                                                               | `stores/catalog.ts`           | `views/Marketplace.vue`, `composables/useMarketplace.ts`, `catalog.test.ts`, `marketplace/*.vue`                            |

**No back-compat shims** — every consumer is migrated in the same commit. The list above is exhaustive (verified via `grep -rn "<oldName>"` before starting Task 3).

**Migration pattern (apply uniformly to every store):**

Old (`stores/memory.ts` — verified verbatim from current `apps/agent-gui/src/stores/memory.ts`, 53 LOC):

```ts
import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { addNotification } from "../composables/useNotifications";

export interface MemoryItem {
  id: string;
  scope: string;
  key: string | null;
  content: string;
  accepted: boolean;
}

export const memoryState = reactive({
  memories: [] as MemoryItem[],
  loading: false,
  filter: "all" as "all" | "session" | "user" | "workspace",
  searchQuery: ""
});

export async function loadMemories(): Promise<void> {
  memoryState.loading = true;
  try {
    const scope = memoryState.filter === "all" ? null : memoryState.filter;
    const keywords = memoryState.searchQuery
      ? memoryState.searchQuery.split(/\s+/).filter(Boolean)
      : null;
    memoryState.memories = await invoke("query_memories", {
      scope,
      keywords,
      limit: 100
    });
  } catch (e) {
    console.error("Failed to load memories:", e);
    addNotification("error", `Failed to load memories: ${e}`);
  } finally {
    memoryState.loading = false;
  }
}

export async function deleteMemoryItem(id: string): Promise<void> {
  try {
    await invoke("delete_memory", { id });
    memoryState.memories = memoryState.memories.filter((m) => m.id !== id);
  } catch (e) {
    console.error("Failed to delete memory:", e);
    addNotification("error", `Failed to delete memory: ${e}`);
  }
}

export function setMemoryFilter(filter: typeof memoryState.filter): void {
  memoryState.filter = filter;
  loadMemories();
}
```

New shape:

```ts
import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useUiStore } from "@/stores/ui";

export interface MemoryItem {
  /* unchanged */
}

export const useMemoryStore = defineStore("memory", () => {
  const memories = ref<MemoryItem[]>([]);
  const loading = ref(false);
  const filter = ref<"all" | "session" | "user" | "workspace">("all");
  const searchQuery = ref("");

  async function loadMemories(): Promise<void> {
    loading.value = true;
    try {
      const scope = filter.value === "all" ? null : filter.value;
      const keywords = searchQuery.value
        ? searchQuery.value.split(/\s+/).filter(Boolean)
        : null;
      memories.value = await invoke("query_memories", {
        scope,
        keywords,
        limit: 100
      });
    } catch (e) {
      console.error("Failed to load memories:", e);
      useUiStore().pushNotification("error", `Failed to load memories: ${e}`);
    } finally {
      loading.value = false;
    }
  }

  async function deleteMemoryItem(id: string): Promise<void> {
    try {
      await invoke("delete_memory", { id });
      memories.value = memories.value.filter((m) => m.id !== id);
    } catch (e) {
      console.error("Failed to delete memory:", e);
      useUiStore().pushNotification("error", `Failed to delete memory: ${e}`);
    }
  }

  function setMemoryFilter(next: typeof filter.value): void {
    filter.value = next;
    void loadMemories();
  }

  return {
    memories,
    loading,
    filter,
    searchQuery,
    loadMemories,
    deleteMemoryItem,
    setMemoryFilter
  };
});
```

Consumer migration:

```ts
// Before
import { memoryState, loadMemories, setMemoryFilter } from "@/stores/memory";

// After
import { useMemoryStore } from "@/stores/memory";
import { storeToRefs } from "pinia";

const memory = useMemoryStore();
const { memories, loading, filter, searchQuery } = storeToRefs(memory);
// Methods stay on the store proxy (NOT destructured):
//   memory.loadMemories()  /  memory.deleteMemoryItem(id)  /  memory.setMemoryFilter("user")
```

**Cross-store dependency replacement:** every `addNotification(level, msg)` call inside the old stores is replaced with `useUiStore().pushNotification(level, msg)` inside the new store actions (lazy resolution avoids circular init). Top-level `addNotification` in `composables/useNotifications.ts` keeps working as a re-export to `useUiStore().pushNotification` for any non-store callers (Step 9).

- [ ] **Step 1: Read every existing store to capture exact state shapes & action signatures**

  Run this once and save output for reference (do not commit):

  ```bash
  cd /Users/chanyu/AIProjects/kairox
  for f in apps/agent-gui/src/stores/{session,taskGraph,agents,mcp,memory,catalog}.ts; do
    echo "===== $f ====="
    cat "$f"
  done > /tmp/kairox-stores-snapshot.txt
  ```

  Use this snapshot to ensure every exported symbol/method is preserved in the new setup-store.

- [ ] **Step 2: Migrate `stores/memory.ts` (smallest, 53 lines — best warm-up)**
  - Read current `apps/agent-gui/src/stores/memory.ts` fully.
  - Replace its entire content with the **New shape** code shown in the inventory above (do NOT keep the old `memoryState` export — both consumers (`MemoryBrowser.vue`, `memory.test.ts`) are migrated in this same commit before any test run).
  - Method names are preserved verbatim (`loadMemories`, `deleteMemoryItem`, `setMemoryFilter`) so the only consumer-side delta is `memoryState.X` → `memory.X` after the `useMemoryStore()` call.
  - **Immediately after** saving the new store, update its only non-test consumer `apps/agent-gui/src/components/MemoryBrowser.vue` so the project compiles:

    ```ts
    // Top of <script setup>
    import { useMemoryStore } from "@/stores/memory";
    import { storeToRefs } from "pinia";

    const memory = useMemoryStore();
    const { memories, loading, filter, searchQuery } = storeToRefs(memory);
    ```

    Then in the SFC body replace every `memoryState.X` with `X.value` (for refs) or `memory.X(...)` (for methods).

- [ ] **Step 3: Replace `stores/memory.test.ts` with the migrated version**

  Verified inventory of the current test file (108 LOC, 6 cases across 3 `describe` blocks): `loadMemories` ×3 (`null scope when filter is all`, `sets loading state during fetch`, `notifies on error`) + `deleteMemoryItem` ×2 (`removes item on success`, `notifies on error and keeps item`) + `setMemoryFilter` ×1 (`updates filter and triggers loadMemories`).

  Two structural changes from the migration:
  1. The `vi.mock("../composables/useNotifications", ...)` block is replaced by `vi.mock("@/stores/ui", ...)` because store actions now call `useUiStore().pushNotification(...)` instead of the top-level `addNotification(...)`.
  2. State reads/writes go through the Pinia store proxy: `memoryState.X` → `memory.X` (where `const memory = useMemoryStore()`).

  Replace `apps/agent-gui/src/stores/memory.test.ts` with the following full content (every original assertion preserved 1:1):

  ```ts
  import { describe, it, expect, beforeEach, vi } from "vitest";
  import { setActivePinia, createPinia } from "pinia";

  vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn()
  }));

  const pushNotificationSpy = vi.fn();
  vi.mock("@/stores/ui", () => ({
    useUiStore: () => ({
      pushNotification: pushNotificationSpy,
      dismissNotification: vi.fn()
    })
  }));

  import { invoke } from "@tauri-apps/api/core";
  import { useMemoryStore } from "@/stores/memory";

  const mockedInvoke = vi.mocked(invoke);

  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    pushNotificationSpy.mockClear();
  });

  describe("loadMemories", () => {
    it("invokes query_memories with null scope when filter is all", async () => {
      const memory = useMemoryStore();
      mockedInvoke.mockResolvedValueOnce([]);
      await memory.loadMemories();
      expect(mockedInvoke).toHaveBeenCalledWith("query_memories", {
        scope: null,
        keywords: null,
        limit: 100
      });
    });

    it("sets loading state during fetch", async () => {
      const memory = useMemoryStore();
      let resolvePromise: (value: unknown) => void;
      const promise = new Promise((resolve) => {
        resolvePromise = resolve;
      });
      mockedInvoke.mockReturnValueOnce(promise as Promise<unknown>);

      const loadPromise = memory.loadMemories();
      expect(memory.loading).toBe(true);

      resolvePromise!([]);
      await loadPromise;
      expect(memory.loading).toBe(false);
    });

    it("notifies on error", async () => {
      const memory = useMemoryStore();
      mockedInvoke.mockRejectedValueOnce(new Error("db error"));
      await memory.loadMemories();
      expect(pushNotificationSpy).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("db error")
      );
    });
  });

  describe("deleteMemoryItem", () => {
    it("removes item from memories on success", async () => {
      const memory = useMemoryStore();
      memory.memories = [
        {
          id: "m1",
          scope: "user",
          key: "lang",
          content: "Rust",
          accepted: true
        },
        {
          id: "m2",
          scope: "session",
          key: null,
          content: "temp",
          accepted: true
        }
      ];
      mockedInvoke.mockResolvedValueOnce(undefined);
      await memory.deleteMemoryItem("m1");
      expect(memory.memories).toHaveLength(1);
      expect(memory.memories[0].id).toBe("m2");
    });

    it("notifies on error and keeps item in local state", async () => {
      const memory = useMemoryStore();
      memory.memories = [
        {
          id: "m1",
          scope: "user",
          key: "lang",
          content: "Rust",
          accepted: true
        }
      ];
      mockedInvoke.mockRejectedValueOnce(new Error("not found"));
      await memory.deleteMemoryItem("m1");
      expect(pushNotificationSpy).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("not found")
      );
      expect(memory.memories).toHaveLength(1);
    });
  });

  describe("setMemoryFilter", () => {
    it("updates filter and triggers loadMemories", async () => {
      const memory = useMemoryStore();
      mockedInvoke.mockResolvedValueOnce([]);
      memory.setMemoryFilter("user");
      expect(memory.filter).toBe("user");
      await vi.waitFor(() => {
        expect(mockedInvoke).toHaveBeenCalledWith("query_memories", {
          scope: "user",
          keywords: null,
          limit: 100
        });
      });
    });
  });
  ```

  Run only this file:

  ```bash
  pnpm --filter agent-gui exec vitest run src/stores/memory.test.ts
  ```

  Expected: all 6 cases pass. A failure means the store migration changed observable behavior — fix the store (not the test; the test encodes the contract).

- [ ] **Step 4: Migrate `stores/agents.ts` + `agents.test.ts` using the same pattern**

  Apply identical migration. Run:

  ```bash
  pnpm --filter agent-gui exec vitest run src/stores/agents.test.ts
  ```

  Expected: green.

- [ ] **Step 5: Migrate `stores/taskGraph.ts` + `taskGraph.test.ts`**

  Special note: `taskGraphState.tasks` is a `reactive` array. New store uses `ref<TaskSnapshot[]>([])`. Consumers that previously did `taskGraphState.tasks.push(...)` now do `taskGraph.tasks.value.push(...)` — but note `storeToRefs(taskGraph).tasks` is a `Ref`, so in components using `<script setup>` with template auto-unwrap, the syntax stays `tasks.push(...)` in the template. In `<script setup>`, mutation needs `.value`.

  Run:

  ```bash
  pnpm --filter agent-gui exec vitest run src/stores/taskGraph.test.ts
  ```

- [ ] **Step 6: Migrate `stores/mcp.ts` + `mcp.test.ts`**

  Run:

  ```bash
  pnpm --filter agent-gui exec vitest run src/stores/mcp.test.ts
  ```

- [ ] **Step 7: Migrate `stores/catalog.ts` + `catalog.test.ts` (largest, 273 lines)**

  Verified inventory of the current `stores/catalog.ts`:
  - **Imports**: `reactive`, `computed` from vue; `invoke` from tauri; 7 generated types (`ServerEntryResponse`, `InstalledEntryResponse`, `InstallOutcomeResponse`, `InstallRequestPayload`, `CatalogQueryRequest`, `CatalogSourceViewResponse`, `AddCatalogSourceRequestPayload`); `addNotification`.
  - **Type aliases**: `CatalogTab = "browse" | "installed"`, `TrustLevel = "unverified" | "community" | "verified"`, `CatalogFilters` interface, `CatalogState` interface.
  - **State** (`catalogState` reactive, 11 fields): `entries`, `installed`, `installState` (record), `loading`, `error`, `tab`, `filters` (nested object), `sources`, `sourceFailures` (record), `selectedSources` (nullable array).
  - **Helpers**: `initial()` factory, `resetCatalogState()`, `TRUST_ORDER` const map.
  - **Computeds (4)**: `filteredEntries`, `hasEntries`, `installedCount`, `allSourceIds`, `visibleEntries` — actually 5.
  - **Actions (10)**: `fetchCatalog`, `fetchInstalled`, `getCatalogEntry`, `installEntry`, `uninstallEntry`, `refreshCatalogSource`, `fetchSources`, `addSource`, `removeSource`, `setSourceEnabled`.
  - **Pure helpers**: `handleSourceFailed`, `isSourceSelected`, `toggleSource`.
  - **Cross-store dependency**: none — `installEntry` only calls its own `fetchInstalled()` (not `fetchServers()` from the mcp store; that was an incorrect assumption in an earlier draft).

  Replace `apps/agent-gui/src/stores/catalog.ts` with the following full setup-store version. Every field, helper, computed, and action below is a 1:1 port from the current file.

  ```ts
  import { defineStore } from "pinia";
  import { ref, computed } from "vue";
  import { invoke } from "@tauri-apps/api/core";
  import type {
    ServerEntryResponse,
    InstalledEntryResponse,
    InstallOutcomeResponse,
    InstallRequestPayload,
    CatalogQueryRequest,
    CatalogSourceViewResponse,
    AddCatalogSourceRequestPayload
  } from "../generated/commands";
  import { useUiStore } from "@/stores/ui";

  export type CatalogTab = "browse" | "installed";
  export type TrustLevel = "unverified" | "community" | "verified";

  export interface CatalogFilters {
    keyword: string;
    category: string | null;
    trustMin: TrustLevel | null;
  }

  const TRUST_ORDER: Record<TrustLevel, number> = {
    unverified: 0,
    community: 1,
    verified: 2
  };

  export const useCatalogStore = defineStore("catalog", () => {
    // ── state ────────────────────────────────────────────────────────
    const entries = ref<ServerEntryResponse[]>([]);
    const installed = ref<InstalledEntryResponse[]>([]);
    const installState = ref<Record<string, InstallOutcomeResponse>>({});
    const loading = ref(false);
    const error = ref<string | null>(null);
    const tab = ref<CatalogTab>("browse");
    const filters = ref<CatalogFilters>({
      keyword: "",
      category: null,
      trustMin: null
    });
    const sources = ref<CatalogSourceViewResponse[]>([]);
    const sourceFailures = ref<Record<string, string>>({});
    const selectedSources = ref<string[] | null>(null);

    // ── helpers ──────────────────────────────────────────────────────
    function reset(): void {
      entries.value = [];
      installed.value = [];
      installState.value = {};
      loading.value = false;
      error.value = null;
      tab.value = "browse";
      filters.value = { keyword: "", category: null, trustMin: null };
      sources.value = [];
      sourceFailures.value = {};
      selectedSources.value = null;
    }

    // ── computeds ────────────────────────────────────────────────────
    const filteredEntries = computed<ServerEntryResponse[]>(() => {
      const kw = filters.value.keyword.trim().toLowerCase();
      const minOrder = filters.value.trustMin
        ? TRUST_ORDER[filters.value.trustMin]
        : -1;
      return entries.value.filter((e) => {
        if (kw) {
          const hay =
            `${e.display_name} ${e.summary} ${e.tags.join(" ")}`.toLowerCase();
          if (!hay.includes(kw)) return false;
        }
        if (
          filters.value.category &&
          !e.categories.includes(filters.value.category)
        ) {
          return false;
        }
        if (filters.value.trustMin) {
          const t = TRUST_ORDER[e.trust as TrustLevel] ?? 0;
          if (t < minOrder) return false;
        }
        return true;
      });
    });

    const hasEntries = computed(() => entries.value.length > 0);
    const installedCount = computed(() => installed.value.length);
    const allSourceIds = computed<string[]>(() => [
      "builtin",
      ...sources.value.map((s) => s.id)
    ]);

    function isSourceSelected(id: string): boolean {
      if (selectedSources.value === null) return true;
      return selectedSources.value.includes(id);
    }

    function toggleSource(id: string): void {
      const current = selectedSources.value ?? allSourceIds.value.slice();
      const next = current.includes(id)
        ? current.filter((x) => x !== id)
        : [...current, id];
      selectedSources.value = next;
    }

    const visibleEntries = computed<ServerEntryResponse[]>(() =>
      filteredEntries.value.filter((e) => isSourceSelected(e.source))
    );

    // ── actions ──────────────────────────────────────────────────────
    async function fetchCatalog(
      query: CatalogQueryRequest = {}
    ): Promise<void> {
      const ui = useUiStore();
      loading.value = true;
      error.value = null;
      try {
        entries.value = await invoke<ServerEntryResponse[]>("list_catalog", {
          query
        });
      } catch (e) {
        error.value = String(e);
        ui.pushNotification("error", `Failed to load catalog: ${e}`);
      } finally {
        loading.value = false;
      }
    }

    async function fetchInstalled(): Promise<void> {
      const ui = useUiStore();
      try {
        installed.value = await invoke<InstalledEntryResponse[]>(
          "list_installed_entries"
        );
      } catch (e) {
        error.value = String(e);
        ui.pushNotification("error", `Failed to load installed entries: ${e}`);
      }
    }

    async function getCatalogEntry(
      id: string,
      source?: string | null
    ): Promise<ServerEntryResponse | null> {
      const ui = useUiStore();
      try {
        return await invoke<ServerEntryResponse | null>("get_catalog_entry", {
          id,
          source: source ?? null
        });
      } catch (e) {
        console.error("Failed to get catalog entry:", e);
        ui.pushNotification(
          "error",
          `Failed to load catalog entry ${id}: ${e}`
        );
        return null;
      }
    }

    async function installEntry(
      request: InstallRequestPayload
    ): Promise<InstallOutcomeResponse | null> {
      const ui = useUiStore();
      try {
        const outcome = await invoke<InstallOutcomeResponse>(
          "install_catalog_entry",
          { request }
        );
        installState.value[request.catalog_id] = outcome;
        if (outcome.kind === "installed") {
          await fetchInstalled();
        }
        return outcome;
      } catch (e) {
        console.error("Failed to install catalog entry:", e);
        ui.pushNotification(
          "error",
          `Failed to install ${request.catalog_id}: ${e}`
        );
        return null;
      }
    }

    async function uninstallEntry(serverId: string): Promise<void> {
      const ui = useUiStore();
      try {
        await invoke("uninstall_catalog_entry", { serverId });
        delete installState.value[serverId];
        await fetchInstalled();
      } catch (e) {
        console.error("Failed to uninstall catalog entry:", e);
        ui.pushNotification("error", `Failed to uninstall ${serverId}: ${e}`);
      }
    }

    async function refreshCatalogSource(
      source: string | null = null
    ): Promise<void> {
      const ui = useUiStore();
      try {
        await invoke("refresh_catalog", { source });
        await fetchCatalog();
      } catch (e) {
        console.error("Failed to refresh catalog source:", e);
        ui.pushNotification("error", `Failed to refresh catalog: ${e}`);
      }
    }

    async function fetchSources(): Promise<void> {
      const ui = useUiStore();
      try {
        sources.value = await invoke<CatalogSourceViewResponse[]>(
          "list_catalog_sources"
        );
      } catch (e) {
        error.value = String(e);
        ui.pushNotification("error", `Failed to load catalog sources: ${e}`);
      }
    }

    async function addSource(
      request: AddCatalogSourceRequestPayload
    ): Promise<void> {
      const ui = useUiStore();
      try {
        await invoke("add_catalog_source", { request });
        await fetchSources();
      } catch (e) {
        console.error("Failed to add catalog source:", e);
        ui.pushNotification(
          "error",
          `Failed to add source ${request.id}: ${e}`
        );
      }
    }

    async function removeSource(id: string): Promise<void> {
      const ui = useUiStore();
      try {
        await invoke("remove_catalog_source", { id });
        delete sourceFailures.value[id];
        await fetchSources();
      } catch (e) {
        console.error("Failed to remove catalog source:", e);
        ui.pushNotification("error", `Failed to remove source ${id}: ${e}`);
      }
    }

    async function setSourceEnabled(
      id: string,
      enabled: boolean
    ): Promise<void> {
      const ui = useUiStore();
      try {
        await invoke("set_catalog_source_enabled", { id, enabled });
        await fetchSources();
      } catch (e) {
        console.error("Failed to toggle catalog source:", e);
        ui.pushNotification("error", `Failed to toggle source ${id}: ${e}`);
      }
    }

    function handleSourceFailed(source: string, errorMsg: string): void {
      sourceFailures.value[source] = errorMsg;
    }

    return {
      // state
      entries,
      installed,
      installState,
      loading,
      error,
      tab,
      filters,
      sources,
      sourceFailures,
      selectedSources,
      // computeds
      filteredEntries,
      hasEntries,
      installedCount,
      allSourceIds,
      visibleEntries,
      // helpers
      reset,
      isSourceSelected,
      toggleSource,
      handleSourceFailed,
      // actions
      fetchCatalog,
      fetchInstalled,
      getCatalogEntry,
      installEntry,
      uninstallEntry,
      refreshCatalogSource,
      fetchSources,
      addSource,
      removeSource,
      setSourceEnabled
    };
  });
  ```

  Note on `resetCatalogState()`: the legacy top-level export is removed; the rebuilt `catalog.test.ts` below uses `setActivePinia(createPinia())` in `beforeEach` instead — Pinia's per-test fresh store replaces the manual reset.

  Replace `apps/agent-gui/src/stores/catalog.test.ts` with the following full content (all 11 cases preserved 1:1 from the current 250-LOC file across the two `describe` blocks). Two systematic transforms applied: (a) `vi.mock("../composables/useNotifications", ...)` replaced by `vi.mock("@/stores/ui", ...)` with a `pushNotificationSpy`; (b) `catalogState.X` reads/writes replaced by `catalog.X` after `const catalog = useCatalogStore()`; (c) standalone exports (`fetchCatalog`, `installEntry`, `filteredEntries`, etc.) replaced by `catalog.X(...)` method calls and `catalog.filteredEntries` computed access:

  ```ts
  import { describe, it, expect, beforeEach, vi } from "vitest";
  import { setActivePinia, createPinia } from "pinia";

  vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn()
  }));

  const pushNotificationSpy = vi.fn();
  vi.mock("@/stores/ui", () => ({
    useUiStore: () => ({
      pushNotification: pushNotificationSpy,
      dismissNotification: vi.fn()
    })
  }));

  import { invoke } from "@tauri-apps/api/core";
  import { useCatalogStore } from "@/stores/catalog";

  const mockedInvoke = vi.mocked(invoke);

  const fixtureEntry = (over: Partial<Record<string, unknown>> = {}) => ({
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
    install_spec_json: "{}",
    requirements_json: "[]",
    default_env_json: "[]",
    ...over
  });

  describe("catalog store", () => {
    beforeEach(() => {
      setActivePinia(createPinia());
      vi.clearAllMocks();
      pushNotificationSpy.mockClear();
    });

    it("loads entries via list_catalog", async () => {
      const catalog = useCatalogStore();
      mockedInvoke.mockResolvedValueOnce([fixtureEntry()] as never);
      await catalog.fetchCatalog();
      expect(mockedInvoke).toHaveBeenCalledWith("list_catalog", {
        query: expect.any(Object)
      });
      expect(catalog.entries.length).toBe(1);
      expect(catalog.entries[0].id).toBe("filesystem");
    });

    it("install dispatches install_catalog_entry and stores outcome", async () => {
      const catalog = useCatalogStore();
      mockedInvoke
        .mockResolvedValueOnce({
          kind: "installed",
          server_id: "filesystem",
          started: true,
          missing_runtimes: [],
          missing_env_keys: []
        } as never)
        .mockResolvedValueOnce([] as never); // refreshInstalled

      const outcome = await catalog.installEntry({
        catalog_id: "filesystem",
        source: "builtin",
        server_id_override: null,
        env_overrides: { WORKSPACE_PATH: "/tmp" },
        trust_grant: true,
        auto_start: true
      });

      expect(outcome?.kind).toBe("installed");
      expect(catalog.installState["filesystem"]).toEqual({
        kind: "installed",
        server_id: "filesystem",
        started: true,
        missing_runtimes: [],
        missing_env_keys: []
      });
    });

    it("filters by keyword + trust client-side", () => {
      const catalog = useCatalogStore();
      catalog.entries = [
        fixtureEntry({
          id: "a",
          display_name: "Alpha",
          summary: "x",
          tags: ["alpha"],
          trust: "verified"
        }),
        fixtureEntry({
          id: "b",
          display_name: "Beta",
          summary: "y",
          tags: ["beta"],
          trust: "community"
        })
      ];
      catalog.filters.keyword = "alpha";
      catalog.filters.trustMin = "verified";
      expect(catalog.filteredEntries.map((e) => e.id)).toEqual(["a"]);
    });

    it("uninstall removes from installState and refreshes installed", async () => {
      const catalog = useCatalogStore();
      catalog.installState["filesystem"] = {
        kind: "installed",
        server_id: "filesystem",
        started: true,
        missing_runtimes: [],
        missing_env_keys: []
      };
      mockedInvoke
        .mockResolvedValueOnce(undefined as never) // uninstall_catalog_entry
        .mockResolvedValueOnce([] as never); // list_installed_entries

      await catalog.uninstallEntry("filesystem");

      expect(mockedInvoke).toHaveBeenNthCalledWith(
        1,
        "uninstall_catalog_entry",
        {
          serverId: "filesystem"
        }
      );
      expect(catalog.installState["filesystem"]).toBeUndefined();
    });

    it("refreshCatalogSource calls refresh_catalog then re-fetches", async () => {
      const catalog = useCatalogStore();
      mockedInvoke
        .mockResolvedValueOnce(undefined as never) // refresh_catalog
        .mockResolvedValueOnce([] as never); // list_catalog

      await catalog.refreshCatalogSource("builtin");

      expect(mockedInvoke).toHaveBeenNthCalledWith(1, "refresh_catalog", {
        source: "builtin"
      });
      expect(mockedInvoke).toHaveBeenNthCalledWith(2, "list_catalog", {
        query: expect.any(Object)
      });
    });

    it("fetchInstalled populates installed list", async () => {
      const catalog = useCatalogStore();
      mockedInvoke.mockResolvedValueOnce([
        {
          server_id: "filesystem",
          catalog_id: "filesystem",
          source: "builtin",
          display_name: "Filesystem",
          installed_at: "2026-05-06T00:00:00Z",
          running: true
        }
      ] as never);
      await catalog.fetchInstalled();
      expect(catalog.installed.length).toBe(1);
      expect(catalog.installed[0].server_id).toBe("filesystem");
    });
  });

  const sampleSource = {
    id: "smithery",
    display_name: "Smithery",
    kind: "smithery",
    url: "https://registry.smithery.ai",
    api_key_env: null,
    priority: 50,
    default_trust: "community",
    enabled: true,
    cache_ttl_seconds: null,
    last_error: null
  };

  describe("catalog store — Phase 2 sources", () => {
    beforeEach(() => {
      setActivePinia(createPinia());
      vi.clearAllMocks();
      pushNotificationSpy.mockClear();
    });

    it("fetchSources loads sources via list_catalog_sources", async () => {
      const catalog = useCatalogStore();
      mockedInvoke.mockResolvedValueOnce([sampleSource] as never);
      await catalog.fetchSources();
      expect(mockedInvoke).toHaveBeenCalledWith("list_catalog_sources");
      expect(catalog.sources).toHaveLength(1);
      expect(catalog.sources[0].id).toBe("smithery");
    });

    it("addSource calls add_catalog_source then re-fetches", async () => {
      const catalog = useCatalogStore();
      mockedInvoke
        .mockResolvedValueOnce(undefined as never)
        .mockResolvedValueOnce([sampleSource] as never);
      await catalog.addSource({
        id: "smithery",
        display_name: "Smithery",
        kind: "smithery",
        url: "https://registry.smithery.ai",
        api_key_env: null,
        priority: 50,
        default_trust: "community",
        enabled: true,
        cache_ttl_seconds: null
      });
      expect(mockedInvoke).toHaveBeenNthCalledWith(1, "add_catalog_source", {
        request: expect.objectContaining({ id: "smithery" })
      });
      expect(mockedInvoke).toHaveBeenNthCalledWith(2, "list_catalog_sources");
      expect(catalog.sources).toHaveLength(1);
    });

    it("removeSource calls remove_catalog_source then re-fetches", async () => {
      const catalog = useCatalogStore();
      catalog.sources = [sampleSource];
      mockedInvoke
        .mockResolvedValueOnce(undefined as never)
        .mockResolvedValueOnce([] as never);
      await catalog.removeSource("smithery");
      expect(mockedInvoke).toHaveBeenNthCalledWith(1, "remove_catalog_source", {
        id: "smithery"
      });
      expect(catalog.sources).toHaveLength(0);
    });

    it("setSourceEnabled toggles a source and re-fetches", async () => {
      const catalog = useCatalogStore();
      catalog.sources = [sampleSource];
      mockedInvoke
        .mockResolvedValueOnce(undefined as never)
        .mockResolvedValueOnce([{ ...sampleSource, enabled: false }] as never);
      await catalog.setSourceEnabled("smithery", false);
      expect(mockedInvoke).toHaveBeenNthCalledWith(
        1,
        "set_catalog_source_enabled",
        { id: "smithery", enabled: false }
      );
      expect(catalog.sources[0].enabled).toBe(false);
    });

    it("handleSourceFailed records sourceFailures keyed by source id", () => {
      const catalog = useCatalogStore();
      catalog.handleSourceFailed("smithery", "timeout");
      expect(catalog.sourceFailures.smithery).toBe("timeout");
    });
  });
  ```

  Run:

  ```bash
  pnpm --filter agent-gui exec vitest run src/stores/catalog.test.ts
  ```

  Expected: all 11 cases pass.

- [ ] **Step 8: Migrate `stores/session.ts` + `session.test.ts` + `session-ipc.test.ts` (largest, 298 lines)**

  Verified inventory of the current `stores/session.ts`:
  - **State** (`sessionState` reactive object, 9 fields + 1 separate `streamsByTask` reactive Map): `sessions`, `currentSessionId`, `workspaceId`, `projection` (nested `SessionProjection`), `currentProfile`, `isStreaming`, `connected`, `initialized`.
  - **Helpers/actions (8 top-level fns)**: `reportSendError`, `applyEvent`, `setProjection`, `resetProjection`, `deleteSession`, `renameSession`, `recoverSessions`. Plus the exported `streamsByTask` reactive Map.
  - **Cross-store dependencies**: `taskGraphState`/`clearTaskGraph` (from `./taskGraph`), `agentState`/`clearAgents` (from `./agents`), `applyTraceEvent`/`clearTrace` (from `../composables/useTraceStore`), `addNotification` (from `../composables/useNotifications`), `agentRoleToProjectedRole` (from `../types`).

  **Replace `apps/agent-gui/src/stores/session.ts` with the following full setup-store version.** Every action body is preserved 1:1; mechanical substitutions: `sessionState.X` → `X.value` for refs, `taskGraphState` → `useTaskGraphStore()` proxy, `agentState.agents` → `useAgentsStore().agents`, `clearAgents()` → `useAgentsStore().clearAgents()`, `clearTaskGraph()` → `useTaskGraphStore().clearTaskGraph()`, `addNotification(level, msg)` → `useUiStore().pushNotification(level, msg)`. Cross-store hooks are resolved **inside** each action body (lazy) to avoid Pinia init cycles.

  ```ts
  import { defineStore } from "pinia";
  import { ref } from "vue";
  import { invoke } from "@tauri-apps/api/core";
  import type {
    SessionProjection,
    SessionInfoResponse,
    DomainEvent
  } from "@/types";
  import { agentRoleToProjectedRole } from "@/types";
  import { clearTrace, applyTraceEvent } from "@/composables/useTraceStore";
  import { useUiStore } from "@/stores/ui";
  import { useTaskGraphStore } from "@/stores/taskGraph";
  import { useAgentsStore } from "@/stores/agents";

  function emptyProjection(): SessionProjection {
    return {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false
    };
  }

  export const useSessionStore = defineStore("session", () => {
    // ── state ────────────────────────────────────────────────────────
    const sessions = ref<SessionInfoResponse[]>([]);
    const currentSessionId = ref<string | null>(null);
    const workspaceId = ref<string | null>(null);
    const projection = ref<SessionProjection>(emptyProjection());
    const currentProfile = ref<string>("fast");
    const isStreaming = ref(false);
    const connected = ref(false);
    const initialized = ref(false);
    const streamsByTask = ref(new Map<string, string>());

    // ── actions ──────────────────────────────────────────────────────
    function reportSendError(message: string) {
      projection.value.messages.push({
        role: "assistant",
        content: `[error] ${message}`
      });
      projection.value.token_stream = "";
      isStreaming.value = false;
    }

    function applyEvent(event: DomainEvent) {
      const p = event.payload;
      const sourceAgentId = event.source_agent_id;
      const agents = useAgentsStore();

      switch (p.type) {
        case "UserMessageAdded": {
          projection.value.messages.push({
            role: "user",
            content: p.content
          });
          isStreaming.value = true;
          break;
        }
        case "ModelTokenDelta": {
          projection.value.token_stream += p.delta;
          break;
        }
        case "AssistantMessageCompleted": {
          const msg: (typeof projection.value.messages)[0] = {
            role: "assistant",
            content: p.content
          };
          if (sourceAgentId && sourceAgentId !== "agent_system") {
            msg.sourceAgentId = sourceAgentId;
            const agent = agents.agents.get(sourceAgentId);
            if (agent) {
              msg.role = agentRoleToProjectedRole(agent.role);
            }
          }
          projection.value.messages.push(msg);
          projection.value.token_stream = "";
          isStreaming.value = false;
          break;
        }
        case "SessionCancelled":
          projection.value.cancelled = true;
          isStreaming.value = false;
          break;
        case "AgentTaskCreated": {
          projection.value.task_titles.push(p.title);
          break;
        }
        case "AgentTaskStarted":
          break;
        case "AgentTaskCompleted": {
          isStreaming.value = false;
          break;
        }
        case "AgentTaskFailed": {
          projection.value.messages.push({
            role: "assistant",
            content: `[error] ${p.error || "Unknown error"}`
          });
          projection.value.token_stream = "";
          isStreaming.value = false;
          break;
        }
        case "TaskDecomposed": {
          projection.value.messages.push({
            role: "system",
            content: `Task decomposed into ${p.sub_task_ids.length} sub-tasks`
          });
          break;
        }
        case "TaskBlocked": {
          projection.value.messages.push({
            role: "system",
            content: `Task blocked: ${p.reason || "dependency failed"}`
          });
          break;
        }
        case "TaskRetried": {
          projection.value.messages.push({
            role: "system",
            content: `Task retry attempt ${p.attempt}`
          });
          break;
        }
        case "AgentSpawned":
        case "AgentIdle":
          break;
        case "SessionInitialized":
        case "ContextAssembled":
        case "ModelRequestStarted":
        case "ModelToolCallRequested":
        case "ToolInvocationStarted":
        case "ToolInvocationCompleted":
        case "ToolInvocationFailed":
        case "PermissionRequested":
        case "PermissionGranted":
        case "PermissionDenied":
        case "FilePatchProposed":
        case "FilePatchApplied":
        case "MemoryProposed":
        case "MemoryAccepted":
        case "MemoryRejected":
        case "ReviewerFindingAdded":
        case "WorkspaceOpened":
          break;
      }
    }

    function setProjection(next: SessionProjection) {
      projection.value = next;
      isStreaming.value = false;
      if (next.task_graph?.tasks) {
        const taskGraph = useTaskGraphStore();
        taskGraph.tasks = next.task_graph.tasks;
        taskGraph.currentSessionId = currentSessionId.value;
      }
    }

    function resetProjection() {
      projection.value = emptyProjection();
      isStreaming.value = false;
      streamsByTask.value.clear();
      useAgentsStore().clearAgents();
    }

    async function switchSession(sessionId: string): Promise<void> {
      if (sessionId === currentSessionId.value) return;
      const target = sessions.value.find((s) => s.id === sessionId);
      if (!target) {
        throw new Error(`Session not found: ${sessionId}`);
      }
      resetProjection();
      clearTrace();
      useTaskGraphStore().clearTaskGraph();
      currentSessionId.value = sessionId;
      currentProfile.value = target.profile;
      const next = await invoke<SessionProjection>("switch_session", {
        sessionId
      });
      setProjection(next);
      const traceStrings = await invoke<string[]>("get_trace", { sessionId });
      for (const jsonStr of traceStrings) {
        try {
          applyTraceEvent(JSON.parse(jsonStr));
        } catch {
          // Skip malformed trace entries
        }
      }
    }

    async function deleteSession(sessionId: string) {
      const ui = useUiStore();
      try {
        await invoke("delete_session", { sessionId });
        sessions.value = sessions.value.filter((s) => s.id !== sessionId);
        if (currentSessionId.value === sessionId) {
          if (sessions.value.length > 0) {
            await switchSession(sessions.value[0].id);
          } else {
            currentSessionId.value = null;
            resetProjection();
            clearTrace();
            useTaskGraphStore().clearTaskGraph();
          }
        }
      } catch (e) {
        console.error("Failed to delete session:", e);
        ui.pushNotification("error", `Failed to delete session: ${e}`);
      }
    }

    async function renameSession(sessionId: string, title: string) {
      const ui = useUiStore();
      try {
        await invoke("rename_session", { sessionId, title });
        const session = sessions.value.find((s) => s.id === sessionId);
        if (session) {
          session.title = title;
        }
      } catch (e) {
        console.error("Failed to rename session:", e);
        ui.pushNotification("error", `Failed to rename session: ${e}`);
      }
    }

    async function recoverSessions(): Promise<boolean> {
      try {
        const workspaces: { workspace_id: string; path: string }[] =
          await invoke("list_workspaces");
        if (workspaces.length === 0) {
          return false;
        }
        const ws = workspaces[0];
        workspaceId.value = ws.workspace_id;
        await invoke("restore_workspace", { workspaceId: ws.workspace_id });
        sessions.value = await invoke("list_sessions");
        if (sessions.value.length > 0) {
          await switchSession(sessions.value[0].id);
        }
        initialized.value = true;
        return true;
      } catch (e) {
        console.error("Failed to recover sessions:", e);
        useUiStore().pushNotification(
          "error",
          `Failed to recover sessions: ${e}`
        );
        return false;
      }
    }

    return {
      // state
      sessions,
      currentSessionId,
      workspaceId,
      projection,
      currentProfile,
      isStreaming,
      connected,
      initialized,
      streamsByTask,
      // actions
      reportSendError,
      applyEvent,
      setProjection,
      resetProjection,
      switchSession,
      deleteSession,
      renameSession,
      recoverSessions
    };
  });
  ```

  Architectural note: `switchSession` consolidates the duplicated `invoke('switch_session') + setProjection + invoke('get_trace') + applyTraceEvent loop` block that currently lives in three places in the old code (`App.vue` lines 49–58, `recoverSessions` lines 252–268, `deleteSession` lines 207–219). Both `recoverSessions` and `deleteSession` now call `switchSession(...)` instead of duplicating the loop. App.vue's pre-router copy will be deleted in Task 5.

  **Replace `apps/agent-gui/src/stores/session.test.ts` with the following full content.** All 14 cases preserved 1:1; transforms: (a) imports come from `useSessionStore`, (b) `agentState.agents.set(...)` becomes `useAgentsStore().agents.set(...)`, (c) `sessionState.X` reads/writes become `session.X` after `const session = useSessionStore()`, (d) `streamsByTask` is now accessed as `session.streamsByTask` (which is a ref to a Map):

  ```ts
  import { describe, it, expect, beforeEach } from "vitest";
  import { setActivePinia, createPinia } from "pinia";
  import { useSessionStore } from "@/stores/session";
  import { useAgentsStore } from "@/stores/agents";
  import type { DomainEvent, AgentRole, EventPayload } from "@/types";

  beforeEach(() => {
    setActivePinia(createPinia());
  });

  function makeEvent(
    payload: EventPayload,
    sourceAgentId = "agent_system"
  ): DomainEvent {
    return {
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-06T00:00:00Z",
      source_agent_id: sourceAgentId,
      privacy: "full_trace",
      event_type: payload.type,
      payload
    } as DomainEvent;
  }

  describe("applyEvent", () => {
    it("projects UserMessageAdded", () => {
      const session = useSessionStore();
      session.applyEvent(
        makeEvent({
          type: "UserMessageAdded",
          message_id: "m1",
          content: "hello"
        })
      );
      expect(session.projection.messages).toHaveLength(1);
      expect(session.projection.messages[0].role).toBe("user");
      expect(session.projection.messages[0].content).toBe("hello");
      expect(session.isStreaming).toBe(true);
    });

    it("accumulates ModelTokenDelta into token_stream", () => {
      const session = useSessionStore();
      session.applyEvent(makeEvent({ type: "ModelTokenDelta", delta: "hel" }));
      session.applyEvent(makeEvent({ type: "ModelTokenDelta", delta: "lo" }));
      expect(session.projection.token_stream).toBe("hello");
    });

    it("finalizes on AssistantMessageCompleted", () => {
      const session = useSessionStore();
      session.applyEvent(
        makeEvent({
          type: "AssistantMessageCompleted",
          message_id: "m2",
          content: "hi there"
        })
      );
      expect(session.projection.messages).toHaveLength(1);
      expect(session.projection.messages[0].role).toBe("assistant");
      expect(session.projection.messages[0].content).toBe("hi there");
      expect(session.projection.token_stream).toBe("");
      expect(session.isStreaming).toBe(false);
    });

    it("attributes AssistantMessageCompleted to agent when source_agent_id is known", () => {
      const session = useSessionStore();
      const agents = useAgentsStore();
      agents.agents.set("agent_w1", {
        id: "agent_w1",
        role: "Worker" as AgentRole,
        taskId: "t1",
        status: "running",
        startedAt: Date.now(),
        completedAt: null
      });
      session.applyEvent(
        makeEvent(
          {
            type: "AssistantMessageCompleted",
            message_id: "m3",
            content: "worker response"
          },
          "agent_w1"
        )
      );
      expect(session.projection.messages).toHaveLength(1);
      expect(session.projection.messages[0].role).toBe("worker");
      expect(session.projection.messages[0].sourceAgentId).toBe("agent_w1");
    });

    it("marks cancelled on SessionCancelled", () => {
      const session = useSessionStore();
      session.applyEvent(
        makeEvent({ type: "SessionCancelled", reason: "user stopped" })
      );
      expect(session.projection.cancelled).toBe(true);
      expect(session.isStreaming).toBe(false);
    });

    it("handles TaskDecomposed event", () => {
      const session = useSessionStore();
      session.applyEvent(
        makeEvent({
          type: "TaskDecomposed",
          parent_task_id: "parent",
          sub_task_ids: ["sub1", "sub2", "sub3"]
        })
      );
      expect(session.projection.messages).toHaveLength(1);
      expect(session.projection.messages[0].role).toBe("system");
      expect(session.projection.messages[0].content).toContain("3 sub-tasks");
    });

    it("handles TaskBlocked event", () => {
      const session = useSessionStore();
      session.applyEvent(
        makeEvent({
          type: "TaskBlocked",
          task_id: "t1",
          blocking_task_id: "t0",
          reason: "dependency failed"
        })
      );
      expect(session.projection.messages).toHaveLength(1);
      expect(session.projection.messages[0].role).toBe("system");
      expect(session.projection.messages[0].content).toContain("blocked");
    });

    it("handles TaskRetried event", () => {
      const session = useSessionStore();
      session.applyEvent(
        makeEvent({ type: "TaskRetried", task_id: "t1", attempt: 2 })
      );
      expect(session.projection.messages).toHaveLength(1);
      expect(session.projection.messages[0].role).toBe("system");
      expect(session.projection.messages[0].content).toContain("attempt 2");
    });

    it("ignores AgentSpawned and AgentIdle events gracefully", () => {
      const session = useSessionStore();
      session.applyEvent(
        makeEvent({
          type: "AgentSpawned",
          agent_id: "a1",
          role: "Worker",
          task_id: "t1"
        })
      );
      session.applyEvent(makeEvent({ type: "AgentIdle", agent_id: "a1" }));
      expect(session.projection.messages).toHaveLength(0);
    });

    it("ignores unknown event types gracefully", () => {
      const session = useSessionStore();
      session.applyEvent(makeEvent({ type: "FutureEvent" } as never));
      expect(session.projection.messages).toHaveLength(0);
    });
  });

  describe("setProjection", () => {
    it("replaces the current projection", () => {
      const session = useSessionStore();
      session.setProjection({
        messages: [
          { role: "user", content: "existing" },
          { role: "assistant", content: "reply" }
        ],
        task_titles: ["task 1"],
        token_stream: "",
        cancelled: false,
        task_graph: { tasks: [] }
      });
      expect(session.projection.messages).toHaveLength(2);
      expect(session.isStreaming).toBe(false);
    });
  });

  describe("resetProjection", () => {
    it("clears all projection state and agent state", () => {
      const session = useSessionStore();
      session.applyEvent(
        makeEvent({ type: "UserMessageAdded", message_id: "m1", content: "hi" })
      );
      session.resetProjection();
      expect(session.projection.messages).toHaveLength(0);
      expect(session.projection.token_stream).toBe("");
      expect(session.projection.cancelled).toBe(false);
      expect(session.isStreaming).toBe(false);
      expect(session.streamsByTask.size).toBe(0);
    });
  });
  ```

  **Replace `apps/agent-gui/src/stores/session-ipc.test.ts` with the following full content.** All 6 cases preserved 1:1; transforms: (a) the `useNotifications` mock is replaced by a `@/stores/ui` mock with a `pushNotificationSpy`, (b) the `./taskGraph` mock now exports `useTaskGraphStore` factory (not `taskGraphState`), (c) `sessionState.X` becomes `session.X` after `const session = useSessionStore()`:

  ```ts
  import { describe, it, expect, beforeEach, vi } from "vitest";
  import { setActivePinia, createPinia } from "pinia";

  vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn()
  }));

  vi.mock("@tauri-apps/api/event", () => ({
    listen: vi.fn(() => Promise.resolve(vi.fn()))
  }));

  const pushNotificationSpy = vi.fn();
  vi.mock("@/stores/ui", () => ({
    useUiStore: () => ({
      pushNotification: pushNotificationSpy,
      dismissNotification: vi.fn(),
      notifications: []
    })
  }));

  vi.mock("@/composables/useTraceStore", () => ({
    applyTraceEvent: vi.fn(),
    clearTrace: vi.fn()
  }));

  vi.mock("@/stores/taskGraph", () => ({
    useTaskGraphStore: () => ({
      tasks: [],
      currentSessionId: null,
      loading: false,
      clearTaskGraph: vi.fn()
    })
  }));

  vi.mock("@/stores/agents", () => ({
    useAgentsStore: () => ({
      agents: new Map(),
      clearAgents: vi.fn(),
      applyAgentEvent: vi.fn()
    })
  }));

  import { invoke } from "@tauri-apps/api/core";
  import { useSessionStore } from "@/stores/session";

  const mockedInvoke = vi.mocked(invoke);

  const makeSession = (id: string, title: string, profile = "fast") => ({
    id,
    title,
    profile,
    model_id: null,
    provider: null
  });

  const emptyProjection = {
    messages: [],
    task_titles: [],
    task_graph: { tasks: [] },
    token_stream: "",
    cancelled: false
  };

  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    pushNotificationSpy.mockClear();
  });

  describe("deleteSession", () => {
    it("removes session from the list on success", async () => {
      const session = useSessionStore();
      session.sessions = [
        makeSession("s1", "Session 1"),
        makeSession("s2", "Session 2")
      ] as never[];
      mockedInvoke.mockResolvedValueOnce(undefined);
      await session.deleteSession("s2");
      expect(session.sessions).toHaveLength(1);
      expect(session.sessions[0].id).toBe("s1");
    });

    it("switches to first remaining session when deleting current", async () => {
      const session = useSessionStore();
      session.sessions = [
        makeSession("s1", "Session 1", "slow"),
        makeSession("s2", "Session 2", "fast")
      ] as never[];
      session.currentSessionId = "s2";
      mockedInvoke.mockResolvedValueOnce(undefined); // delete_session
      mockedInvoke.mockResolvedValueOnce(emptyProjection); // switch_session
      mockedInvoke.mockResolvedValueOnce([]); // get_trace
      await session.deleteSession("s2");
      expect(session.currentSessionId).toBe("s1");
    });

    it("notifies on error", async () => {
      const session = useSessionStore();
      mockedInvoke.mockRejectedValueOnce(new Error("delete failed"));
      await session.deleteSession("s1");
      expect(pushNotificationSpy).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("delete failed")
      );
    });
  });

  describe("renameSession", () => {
    it("updates local title on success", async () => {
      const session = useSessionStore();
      session.sessions = [makeSession("s1", "Old Title")] as never[];
      mockedInvoke.mockResolvedValueOnce(undefined);
      await session.renameSession("s1", "New Title");
      expect(session.sessions[0].title).toBe("New Title");
    });

    it("notifies on error", async () => {
      const session = useSessionStore();
      mockedInvoke.mockRejectedValueOnce(new Error("rename failed"));
      await session.renameSession("s1", "New Title");
      expect(pushNotificationSpy).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("rename failed")
      );
    });
  });

  describe("recoverSessions", () => {
    it("restores workspace and sessions on success", async () => {
      const session = useSessionStore();
      mockedInvoke.mockResolvedValueOnce([
        { workspace_id: "ws1", path: "/tmp" }
      ]); // list_workspaces
      mockedInvoke.mockResolvedValueOnce(undefined); // restore_workspace
      mockedInvoke.mockResolvedValueOnce([
        {
          id: "s1",
          title: "Recovered",
          profile: "fast",
          model_id: null,
          provider: null
        }
      ]); // list_sessions
      mockedInvoke.mockResolvedValueOnce(emptyProjection); // switch_session
      mockedInvoke.mockResolvedValueOnce([]); // get_trace
      const result = await session.recoverSessions();
      expect(result).toBe(true);
      expect(session.workspaceId).toBe("ws1");
      expect(session.sessions).toHaveLength(1);
      expect(session.currentSessionId).toBe("s1");
    });

    it("returns false when no workspaces exist", async () => {
      const session = useSessionStore();
      mockedInvoke.mockResolvedValueOnce([]); // list_workspaces
      const result = await session.recoverSessions();
      expect(result).toBe(false);
    });
  });
  ```

  Run:

  ```bash
  pnpm --filter agent-gui exec vitest run src/stores/session.test.ts src/stores/session-ipc.test.ts
  ```

  Expected: 14 + 6 = 20 cases pass. Failure typically signals a missed `.value` unwrap in a test assertion — fix the assertion, not the store.

- [ ] **Step 9: Wrap `useNotifications` into a tiny `ui` notifications store**

  Verified inventory:
  - `composables/useNotifications.ts` (27 LOC) currently exports `useNotifications()` + a top-level `addNotification(level, msg)` helper.
  - **Top-level `addNotification` callers** (must keep working — they live in non-component code where `useUiStore()` is fine to call): `stores/session.ts`, `stores/memory.ts`, `stores/mcp.ts`, `stores/catalog.ts`, `composables/useTauriEvents.ts`, `App.vue`.
  - There is no `useNotifications.test.ts` for **this** composable currently — only `composables/useNotifications.test.ts` (61 LOC) for the broader notifications API. Reuse that test file.

  Create `apps/agent-gui/src/stores/ui.ts` (minimal version):

  ```ts
  import { defineStore } from "pinia";
  import { ref } from "vue";

  export type NotificationLevel = "info" | "success" | "warning" | "error";
  export interface NotificationItem {
    id: string;
    level: NotificationLevel;
    message: string;
    timestamp: number;
  }

  export const useUiStore = defineStore("ui", () => {
    const notifications = ref<NotificationItem[]>([]);

    function pushNotification(level: NotificationLevel, message: string) {
      notifications.value.push({
        id: crypto.randomUUID(),
        level,
        message,
        timestamp: Date.now()
      });
    }

    function dismissNotification(id: string) {
      notifications.value = notifications.value.filter((n) => n.id !== id);
    }

    return { notifications, pushNotification, dismissNotification };
  });
  ```

  Refactor `apps/agent-gui/src/composables/useNotifications.ts` to delegate:

  ```ts
  import { storeToRefs } from "pinia";
  import { useUiStore, type NotificationLevel } from "@/stores/ui";

  export function useNotifications() {
    const ui = useUiStore();
    const { notifications } = storeToRefs(ui);
    return {
      notifications,
      addNotification: (level: NotificationLevel, message: string) =>
        ui.pushNotification(level, message),
      dismissNotification: (id: string) => ui.dismissNotification(id)
    };
  }

  // Back-compat top-level fn used by other modules (App.vue, session store)
  export function addNotification(level: NotificationLevel, message: string) {
    useUiStore().pushNotification(level, message);
  }
  ```

  Then update `apps/agent-gui/src/composables/useNotifications.test.ts` (61 LOC) — wrap each test's setup with:

  ```ts
  import { setActivePinia, createPinia } from "pinia";
  beforeEach(() => setActivePinia(createPinia()));
  ```

  Existing assertions (about `notifications.value` after `addNotification(...)`) keep working unchanged because the new helper still mutates the same backing array via the store.

  Run:

  ```bash
  pnpm --filter agent-gui exec vitest run src/composables/useNotifications.test.ts
  ```

  Expected: green.

- [ ] **Step 10: Update `composables/useTauriEvents.ts` consumer (143 LOC, 4 store imports)**

  Verified current imports (lines 4, 6, 8, 9, 10, 11):

  ```ts
  import { sessionState, applyEvent } from "../stores/session";
  import { applyTraceEvent } from "./useTraceStore";
  import { taskGraphState } from "../stores/taskGraph";
  import { addNotification } from "./useNotifications";
  import { handleMcpEvent } from "../stores/mcp";
  import { applyAgentEvent } from "../stores/agents";
  import { fetchSources, handleSourceFailed } from "../stores/catalog";
  ```

  Replace with:

  ```ts
  import { useSessionStore } from "@/stores/session";
  import { applyTraceEvent } from "./useTraceStore";
  import { useTaskGraphStore } from "@/stores/taskGraph";
  import { useUiStore } from "@/stores/ui";
  import { useMcpStore } from "@/stores/mcp";
  import { useAgentsStore } from "@/stores/agents";
  import { useCatalogStore } from "@/stores/catalog";
  ```

  Then **inside the `useTauriEvents()` body** (which is a composable run inside a setup scope, so Pinia is active), bind the stores once:

  ```ts
  export function useTauriEvents() {
    const session = useSessionStore();
    const taskGraph = useTaskGraphStore();
    const ui = useUiStore();
    const mcp = useMcpStore();
    const agents = useAgentsStore();
    const catalog = useCatalogStore();

    let unlisten: (() => void) | null = null;

    onMounted(async () => {
      unlisten = await listen<DomainEvent>("session-event", (tauriEvent) => {
        const domainEvent = tauriEvent.payload;
        const sessionId: string | undefined = domainEvent.session_id;
        if (
          sessionId &&
          session.currentSessionId &&
          sessionId === session.currentSessionId
        ) {
          session.applyEvent(domainEvent);
          applyTraceEvent(domainEvent);

          const p = domainEvent.payload;
          switch (p.type) {
            case "AgentTaskCreated": {
              if (!taskGraph.tasks.some((t) => t.id === p.task_id)) {
                taskGraph.tasks.push({
                  id: p.task_id,
                  title: p.title,
                  role: p.role,
                  state: "Pending" as TaskState,
                  dependencies: p.dependencies,
                  error: null,
                  retry_count: 0,
                  max_retries: 3,
                  assigned_agent_id: null,
                  failure_reason: null
                });
                if (taskGraph.currentSessionId === sessionId) {
                  taskGraph.tasks = [...taskGraph.tasks];
                }
              }
              break;
            }
            case "AgentTaskStarted": {
              const task = taskGraph.tasks.find((t) => t.id === p.task_id);
              if (task) {
                task.state = "Running" as TaskState;
                taskGraph.tasks = [...taskGraph.tasks];
              }
              break;
            }
            case "AgentTaskCompleted": {
              const task = taskGraph.tasks.find((t) => t.id === p.task_id);
              if (task) {
                task.state = "Completed" as TaskState;
                taskGraph.tasks = [...taskGraph.tasks];
              }
              break;
            }
            case "AgentTaskFailed": {
              const task = taskGraph.tasks.find((t) => t.id === p.task_id);
              if (task) {
                task.state = "Failed" as TaskState;
                task.error = p.error;
                taskGraph.tasks = [...taskGraph.tasks];
              }
              if (p.error) {
                ui.pushNotification("error", p.error);
              }
              break;
            }
            case "TaskBlocked": {
              const task = taskGraph.tasks.find((t) => t.id === p.task_id);
              if (task) {
                task.state = "Blocked" as TaskState;
                task.error = p.reason || "Dependency failed";
                taskGraph.tasks = [...taskGraph.tasks];
              }
              break;
            }
            case "TaskDecomposed":
              break;
            case "TaskRetried": {
              const task = taskGraph.tasks.find((t) => t.id === p.task_id);
              if (task) {
                task.state = "Running" as TaskState;
                task.retry_count = p.attempt;
                task.error = null;
                taskGraph.tasks = [...taskGraph.tasks];
              }
              break;
            }
          }

          agents.applyAgentEvent(domainEvent.payload);
        }

        const payload = domainEvent.payload;
        switch (payload.type) {
          case "McpServerStarting":
          case "McpServerReady":
          case "McpServerStopped":
          case "McpServerFailed":
          case "McpToolCallStarted":
          case "McpToolCallCompleted":
          case "McpTrustGranted":
          case "McpTrustRevoked":
            mcp.handleMcpEvent(payload);
            break;
          case "CatalogSourceAdded":
            void catalog.fetchSources();
            break;
          case "CatalogSourceFailed":
            catalog.handleSourceFailed(payload.source, payload.error);
            break;
        }
      });
      session.connected = true;
    });

    onUnmounted(() => {
      unlisten?.();
      session.connected = false;
    });
  }
  ```

  Note: this commit keeps the explicit `onMounted` / `onUnmounted` lifecycle. The `tryOnScopeDispose`-based cleanup style is introduced in Task 4 Step 4 once `@vueuse/core` is wired in.

- [ ] **Step 11: Update `composables/useTraceStore.ts` if it imports any store state**

  ```bash
  grep -n "from.*stores/" apps/agent-gui/src/composables/useTraceStore.ts
  ```

  If any imports exist, apply the same pattern. If none, skip this step.

- [ ] **Step 12: Update `App.vue`**

  Verified current `App.vue` (line 8) imports:

  ```ts
  import {
    sessionState,
    recoverSessions,
    setProjection
  } from "./stores/session";
  import { addNotification } from "./composables/useNotifications";
  ```

  And uses (line 40-58 inside `onMounted`):

  ```ts
  sessionState.initialized = true;
  sessionState.workspaceId = workspaceInfo.workspace_id;
  sessionState.sessions = await invoke("list_sessions");
  // ... + manual switch_session invoke + setProjection chain
  ```

  Replace the entire `<script setup>` block with:

  ```ts
  import { onMounted } from "vue";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { useTauriEvents } from "./composables/useTauriEvents";
  import { useUpdater } from "./composables/useUpdater";
  import { useSessionStore } from "@/stores/session";
  import { useUiStore } from "@/stores/ui";
  import ChatPanel from "./components/ChatPanel.vue";
  import SessionsSidebar from "./components/SessionsSidebar.vue";
  import StatusBar from "./components/StatusBar.vue";
  import TraceTimeline from "./components/TraceTimeline.vue";
  import PermissionCenter from "./components/PermissionCenter.vue";
  import NotificationToast from "./components/NotificationToast.vue";
  import Marketplace from "./views/Marketplace.vue";

  // (Workbench / Marketplace `view = ref(...)` toggle is preserved in this commit.
  //  Routing replaces it in Task 5; this commit only swaps the store API surface.)
  import { ref } from "vue";
  type View = "workbench" | "marketplace";
  const view = ref<View>("workbench");

  const session = useSessionStore();
  const ui = useUiStore();

  useTauriEvents();
  useUpdater();

  onMounted(async () => {
    await listen<{ type: string; error: string; session_id: string }>(
      "session-error",
      (event) => {
        ui.pushNotification("error", event.payload.error);
      }
    );

    const recovered = await session.recoverSessions();
    if (recovered) return;

    try {
      const workspaceInfo: { workspace_id: string; path: string } =
        await invoke("initialize_workspace");
      session.initialized = true;
      session.workspaceId = workspaceInfo.workspace_id;
      session.sessions = await invoke("list_sessions");
      if (session.sessions.length > 0) {
        try {
          await session.switchSession(session.sessions[0].id);
        } catch {
          // Initial session may have minimal data — non-critical.
        }
      }
    } catch (e) {
      console.error("Failed to initialize workspace:", e);
      ui.pushNotification("error", `Failed to initialize workspace: ${e}`);
    }
  });
  ```

  The template stays unchanged in this commit — Task 5 replaces it with `<AppLayout />`. The point of Step 12 is purely to swap `sessionState` → `session` (store proxy) and `addNotification` → `ui.pushNotification`, while preserving everything else byte-identical.

- [ ] **Step 13: Update all 14 + 6 component files' store imports**

  Run a grep to enumerate consumers:

  ```bash
  grep -rln "from.*stores/" apps/agent-gui/src/components apps/agent-gui/src/views 2>/dev/null
  ```

  For each file: replace `import { xxxState, action } from "../stores/xxx"` with `import { useXxxStore } from "@/stores/xxx"` + `const xxx = useXxxStore(); const { state } = storeToRefs(xxx);`. Action calls stay as `xxx.action(...)`.

  **Do this file-by-file. Run vitest after each component to catch regressions early:**

  ```bash
  pnpm --filter agent-gui exec vitest run src/components/<NameThatJustChanged>.test.ts
  ```

- [ ] **Step 14: Create `src/test-utils/mount.ts` for tests that need Pinia**

  Create `apps/agent-gui/src/test-utils/mount.ts`:

  ```ts
  import {
    mount as baseMount,
    type ComponentMountingOptions
  } from "@vue/test-utils";
  import type { Component } from "vue";
  import { createPinia, setActivePinia } from "pinia";
  import { createI18n } from "vue-i18n";
  import { createRouter, createMemoryHistory } from "vue-router";
  import en from "@/locales/en.json";
  import { routes } from "@/router/routes";

  export function mountWithPlugins<T extends Component>(
    comp: T,
    options: ComponentMountingOptions<T> = {}
  ) {
    const pinia = createPinia();
    setActivePinia(pinia);
    const i18n = createI18n({
      legacy: false,
      locale: "en",
      messages: { en }
    });
    const router = createRouter({ history: createMemoryHistory(), routes });
    return baseMount(comp, {
      ...options,
      global: {
        plugins: [pinia, i18n, router],
        ...(options.global ?? {})
      }
    });
  }
  ```

  Update tests that mount components to use `mountWithPlugins` instead of `mount` from `@vue/test-utils` directly. Use grep:

  ```bash
  grep -rln "from \"@vue/test-utils\"" apps/agent-gui/src/components apps/agent-gui/src/composables
  ```

- [ ] **Step 15: Run full vitest suite**

  ```bash
  pnpm --filter agent-gui run test
  ```

  Expected: ≥23 specs pass (some may now be ≥24 if you added a `ui.test.ts`). **Zero failures.**

- [ ] **Step 16: Run lint and build**

  ```bash
  pnpm run lint:eslint
  pnpm --filter agent-gui run build
  ```

  Expected: lint clean, build succeeds.

- [ ] **Step 17: Commit**

  ```bash
  git add apps/agent-gui/src
  git commit -m "refactor(gui): migrate stores to pinia setup-store style"
  ```

  Expected: husky runs lint-staged on all touched files; commit succeeds.

---

## Task 4: Integrate @vueuse/core (theme + locale + listener cleanup) (commit 4)

**Branch:** `feat/frontend-engineering`
**Commit message:** `feat(gui): integrate @vueuse/core (useDark, useColorMode, useStorage, tryOnScopeDispose)`
**Why fourth:** the `ui` store skeleton from Task 3 is expanded with vueuse-backed state (theme + locale persistence). `useTauriEvents.ts` gets `tryOnScopeDispose` cleanup. No NaiveUI yet — that's Task 6.

> **📋 Carry-over from Task 3 code-quality review (commit `ca15a29`) — bundle these with the @vueuse work below since this Task already touches `stores/ui.ts` and `composables/useTauriEvents.ts`. None are blockers; all are low/medium severity cleanups the Task 3 reviewer flagged for follow-up:**
>
> 1. **(medium) Move task-event mutations from `useTauriEvents.ts` into a new `taskGraph.applyTaskEvent(payload)` action.** Today `apps/agent-gui/src/composables/useTauriEvents.ts:39-101` directly mutates `taskGraph.tasks.push(...)`, sets `task.state = "Running"`, and reassigns `taskGraph.tasks = [...taskGraph.tasks]` (6 occurrences) from outside the store. Mirror the existing `agents.applyAgentEvent(payload)` (line 102 of the same file) and `mcp.handleMcpEvent(payload)` (line 113) patterns: add `applyTaskEvent(payload: EventPayload)` to `apps/agent-gui/src/stores/taskGraph.ts` containing every per-event branch (`task-spawned`, `task-state-changed`, `task-output`, etc.) currently inlined in `useTauriEvents.ts`, then have the composable call `taskGraph.applyTaskEvent(payload)`. Drop all six `taskGraph.tasks = [...taskGraph.tasks]` reassignments — they were `reactive()`-era reactivity pokes that are wasted allocations under Pinia + `ref()` (Vue 3.4+ tracks the proxy mutations natively).
> 2. **(low) Replace App.vue first-run init mutations with a `session.initializeWorkspace()` action.** `apps/agent-gui/src/App.vue:43-47` currently writes `session.initialized = true; session.workspaceId = workspaceInfo.workspace_id; session.sessions = await invoke("list_sessions")` directly. Promote this to an action on `apps/agent-gui/src/stores/session.ts` so the store owns its initialization protocol; App.vue then awaits one call.
> 3. **(low) `session.setProjection` should call the neighbor store's action, not poke its fields.** `apps/agent-gui/src/stores/session.ts:142-148` writes `taskGraph.tasks = next.task_graph.tasks; taskGraph.currentSessionId = currentSessionId.value` directly. Replace with `useTaskGraphStore().setTaskGraph(next.task_graph.tasks, currentSessionId.value)` — the action already exists at `stores/taskGraph.ts:25-28`. The same store already calls `useTaskGraphStore().clearTaskGraph()` correctly on line 159, so this restores internal consistency.
> 4. **(low) Delete `composables/useNotifications.ts` and `composables/useNotifications.test.ts`.** Repo-wide grep `grep -rn "from.*useNotifications" apps/agent-gui/src` returns exactly one consumer: `useNotifications.test.ts` itself. The shim has no production callers (every store and SFC consumer uses `useUiStore()` directly). Since this Task is the natural place to expand `stores/ui.ts`, also delete the shim and its self-test. Do **not** add re-export aliases — Task 3 already established the no-back-compat-shim discipline.
> 5. **(low) Pick one cross-store-call style and apply it consistently.** Currently:
>    - `stores/catalog.ts` does `const ui = useUiStore(); … ui.pushNotification(…)` (binds once at action top, 10 occurrences)
>    - `stores/session.ts:193,214` uses the same bound style; `stores/session.ts:245` uses inline `useUiStore().pushNotification(…)`
>    - `stores/mcp.ts` (6 occurrences) and `stores/memory.ts` (2 occurrences) always inline
>      Both styles are functionally correct (Pinia caches the store handle) and lazy (no top-level call), but inconsistency across files — and even within `session.ts` — is a smell. Adopt the **`const ui = useUiStore()` at action top** form everywhere. Apply uniformly across all four stores.
> 6. **(low) Extract a shared `createUiStoreMock()` helper in `apps/agent-gui/src/test-utils/`.** Four test files (`stores/{memory,mcp,catalog}.test.ts` + `stores/session-ipc.test.ts`) hand-roll `vi.mock("@/stores/ui", …)` factories with subtle drift: `session-ipc.test.ts:13-19` includes `notifications: []` while the other three don't. Today this is harmless (no test reads `notifications` through the mock), but if a future store action ever reads `useUiStore().notifications`, three of the four mocks will return `undefined` and only one will pass. Extract the canonical full-shape factory once, have all four mocks delegate to it.
> 7. **(informational) Consider migrating `composables/useTraceStore.ts` to a Pinia setup-store as part of this Task.** It is the only remaining `reactive(...)` top-level state in `apps/agent-gui/src/` (verified via `grep -rn "reactive(" apps/agent-gui/src/`). Migrating it here keeps the codebase fully Pinia-uniform and removes the asymmetry. **Decision is yours**: if scope is tight, defer to a separate `chore(gui): migrate useTraceStore to pinia` follow-up commit on this same branch. If you do migrate it, also rename the file to `apps/agent-gui/src/stores/trace.ts` for consistency, and update its sole consumer (`apps/agent-gui/src/composables/useTraceStore.test.ts` is the largest test file in the repo at 21 KB — handle with care).
>
> All seven items together comprise a single "tighten store boundaries + extend ui store" commit alongside the @vueuse integration. Keep the commit message header the same as already specified above; expand the body to mention the carried-over cleanups.

**Files:**

- Modify: `apps/agent-gui/src/stores/ui.ts`
- Modify: `apps/agent-gui/src/composables/useTauriEvents.ts`
- Modify: `apps/agent-gui/src/locales/index.ts` (sync locale state with the store)
- Create: `apps/agent-gui/src/stores/ui.test.ts`

- [ ] **Step 1: Expand `stores/ui.ts` with theme + locale + sidebar state**

  Replace `apps/agent-gui/src/stores/ui.ts` entirely:

  ```ts
  import { defineStore } from "pinia";
  import { ref } from "vue";
  import { useDark, useColorMode, useStorage } from "@vueuse/core";

  export type NotificationLevel = "info" | "success" | "warning" | "error";
  export interface NotificationItem {
    id: string;
    level: NotificationLevel;
    message: string;
    timestamp: number;
  }
  export type ThemeMode = "auto" | "light" | "dark";
  export type SupportedLocale = "en" | "zh-CN";

  export const useUiStore = defineStore("ui", () => {
    // ── Theme ───────────────────────────────────────────────
    const colorMode = useColorMode({
      storageKey: "kairox.color-mode",
      initialValue: "auto"
    });
    const isDark = useDark({ storageKey: "kairox.color-mode" });

    function setTheme(mode: ThemeMode) {
      colorMode.value = mode;
    }

    // ── Locale ──────────────────────────────────────────────
    const locale = useStorage<SupportedLocale>(
      "kairox.locale",
      "en",
      undefined,
      {
        serializer: {
          read: (v) => (v === "zh-CN" || v === "en" ? v : "en"),
          write: (v) => v
        }
      }
    );

    function setLocale(next: SupportedLocale) {
      locale.value = next;
    }

    // ── Sidebar (future-proof) ──────────────────────────────
    const sidebarCollapsed = useStorage("kairox.sidebar-collapsed", false);

    // ── Notifications ───────────────────────────────────────
    const notifications = ref<NotificationItem[]>([]);

    function pushNotification(level: NotificationLevel, message: string) {
      notifications.value.push({
        id: crypto.randomUUID(),
        level,
        message,
        timestamp: Date.now()
      });
    }

    function dismissNotification(id: string) {
      notifications.value = notifications.value.filter((n) => n.id !== id);
    }

    return {
      colorMode,
      isDark,
      setTheme,
      locale,
      setLocale,
      sidebarCollapsed,
      notifications,
      pushNotification,
      dismissNotification
    };
  });
  ```

- [ ] **Step 2: Sync locale store value into i18n on change**

  Replace `apps/agent-gui/src/locales/index.ts` with:

  ```ts
  import { createI18n } from "vue-i18n";
  import { watch } from "vue";
  import en from "./en.json";
  import zhCN from "./zh-CN.json";

  export type SupportedLocale = "en" | "zh-CN";

  function detectInitialLocale(): SupportedLocale {
    if (typeof window === "undefined") return "en";
    const stored = window.localStorage.getItem("kairox.locale");
    return stored === "zh-CN" || stored === "en" ? stored : "en";
  }

  export const i18n = createI18n({
    legacy: false,
    locale: detectInitialLocale(),
    fallbackLocale: "en",
    messages: { en, "zh-CN": zhCN }
  });

  /**
   * Wire the ui store's `locale` ref to i18n's runtime locale.
   * Must be called after `app.use(createPinia())` runs.
   */
  export function bindLocaleToStore() {
    void import("@/stores/ui").then(({ useUiStore }) => {
      const ui = useUiStore();
      ui.locale = i18n.global.locale.value as SupportedLocale;
      watch(
        () => ui.locale,
        (next) => {
          i18n.global.locale.value = next;
        }
      );
    });
  }
  ```

- [ ] **Step 3: Call `bindLocaleToStore()` from `main.ts`**

  Edit `apps/agent-gui/src/main.ts`:

  ```ts
  import { createApp } from "vue";
  import { createPinia } from "pinia";
  import App from "./App.vue";
  import { router } from "./router";
  import { i18n, bindLocaleToStore } from "./locales";
  import "./assets/main.css";
  import "highlight.js/styles/github-dark.css";

  const app = createApp(App);
  app.use(createPinia());
  app.use(router);
  app.use(i18n);
  bindLocaleToStore();
  app.mount("#app");
  ```

- [ ] **Step 4: Refactor `useTauriEvents.ts` to use `tryOnScopeDispose`**

  Read `apps/agent-gui/src/composables/useTauriEvents.ts` first to capture every `listen()` call site.

  Apply this pattern uniformly to every site:

  Old:

  ```ts
  const unlisten = await listen<DomainEvent>("domain-event", handler);
  onUnmounted(() => unlisten());
  ```

  New (concrete full replacement of `apps/agent-gui/src/composables/useTauriEvents.ts` — verified verbatim against the current 143-LOC source). The change is purely lifecycle: the original used `onMounted` to await `listen()` and `onUnmounted` to call `unlisten()`; the migrated version awaits `listen()` eagerly inside an IIFE and registers cleanup via `tryOnScopeDispose`. Every store-routing branch (8 task-graph cases + 8 MCP cases + 2 catalog cases) and the agent-event routing call are preserved 1:1, with two store-API substitutions: `sessionState.X` → `session.X` and `taskGraphState.X` → `taskGraph.X`; `addNotification(...)` → `ui.pushNotification(...)`; `handleMcpEvent(payload)` → `mcp.handleMcpEvent(payload)`; `applyAgentEvent(...)` → `agents.applyAgentEvent(...)`; `fetchSources()` → `catalog.fetchSources()`; `handleSourceFailed(...)` → `catalog.handleSourceFailed(...)`.

  Replace the file with:

  ```ts
  import { tryOnScopeDispose } from "@vueuse/core";
  import { listen } from "@tauri-apps/api/event";
  import type { DomainEvent, TaskState } from "@/types";
  import { useSessionStore, applyEvent } from "@/stores/session";
  import { applyTraceEvent } from "@/composables/useTraceStore";
  import { useTaskGraphStore } from "@/stores/taskGraph";
  import { useUiStore } from "@/stores/ui";
  import { useMcpStore } from "@/stores/mcp";
  import { useAgentsStore } from "@/stores/agents";
  import { useCatalogStore } from "@/stores/catalog";

  export function useTauriEvents() {
    const session = useSessionStore();
    const taskGraph = useTaskGraphStore();
    const ui = useUiStore();
    const mcp = useMcpStore();
    const agents = useAgentsStore();
    const catalog = useCatalogStore();

    // listen() returns a Promise<UnlistenFn>; await it eagerly inside an IIFE
    // so cleanup registration via tryOnScopeDispose runs in the same setup tick.
    void (async () => {
      const unlisten = await listen<DomainEvent>(
        "session-event",
        (tauriEvent) => {
          // Only process session-scoped events for the current session.
          const domainEvent = tauriEvent.payload;
          const sessionId: string | undefined = domainEvent.session_id;
          if (
            sessionId &&
            session.currentSessionId &&
            sessionId === session.currentSessionId
          ) {
            applyEvent(domainEvent);
            applyTraceEvent(domainEvent);

            // Mirror the Rust SessionProjection::apply() task-graph mutations
            // so the Tasks panel updates immediately without an async invoke.
            const p = domainEvent.payload;
            switch (p.type) {
              case "AgentTaskCreated": {
                if (!taskGraph.tasks.some((t) => t.id === p.task_id)) {
                  taskGraph.tasks.push({
                    id: p.task_id,
                    title: p.title,
                    role: p.role,
                    state: "Pending" as TaskState,
                    dependencies: p.dependencies,
                    error: null,
                    retry_count: 0,
                    max_retries: 3,
                    assigned_agent_id: null,
                    failure_reason: null
                  });
                  if (taskGraph.currentSessionId === sessionId) {
                    taskGraph.tasks = [...taskGraph.tasks];
                  }
                }
                break;
              }
              case "AgentTaskStarted": {
                const task = taskGraph.tasks.find((t) => t.id === p.task_id);
                if (task) {
                  task.state = "Running" as TaskState;
                  taskGraph.tasks = [...taskGraph.tasks];
                }
                break;
              }
              case "AgentTaskCompleted": {
                const task = taskGraph.tasks.find((t) => t.id === p.task_id);
                if (task) {
                  task.state = "Completed" as TaskState;
                  taskGraph.tasks = [...taskGraph.tasks];
                }
                break;
              }
              case "AgentTaskFailed": {
                const task = taskGraph.tasks.find((t) => t.id === p.task_id);
                if (task) {
                  task.state = "Failed" as TaskState;
                  task.error = p.error;
                  taskGraph.tasks = [...taskGraph.tasks];
                }
                if (p.error) {
                  ui.pushNotification("error", p.error);
                }
                break;
              }
              case "TaskBlocked": {
                const task = taskGraph.tasks.find((t) => t.id === p.task_id);
                if (task) {
                  task.state = "Blocked" as TaskState;
                  task.error = p.reason || "Dependency failed";
                  taskGraph.tasks = [...taskGraph.tasks];
                }
                break;
              }
              case "TaskDecomposed": {
                // Informational — sub-tasks arrive via separate AgentTaskCreated events.
                break;
              }
              case "TaskRetried": {
                const task = taskGraph.tasks.find((t) => t.id === p.task_id);
                if (task) {
                  task.state = "Running" as TaskState;
                  task.retry_count = p.attempt;
                  task.error = null;
                  taskGraph.tasks = [...taskGraph.tasks];
                }
                break;
              }
            }

            // Route agent lifecycle events to the agents store.
            agents.applyAgentEvent(domainEvent.payload);
          }

          // MCP and catalog source events are global, not session-scoped.
          const payload = domainEvent.payload;
          switch (payload.type) {
            case "McpServerStarting":
            case "McpServerReady":
            case "McpServerStopped":
            case "McpServerFailed":
            case "McpToolCallStarted":
            case "McpToolCallCompleted":
            case "McpTrustGranted":
            case "McpTrustRevoked":
              mcp.handleMcpEvent(payload);
              break;
            case "CatalogSourceAdded":
              void catalog.fetchSources();
              break;
            case "CatalogSourceFailed":
              catalog.handleSourceFailed(payload.source, payload.error);
              break;
          }
        }
      );

      tryOnScopeDispose(() => {
        unlisten();
        session.connected = false;
      });
      session.connected = true;
    })();
  }
  ```

  Reasoning for the structural change: the original `onMounted`/`onUnmounted` pair only worked because `useTauriEvents()` was called from `App.vue`'s setup. After the layout refactor (Task 5), the call site moves to `AppLayout.vue`'s setup, which is also a component scope, so `tryOnScopeDispose` is functionally equivalent. The benefit is that `useTauriEvents()` becomes callable from any reactive scope (`effectScope`, future composable wrappers), not just component setup — matching the @vueuse/core convention used throughout this branch.

- [ ] **Step 5: Write tests for the `ui` store**

  Create `apps/agent-gui/src/stores/ui.test.ts`:

  ```ts
  import { describe, it, expect, beforeEach } from "vitest";
  import { setActivePinia, createPinia } from "pinia";
  import { useUiStore } from "@/stores/ui";

  describe("ui store", () => {
    beforeEach(() => {
      window.localStorage.clear();
      setActivePinia(createPinia());
    });

    describe("notifications", () => {
      it("starts empty", () => {
        const ui = useUiStore();
        expect(ui.notifications).toEqual([]);
      });

      it("push then dismiss", () => {
        const ui = useUiStore();
        ui.pushNotification("info", "hello");
        expect(ui.notifications.length).toBe(1);
        const id = ui.notifications[0].id;
        ui.dismissNotification(id);
        expect(ui.notifications).toEqual([]);
      });

      it("each notification has unique id", () => {
        const ui = useUiStore();
        ui.pushNotification("info", "a");
        ui.pushNotification("info", "b");
        const ids = ui.notifications.map((n) => n.id);
        expect(new Set(ids).size).toBe(2);
      });
    });

    describe("theme", () => {
      it("defaults to auto color mode", () => {
        const ui = useUiStore();
        expect(ui.colorMode).toBe("auto");
      });

      it("setTheme updates the colorMode ref", () => {
        const ui = useUiStore();
        ui.setTheme("dark");
        expect(ui.colorMode).toBe("dark");
      });
    });

    describe("locale", () => {
      it("defaults to en when storage is empty", () => {
        const ui = useUiStore();
        expect(ui.locale).toBe("en");
      });

      it("setLocale persists to localStorage", () => {
        const ui = useUiStore();
        ui.setLocale("zh-CN");
        expect(ui.locale).toBe("zh-CN");
        expect(window.localStorage.getItem("kairox.locale")).toBe("zh-CN");
      });

      it("rejects invalid locale from storage", () => {
        window.localStorage.setItem("kairox.locale", "fr-FR");
        setActivePinia(createPinia());
        const ui = useUiStore();
        expect(ui.locale).toBe("en");
      });
    });
  });
  ```

- [ ] **Step 6: Run vitest for the new file**

  ```bash
  pnpm --filter agent-gui exec vitest run src/stores/ui.test.ts
  ```

  Expected: all ui store tests pass.

- [ ] **Step 7: Run full test suite + lint + build**

  ```bash
  pnpm --filter agent-gui run test
  pnpm run lint:eslint
  pnpm --filter agent-gui run build
  ```

  Expected: vitest pass count ≥ Task 3 + 1 (new `ui.test.ts`); lint clean; build succeeds.

- [ ] **Step 8: Commit**

  ```bash
  git add apps/agent-gui/src/stores/ui.ts apps/agent-gui/src/stores/ui.test.ts \
          apps/agent-gui/src/composables/useTauriEvents.ts \
          apps/agent-gui/src/locales/index.ts apps/agent-gui/src/main.ts
  git commit -m "feat(gui): integrate @vueuse/core (useDark, useColorMode, useStorage, tryOnScopeDispose)"
  ```

---

> **Note for Task 5:** the `switchSession` action is already added to the session store in **Task 3 Step 8** (verified above). Task 5 may freely call `session.switchSession(id)` without redefining it.

> **⚠️ MANDATORY for Task 5 (carried over from Task 2 spec-review follow-up):** Task 2 shipped `apps/agent-gui/src/router/routes.ts` with a `placeholderComponent(name)` factory in place of the spec's `() => import("@/views/*View.vue")` lazy imports, because rolldown (Vite 8 default bundler) statically resolves dynamic import paths and hard-fails when target SFCs are missing. Task 5 creates those SFCs, so **Task 5 MUST** apply this exact rewrite to `apps/agent-gui/src/router/routes.ts`:
>
> 1. Open `apps/agent-gui/src/router/routes.ts` with `read_file` to confirm the current shape (verified at Task 2 commit `aacbeeb`: 32 lines total). Then delete lines 3–11 inclusive — these 9 lines contain BOTH regions of the placeholder helper. The exact lines to remove are:
>
>    ```ts
>    // TODO(Task 5): swap these placeholder components for real lazy imports of
>    // `@/views/WorkbenchView.vue`, `@/views/MarketplaceView.vue`, and
>    // `@/views/SettingsView.vue` once those SFCs exist. Vite's build cannot
>    // statically resolve dynamic imports to non-existent modules, so we ship
>    // runtime stubs in Task 2 to keep the build green.
>    const placeholderComponent = (name: string) => () =>
>      Promise.resolve({
>        default: { name, template: `<div>view-placeholder:${name}</div>` }
>      });
>    ```
>
>    If Prettier has reformatted the file since Task 2 (line numbers may have shifted by ±1), match by content rather than line number: remove the entire 5-line `// TODO(Task 5): ...` comment block AND the entire 4-line `const placeholderComponent = ...` declaration that immediately follows it (ending with `});`).
>
> 2. Keep the `import type { RouteRecordRaw } from "vue-router";` line at the top of the file unchanged — it is still required by the `RouteRecordRaw[]` type annotation on the `routes` constant.
> 3. Replace each route's `component` field as follows (verbatim, all three must be applied — no `(etc.)` shortcuts):
>    - `component: placeholderComponent("WorkbenchView")` → `component: () => import("@/views/WorkbenchView.vue")`
>    - `component: placeholderComponent("MarketplaceView")` → `component: () => import("@/views/MarketplaceView.vue")`
>    - `component: placeholderComponent("SettingsView")` → `component: () => import("@/views/SettingsView.vue")`
> 4. The rest of the `routes` array (paths, names, `props: true` on workbench, root redirect, `:pathMatch(.*)*` wildcard, route ordering) MUST remain byte-for-byte identical to the spec block in this plan at **Task 2 Step 3 (plan L230–L249)**. Do not add, remove, or reorder routes.
> 5. After the rewrite:
>    - Run `pnpm --filter agent-gui run build` and confirm rolldown can now statically resolve all three imports (no `UNLOADABLE_DEPENDENCY` errors); the build output should now show three additional lazy-loaded chunks (one per view).
>    - Run `pnpm --filter agent-gui run test`. The test count MUST NOT regress relative to the previous task's verification step. Determine the floor by inspecting the previous task's implementer report (the most recent task before Task 5 — i.e. Task 4) for its final `pnpm --filter agent-gui run test` output, and using that exact reported count as the floor. Do not hard-code a number; always derive the floor from the prior task's actual measured count, because earlier tasks (Task 3 in particular) modify many existing `*.test.ts` files and may add or remove individual test cases as part of the Pinia migration. Task 5 itself adds no new unit tests (it only adds `data-test` attributes consumed by Playwright in Task 8), so the new count should equal the prior task's count exactly. If the count is lower, treat it as a regression and stop.
>
> Failing to do this means GUI navigation will forever render `<div>view-placeholder:WorkbenchView</div>` instead of the real views, and code-splitting (one chunk per view) will not work.

## Task 5: Add SettingsView + AppLayout + WorkbenchView/MarketplaceView wrappers (commit 5)

**Branch:** `feat/frontend-engineering`
**Commit message:** `feat(gui): add settings view, app layout, and route-level views`
**Why fifth:** the router defined in Task 2 references views that do not exist. This commit creates them as **plain HTML wrappers** (no NaiveUI yet) so navigation works end-to-end. NaiveUI replaces the styling in Task 6. Settings view is the demo destination for locale + theme switching.

> **📋 Carry-over from Task 3 code-quality review (commit `ca15a29`) — bundle this with the view-creation work below since this Task is the first one that actually consumes the `apps/agent-gui/src/test-utils/mount.ts` helper that Task 3 created:**
>
> - **(low) Add an import-only smoke test for `test-utils/mount.ts` and verify its dependency paths resolve.** The helper was created in Task 3 (commit `ca15a29`, file `apps/agent-gui/src/test-utils/mount.ts`, 31 LOC) but is currently zero-referenced (`grep -rn "mountWithPlugins\|from.*test-utils/mount" apps/agent-gui/` returns no call sites). The helper imports `@/locales/en.json` (exists, created in Task 2) and `@/router/routes` (exists, but at the start of this Task 5 it still exports the `placeholderComponent`-based routes from Task 2 — the MANDATORY follow-up that appears **earlier in this plan, immediately above this Task 5 heading** converts those routes to real `() => import("@/views/*View.vue")` lazy imports). Two concrete actions:
>   1. After you have applied the MANDATORY router rewrite (the `⚠️ MANDATORY for Task 5` block immediately preceding this Task 5 section), create a one-test smoke spec at `apps/agent-gui/src/test-utils/mount.test.ts` that does `import { mountWithPlugins } from "./mount"` and asserts `typeof mountWithPlugins === "function"`. This guarantees both `@/locales/en.json` and `@/router/routes` resolve at test time before any real consumer (Task 6+) tries to use it. The spec must use the same vitest patterns as the rest of this repo (`describe` + `it` + `expect` from `vitest/globals`).
>   2. Add a JSDoc comment block to `mountWithPlugins` documenting: (a) when to use it (component tests that need router-link / `<RouterView>` / `$t` resolution), (b) what it costs (creates a fresh memory-history router and English-only i18n on every mount — overkill for tests that don't need them), (c) recommended alternative for store-only tests (use `setActivePinia(createPinia())` + plain `mount(Component)` from `@vue/test-utils` directly).
>
> **📋 Carry-over from Task 4 code-quality re-review (commit `95db18b`) — bundle this with Task 5 since `useTauriEvents.ts` will likely be imported into the new `AppLayout.vue` (the layout is the natural mount point for the global session-event subscription) and any change to its error-surfacing behavior is best reviewed alongside its first layout-level consumer:**
>
> - **(IMPORTANT) Surface `listen()` rejection in `useTauriEvents.ts` instead of silently dropping it.** Currently `apps/agent-gui/src/composables/useTauriEvents.ts:80-82` reads:
>
>   ```ts
>   void unlistenPromise.then(() => {
>     session.setConnected(true);
>   });
>   ```
>
>   This chain has NO `.catch()`. If `listen()` rejects (Tauri channel down, IPC severed, missing permissions, etc.) the promise becomes an **unhandled rejection** — the runtime emits a console-only warning, and the user sees only the "disconnected" status indicator with no explanation. The cleanup chain at line 75-78 correctly uses `.catch(() => {})` (silent on dispose is right because the component is already unmounting and cannot show a toast), but the setup chain must NOT be silent.
>
>   **Required fix:**
>
>   ```ts
>   void unlistenPromise
>     .then(() => session.setConnected(true))
>     .catch((err) => {
>       ui.pushNotification(
>         "error",
>         `Failed to subscribe to session events: ${err}`
>       );
>     });
>   ```
>
>   The `ui` store handle is already bound at the top of `useTauriEvents()` (it's used by the existing `AgentTaskFailed → ui.pushNotification(...)` branch on line 64), so no new imports are needed.
>
>   **Test addition:** Since `useTauriEvents.ts` doesn't currently have a dedicated test file, add a minimal one at `apps/agent-gui/src/composables/useTauriEvents.test.ts` with one test case: mock `@tauri-apps/api/event`'s `listen` to return `Promise.reject(new Error("channel closed"))`, mount a tiny dummy component that calls `useTauriEvents()`, await a microtask flush, then assert `useUiStore().notifications` contains an entry with `level: "error"` and message starting with `"Failed to subscribe to session events"`. This locks in the fix and prevents future regressions. **Do NOT** add tests for the happy path here — those are covered indirectly by the existing 27 test files.
>
>   **Out-of-scope explicitly:** the parallel NIT the reviewer mentioned (unifying setup-chain + cleanup-chain into a single promise chain) is purely cosmetic; defer to a future refactor commit on this same branch only if you happen to be touching this file again.

**Files:**

- Create: `apps/agent-gui/src/layouts/AppLayout.vue`
- Create: `apps/agent-gui/src/views/WorkbenchView.vue`
- Create: `apps/agent-gui/src/views/MarketplaceView.vue` (moved from `views/Marketplace.vue`)
- Create: `apps/agent-gui/src/views/SettingsView.vue`
- Modify: `apps/agent-gui/src/App.vue`
- Delete: `apps/agent-gui/src/views/Marketplace.vue` (after move)
- Modify: `apps/agent-gui/src/components/SessionsSidebar.vue` (route navigation on session click)

- [ ] **Step 1: Inspect current `views/Marketplace.vue`**

  ```bash
  cat apps/agent-gui/src/views/Marketplace.vue
  ```

  Capture the full content for the move in Step 2.

- [ ] **Step 2: Create `MarketplaceView.vue`**
  - Copy the entire content of `views/Marketplace.vue` into a new file `apps/agent-gui/src/views/MarketplaceView.vue`.
  - Adjust relative imports if needed (same `views/` dir, so paths usually unchanged).
  - Confirm consumers:

    ```bash
    grep -rn "views/Marketplace" apps/agent-gui/src apps/agent-gui/e2e
    ```

    Expected: only `App.vue` imports it. Defer deletion of the old file until Step 8.

- [ ] **Step 3: Create `WorkbenchView.vue`**

  Create `apps/agent-gui/src/views/WorkbenchView.vue`:

  ```vue
  <script setup lang="ts">
  import { onMounted, watch, computed } from "vue";
  import { useRoute, useRouter } from "vue-router";
  import { storeToRefs } from "pinia";
  import { useSessionStore } from "@/stores/session";
  import { useUiStore } from "@/stores/ui";
  import SessionsSidebar from "@/components/SessionsSidebar.vue";
  import ChatPanel from "@/components/ChatPanel.vue";
  import TraceTimeline from "@/components/TraceTimeline.vue";
  import PermissionCenter from "@/components/PermissionCenter.vue";

  const route = useRoute();
  const router = useRouter();
  const session = useSessionStore();
  const ui = useUiStore();
  const { currentSessionId } = storeToRefs(session);

  const routeSessionId = computed(() => {
    const v = route.params.sessionId;
    return Array.isArray(v) ? v[0] : v;
  });

  async function syncRouteToSession(id: string | undefined) {
    if (!id) return;
    if (id === currentSessionId.value) return;
    try {
      await session.switchSession(id);
    } catch {
      ui.pushNotification("error", `Session not found: ${id}`);
      await router.replace({ name: "workbench" });
    }
  }

  onMounted(() => {
    void syncRouteToSession(routeSessionId.value);
  });

  watch(routeSessionId, (next) => {
    void syncRouteToSession(next);
  });

  // Reflect store changes back into URL.
  watch(currentSessionId, (next) => {
    if (next && next !== routeSessionId.value) {
      void router.replace({ name: "workbench", params: { sessionId: next } });
    }
  });
  </script>

  <template>
    <main class="workbench" data-test="view-workbench">
      <SessionsSidebar />
      <ChatPanel />
      <aside class="right-sidebar">
        <TraceTimeline />
        <PermissionCenter />
      </aside>
    </main>
  </template>

  <style scoped>
  .workbench {
    display: grid;
    grid-template-columns: 220px 1fr 280px;
    flex: 1;
    overflow: hidden;
  }
  .right-sidebar {
    display: flex;
    flex-direction: column;
    border-left: 1px solid #d7d7d7;
    overflow: hidden;
  }
  </style>
  ```

  Note: `session.switchSession(id)` was added in Task 3 Step 8 (sets `currentSessionId`, calls `invoke('switch_session', ...)` + `setProjection`, then loads trace). No new store method is required here.

- [ ] **Step 4: Create `SettingsView.vue`**

  Create `apps/agent-gui/src/views/SettingsView.vue`:

  ```vue
  <script setup lang="ts">
  import { useI18n } from "vue-i18n";
  import { storeToRefs } from "pinia";
  import {
    useUiStore,
    type ThemeMode,
    type SupportedLocale
  } from "@/stores/ui";

  const { t } = useI18n();
  const ui = useUiStore();
  const { locale, colorMode } = storeToRefs(ui);

  const themes: { value: ThemeMode; labelKey: string }[] = [
    { value: "auto", labelKey: "settings.themeAuto" },
    { value: "light", labelKey: "settings.themeLight" },
    { value: "dark", labelKey: "settings.themeDark" }
  ];

  const locales: { value: SupportedLocale; labelKey: string }[] = [
    { value: "en", labelKey: "settings.localeEn" },
    { value: "zh-CN", labelKey: "settings.localeZh" }
  ];
  </script>

  <template>
    <section class="settings" data-test="view-settings">
      <h2>{{ t("settings.title") }}</h2>

      <div class="settings__row">
        <label>{{ t("settings.locale") }}</label>
        <select
          v-model="locale"
          data-test="settings-locale"
          @change="
            ui.setLocale(
              ($event.target as HTMLSelectElement).value as SupportedLocale
            )
          "
        >
          <option v-for="opt in locales" :key="opt.value" :value="opt.value">
            {{ t(opt.labelKey) }}
          </option>
        </select>
      </div>

      <div class="settings__row">
        <label>{{ t("settings.theme") }}</label>
        <select
          v-model="colorMode"
          data-test="settings-theme"
          @change="
            ui.setTheme(($event.target as HTMLSelectElement).value as ThemeMode)
          "
        >
          <option v-for="opt in themes" :key="opt.value" :value="opt.value">
            {{ t(opt.labelKey) }}
          </option>
        </select>
      </div>
    </section>
  </template>

  <style scoped>
  .settings {
    padding: 16px;
    max-width: 480px;
  }
  .settings__row {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-block: 12px;
  }
  .settings__row label {
    min-width: 100px;
  }
  </style>
  ```

- [ ] **Step 5: Create `AppLayout.vue`**

  Create `apps/agent-gui/src/layouts/AppLayout.vue`:

  ```vue
  <script setup lang="ts">
  import { useI18n } from "vue-i18n";
  import StatusBar from "@/components/StatusBar.vue";
  import NotificationToast from "@/components/NotificationToast.vue";

  const { t } = useI18n();
  </script>

  <template>
    <div class="app-shell" data-test="app-shell">
      <nav class="app-nav" data-test="app-nav">
        <RouterLink to="/workbench" data-test="nav-workbench">
          {{ t("nav.workbench") }}
        </RouterLink>
        <RouterLink to="/marketplace" data-test="nav-marketplace">
          {{ t("nav.marketplace") }}
        </RouterLink>
        <RouterLink to="/settings" data-test="nav-settings">
          {{ t("nav.settings") }}
        </RouterLink>
      </nav>
      <RouterView />
      <StatusBar />
      <NotificationToast />
    </div>
  </template>

  <style scoped>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .app-nav {
    display: flex;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid #d7d7d7;
    background: var(--surface-alt, #f7f7f7);
  }
  .app-nav a {
    padding: 4px 10px;
    border: 1px solid var(--border, #ccc);
    text-decoration: none;
    color: inherit;
    font-size: 12px;
  }
  .app-nav a.router-link-active {
    background: var(--accent, #345);
    color: #fff;
  }
  </style>
  ```

- [ ] **Step 6: Simplify `App.vue`**

  Read current `apps/agent-gui/src/App.vue` first to capture the workspace-init logic (use `read_file` to confirm it matches the snippet below; if it diverges, use the actual code as the source of truth, just relocate it).

  Replace `apps/agent-gui/src/App.vue` with:

  ```vue
  <script setup lang="ts">
  import { onMounted } from "vue";
  import { invoke } from "@tauri-apps/api/core";
  import { useTauriEvents } from "@/composables/useTauriEvents";
  import { useUpdater } from "@/composables/useUpdater";
  import { useSessionStore } from "@/stores/session";
  import { useUiStore } from "@/stores/ui";
  import AppLayout from "@/layouts/AppLayout.vue";

  const session = useSessionStore();
  const ui = useUiStore();

  useTauriEvents();
  useUpdater();

  onMounted(async () => {
    const recovered = await session.recoverSessions();
    if (recovered) return;

    try {
      const workspaceInfo: { workspace_id: string; path: string } =
        await invoke("initialize_workspace");
      session.workspaceId = workspaceInfo.workspace_id;
      session.initialized = true;
      session.sessions = await invoke("list_sessions");
      if (session.sessions.length > 0) {
        await session.switchSession(session.sessions[0].id);
      }
    } catch (e) {
      console.error("Failed to initialize workspace:", e);
      ui.pushNotification("error", `Failed to initialize workspace: ${e}`);
    }
  });
  </script>

  <template>
    <AppLayout />
  </template>
  ```

- [ ] **Step 7: Update `SessionsSidebar.vue` to navigate via router on click**

  ```bash
  grep -n "currentSessionId\|switchSession\|@click" apps/agent-gui/src/components/SessionsSidebar.vue | head -20
  ```

  In the click handler, after `await session.switchSession(id)`, add:

  ```ts
  await router.push({ name: "workbench", params: { sessionId: id } });
  ```

  Add at the top of `<script setup>`:

  ```ts
  import { useRouter } from "vue-router";
  const router = useRouter();
  ```

- [ ] **Step 8: Delete the old `views/Marketplace.vue`**

  ```bash
  grep -rn "views/Marketplace\b" apps/agent-gui/src apps/agent-gui/e2e
  ```

  Expected: no remaining matches (only `MarketplaceView` references). Then:

  ```bash
  git rm apps/agent-gui/src/views/Marketplace.vue
  ```

- [ ] **Step 9: Manual smoke via dev server**

  ```bash
  pnpm --filter agent-gui run dev
  ```

  In a browser at `http://localhost:1420`:
  - URL becomes `#/workbench` (catchall redirect)
  - Click each nav link, URL updates: `#/workbench`, `#/marketplace`, `#/settings`
  - Settings view: switch locale → nav link text immediately localizes
  - Settings view: switch theme to Dark → `<html class="dark">` appears (via vueuse `useDark`)
  - Browser back/forward navigates between views
  - Manually visit `#/workbench/bogus-id` → notification "Session not found: bogus-id" + redirect to `#/workbench`

  Then Ctrl-C the dev server.

- [ ] **Step 10: Run vitest + lint + build**

  ```bash
  pnpm --filter agent-gui run test
  pnpm run lint:eslint
  pnpm --filter agent-gui run build
  ```

  Expected: tests pass; lint clean; build succeeds (lazy-loaded view chunks now resolve).

- [ ] **Step 11: Commit**

  ```bash
  git add apps/agent-gui/src/views apps/agent-gui/src/layouts apps/agent-gui/src/App.vue \
          apps/agent-gui/src/components/SessionsSidebar.vue
  git commit -m "feat(gui): add settings view, app layout, and route-level views"
  ```

---

## Task 6: Integrate NaiveUI providers + theme overrides (commit 6)

**Branch:** `feat/frontend-engineering`
**Commit message:** `feat(gui): integrate naive-ui with provider stack and theme overrides`
**Why sixth:** wires NaiveUI's `NConfigProvider` + 4 service providers and maps existing CSS variables (`--accent`, `--border`, `--surface-alt`) to NaiveUI's theme tokens. **No SFC migration yet** — that's Task 7. After this commit, NaiveUI is available to call but no existing component uses it.

**MANDATORY pre-work (carried over from Task 5 code-quality review — collect them BEFORE the NaiveUI work in Step 1; same commit):**

- **Pre-work A — `WorkbenchView.vue` syncing guard (Task 5 IMPORTANT #1):**
  Add a `const syncing = ref(false)` to `apps/agent-gui/src/views/WorkbenchView.vue`. Set it to `true` at the top of `syncRouteToSession`, reset in a `finally` block. The reverse watcher (`watch(() => session.currentSessionId, …)` that calls `router.replace`) must early-return when `syncing.value` is `true`. This closes the race where a not-found `router.replace({ name: "workbench" })` is immediately undone by the `currentSessionId` watcher writing the stale id back into the URL. Existing `WorkbenchView` tests must still pass; add at minimum one new vitest case asserting that, after a `switchSession` rejection, the URL ends up at `/workbench` (no sessionId param) and stays there for at least one tick.

- **Pre-work B — `SettingsView.vue` single-write to ui store (Task 5 IMPORTANT #2):**
  In `apps/agent-gui/src/views/SettingsView.vue` change both `<select>` elements from `v-model="locale" @change="ui.setLocale(...)"` (and the matching `colorMode` select) to `:value="locale" @change="(e) => ui.setLocale(...)"` so that the store action is the only write path. Do NOT keep `v-model` — the destructured `locale` / `colorMode` refs from `storeToRefs(ui)` must not be mutated directly from the template. Update `SettingsView.test.ts` (or add it if not present in Task 5) so it asserts that selecting an option calls `ui.setLocale` / `ui.setTheme` exactly once with the selected value, and that the destructured refs do NOT change unless the action runs.

Both pre-work items must land in the same `feat(gui): integrate naive-ui …` commit as the rest of Task 6. Do not amend Task 5; do not introduce a separate commit.

**Files:**

- Create: `apps/agent-gui/src/styles/naive-theme.ts`
- Modify: `apps/agent-gui/src/views/WorkbenchView.vue` (Pre-work A: syncing guard)
- Modify: `apps/agent-gui/src/views/SettingsView.vue` (Pre-work B: drop v-model, single-write via action)
- Modify (or create if absent): `apps/agent-gui/src/views/SettingsView.test.ts` (Pre-work B regression test)
- Modify (or extend the existing test): `apps/agent-gui/src/views/WorkbenchView.test.ts` (Pre-work A regression test)
- Modify: `apps/agent-gui/src/layouts/AppLayout.vue` (wrap with provider stack)
- Modify: `apps/agent-gui/src/composables/useNotifications.ts` (delegate to NaiveUI's `useMessage` for toast UI; keep store as the source of truth)

- [ ] **Step 1: Verified palette inventory from current codebase**

  Verified by `grep -n "^\s*--\|background\|color:" apps/agent-gui/src/assets/main.css` and `grep -rn "var(--" apps/agent-gui/src`. Findings (these are the **only** color-bearing values currently shipped, no `:root { --xxx }` declarations exist):

  | Source                                                          | Value                                            | Purpose               |
  | --------------------------------------------------------------- | ------------------------------------------------ | --------------------- |
  | `main.css` `body`                                               | `#fff` background, `#333` text                   | global page colors    |
  | `App.vue` `var(--surface-alt, #f7f7f7)`                         | nav background fallback                          | surface-alt           |
  | `App.vue` `var(--border, #ccc)`                                 | nav button border fallback                       | divider/border        |
  | `App.vue` `var(--accent, #345)`                                 | active nav button background fallback            | brand accent          |
  | other components (`ChatPanel.vue`, `SessionsSidebar.vue`, etc.) | hard-coded `#d7d7d7` borders, `#f7f7f7` surfaces | matches above palette |

  We use these **exact** values in the NaiveUI overrides below. The remaining tokens (text-color tiers, modal background, hover/pressed states) are computed from the four base colors using NaiveUI's standard contrast ratios.

- [ ] **Step 2: Create `styles/naive-theme.ts`**

  Create `apps/agent-gui/src/styles/naive-theme.ts`:

  ```ts
  import type { GlobalThemeOverrides } from "naive-ui";

  /**
   * NaiveUI theme overrides derived from the palette currently shipped in
   * `apps/agent-gui/src/assets/main.css` + `App.vue` fallbacks:
   *   accent      = #334455 (App.vue --accent fallback "#345")
   *   border      = #cccccc (App.vue --border fallback "#ccc")
   *   surface-alt = #f7f7f7 (App.vue --surface-alt fallback)
   *   body fg/bg  = #333 / #fff (main.css)
   *
   * Hover/pressed tones are 12% lighter / 12% darker than primary, matching
   * NaiveUI's default contrast convention.
   */
  export const lightThemeOverrides: GlobalThemeOverrides = {
    common: {
      primaryColor: "#334455",
      primaryColorHover: "#4d6273",
      primaryColorPressed: "#1f2c38",
      primaryColorSuppl: "#334455",
      borderColor: "#cccccc",
      dividerColor: "#d7d7d7",
      bodyColor: "#ffffff",
      cardColor: "#ffffff",
      modalColor: "#ffffff",
      popoverColor: "#ffffff",
      tableColor: "#ffffff",
      hoverColor: "#f7f7f7",
      textColorBase: "#333333",
      textColor1: "#333333",
      textColor2: "#555555",
      textColor3: "#888888"
    },
    Card: { paddingSmall: "12px", paddingMedium: "16px" },
    Button: { borderRadiusMedium: "4px" },
    Menu: { itemHeight: "32px" }
  };

  /**
   * Dark palette mirrors the light one with inverted lightness:
   *   accent      = #6688aa (lightened brand)
   *   border/divider = #3a3f47 (matches existing dark surface tokens elsewhere)
   *   body bg     = #1a1d22 (sits below cardColor for contrast)
   *   card bg     = #22262c
   *   text        = #e6e8eb (matches WCAG AA against #1a1d22)
   */
  export const darkThemeOverrides: GlobalThemeOverrides = {
    common: {
      primaryColor: "#6688aa",
      primaryColorHover: "#809fbe",
      primaryColorPressed: "#4d6f91",
      primaryColorSuppl: "#6688aa",
      borderColor: "#3a3f47",
      dividerColor: "#3a3f47",
      bodyColor: "#1a1d22",
      cardColor: "#22262c",
      modalColor: "#22262c",
      popoverColor: "#22262c",
      tableColor: "#22262c",
      hoverColor: "#2a2f36",
      textColorBase: "#e6e8eb",
      textColor1: "#e6e8eb",
      textColor2: "#c0c4c9",
      textColor3: "#8b9098"
    },
    Card: { paddingSmall: "12px", paddingMedium: "16px" },
    Button: { borderRadiusMedium: "4px" },
    Menu: { itemHeight: "32px" }
  };
  ```

  All values above are derived from existing palette tokens — no guesses. If a future palette change happens, update both this file and the `App.vue` CSS-var fallbacks together.

  Then expose those `--app-*` variables from `<script setup>` using NaiveUI's `useThemeVars()` composable. Reasoning: NaiveUI's runtime CSS variables (`--n-color`, etc.) are component-scoped and not part of the public API; using `useThemeVars()` is the documented way to read theme tokens in custom CSS.

  Add to the `<script setup>` block of `AppLayout.vue` (next to the existing imports introduced in Step 3 below):

  ```ts
  import { useThemeVars } from "naive-ui";

  const themeVars = useThemeVars();
  ```

  And bind them via inline `:style` on the root `<div class="app-shell">`:

  ```vue
  <div
    class="app-shell"
    data-test="app-shell"
    :style="{
      '--app-body-color': themeVars.bodyColor,
      '--app-card-color': themeVars.cardColor,
      '--app-border-color': themeVars.borderColor,
      '--app-text-color': themeVars.textColor1,
      '--app-primary-color': themeVars.primaryColor
    }"
  >
  ```

  This way the scoped CSS reads stable, documented theme values that automatically swap with light/dark mode.

- [ ] **Step 3: Wrap `AppLayout.vue` with NaiveUI provider stack**

  Replace the current `apps/agent-gui/src/layouts/AppLayout.vue` with:

  ```vue
  <script setup lang="ts">
  import { computed } from "vue";
  import { storeToRefs } from "pinia";
  import { useI18n } from "vue-i18n";
  import {
    NConfigProvider,
    NLoadingBarProvider,
    NMessageProvider,
    NDialogProvider,
    NNotificationProvider,
    darkTheme,
    type GlobalTheme
  } from "naive-ui";
  import { useUiStore } from "@/stores/ui";
  import {
    lightThemeOverrides,
    darkThemeOverrides
  } from "@/styles/naive-theme";
  import StatusBar from "@/components/StatusBar.vue";
  import NotificationToast from "@/components/NotificationToast.vue";

  const { t } = useI18n();
  const ui = useUiStore();
  const { isDark } = storeToRefs(ui);

  const theme = computed<GlobalTheme | null>(() =>
    isDark.value ? darkTheme : null
  );
  const themeOverrides = computed(() =>
    isDark.value ? darkThemeOverrides : lightThemeOverrides
  );
  </script>

  <template>
    <NConfigProvider :theme="theme" :theme-overrides="themeOverrides">
      <NLoadingBarProvider>
        <NMessageProvider>
          <NDialogProvider>
            <NNotificationProvider>
              <div class="app-shell" data-test="app-shell">
                <nav class="app-nav" data-test="app-nav">
                  <RouterLink to="/workbench" data-test="nav-workbench">
                    {{ t("nav.workbench") }}
                  </RouterLink>
                  <RouterLink to="/marketplace" data-test="nav-marketplace">
                    {{ t("nav.marketplace") }}
                  </RouterLink>
                  <RouterLink to="/settings" data-test="nav-settings">
                    {{ t("nav.settings") }}
                  </RouterLink>
                </nav>
                <RouterView />
                <StatusBar />
                <NotificationToast />
              </div>
            </NNotificationProvider>
          </NDialogProvider>
        </NMessageProvider>
      </NLoadingBarProvider>
    </NConfigProvider>
  </template>

  <style scoped>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--app-body-color);
    color: var(--app-text-color);
  }
  .app-nav {
    display: flex;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--app-border-color);
    background: var(--app-card-color);
  }
  .app-nav a {
    padding: 4px 10px;
    border: 1px solid var(--app-border-color);
    text-decoration: none;
    color: inherit;
    font-size: 12px;
    border-radius: 4px;
  }
  .app-nav a.router-link-active {
    background: var(--app-primary-color);
    color: #fff;
    border-color: transparent;
  }
  </style>
  ```

  Note: this scoped CSS reads the `--app-*` variables that Step 2 (above) binds via inline `:style` from `useThemeVars()`. NaiveUI's internal `--n-*` variables are component-scoped and undocumented — never reference them directly.

- [ ] **Step 4: Sanity check — providers render without errors**

  ```bash
  pnpm --filter agent-gui run dev
  ```

  Browser at `http://localhost:1420`:
  - Page loads, no console error about "Cannot read properties of null" from NaiveUI hooks
  - Toggle theme on Settings view: nav background swaps light ↔ dark, no flash, no errors
  - Existing components (ChatPanel, SessionsSidebar, etc.) still render exactly as before — they have not been migrated yet, so they still use their own styles

  Ctrl-C the dev server.

- [ ] **Step 5: Run vitest + lint + build**

  ```bash
  pnpm --filter agent-gui run test
  pnpm run lint:eslint
  pnpm --filter agent-gui run build
  ```

  Expected: tests pass; lint clean; build succeeds. Build size grows because NaiveUI ships, but tree-shaking still discards unused components.

  **If a vitest spec breaks because it mounts `AppLayout` and now needs providers:** update those tests to use `mountWithPlugins` from `src/test-utils/mount.ts` (already created in Task 3).

- [ ] **Step 6: Commit**

  ```bash
  git add apps/agent-gui/src/styles apps/agent-gui/src/layouts/AppLayout.vue
  git commit -m "feat(gui): integrate naive-ui with provider stack and theme overrides"
  ```

---

## Task 7: Migrate all 14 + 6 SFCs to NaiveUI components (commit 7, splittable)

**Branch:** `feat/frontend-engineering`
**Commit message:** `refactor(gui): migrate components to naive-ui`
**Why seventh:** with providers in place (Task 6), we replace handcrafted UI in every SFC with NaiveUI equivalents. Per spec §5.5, this is one logical commit; if the diff exceeds **1500 LOC net** measured by `git diff --stat HEAD --shortstat` (insertions + deletions, after `pnpm format`), split into 2-3 sub-commits using the groups defined in Step 0.

**Component → NaiveUI mapping (from spec §5.5):**

| Existing component                   | NaiveUI replacement(s)                                                                                                          |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| `ChatPanel.vue`                      | `NScrollbar`, `NInput` (textarea, autosize), `NButton`, `NSpace`, `NTag`, `NAlert`, `NSkeleton`                                 |
| `SessionsSidebar.vue`                | `NScrollbar`, `NList`, `NListItem`, `NThing`, `NPopconfirm`, `NIcon`, `NButton`, `NEmpty`                                       |
| `TraceTimeline.vue`                  | `NScrollbar`, `NTimeline`, `NTimelineItem`, `NCollapse`, `NCollapseItem`, `NText`, `NEmpty`                                     |
| `TraceEntry.vue`                     | `NTag`, `NTime`, `NText`, `NEllipsis`                                                                                           |
| `TaskSteps.vue`                      | `NSteps`, `NStep`                                                                                                               |
| `TaskNode.vue`                       | `NCard`, `NTag`, `NDivider`, `NText`                                                                                            |
| `PermissionPrompt.vue`               | `NModal`, `NCard`, `NRadioGroup`, `NRadio`, `NButton`, `NSpace`, `NAlert`                                                       |
| `PermissionCenter.vue`               | `NCard`, `NList`, `NListItem`, `NSwitch`, `NTag`                                                                                |
| `MemoryBrowser.vue`                  | `NTabs`, `NTabPane`, `NDataTable`, `NInput`, `NSelect`, `NButton`, `NPopconfirm`                                                |
| `McpServerManager.vue`               | `NCard`, `NList`, `NListItem`, `NSwitch`, `NButton`, `NTag`, `NTooltip`                                                         |
| `McpStatusIndicator.vue`             | `NTag`, `NTooltip`, `NIcon`                                                                                                     |
| `StatusBar.vue`                      | `NSpace`, `NTag`, `NText`, `NTooltip`, `NIcon`                                                                                  |
| `NotificationToast.vue`              | replaced by `useMessage()` (still keeps file as a slim adapter that watches `ui.notifications` and calls `message.create(...)`) |
| `ConfirmDialog.vue`                  | replaced by `useDialog().warning({ ... })` (file deleted, callers switch to `useDialog`)                                        |
| `CatalogSourcesSettings.vue`         | `NCard`, `NList`, `NListItem`, `NInput`, `NSwitch`, `NButton`, `NSpace`, `NPopconfirm`                                          |
| `marketplace/CatalogList.vue`        | `NScrollbar`, `NEmpty`, `NSpin`                                                                                                 |
| `marketplace/CatalogCard.vue`        | `NCard`, `NTag`, `NButton`, `NSpace`, `NEllipsis`                                                                               |
| `marketplace/CatalogDetail.vue`      | `NDescriptions`, `NDescriptionsItem`, `NTabs`, `NTabPane`, `NCode`, `NButton`                                                   |
| `marketplace/InstalledList.vue`      | `NList`, `NListItem`, `NTag`, `NButton`, `NPopconfirm`                                                                          |
| `marketplace/InstallProgress.vue`    | `NProgress`, `NSpin`, `NAlert`, `NText`                                                                                         |
| `marketplace/RuntimeMissingHint.vue` | `NAlert`, `NButton`, `NCode`                                                                                                    |

**MANDATORY pre-work (carried over from Task 6 code-quality review — collect them BEFORE the SFC migration starts; same logical commit / split group):**

- **Pre-work A — `useNotifications` ↔ `NotificationToast` semantic boundary (Task 6 IMPORTANT-1):**
  Today `notify()` calls both `ui.pushNotification(level, content)` (which `NotificationToast.vue` renders as a custom slide-in toast) AND `message.<level>(content)` (NaiveUI top toast). Once any caller hooks `useNotifications`, the same event will appear twice. Resolve before wiring the first caller in this Task 7 SFC migration. Recommended approach: keep `ui.pushNotification` + `useNotifications.notify` as the single, intentional "user-facing transient + persistent log" path, and **migrate `NotificationToast.vue` per the spec §5.5 mapping** ("replaced by `useMessage()` (still keeps file as a slim adapter that watches `ui.notifications` and calls `message.create(...)`)") so that the store remains the source of truth and the visual layer is owned 100% by NaiveUI's `useMessage` — eliminating the dual render. Add a JSDoc note on `useNotifications.notify()` explicitly stating "the NaiveUI `<NotificationToast />` adapter consumes the same store entries; do NOT also call `message.*` directly elsewhere".
- **Pre-work B — `useNotifications` runtime guard (Task 6 IMPORTANT-2):**
  `useMessage()` throws if called outside an `<NMessageProvider>` subtree. Add a try/catch around the `useMessage()` call inside `useNotifications`, and on failure (a) `console.error` a clear "useNotifications must be called inside the AppLayout subtree" message, then (b) return a `notify()` that only writes to `ui.pushNotification` (never throws). This prevents a misuse from a store / router-guard / top-level service module from crashing the host component. Update / add `useNotifications.test.ts` with one new case mocking `useMessage` to throw and asserting `notify()` still pushes to the store and does not propagate the error.
- **Pre-work C — `Session not found` i18n (Task 6 NIT-5):**
  In `apps/agent-gui/src/views/WorkbenchView.vue` replace the hardcoded `` `Session not found: ${id}` `` with `t("workbench.sessionNotFound", { id })` (use `useI18n` already available in the file or add the import). Add the new key to **every** locale file under `apps/agent-gui/src/locales/` (currently `en.json` and `zh-CN.json`). Keep the `console.error('[WorkbenchView] switchSession failed:', err)` in English (developer-facing). The existing `WorkbenchView.test.ts` regression assertion that uses substring `badId` keeps working because `{ id }` interpolation still includes the id; verify by running the test.

These three items must land in the same `refactor(gui): migrate components to naive-ui` commit (or, if Task 7 is split per the LOC-threshold rule below, in the **first** sub-commit so later sub-commits can rely on the cleaned `useNotifications` contract). Do not amend Task 6.

**Carry-over from Task 5 review (collect opportunistically during the SFC migration — no extra commits, just take care of them when the file is open):**

- **Task 5 IMPORTANT #4 — `SessionsSidebar.test.ts` `setTimeout(0)` brittleness:** when migrating `SessionsSidebar.vue` to NaiveUI components, replace the `await new Promise((r) => setTimeout(r, 0))` flush in the existing test with `await flushPromises()` from `@vue/test-utils` plus `await router.isReady()` so the assertion is robust to future `vi.useFakeTimers()` setup.
- **Task 5 NIT #10 — `SessionsSidebar.createSession` side-effect surface:** when touching `SessionsSidebar.vue`, move the `currentProfile = result.profile` mutation, `resetProjection`, and global `clearTrace()` call into the `session.createSession()` action so the view layer only does `router.push`. This keeps the NaiveUI migration diff focused on UI markup rather than mixed UI + business logic.
- **Task 5 NIT #8 — `SessionsSidebar.vue` native `<dialog open>`:** the spec §5.5 mapping does NOT list `NModal`/`NDrawer` for `SessionsSidebar.vue`, so this is **out of scope for Task 7** — leave the `<dialog>` element as-is. Track it as a separate follow-up if dialog UX matters; do not add it here.
- **Task 5 NIT #9 — `MarketplaceView.vue` hardcoded English strings:** when this file appears in the Task 7 SFC migration, also replace the hardcoded `"Marketplace"` / `"Browse"` / `"Installed"` / `"Catalog source settings"` etc. with `t("...")` calls and add the keys to all locale files. Same i18n discipline as Pre-work C.

**Files (every consumer of the above):**

- 14 components in `src/components/`
- 6 components in `src/components/marketplace/`
- All `*.test.ts` adjacent to migrated components
- 10 e2e specs in `apps/agent-gui/e2e/` — selector updates handled in **Task 8**, NOT here
- `apps/agent-gui/e2e/tauri-mock.js` — handled in Task 8

- [ ] **Step 0: Decide split — measure expected diff size**

  Run a dry estimate by counting current LOC:

  ```bash
  wc -l \
    apps/agent-gui/src/components/ChatPanel.vue \
    apps/agent-gui/src/components/SessionsSidebar.vue \
    apps/agent-gui/src/components/TraceTimeline.vue \
    apps/agent-gui/src/components/TraceEntry.vue \
    apps/agent-gui/src/components/TaskSteps.vue \
    apps/agent-gui/src/components/TaskNode.vue \
    apps/agent-gui/src/components/PermissionPrompt.vue \
    apps/agent-gui/src/components/PermissionCenter.vue \
    apps/agent-gui/src/components/MemoryBrowser.vue \
    apps/agent-gui/src/components/McpServerManager.vue \
    apps/agent-gui/src/components/McpStatusIndicator.vue \
    apps/agent-gui/src/components/StatusBar.vue \
    apps/agent-gui/src/components/NotificationToast.vue \
    apps/agent-gui/src/components/ConfirmDialog.vue \
    apps/agent-gui/src/components/CatalogSourcesSettings.vue \
    apps/agent-gui/src/components/marketplace/*.vue
  ```

  Add the totals (current baseline sum is **~3590 LOC** across these files; expected migrated diff is roughly 1.5× that → split is essentially mandatory).

  **Decision rule (deterministic, no judgement):** if the LOC sum from above is **> 1500**, ALWAYS split into the 3 sub-commits below. Only if the sum is **≤ 1500** (which today is not the case) treat as one commit.
  - **7a — chat & sessions:** `ChatPanel.vue`, `SessionsSidebar.vue`, `StatusBar.vue`, `NotificationToast.vue`, `ConfirmDialog.vue` (+ its test file + e2e selector update)
  - **7b — trace & tasks & permissions & memory:** `TraceTimeline.vue`, `TraceEntry.vue`, `TaskSteps.vue`, `TaskNode.vue`, `PermissionPrompt.vue`, `PermissionCenter.vue`, `MemoryBrowser.vue`
  - **7c — mcp & marketplace:** `McpServerManager.vue`, `McpStatusIndicator.vue`, `CatalogSourcesSettings.vue`, `marketplace/*.vue`

  Each sub-commit follows the same per-component recipe (Steps 1-5 below) and ends with vitest + lint + build green. Use commit messages `refactor(gui): migrate chat & sessions to naive-ui`, `refactor(gui): migrate trace/tasks/permissions/memory to naive-ui`, `refactor(gui): migrate mcp & marketplace to naive-ui`.

  **Status of sub-commits:**
  - **7a — DONE** (commit `65bb7b0`, double-reviewed). Pre-work A/B/C all landed (NotificationToast → useMessage adapter; useNotifications wraps useMessage in try/catch with a fallback notify; WorkbenchView's "Session not found" goes through `t('workbench.sessionNotFound', { id })` with en/zh-CN keys). Task 5 carry-overs (#4 flushPromises + router.isReady; #10 createSession side-effect ownership in store) also landed. ConfirmDialog.vue / ConfirmDialog.test.ts kept on disk because MemoryBrowser still consumes them — they get removed in 7b along with MemoryBrowser's migration.
  - **7b — DONE** (commit `87072e1`, double-reviewed; one IMPORTANT from quality review fixed in-place via `git commit --amend`). Migrated TraceTimeline / TraceEntry / TaskSteps / TaskNode / PermissionPrompt / PermissionCenter / MemoryBrowser to NaiveUI. Deleted `ConfirmDialog.vue` + `ConfirmDialog.test.ts` (MemoryBrowser was the last consumer; it now uses `useDialog().warning()`). All 7 of the 7a → 7b carry-overs (NotificationToast Set GC, useNotifications degraded log, dead i18n key removal, SessionsSidebar `<ul>` comment, ChatPanel/SessionsSidebar tests using `mountWithPlugins({ withNaiveProviders: true })`, i18n of remaining hardcoded strings, SessionsSidebar rename ref function form) bundled into the same commit. Spec §5.5 deviations on TraceTimeline (NButton tab strip vs NTabs — heterogeneous panels + NTabPane teleport semantics), TaskSteps (recursive TaskNode tree vs NSteps linear), TraceEntry (no `NTime` because data only has `durationMs`), PermissionPrompt (inline prompt, not modal — no `NModal`), and MemoryBrowser (no `NTabs/NDataTable` because text-heavy memory list is better served by `<ul>` + NTag/NText) have inline code comments justifying each engineering choice; all primary spec primitives (NScrollbar / NEmpty / NCard / NTag / NDivider / NText / NSpace / NButton / NSelect / NInput / NSpin / NList / NListItem / NPopconfirm-via-useDialog / NAlert) are in use.
  - **7c — IN PROGRESS** (final sub-commit of Task 7).

  **Opportunistic carry-overs from 7b code-quality review (collect during 7c — same logical commit, no extra commit):**
  1. **[IMPORTANT] `PermissionPrompt.vue` "Trust this server" checkbox is a native `<input type="checkbox">`** (lines ~131-134), kept that way during 7b to preserve the `.mcp-trust-check input[type='checkbox']` test selector. The native input does not follow NaiveUI's dark-theme palette, producing a visual seam against the surrounding NTag / NButton / NCard. Migrate to `NCheckbox`, attach `[data-test="trust-server-checkbox"]` to the `NCheckbox`, and rewrite the test selector to use `wrapper.findComponent({ name: "NCheckbox" }).vm.$emit("update:checked", true)` (or query the `[data-test=...]` attribute directly). Production UI must match the rest of the prompt; the test must not reach into NaiveUI's internal DOM.

  2. **[IMPORTANT] `NotificationToast.vue` dispatched-Set GC has an implicit contract on `useUiStore.dismissNotification`'s immutability.** The watcher iterates `for (const n of items)` while synchronously calling `ui.dismissNotification(n.id)`. Today the 5-push test passes, which implies `dismissNotification` reassigns `notifications.value = notifications.value.filter(...)` (new reference, the watcher's `items` parameter is decoupled). If a future maintainer "optimises" `dismissNotification` to `notifications.value.splice(idx, 1)` (in-place mutation), the `for...of` iterator skips elements and notifications get silently dropped. Add a `// MUST return a new array (filter), not in-place mutation. NotificationToast.vue's watcher iterates the same reference.` comment on `useUiStore.dismissNotification`, and add a one-line regression test that asserts `notifications.value` is a new reference after dismiss.

  3. **[NIT] `sessions.newButton` i18n key bakes the visual prefix `+ ` into the translation** (`"+ New"` / `"+ 新建"`). Translations should carry text only; the `+` glyph belongs in the template (`+ {{ t('sessions.newButton') }}`) or as an NButton icon slot. Also update zh-CN to `"新建"` (no Chinese-mixed `+` punctuation).

  4. **[NIT] `StatusBar.vue` body text is still hardcoded English** despite the 7a/7b commit message claiming "StatusBar hardcoded English strings are now t(...) calls". Only the NTooltip content (`status.activeProfile`) was actually translated. Add and use `status.streaming` / `status.connected` / `status.profileLabel` / `status.sessionsLabel` / `status.modeLabel` / `status.yes` / `status.no` (or compose `streaming: yes/no` etc. via parameterised keys) in both en/zh-CN.

  5. **[NIT] `PermissionPrompt.vue` business terms** — `Permission Required` / `Memory Proposed` / `Allow` / `Deny` / `Accept` / `Reject` / `Trust this server` (~lines 55-65, 131) — are hardcoded English. Add `permissions.requiredTitle` / `permissions.memoryProposedTitle` / `permissions.allow` / `permissions.deny` / `permissions.accept` / `permissions.reject` / `permissions.trustServer` to en/zh-CN.

  6. **[NIT] `TraceTimeline.vue` tab labels (`Trace` / `Tasks` / `Memory`) and the empty-state strings (`"No trace events yet"` / `"No tasks yet"` / PermissionCenter's `"Permissions"` / `"No pending requests"`)** are hardcoded English. Add `trace.tabTrace` / `trace.tabTasks` / `trace.tabMemory` / `trace.empty` / `tasks.empty` / `permissions.title` / `permissions.empty` etc.

  7. **[NIT — from 7b I-1 fix re-review]** `MemoryBrowser.test.ts` comment near the `findComponent({ name: "Select" })` call says `"the SFC renders multiple internal `Select` subcomponents"`, which can be misread as "MemoryBrowser renders multiple `<NSelect>` itself". Reword to `"NaiveUI's NSelect implementation expands into multiple internal Select-named subcomponents (NSelectMenu etc.)"` so the next reader does not look for non-existent extra NSelects in the SFC.

  8. **[NIT — from 7b I-1 fix re-review, optional]** The new NSelect-driven test asserts `expect(memory.filter).toBe(scope)` only — that is a state-level check that walks `handleFilterChange → setMemoryFilter → filter` end-to-end. If you want to additionally pin the contract that `setMemoryFilter` is the path used (so that future renames of the store action surface as failures), add `expect(setMemoryFilterSpy).toHaveBeenCalledWith(scope)` alongside the existing assertion. Skip if you prefer the looser end-to-end coupling.

  Note on 7c carry-over scope: items 1-2 (IMPORTANT) MUST land inside the 7c commit. Items 3-8 (NIT) may be deferred to Task 9 (clean-up sweep) if the 7c diff already exceeds the LOC split threshold; in that case, list each deferred NIT explicitly in the 7c commit message body.

  **Opportunistic carry-overs from 7a code-quality review (collect during 7b — same logical commit, no extra commit):**
  1. **[IMPORTANT] `NotificationToast.vue` `dispatched` Set is monotonically growing.** The Set holds every `notification.id` ever forwarded but never removes them. `ui.pushNotification` mints fresh `crypto.randomUUID()` ids, so the Set strictly grows for the entire app lifetime. Quantitatively mild today (~80 bytes per entry), but it is a real long-running-session leak in `<NotificationToast>` which lives inside `AppLayout` for the whole app life. Pick the cleanest of these three approaches and explain the choice in a JSDoc on the Set:
     - (a) `dispatched.delete(id)` immediately after `message.<level>(...)` returns. Verify the watcher does not re-enter and re-dispatch (`dismissNotification` shrinks the array, watcher re-runs, the now-deleted id is no longer in `items`, so `dispatched.has(id)` short-circuit still applies — but the new entry on next push is truly new id, so the recheck path is fine).
     - (b) Each watcher tick GC against the current `items` array: `for (const id of dispatched) if (!items.find(n => n.id === id)) dispatched.delete(id)`. Add a hard cap (e.g. 500) as a belt-and-suspenders guard.
     - (c) Replace the Set entirely with a `lastDispatchedIndex` cursor over the (append-only) `notifications` array. Process `items.slice(lastDispatchedIndex)` per tick. Requires the store to be append-only. Most invasive but cleanest semantically.

     Recommended default: **(a) immediate delete** plus a 5-line JSDoc on the Set explaining "ids never repeat (UUID), so without delete this would grow unbounded — we delete after dispatch instead of keeping a permanent history".

  2. **[IMPORTANT] `useNotifications.ts` error-level fallback is silent after the first crash.** When `useMessage()` throws on construction, the catch path `console.error`s once, then every subsequent `notify("error", ...)` only writes to the store (no further console output). Keep the no-throw design (good — protects callers), but capture the degraded state (`let providerAvailable = true; ... catch { providerAvailable = false }`) and, when `providerAvailable === false` and `level === "error"`, also `console.error("[useNotifications] (degraded) error:", content)` so each individual error still leaves a console trace. One-line guard, no API change.

  3. **[NIT] Delete dead i18n key `notifications.sessionNotFound`** in both `apps/agent-gui/src/locales/en.json` (line ~44) and `zh-CN.json` (same line). The live key is `workbench.sessionNotFound`; the `notifications.*` one was left over from earlier iterations and has zero live references in the repo (verified by `git grep`). Trivial 1-line × 2-file delete.

  4. **[NIT] Add a single-line comment on `SessionsSidebar.vue`'s `<ul class="session-list">` block** explaining why it stayed hand-rolled instead of migrating to `NList/NListItem/NThing`: the `.session-actions` element is hover-only (`.session-item:hover .session-actions { display: flex }`) and `NListItem`'s `#suffix` slot cannot be conditionally shown based on the parent `<li>:hover` state. This avoids future contributors thinking it was overlooked.

  5. **[NIT] Migrate `ChatPanel.test.ts` and `SessionsSidebar.test.ts` to use `mountWithPlugins(comp, { withNaiveProviders: true, initialRoute: '/workbench' })`.** The helper that 7a added to `apps/agent-gui/src/test-utils/mount.ts` is currently NOT used by the very tests that need NaiveUI providers; both files hand-roll a 5-layer provider tree. Switching saves ~25 lines per file and demonstrates the helper's intended use site for future tests.

  6. **[NIT] i18n the remaining hardcoded English strings** in:
     - `SessionsSidebar.vue`: `+ New`, `Create`, `Cancel`, `Rename` (button title), `Delete` (button title), `No sessions yet`, `New Session` (modal title)
     - `ChatPanel.vue`: `Failed to send message: ${e}`, `Cancel failed: ${e}`, the `roleDisplay` map literals (`You`/`Agent`), and the streaming `<span class="message-role">Agent</span>` literal
     - `StatusBar.vue`: tooltip strings such as `Active profile`

     Add the keys (`sessions.newButton` / `sessions.createButton` / `sessions.cancelButton` / `sessions.renameTitle` / `sessions.deleteTitle` / `sessions.emptyHint` / `sessions.newDialogTitle` / `chat.sendFailed` / `chat.cancelFailed` / `chat.roleYou` / `chat.roleAgent` / `status.activeProfile` etc.) to BOTH `en.json` and `zh-CN.json`.

  7. **[NIT — fix during the SessionsSidebar work in 7b]** `SessionsSidebar.vue` rename `<input ref="renameInput">` is inside a `v-for`, so Vue 3 binds it as an array, not a single ref. Today this happens to work because `editingSessionId === item.id` ensures only one `<input>` is rendered, but it is a latent foot-gun. If you touch the file in 7b, switch to `:ref="(el) => { if (editingSessionId === item.id) renameInput.value = el as HTMLInputElement }"`. (If 7b does not touch SessionsSidebar.vue, skip this and roll it into the dialog/dropdown follow-up tracked under Task 5 NIT #8.)

  **Defer to Task 8 (E2E):**
  - Verify `NButton title` attribute → tooltip survives the migration in `SessionsSidebar` action buttons.
  - Verify `NInput keydown` handler still receives Enter / Shift+Enter in `ChatPanel`.
  - Migration of the native `<dialog>` + custom profile dropdown in `SessionsSidebar` (Task 5 NIT #8 explicit defer; spec §5.5 does NOT list NModal/NDrawer for this file).

  **Pure type cleanup (Task 8 / 9 sweep, not 7b/7c):**
  - The inline indexed-access type `(typeof session.projection.messages)[0]` appears in both `ChatPanel.vue` (~line 41) and `stores/session.ts` (~line 71). Extract a `ProjectedMessage` type in `apps/agent-gui/src/types/` and import it in both places.

- [ ] **Step 1: Replace `ConfirmDialog.vue` with `useDialog()` callsites first**

  Why first: it's the smallest scope but the cleanest validation of the NaiveUI service-hook pattern before tackling larger components.

  Verified inventory (from `grep -rn "ConfirmDialog\|dialog-box" apps/agent-gui`):
  - **2 SFC consumers**: `apps/agent-gui/src/components/MemoryBrowser.vue` (lines 10, 129) and `apps/agent-gui/src/components/SessionsSidebar.vue` (lines 14, 274).
  - **1 test file** to delete: `apps/agent-gui/src/components/ConfirmDialog.test.ts` (5 cases).
  - **1 e2e selector** to update: `apps/agent-gui/e2e/session-lifecycle.spec.ts` lines 115-117 use `.dialog-box` and `.dialog-box >> button`. NaiveUI dialogs render under `.n-dialog` with action buttons inside `.n-dialog__action`.
  - Add the new i18n key `common.deleteConfirm: 'Delete "{name}"?'` to both `apps/agent-gui/src/locales/en.json` and `zh-CN.json` (the en JSON already has `common.delete`/`common.yes`/`common.no` from Task 2 Step 5; this only adds the `deleteConfirm` parameterised entry).

  Substeps:

  1.1 Add the new locale entry (en):

  ```json
  // apps/agent-gui/src/locales/en.json — under "common"
  "deleteConfirm": "Delete \"{name}\"?"
  ```

  zh-CN:

  ```json
  // apps/agent-gui/src/locales/zh-CN.json — under "common"
  "deleteConfirm": "删除「{name}」？"
  ```

  1.2 In **both** consumer SFCs (`MemoryBrowser.vue` and `SessionsSidebar.vue`), replace the `<ConfirmDialog ... />` template block with no template output (the dialog is portal-rendered by NaiveUI). Add to `<script setup>`:

  ```ts
  import { useDialog } from "naive-ui";
  import { useI18n } from "vue-i18n";

  const dialog = useDialog();
  const { t } = useI18n();

  function confirmDelete(name: string, onYes: () => void | Promise<void>) {
    dialog.warning({
      title: t("common.confirm"),
      content: t("common.deleteConfirm", { name }),
      positiveText: t("common.yes"),
      negativeText: t("common.no"),
      onPositiveClick: () => {
        void onYes();
      }
    });
  }
  ```

  Replace the previous `confirmDialogVisible.value = true` (or equivalent) trigger with a direct `confirmDelete(itemName, () => deleteItem(id))` call at the existing delete-button click handler. Delete the now-unused `ref` and `<ConfirmDialog>` markup.

  1.3 Delete the SFC and its test:

  ```bash
  git rm apps/agent-gui/src/components/ConfirmDialog.vue \
         apps/agent-gui/src/components/ConfirmDialog.test.ts
  ```

  1.4 Update the e2e spec `apps/agent-gui/e2e/session-lifecycle.spec.ts` lines 115-117. Old:

  ```ts
  // ConfirmDialog should appear (uses .dialog-box class)
  await expect(page.locator(".dialog-box")).toBeVisible();
  await page.locator(".dialog-box >> button", { hasText: "Delete" }).click();
  ```

  New (NaiveUI `useDialog` renders under `.n-dialog`, positive button is `.n-dialog .n-button--primary-type`):

  ```ts
  // NaiveUI dialog (replaces the old ConfirmDialog component)
  const naiveDialog = page.locator(".n-dialog");
  await expect(naiveDialog).toBeVisible();
  await naiveDialog
    .locator(".n-dialog__action button.n-button--primary-type")
    .click();
  ```

  1.5 Verify nothing else references the deleted file or `.dialog-box` class:

  ```bash
  grep -rln "ConfirmDialog\|dialog-box" apps/agent-gui/src apps/agent-gui/e2e
  ```

  Expected: empty output.

  1.6 Run vitest + the affected e2e spec:

  ```bash
  pnpm --filter agent-gui exec vitest run
  pnpm --filter agent-gui exec playwright test e2e/session-lifecycle.spec.ts --reporter=line
  ```

  Expected: vitest count drops by 5 (deleted ConfirmDialog test cases); session-lifecycle e2e passes.

- [ ] **Step 2: Migrate `NotificationToast.vue` to use `useMessage()` adapter**

  Replace `apps/agent-gui/src/components/NotificationToast.vue` with:

  ```vue
  <script setup lang="ts">
  import { watch } from "vue";
  import { storeToRefs } from "pinia";
  import { useMessage } from "naive-ui";
  import { useUiStore } from "@/stores/ui";

  const ui = useUiStore();
  const message = useMessage();
  const { notifications } = storeToRefs(ui);

  // Each newly added notification is forwarded to NaiveUI's <NMessageProvider>
  // and immediately dismissed from the store; NaiveUI owns the visual lifecycle.
  watch(
    notifications,
    (items) => {
      for (const n of items) {
        switch (n.level) {
          case "info":
            message.info(n.message);
            break;
          case "success":
            message.success(n.message);
            break;
          case "warning":
            message.warning(n.message);
            break;
          case "error":
            message.error(n.message, { duration: 8000 });
            break;
        }
        ui.dismissNotification(n.id);
      }
    },
    { deep: true }
  );
  </script>

  <template>
    <!-- Visual rendering is handled by NMessageProvider; this component is logic-only. -->
  </template>
  ```

  Update `NotificationToast.vue`'s test (if it exists) to assert the watcher dispatches and store empties:

  ```ts
  import { mountWithPlugins } from "@/test-utils/mount";
  import NotificationToast from "@/components/NotificationToast.vue";
  import { useUiStore } from "@/stores/ui";

  it("forwards notifications to NMessageProvider and clears the store", async () => {
    const wrapper = mountWithPlugins(NotificationToast);
    const ui = useUiStore();
    ui.pushNotification("info", "hello");
    await wrapper.vm.$nextTick();
    expect(ui.notifications).toEqual([]);
  });
  ```

  Run:

  ```bash
  pnpm --filter agent-gui exec vitest run src/components/NotificationToast.test.ts 2>/dev/null || true
  ```

- [ ] **Step 3: Per-component migration recipe (apply uniformly to remaining 18 SFCs)**

  For every component listed in the mapping table, follow this 6-step inner recipe. **Do one component at a time. Run vitest for that component's `*.test.ts` after each migration before moving to the next.**
  1. **Read** the current file in full with `read_file`.
  2. **Identify** every native HTML element + manual class that maps to a NaiveUI component per the table above.
  3. **Replace** template fragments. Examples:
     - `<button class="primary" @click="onSend">{{ t("common.send") }}</button>` → `<NButton type="primary" @click="onSend">{{ t("common.send") }}</NButton>`
     - `<textarea v-model="input" />` → `<NInput v-model:value="input" type="textarea" :autosize="{ minRows: 1, maxRows: 6 }" />` (note `:value` not `:model-value`)
     - `<div class="scroll-pane">…</div>` → `<NScrollbar style="height: 100%">…</NScrollbar>`
     - `<ul class="session-list">…</ul>` + `<li>` → `<NList>` + `<NListItem>`
     - hand-rolled modal divs → `<NModal v-model:show="visible" preset="card" :title="t('...')">`
     - hand-rolled tabs → `<NTabs v-model:value="activeTab">` + `<NTabPane name="...">…</NTabPane>`
  4. **Adjust styles**: replace bespoke CSS with NaiveUI props where possible (e.g. `:bordered`, `size="small"`, `type="primary"`). Keep `<style scoped>` only for layout classes that NaiveUI does not cover (grid templates, flex containers).
  5. **Replace hand-coded text** with `t("common.<key>")` where a key already exists in `locales/en.json`. If a string is component-specific and not in `common.*` / `nav.*` / `settings.*` / `notifications.*` / `status.*`, leave it as-is (per Q5: only common-copy is i18n'd).
  6. **Run that component's test**:

     ```bash
     pnpm --filter agent-gui exec vitest run src/components/<Name>.test.ts
     ```

     If it fails because it queries by `button.primary` selector and we now render `<NButton>`, update the test to query by `data-test="..."` (add the attribute to the SFC) or by component (`wrapper.findComponent(NButton)`).

  **Order of execution (smallest first to validate the recipe):**
  1. `McpStatusIndicator.vue` (42 lines, simplest)
  2. `StatusBar.vue` (71 lines)
  3. `TaskSteps.vue` (62 lines)
  4. `marketplace/RuntimeMissingHint.vue`
  5. `marketplace/InstallProgress.vue`
  6. `marketplace/CatalogCard.vue`
  7. `marketplace/InstalledList.vue`
  8. `marketplace/CatalogList.vue`
  9. `marketplace/CatalogDetail.vue`
  10. `TraceEntry.vue` (162 lines)
  11. `PermissionCenter.vue` (49 lines)
  12. `McpServerManager.vue` (214 lines)
  13. `PermissionPrompt.vue` (222 lines)
  14. `CatalogSourcesSettings.vue` (254 lines)
  15. `MemoryBrowser.vue` (277 lines)
  16. `TaskNode.vue` (286 lines)
  17. `TraceTimeline.vue` (118 lines, but complex render)
  18. `ChatPanel.vue` (323 lines, biggest interaction surface)
  19. `SessionsSidebar.vue` (498 lines, biggest)

- [ ] **Step 4: After all components migrated, run full vitest**

  ```bash
  pnpm --filter agent-gui run test
  ```

  Expected: all 23+ specs pass. Common failures + fixes:
  - **"`useMessage` returned `null`"** — the test mounts a component that calls `useMessage()` without an `<NMessageProvider>` ancestor. Wrap the test mount with a tiny provider helper, or use `useMessage` only in components that are always rendered inside `AppLayout`.
  - **"Cannot read properties of undefined (reading 'value')"** — `storeToRefs` was applied to a non-state property; restructure imports so only `ref`/`computed` go through `storeToRefs` and methods stay on the store proxy.
  - **selector misses (`wrapper.find("button.primary")` returns nothing)** — switch to `wrapper.find('[data-test="send-button"]')` and add the attribute to the SFC.

- [ ] **Step 5: Lint + build + dev smoke**

  ```bash
  pnpm run lint:eslint
  pnpm --filter agent-gui run build
  ```

  Expected: lint clean (NaiveUI components are auto-imported only after Task 9; until then keep explicit `import { NButton, NInput, ... } from "naive-ui"` at the top of each SFC), build succeeds.

  Then:

  ```bash
  pnpm --filter agent-gui run dev
  ```

  Browser smoke at `http://localhost:1420`:
  - Workbench: send a message → ChatPanel uses `NInput` + `NButton`; click Cancel/Retry on a message
  - SessionsSidebar: click a session → URL updates, ChatPanel renders that session
  - PermissionCenter: toggle a permission switch → state persists across reload
  - PermissionPrompt: simulate a prompt event via tauri-mock (or wait for real Tauri integration in dev with `pnpm tauri dev`) → modal appears with NaiveUI styling
  - MemoryBrowser: tabs switch, table renders
  - Marketplace: catalog list renders, click an entry → detail tabs work
  - Settings: toggle theme → all NaiveUI components swap dark/light without flicker
  - Trigger an error → NaiveUI message toast appears top-center, auto-dismisses

  Ctrl-C the dev server.

- [ ] **Step 6: Commit (single or split per Step 0 decision)**

  Single-commit case:

  ```bash
  git add apps/agent-gui/src/components apps/agent-gui/src/views
  git rm apps/agent-gui/src/components/ConfirmDialog.vue 2>/dev/null || true
  git commit -m "refactor(gui): migrate components to naive-ui"
  ```

  Split case (3 commits): stage and commit per group from Step 0. Each commit must independently pass `pnpm test && pnpm lint:eslint && pnpm build`.

---

## Task 8: Update Playwright E2E selectors + tauri-mock.js (commit 8)

**Branch:** `feat/frontend-engineering`
**Commit message:** `test(gui): update playwright e2e selectors and tauri-mock for new layout`
**Why eighth:** after Task 5 (router) + Task 7 (NaiveUI), e2e specs may break because (a) URL paths now use hash routes, (b) old selectors (`button.primary`, `.session-row`) reference vanished classes, (c) `useTauriEvents` listeners changed lifecycle. We update specs + mock together to keep `just test-e2e` green.

> **7c carry-over (must be addressed in this commit, before or alongside the e2e selector sweep):**
>
> 1. **[7c IMPORTANT-2 — `CatalogList.vue` hidden `<select data-test="catalog-trust">` is dead code]** The 7c review confirmed no current Vitest spec references `catalog-trust`; verify whether any Playwright e2e fixture uses it. If yes, migrate the fixture to drive `NSelect` via `findComponent({ name: "Select" })` or NSelect's `select-props` and then **delete** the hidden `<select>` from `CatalogList.vue`. If no, just delete the hidden `<select>` (and its `v-model="catalog.filters.trustMin"` binding) — it currently double-binds the same store filter and creates an a11y/maintenance hazard. Rationale: it was kept in 7c only as a defensive bridge; this is the right commit to close it because the e2e sweep is the only place it could matter.
> 2. **[7c NIT-1 — `<InstallProgress>` lifetime coupled to `<NDrawer>`]** In `apps/agent-gui/src/components/marketplace/CatalogDetail.vue`, `<InstallProgress>` is currently nested inside the `<NDrawer>` subtree, so closing the drawer mid-install unmounts the install-progress modal. Move `<InstallProgress>` up one level — render it from `MarketplaceView.vue` (or `CatalogList.vue`) so it survives drawer close. Update any spec/e2e that asserted on its DOM location at the same time.
> 3. **[7c NIT-2 — `InstallProgress` NModal `title` does not track outcome]** In `apps/agent-gui/src/components/marketplace/InstallProgress.vue`, replace the static `title="Installing…"` with a `computed` driven by `outcome.kind` (e.g. `"Installing…"` / `"Install complete."` / `"Install failed"`). This is a 5-line change but materially fixes the visual mismatch the 7c review flagged. If the e2e specs assert on the title text, update those assertions in the same commit.
>
> All three items are non-blocking for cb8944f's merge but must land before this commit (Task 8) is closed; they are scoped to the e2e/selector surface area Task 8 already touches, so bundling them avoids a separate commit just for ~10 LOC of cleanup.

**Files:**

- Modify: every spec under `apps/agent-gui/e2e/*.spec.ts` (10 files)
- Modify: `apps/agent-gui/e2e/tauri-mock.js` — verified: this branch introduces **no new `invoke()` command names** (the new `switchSession` store action calls the existing `switch_session` Tauri command, already handled by the mock). This file is therefore a **no-op for Task 8**; only update it if Step 4's diff check reveals an unexpected new command name.
- Confirm: `apps/agent-gui/playwright.config.ts` is already configured with `testIdAttribute: "data-test"` (it is — see Pre-flight reading).

**Selector strategy:** every interactive element added/changed in Task 5-7 must carry a `data-test` attribute. Migrated SFCs in Task 7 should already have these (per Step 3 fix in Task 7); this task only repairs spec files.

- [ ] **Step 1: Inventory current selectors and route assumptions in specs**

  ```bash
  cd /Users/chanyu/AIProjects/kairox
  grep -n "page.goto\|page.locator\|getByTestId\|page.click" apps/agent-gui/e2e/*.spec.ts | head -100
  ```

  Save the output to a scratch file:

  ```bash
  grep -n "page.goto\|locator\|getByTestId\|click\|page.url" apps/agent-gui/e2e/*.spec.ts > /tmp/kairox-e2e-selectors.txt
  ```

  Use this list to drive the per-spec changes below.

- [ ] **Step 2: Run the spec suite once to capture all failures up-front**

  ```bash
  pnpm --filter agent-gui exec playwright install chromium 2>/dev/null
  pnpm --filter agent-gui run test:e2e 2>&1 | tee /tmp/kairox-e2e-run-1.log | tail -60
  ```

  Expected after Tasks 1-7: many specs will fail because (a) Task 5 changed routing to hash mode (`/workbench` → `/#/workbench`) and (b) Task 7 replaced hand-rolled markup with NaiveUI components (`button.primary` → `<NButton>`, `.session-row` → `<NListItem>`, `.dialog-box` → `.n-dialog`, `.modal-overlay` → `.n-modal`). Open `/tmp/kairox-e2e-run-1.log` and categorise every failing assertion using the table below before editing any spec.

  Common failure categories:
  - **`page.goto("/workbench")` returns 404** → switch to `page.goto("/#/workbench")` (hash mode)
  - **`page.locator("button.primary")` finds nothing** → switch to `page.getByTestId("send-button")` after adding `data-test="send-button"` to `<NButton>` in `ChatPanel.vue`
  - **`page.locator(".session-row")` finds nothing** → switch to `page.getByTestId("session-row")` and add the attribute on each `<NListItem>` in `SessionsSidebar.vue`
  - **modal selector `.modal-overlay` missing** → NaiveUI's `<NModal>` renders into a portal under `body`. Use `page.locator(".n-modal").getByRole(...)` or `page.getByTestId("permission-modal")`
  - **toast assertions** → NaiveUI messages render under `.n-message-container`. Use `page.locator(".n-message").filter({ hasText: "..." })`

- [ ] **Step 3: Update each spec file**

  Per spec, apply the failure-driven fixes from Step 2's log. **Do one spec at a time.** Re-run only that spec after each fix:

  ```bash
  pnpm --filter agent-gui exec playwright test e2e/chat-flow.spec.ts --reporter=line
  ```

  Suggested order (smallest blast radius first):
  1. `notifications.spec.ts`
  2. `trace-panel.spec.ts`
  3. `permission-memory.spec.ts`
  4. `memory-browser.spec.ts`
  5. `task-graph.spec.ts`
  6. `task-graph-interaction.spec.ts`
  7. `mcp.spec.ts`
  8. `multi-agent-flow.spec.ts`
  9. `session-lifecycle.spec.ts`
  10. `chat-flow.spec.ts`

  For each spec:
  - Replace path-mode URLs with hash-mode (`/workbench` → `/#/workbench`).
  - Replace class-based selectors with `getByTestId(...)`. If the corresponding `data-test` attribute is missing in the SFC, add it as part of this step (cross-edit the SFC).
  - For NaiveUI modals/messages/notifications, use NaiveUI's class hooks (`.n-modal`, `.n-message`, `.n-notification`) combined with `filter({ hasText })`.

- [ ] **Step 4: Update `tauri-mock.js` if any IPC contract changed**

  ```bash
  grep -n "invoke\|listen\|emit" apps/agent-gui/e2e/tauri-mock.js | head -40
  diff <(grep -oE 'invoke[^(]*\("[^"]+"' apps/agent-gui/src --include="*.ts" --include="*.vue" -rho | sort -u) \
       <(grep -oE '"[^"]+":' apps/agent-gui/e2e/tauri-mock.js | sort -u) || true
  ```

  If `switchSession` (or any other store action introduced in Task 3) calls a new `invoke("switch_session", ...)` and the mock does not yet handle that command name, add a handler in `tauri-mock.js`:

  ```js
  case "switch_session": {
    const sid = args.sessionId;
    return MOCK_PROJECTIONS[sid] ?? { messages: [], token_stream: "", task_graph: [] };
  }
  ```

  (`switch_session` already existed in current `mock.js`; verify with the diff above.)

- [ ] **Step 5: Run the full e2e suite**

  ```bash
  pnpm --filter agent-gui run test:e2e 2>&1 | tail -40
  ```

  Expected: every spec passes. Failures must be addressed in-place — do **not** disable a spec, do **not** add `test.skip()`, do **not** comment out assertions. Two valid resolutions exist:
  1. The spec's selector or URL is stale relative to Tasks 5-7 (NaiveUI markup or hash-mode routes). Update the selector/URL in the spec to match the new code.
  2. The spec encodes a behavior that this branch **intentionally** changed (e.g. the workbench is now reached via `#/workbench` instead of an in-page `view='workbench'` toggle). Update the spec's assertions to match the new contract; document the behavior delta in the commit body.

  Any spec that cannot be made green by one of these two routes is a regression in this branch — go back to the originating Task (5-7) and fix the source.

- [ ] **Step 6: Run vitest + lint + build to confirm no regression in unit tests**

  ```bash
  pnpm --filter agent-gui run test
  pnpm run lint:eslint
  pnpm --filter agent-gui run build
  ```

  Expected: all green.

- [ ] **Step 7: Commit**

  ```bash
  git add apps/agent-gui/e2e apps/agent-gui/src/components apps/agent-gui/src/views \
          apps/agent-gui/src/layouts
  git commit -m "test(gui): update playwright e2e selectors and tauri-mock for new layout"
  ```

  Note: SFCs may show in the diff because `data-test` attributes were added during Step 3. That is expected and intended.

---

## Task 9: Add unplugin-auto-import + unplugin-vue-components (commit 9)

**Branch:** `feat/frontend-engineering`
**Commit message:** `feat(gui): add unplugin-auto-import and unplugin-vue-components`
**Why ninth (not earlier):** auto-imports change the surface area of every file. Doing it last keeps earlier commits' diffs explicit (every import visible), and lets us delete redundant `import` statements as a single batched cleanup.

> **Optional follow-up from Task 2 code-quality review (non-blocking, do at your discretion while editing `vite.config.ts`):**
>
> - In `apps/agent-gui/vite.config.ts`, change `server: { port: 1420, host: "0.0.0.0" }` to `server: { port: 1420, host: "127.0.0.1", strictPort: true }`. Tauri only loads from localhost; binding to `0.0.0.0` exposes the dev server (and HMR socket) to the LAN, which is unintended for a desktop-only project. Anyone needing LAN debugging can override with `vite --host` on the CLI. `strictPort: true` makes port collisions fail loudly instead of silently shifting, which matches Tauri's expectation of a fixed port.

> **7c carry-over (clean-up sweep — fold into this Task 9 commit, the natural home for cross-cutting test infra + i18n cleanup):**
>
> 1. **[7c IMPORTANT-1 — `mountWithPlugins` lacks NaiveUI provider option]** `apps/agent-gui/src/components/marketplace/Marketplace.test.ts` mounts `MarketplaceView` (which renders `NTabs`/`NCard`/`NButton`) without an `NConfigProvider` ancestor. Today this passes by NaiveUI's silent fallback to default theme, but a future `useMessage()` call on this view will crash the spec, and the topology is asymmetric vs other view-level specs. Add a `wrapInNConfigProvider?: boolean` (or rename existing `withNaiveProviders` to consistently include `NConfigProvider`) option to `apps/agent-gui/src/test-utils/mount.ts`'s `mountWithPlugins`, default it to `true` for any spec mounting a view-level component, and update `Marketplace.test.ts` (and any other spec the audit reveals) to use it.
> 2. **[7c IMPORTANT-3 — `CatalogSourcesSettings.vue` `.src-enable` native checkbox]** Migrate the remaining native `<input type="checkbox" :data-test="src-enable-${src.id}">` to `<NCheckbox>` with `data-test` forwarded — same pattern as the 7c IMPORTANT-1 PermissionPrompt fix. The 7c review verified `CatalogSourcesSettings.test.ts` does **not** assert on the checkbox toggle behaviour today, so no test rewrite is required (only confirm `pnpm test` stays green). This eliminates the last "native checkbox island" in the mcp/marketplace surface and removes the dark-theme visual break.
> 3. **[7c NIT-5/6/7 + Task 7c-NIT-3/4/5/6 i18n sweep]** Convert remaining hardcoded English strings in the mcp + marketplace surface to `t()` calls under existing namespaces (or new `mcp.*` / `marketplace.*` keys), in en.json + zh-CN.json:
>    - `McpServerManager.vue` `statusLabel(status)` returns (`🟢 Running` / `🟡 Starting` / `🔴 Failed` / `⚪ Stopped`) — split emoji from text and i18n the text
>    - `CatalogSourcesSettings.vue` `formError` strings (`"id and display_name are required"`, `"URL must start with http:// or https://"`)
>    - `NEmpty` `description` strings: `CatalogList` (`"No catalog entries match the current filters"`), `InstalledList` (`"No installed servers yet"`), `CatalogSourcesSettings` (`"No remote catalog sources configured."`)
>    - Original 7c-deferred NIT 3/4/5/6: sessions.newButton bakes `"+ "` prefix into the i18n key (split presentation from value); `StatusBar` body text still hardcoded; `PermissionPrompt` business terms (Allow/Deny/Accept/Reject etc.); `TraceTimeline` tab labels + multi-SFC empty-state strings.
> 4. **[7c NIT-4 — `MarketplaceView` `<h1>` style consistency]** Change `<h1>{{ t("marketplace.title") }}</h1>` to `<NText tag="h1" :depth="1">…</NText>` (or equivalent), to match `CatalogSourcesSettings`'s `<NText strong tag="h3">`. Trivial visual unification, ~1 line.
> 5. **[7c NIT-3 — PermissionPrompt `.mcp-trust-check` dead `cursor: pointer`]** After 7c migrated the inner control to `<NCheckbox>`, the wrapper `.mcp-trust-check { cursor: pointer; }` is dead style (clicking the wrapper no longer toggles the checkbox). Remove the `cursor: pointer` declaration from the `.mcp-trust-check` block.
> 6. **[7c NIT-7 — MemoryBrowser.test comment phrasing]** In `MemoryBrowser.test.ts`, refine the comment that currently reads roughly "the SFC renders multiple internal Select subcomponents" to be precise about which `findComponent({ name: "Select" })` call targets which scope filter. Cosmetic.
> 7. **[7c NIT-8 — MemoryBrowser.test optional spy enhancement]** In `MemoryBrowser.test.ts`, add `expect(setMemoryFilterSpy).toHaveBeenCalledWith(scope)` after the `update:value` emit to lock the contract between the SFC and the store action, not just rely on the side-effect of `selectedScope.value`.
>
> Items 1+2 are IMPORTANT (they close real risks the 7c review surfaced); items 3-7 are NIT and may be split off if the auto-import diff already exceeds the LOC split threshold — in that case, list each deferred item explicitly in the commit message body.

**Files:**

- Modify: `apps/agent-gui/vite.config.ts`
- Modify: `eslint.config.js` (root)
- Modify: `apps/agent-gui/tsconfig.json` (include the new generated `.d.ts` files)
- Modify: `.gitignore` (add `auto-imports.d.ts`, `components.d.ts`)
- Bulk delete redundant imports across `apps/agent-gui/src/**/*.{ts,vue}`
- Auto-generated (do not commit): `apps/agent-gui/src/auto-imports.d.ts`, `apps/agent-gui/src/components.d.ts`, `apps/agent-gui/.eslintrc-auto-import.json`

- [ ] **Step 1: Update `vite.config.ts` with both plugins**

  Replace `apps/agent-gui/vite.config.ts` with:

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
        // Whitelist only — no business stores, per spec §3 Q7.
        imports: [
          "vue",
          "vue-router",
          "pinia",
          "vue-i18n",
          {
            "@vueuse/core": [
              "useDark",
              "useColorMode",
              "useStorage",
              "useEventListener",
              "tryOnScopeDispose",
              "useDebounceFn",
              "useThrottleFn",
              "useIntervalFn",
              "useTimeoutFn",
              "useClipboard",
              "useFocus"
            ]
          }
        ],
        dts: "src/auto-imports.d.ts",
        eslintrc: {
          enabled: true,
          filepath: "./.eslintrc-auto-import.json",
          globalsPropValue: true
        },
        dirs: [],
        vueTemplate: true
      }),
      Components({
        // Naive UI components are auto-imported on use.
        resolvers: [NaiveUiResolver()],
        // Project SFCs under src/components are also auto-registered for templates.
        dirs: ["src/components"],
        extensions: ["vue"],
        deep: true,
        dts: "src/components.d.ts"
      })
    ],
    resolve: {
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url))
      }
    },
    clearScreen: false,
    server: { port: 1420, host: "0.0.0.0" }
  });
  ```

- [ ] **Step 2: Trigger one dev/build cycle to generate the .d.ts files**

  ```bash
  pnpm --filter agent-gui run build
  ```

  Expected: `apps/agent-gui/src/auto-imports.d.ts`, `apps/agent-gui/src/components.d.ts`, and `apps/agent-gui/.eslintrc-auto-import.json` are created. No build error.

- [ ] **Step 3: Add the generated artifacts to `.gitignore`**

  Append to root `.gitignore`:

  ```
  # unplugin-auto-import / unplugin-vue-components generated artifacts
  apps/agent-gui/src/auto-imports.d.ts
  apps/agent-gui/src/components.d.ts
  apps/agent-gui/.eslintrc-auto-import.json
  ```

  Verify they are now ignored:

  ```bash
  git check-ignore -v \
    apps/agent-gui/src/auto-imports.d.ts \
    apps/agent-gui/src/components.d.ts \
    apps/agent-gui/.eslintrc-auto-import.json
  ```

  Expected: all three lines confirm the rule. If any was previously committed (it should not be, since these are new), `git rm --cached` it.

- [ ] **Step 4: Add the auto-import globals to ESLint config (precise diff)**

  Verified current `eslint.config.js` (root) is a flat-config array of 6 blocks. The third block (after `js.configs.recommended` and `tseslint.configs.recommended` + `pluginVue.configs["flat/recommended"]`) is the one scoped to `apps/agent-gui/**/*.{ts,tsx,js,jsx,vue}` with its own `languageOptions.globals = { ...globals.browser, ...globals.node }`.

  Apply two precise edits:

  4.1 Insert the loader at the **top** of the file, immediately after the existing `import` block (between `import tseslint from "typescript-eslint";` and the default-export). The new lines:

  ```js
  import { readFileSync, existsSync } from "node:fs";
  import { fileURLToPath } from "node:url";
  import { dirname, resolve } from "node:path";

  const __dirname = dirname(fileURLToPath(import.meta.url));
  const autoImportGlobals = (() => {
    const path = resolve(
      __dirname,
      "apps/agent-gui/.eslintrc-auto-import.json"
    );
    if (!existsSync(path)) return {};
    try {
      return JSON.parse(readFileSync(path, "utf8")).globals ?? {};
    } catch {
      return {};
    }
  })();
  ```

  4.2 Modify the existing `apps/agent-gui` block. Locate this block in the array:

  ```js
  {
    files: ["apps/agent-gui/**/*.{ts,tsx,js,jsx,vue}"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: {
        ...globals.browser,
        ...globals.node
      },
      parserOptions: {
        parser: tseslint.parser,
        extraFileExtensions: [".vue"]
      }
    },
    rules: {
      "vue/multi-word-component-names": "off"
    }
  },
  ```

  Replace the `globals` object so it picks up the auto-import keys:

  ```js
      globals: {
        ...globals.browser,
        ...globals.node,
        ...autoImportGlobals
      },
  ```

  These are the only two textual changes in `eslint.config.js`. No other block (root ignores, `scripts/**/*.cjs`, `apps/agent-gui/e2e/**`, `eslintConfigPrettier`) is touched. After the change, `pnpm run lint:eslint` accepts uses of `ref`, `computed`, `useRoute`, `defineStore`, `storeToRefs`, `useI18n`, `useDark`, etc. without explicit imports.

- [ ] **Step 5: Update `tsconfig.json` to include generated d.ts**

  Edit `apps/agent-gui/tsconfig.json` `include`:

  ```json
  "include": [
    "src/**/*.ts",
    "src/**/*.vue",
    "src/**/*.json",
    "src/auto-imports.d.ts",
    "src/components.d.ts"
  ]
  ```

  (Even though the files are gitignored, they exist on disk after `pnpm dev`/`pnpm build` and TS needs to see them for type-checking.)

- [ ] **Step 6: Bulk-remove redundant imports (deterministic per-file process)**

  This is the largest mechanical edit. The process is per-file (one file per `read_file` + `file_replace` cycle), never blanket-substitute across files.

  **6.1 Build the candidate list.** Run:

  ```bash
  cd /Users/chanyu/AIProjects/kairox
  grep -rln '\bfrom "vue"\b\|\bfrom "vue-router"\b\|\bfrom "pinia"\b\|\bfrom "vue-i18n"\b\|\bfrom "@vueuse/core"\b' \
    apps/agent-gui/src --include="*.ts" --include="*.vue" \
    > /tmp/kairox-autoimport-candidates.txt
  wc -l /tmp/kairox-autoimport-candidates.txt
  ```

  **6.2 Define the deletion rules table.** Apply these **exactly** (any deviation must be a deliberate exception with an inline comment):

  | Import line pattern                                                                                                                                                                                                                                                                                                                                                                                                                                           | Action                                                                      | Rationale                                                            |
  | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------- |
  | `import { … } from "vue"` where every name is in `{ ref, computed, watch, watchEffect, reactive, readonly, onMounted, onUnmounted, onBeforeMount, onBeforeUnmount, nextTick, defineComponent, defineProps, defineEmits, defineExpose, defineModel, toRef, toRefs, isRef, unref, shallowRef, shallowReactive, markRaw, provide, inject, useSlots, useAttrs, h, withDirectives, mergeProps, getCurrentInstance, getCurrentScope, onScopeDispose, effectScope }` | DELETE the entire line                                                      | All these are in `unplugin-auto-import`'s `vue` preset               |
  | `import { … } from "vue"` where at least one name is NOT in the list above (e.g. `App`, custom util)                                                                                                                                                                                                                                                                                                                                                          | KEEP the import line, but remove only the auto-imported names from the `{}` | Mixed usage — keep what's not auto-imported                          |
  | `import { … } from "vue-router"` where all names are in `{ useRoute, useRouter, useLink, onBeforeRouteLeave, onBeforeRouteUpdate, createRouter, createWebHashHistory, createWebHistory, createMemoryHistory, RouterLink, RouterView }`                                                                                                                                                                                                                        | DELETE the entire line                                                      | All in vue-router preset                                             |
  | `import { … } from "pinia"` where all names are in `{ defineStore, storeToRefs, acceptHMRUpdate, getActivePinia, setActivePinia, createPinia, mapActions, mapState, mapGetters, mapStores }`                                                                                                                                                                                                                                                                  | DELETE the entire line                                                      | All in pinia preset                                                  |
  | `import { … } from "vue-i18n"` where all names are in `{ useI18n, createI18n }`                                                                                                                                                                                                                                                                                                                                                                               | DELETE the entire line                                                      | Both in vue-i18n preset                                              |
  | `import { … } from "@vueuse/core"` where every name is in the Step 1 whitelist (`useDark`, `useColorMode`, `useStorage`, `useEventListener`, `tryOnScopeDispose`, `useDebounceFn`, `useThrottleFn`, `useIntervalFn`, `useTimeoutFn`, `useClipboard`, `useFocus`)                                                                                                                                                                                              | DELETE the entire line                                                      | Whitelisted hooks are auto-imported                                  |
  | `import { … } from "@vueuse/core"` where at least one name is NOT in the whitelist                                                                                                                                                                                                                                                                                                                                                                            | KEEP the line, remove only whitelisted names                                | Non-whitelisted hooks remain explicit                                |
  | `import { … } from "naive-ui"` where every name matches `/^N[A-Z]/` (component pattern, e.g. `NButton`, `NCard`, `NInput`)                                                                                                                                                                                                                                                                                                                                    | DELETE the entire line                                                      | `unplugin-vue-components` + `NaiveUiResolver` handles all components |
  | `import { … } from "naive-ui"` containing service hooks (`useMessage`, `useDialog`, `useNotification`, `useLoadingBar`, `useThemeVars`, `darkTheme`, `lightTheme`, `GlobalTheme`, `GlobalThemeOverrides`)                                                                                                                                                                                                                                                     | KEEP the line, remove only the `/^N[A-Z]/`-pattern component names          | Service hooks + theme exports are NOT auto-registered                |

  **6.3 Per-file execution.** For each file in `/tmp/kairox-autoimport-candidates.txt`:

  ```bash
  read_file <path>                   # via the read_file tool
  # apply rules from 6.2 to the file's first 30 lines
  file_replace <path> <old-import-block> <new-import-block> false
  ```

  After every batch of ~5 files, run `pnpm run lint:eslint apps/agent-gui/src` to catch any premature deletions before the diff grows too large to bisect.

  **6.4 Test files (`*.test.ts`)** also benefit from auto-imports, but **keep `setActivePinia`, `createPinia`, `setMount` (any explicit setup helper) imported explicitly** — readability of test setup wins over brevity. Only delete `import { ref } from "vue"` style lines from test files when those refs are used solely inside the assertion bodies.

- [ ] **Step 7: Run vitest + lint + build to verify nothing broke**

  ```bash
  pnpm --filter agent-gui run test
  pnpm run lint:eslint
  pnpm --filter agent-gui run build
  ```

  Expected:
  - `vitest`: green. The test files also benefit from auto-imports — but `setActivePinia`/`createPinia` should still be imported explicitly in test files for clarity.
  - `lint:eslint`: clean. If a `'foo' is not defined` error appears, either (a) `foo` is not whitelisted (re-add the explicit import) or (b) the auto-import globals JSON did not regenerate (run `pnpm --filter agent-gui run build` again to regenerate, then re-run lint).
  - `build`: succeeds, dist/ size roughly identical (auto-import is build-time codegen, not runtime).

- [ ] **Step 8: Run e2e once more to confirm runtime parity**

  ```bash
  pnpm --filter agent-gui run test:e2e 2>&1 | tail -20
  ```

  Expected: same green status as Task 8.

- [ ] **Step 9: Commit**

  ```bash
  git add apps/agent-gui/vite.config.ts apps/agent-gui/tsconfig.json eslint.config.js .gitignore \
          apps/agent-gui/src
  git commit -m "feat(gui): add unplugin-auto-import and unplugin-vue-components"
  ```

  Confirm the generated `.d.ts` and `.eslintrc-auto-import.json` are NOT staged:

  ```bash
  git status --porcelain | grep -E "(auto-imports|components)\.d\.ts|eslintrc-auto-import" | cat
  ```

  Expected: empty output (the gitignore rules from Step 3 hide them).

---

## Task 10: Update AGENTS.md to reflect the new stack (commit 10)

**Branch:** `feat/frontend-engineering`
**Commit message header (≤100 chars, verified 56):** `docs(gui): update AGENTS.md for frontend foundation deps`
**Commit message body:**

```
Reflect pinia, vue-router, vue-i18n, naive-ui, @vueuse/core,
and the unplugin auto-imports introduced in this branch.
```

**Why last:** documentation always reflects shipped state, not aspirational state. We update only after every implementation commit lands.

**Files:**

- Modify: `AGENTS.md` (sections: "TypeScript / Vue" coding conventions, "Project structure" tree, "When modifying the GUI" recipe, "Common pitfalls")

- [ ] **Step 1: Read the current AGENTS.md GUI-related sections**

  Already in context (the file was provided in `additional_data`). Confirm by running:

  ```bash
  grep -n "Pinia\|vue-router\|vue-i18n\|NaiveUI\|naive-ui\|@vueuse\|unplugin" AGENTS.md | head -40
  ```

- [ ] **Step 2: Edit "TypeScript / Vue" section**

  Find the `### TypeScript / Vue` block (under `## Coding conventions`). Replace its bullet list with:

  ```md
  ### TypeScript / Vue

  - **Framework**: Vue 3 Composition API + TypeScript (`<script setup lang="ts">`)
  - **State management**: Pinia setup-stores (`defineStore('name', () => { /* state, getters, actions */ })`) under `apps/agent-gui/src/stores/`. Composables in `composables/`. Use `useXxxStore()` + `storeToRefs()` in consumers.
  - **Routing**: vue-router with `createWebHashHistory()`. Route table at `apps/agent-gui/src/router/routes.ts`. Workbench routes are nested: `/workbench/:sessionId?`.
  - **i18n**: vue-i18n v9 (composition API mode). Locale messages under `apps/agent-gui/src/locales/{en,zh-CN}.json`. Only common copy (`common.*`, `nav.*`, `settings.*`, `notifications.*`, `status.*`) is translated; per-feature strings stay inline.
  - **UI library**: NaiveUI. Provider stack lives in `apps/agent-gui/src/layouts/AppLayout.vue` (`NConfigProvider` → `NLoadingBarProvider` → `NMessageProvider` → `NDialogProvider` → `NNotificationProvider`). Theme overrides in `apps/agent-gui/src/styles/naive-theme.ts` mirror existing CSS variables.
  - **Composable utilities**: `@vueuse/core` (whitelisted via auto-import: `useDark`, `useColorMode`, `useStorage`, `useEventListener`, `tryOnScopeDispose`, `useDebounceFn`, `useThrottleFn`, `useIntervalFn`, `useTimeoutFn`, `useClipboard`, `useFocus`).
  - **Auto-imports**: `unplugin-auto-import` + `unplugin-vue-components` are configured in `vite.config.ts`. The whitelist covers `vue`, `vue-router`, `pinia`, `vue-i18n`, and selected `@vueuse/core` hooks. NaiveUI components are auto-registered in templates; `useMessage`/`useDialog`/`useNotification`/`useLoadingBar` are functions and must still be imported explicitly. Generated artifacts (`src/auto-imports.d.ts`, `src/components.d.ts`, `.eslintrc-auto-import.json`) are gitignored — Vite regenerates them on dev/build.
  - **Path alias**: `@/*` resolves to `apps/agent-gui/src/*` (configured in `vite.config.ts` and `tsconfig.json`).
  - **Types**: Centralized in `apps/agent-gui/src/types/`. Mirror Rust event types for Tauri IPC.
  - **Testing**: Vitest with `vitest/globals` + `@vue/test-utils`. Test helper `src/test-utils/mount.ts` exposes `mountWithPlugins()` that injects pinia, i18n, and a memory-history router. Use `@pinia/testing`'s `createTestingPinia()` when you want spy-able actions.
  - **Style**: Prettier + ESLint + Stylelint. See lint-staged config for auto-fix rules.
  ```

- [ ] **Step 3: Update the project tree under `## Project structure`**

  Find the `apps/agent-gui/src/` block. Update it to include the new directories:

  ```md
  │ ├── src/ # Vue frontend
  │ │ ├── App.vue # thin root: mounts AppLayout, handles workspace bootstrap
  │ │ ├── main.ts # createApp + pinia + router + i18n + bindLocaleToStore
  │ │ ├── layouts/AppLayout.vue # NaiveUI provider stack + nav + RouterView
  │ │ ├── views/ # WorkbenchView, MarketplaceView, SettingsView (lazy)
  │ │ ├── router/ # index.ts (createWebHashHistory) + routes.ts
  │ │ ├── locales/ # en.json, zh-CN.json, index.ts (i18n instance)
  │ │ ├── styles/naive-theme.ts # NaiveUI theme overrides (light + dark)
  │ │ ├── components/ # ChatPanel, TraceTimeline, TaskSteps, TaskNode,
  │ │ │ # PermissionPrompt, PermissionCenter, MemoryBrowser,
  │ │ │ # McpServerManager, McpStatusIndicator, SessionsSidebar,
  │ │ │ # StatusBar, NotificationToast, TraceEntry,
  │ │ │ # marketplace/\* (Catalog{List,Card,Detail},
  │ │ │ # InstalledList, InstallProgress, RuntimeMissingHint)
  │ │ ├── stores/ # session, taskGraph, agents, mcp, memory, catalog, ui
  │ │ ├── composables/ # useTauriEvents (session-filtered), useTraceStore,
  │ │ │ # useNotifications (delegates to ui store), useUpdater,
  │ │ │ # useMarketplace
  │ │ ├── test-utils/mount.ts # mountWithPlugins helper for vitest
  │ │ ├── types/ # TypeScript type definitions (re-exports from generated/)
  │ │ │ └── events-helpers.ts # ExtractPayload, EventPayloadHandlers, matchPayload
  │ │ └── generated/ # specta-generated bindings (commands.ts, events.ts)
  ```

  Additional concrete edits to the AGENTS.md project tree (verified by `grep -n "Marketplace\|ConfirmDialog" AGENTS.md` against the current file at the start of this task):
  - Remove `ConfirmDialog.vue` from the inline component enumeration (it was deleted in Task 7 Step 1).
  - The current AGENTS.md project-tree entry for `apps/agent-gui/src/views/` mentions only `Marketplace.vue` implicitly via the components/marketplace path; if a future grep shows a `views/Marketplace.vue` literal anywhere in AGENTS.md, replace it with `views/MarketplaceView.vue` (Task 5 renamed the file).

- [ ] **Step 4: Update "When modifying the GUI" recipe**

  Find the `### When modifying the GUI` block and replace it with:

  ```md
  ### When modifying the GUI

  - Vue components go in `apps/agent-gui/src/components/`. Prefer NaiveUI components over hand-rolled markup; reach for `<NCard>`, `<NButton>`, `<NList>`, `<NModal>`, etc. before writing new CSS.
  - Pinia stores live in `apps/agent-gui/src/stores/` and use the setup-store form (`defineStore('name', () => ({ /* state, getters, actions */ }))`). Cross-store dependencies should be resolved lazily inside actions (e.g. `const session = useSessionStore()` _inside_ the function body, not at module top level).
  - Composables go in `apps/agent-gui/src/composables/`. Use `tryOnScopeDispose` (auto-imported from `@vueuse/core`) for cleanup of `listen()` subscriptions.
  - Routes go in `apps/agent-gui/src/router/routes.ts`. Use `useRoute`/`useRouter` (auto-imported) inside components.
  - i18n: add new common-copy keys to BOTH `apps/agent-gui/src/locales/en.json` AND `apps/agent-gui/src/locales/zh-CN.json`. Reach for `t("common.send")` in templates. Per-feature strings can stay inline.
  - Theme: extend `apps/agent-gui/src/styles/naive-theme.ts` for both `lightThemeOverrides` and `darkThemeOverrides`. Toggle dark mode via `useUiStore().setTheme('dark')`.
  - TypeScript types go in `apps/agent-gui/src/types/`.
  - Auto-generated event types are in `apps/agent-gui/src/generated/events.ts` — **never edit this file manually**, run `just gen-types` instead.
  - Event helper types (`ExtractPayload`, `EventPayloadHandlers`, `matchPayload`) are in `apps/agent-gui/src/types/events-helpers.ts`.
  - Always update the corresponding Rust `#[tauri::command]` in `commands.rs` if the IPC surface changes.
  - Use `useTauriEvents.ts` for real-time Rust→Vue event streaming.
  - Use TypeScript discriminated union narrowing (not `as` casts) when handling `EventPayload` variants.
  - For tests, prefer `mountWithPlugins` from `src/test-utils/mount.ts` over the raw `mount` from `@vue/test-utils` so the component receives pinia + i18n + router automatically.
  ```

- [ ] **Step 5: Add new pitfalls under `### Common pitfalls`**

  Append to the existing list:

  ```md
  - **Don't import what's auto-imported**: `vue`, `vue-router`, `pinia`, `vue-i18n`, and the whitelisted `@vueuse/core` hooks listed in `vite.config.ts` are globals. Re-importing them creates a "duplicate import" warning at lint time. The exception is when shadowing or aliasing — use explicit imports then.
  - **Don't import NaiveUI components for templates**: `<NButton>`, `<NCard>`, etc. are auto-resolved by `NaiveUiResolver`. NaiveUI **functions** like `useMessage()`, `useDialog()`, `useNotification()`, `useLoadingBar()` are NOT components and DO need explicit imports.
  - **Don't commit `apps/agent-gui/src/auto-imports.d.ts`, `apps/agent-gui/src/components.d.ts`, or `apps/agent-gui/.eslintrc-auto-import.json`** — they are regenerated on every Vite dev/build and are listed in `.gitignore`.
  - **Don't reach for `useMessage()` outside a component wrapped by `<NMessageProvider>`** — it returns null and crashes at use. The provider tree lives in `AppLayout.vue`. For tests, mount via `mountWithPlugins` and add a thin `<NMessageProvider>` wrapper if your component calls `useMessage()`.
  - **Don't navigate via `view = ref('workbench')` patterns**: vue-router is the source of truth. Use `router.push({ name: 'workbench', params: { sessionId } })` and read state via `useRoute()`.
  ```

- [ ] **Step 6: Verify formatting and commit**

  ```bash
  pnpm exec prettier --write AGENTS.md
  pnpm run format:check:web
  ```

  Expected: format check passes (or completes silently).

  ```bash
  git add AGENTS.md
  git commit -m "docs: update AGENTS.md GUI section for pinia, vue-router, vue-i18n, naive-ui, vueuse, unplugin"
  ```

---

## Final verification (do this before opening the PR)

After all 10 commits land on `feat/frontend-engineering`:

- [ ] **FV-1: Full CI gate locally**

  ```bash
  cd /Users/chanyu/AIProjects/kairox
  pnpm run format:check
  pnpm run lint
  cargo test --workspace --all-targets
  just check-types
  pnpm --filter agent-gui run test
  pnpm --filter agent-gui run build
  pnpm --filter agent-gui run test:e2e
  ```

  Expected: every command exits zero. Capture and save the final summary line of each command in the PR description for reviewer convenience.

- [ ] **FV-2: Inspect commit log**

  ```bash
  git log --oneline main..HEAD | cat
  ```

  Expected: 10 commits (or 12 if Task 7 split into 7a/7b/7c), each with a clean Conventional Commits message and a focused diff.

- [ ] **FV-3: Tauri dev smoke (full stack, not just web)**

  ```bash
  pnpm --filter agent-gui run tauri:dev
  ```

  Wait for the desktop window. Smoke-test:
  - Workbench loads, sessions sidebar populates from real backend
  - Send a message → real model responds, streaming text appears in `<NScrollbar>`
  - URL bar inside Tauri's webview shows hash routes
  - Permission modal appears and works
  - Theme toggle works
  - Locale toggle changes nav text immediately
  - Quit & relaunch → locale + theme + last session restore from localStorage

  Ctrl-C the Tauri dev server.

- [ ] **FV-4: Open PR**

  ```bash
  git push -u origin feat/frontend-engineering
  ```

  Then open a PR against `main`, paste the FV-1 summary lines, and request review.

---

## Self-Review

The following checks were applied to this plan after writing it; any issues found were fixed inline.

### Spec coverage

| Spec section                                                   | Plan coverage                                                                                                                                                                                                                                                     |
| -------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| §3 Q1 落地策略 (single PR, branch `feat/frontend-engineering`) | Pre-flight + every Task uses the same branch; Final Verification opens one PR                                                                                                                                                                                     |
| §3 Q2 NaiveUI 全量迁移 14+6 SFC                                | Task 6 (providers + theme overrides + `useThemeVars` integration) + Task 7 (per-component recipe with 19 SFCs in deterministic execution order, plus full `ConfirmDialog → useDialog` migration including test-file deletion + e2e selector update)               |
| §3 Q3 Pinia setup-store                                        | Task 3 (all 6 stores migrated to `defineStore('name', () => {...})` with cross-store lazy resolution; verified consumer inventory in store-by-store table)                                                                                                        |
| §3 Q4 vue-router 嵌套 + hash                                   | Task 2 (`createWebHashHistory`, `/workbench/:sessionId?`, `/marketplace`, `/settings`, catchall redirect) + Task 5 Step 3 (`WorkbenchView` with bidirectional URL↔store sync)                                                                                     |
| §3 Q5 vue-i18n common-copy en + zh-CN                          | Task 2 Step 5-7 (full common-copy locale JSONs + type-safe schema augmentation) + Task 4 Step 2-3 (`bindLocaleToStore` watcher) + Task 5 Step 4 (Settings view with locale selector) + Task 7 Step 1.1 (added `common.deleteConfirm` for ConfirmDialog migration) |
| §3 Q6 @vueuse/core                                             | Task 4 (`useDark`, `useColorMode`, `useStorage` in `ui` store; `tryOnScopeDispose` in `useTauriEvents` listener cleanup)                                                                                                                                          |
| §3 Q7 unplugin-auto-import whitelist                           | Task 9 Step 1 (vue, vue-router, pinia, vue-i18n + 11 whitelisted @vueuse/core hooks) + Step 6 deterministic deletion rules table                                                                                                                                  |
| §3 Q8 unplugin-vue-components                                  | Task 9 Step 1 (`NaiveUiResolver()` + `dirs: ['src/components']`); rule table in Step 6.2 distinguishes auto-registered components from must-import service hooks                                                                                                  |
| §4.1 文件结构                                                  | All new dirs created in Task 2 (`router/`, `locales/`), Task 3 Step 9 (`stores/ui.ts`), Task 5 (`views/`, `layouts/`), Task 6 (`styles/`); Task 3 Step 14 adds `test-utils/mount.ts`                                                                              |
| §5.5 NaiveUI 组件迁移映射表                                    | Task 7 reproduces the 21-component map verbatim with confirmed execution order from smallest (42 LOC) to largest (498 LOC)                                                                                                                                        |
| §5.10 Vite 配置                                                | Task 9 Step 1 (full `vite.config.ts` content)                                                                                                                                                                                                                     |
| §8 Commit plan (10 提交)                                       | Tasks 1-10. Task 7 split is **mandatory** when LOC sum > 1500 (current baseline ≈3590 LOC, so split is the default path, with deterministic 7a/7b/7c grouping).                                                                                                   |
| §9 DoD                                                         | Final Verification (FV-1 ~ FV-4)                                                                                                                                                                                                                                  |

No gaps found.

### Placeholder & assumption scan

Re-ran `grep -nE "\b(TBD|TODO|FIXME|placeholder|XXX|HACK|guess|maybe|approximation|likely|might|may need|may not|to be filled|to be determined|simplifi|probably|possibly|rough)\b"` and `grep -nE "/\* \.\.\.|/\* TODO|/\* same body|/\* port over|/\* fill"` against the plan. The only remaining hits are:

- `common-copy` (i18n terminology, not a placeholder) — kept.
- L1139 `setProjection — same as today, plus replaces taskGraphState.tasks = ... with useTaskGraphStore().tasks = ...` — this is a **delta description**, not a code placeholder. The full migrated code body is given verbatim two paragraphs above in Task 3 Step 8's session-store migration block — kept.

After the most recent fixes, **every** code-step body (stores, composables, tests, vite/eslint configs, locale JSON, theme overrides) is given as fully self-contained code. Specifically:

- Task 3 Step 3 (memory.test.ts) — full 6-case migration (no `port over`).
- Task 3 Step 7 (catalog store) — full state/getter/action set, 1:1 with current source (no `same body`).
- Task 3 Step 8 (session store) — full state and method bodies inline.
- Task 4 Step 4 (useTauriEvents.ts) — full lifecycle refactor with every routing branch (no `unchanged from Task 3`).

No `TBD`, `TODO`, `FIXME`, `XXX`, `HACK`, `guess`, `approximation`, `likely`, `might`, `may need`, `simplification`, `probably`, `possibly`, `rough`, `same body`, or `port over` placeholders remain in actionable plan steps.

### Type & API consistency

- **Store names** are stable across all tasks: `useSessionStore`, `useTaskGraphStore`, `useAgentsStore`, `useMcpStore`, `useMemoryStore`, `useCatalogStore`, `useUiStore`.
- **`switchSession(id)`** is defined exactly once (Task 3 Step 8). Task 5 Step 3 (`WorkbenchView`), Task 5 Step 6 (`App.vue`), Task 5 Step 7 (`SessionsSidebar.vue`) all consume this single definition. Both forward-references in Task 5 explicitly point back to Task 3 Step 8 instead of redefining the method.
- **`ui.pushNotification(level, message)`** and **`ui.dismissNotification(id)`** are defined in Task 3 Step 9 / Task 4 Step 1 and used identically in Task 4 (`ui.test.ts`), Task 5 (`WorkbenchView`, `App.vue`), Task 6 (`NotificationToast.vue` adapter), Task 7 store actions.
- **Theme tokens** `lightThemeOverrides` / `darkThemeOverrides` are defined in Task 6 Step 2 and consumed in Task 6 Step 3 via `:theme-overrides` binding.
- **`--app-*` CSS variables** (`--app-body-color`, `--app-card-color`, `--app-border-color`, `--app-text-color`, `--app-primary-color`) are defined in Task 6 Step 2 (inline `:style` from `useThemeVars()`) and consumed in Task 6 Step 3's `<style scoped>`.
- **i18n keys** (`common.*`, `nav.*`, `settings.*`, `notifications.*`, `status.*`, plus `common.deleteConfirm` added in Task 7) are defined in Task 2 Step 5 and (for `deleteConfirm`) Task 7 Step 1.1, consumed in Task 5 + Task 7.
- **ESLint config** changes are scoped to two precise textual edits in `eslint.config.js` (Task 9 Step 4.1 + 4.2), no other block touched.

No inconsistencies found.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-07-frontend-engineering.md`. Two execution options:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration, parallel-safe within a task. **REQUIRED SUB-SKILL:** `superpowers:subagent-driven-development`.
2. **Inline Execution** — execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints for review. Slower but you see every command live.

Which approach?
