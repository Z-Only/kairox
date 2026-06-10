# GUI Polish, Marketplace Fix & NaiveUI Removal — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove NaiveUI dependency, fix 19 display/marketplace issues, replace with native HTML + CSS variables + self-built services.

**Architecture:** Pure CSS theme system (`styles/theme.css` + `html.dark` class via `useDark()`), self-built Toast/ConfirmDialog services, native HTML elements with shared CSS classes (`styles/components.css`). Four-phase migration: infrastructure → low-complexity → medium/high-complexity → cleanup.

**Tech Stack:** Vue 3 Composition API, TypeScript, @vueuse/core (useDark), Pinia, CSS custom properties, native `<dialog>` element.

**Spec:** `docs/superpowers/specs/2026-05-07-gui-polish-and-marketplace-fix-design.md`

---

## File Structure

### New files

| File                                | Responsibility                                               |
| ----------------------------------- | ------------------------------------------------------------ |
| `src/styles/theme.css`              | Light/dark CSS variable definitions (13 `--app-*` vars)      |
| `src/styles/components.css`         | Shared CSS classes (`.btn`, `.tag`, `.card`, `.alert`, etc.) |
| `src/composables/useToast.ts`       | Toast notification convenience API                           |
| `src/composables/useConfirm.ts`     | Confirmation dialog convenience API (provide/inject)         |
| `src/components/ToastContainer.vue` | Toast notification renderer (Teleport + TransitionGroup)     |
| `src/components/ConfirmDialog.vue`  | Global confirmation dialog (native `<dialog>`)               |

### Modified files

| File                                        | Changes                                                                     |
| ------------------------------------------- | --------------------------------------------------------------------------- |
| `src/main.ts`                               | Import `theme.css` + `components.css`                                       |
| `src/stores/ui.ts`                          | Add toast state + actions, bridge notifications→toasts                      |
| `src/layouts/AppLayout.vue`                 | Remove 5-layer NaiveUI Provider stack, mount ToastContainer + ConfirmDialog |
| `src/composables/useNotifications.ts`       | Remove `useMessage` import, simplify to store-only                          |
| `src/components/NotificationToast.vue`      | Delete (replaced by ToastContainer)                                         |
| `src/views/SettingsView.vue`                | Replace NTabs/NSelect with native tabs/select                               |
| `src/components/SessionsSidebar.vue`        | Replace `useDialog` → `useConfirm`, fix hard-coded colors                   |
| `src/components/ChatPanel.vue`              | Remove `ScrollbarInst` type import, fix input width                         |
| `src/components/MemoryBrowser.vue`          | Replace `useDialog` → `useConfirm`, remove `SelectOption` type              |
| `src/components/TraceEntry.vue`             | Replace NTag/NEllipsis with native HTML                                     |
| `src/components/TraceTimeline.vue`          | Replace NButton/NScrollbar/NEmpty with native HTML                          |
| `src/components/TaskSteps.vue`              | Replace NTag with `<span class="tag">`                                      |
| `src/components/TaskNode.vue`               | Replace NCard/NSpace/NTag/NDivider with native HTML                         |
| `src/components/McpStatusIndicator.vue`     | Replace NTag with `<span class="tag">`                                      |
| `src/components/PermissionCenter.vue`       | Replace NButton/NCard with native HTML                                      |
| `src/components/McpServerManager.vue`       | Replace all NaiveUI components                                              |
| `src/components/MarketplacePane.vue`        | Add `fetchCatalog()` to onMounted                                           |
| `src/components/marketplace/*.vue`          | Replace all NaiveUI components                                              |
| `src/components/CatalogSourcesSettings.vue` | Replace `SelectOption` type + NaiveUI components                            |
| `src/styles/naive-theme.ts`                 | Delete                                                                      |
| `src/test-utils/mount.ts`                   | Remove NaiveUI Provider harness, add confirmDialog mock                     |
| `vite.config.ts`                            | Remove `NaiveUiResolver`                                                    |
| `vitest.config.ts`                          | Remove `NaiveUiResolver`                                                    |
| `package.json`                              | Remove `naive-ui` dependency                                                |
| `AGENTS.md`                                 | Update UI library documentation                                             |

---

## Phase 1: Infrastructure (Tasks 1–7)

### Task 1: Create theme.css — pure CSS variable definitions

**Files:**

- Create: `src/styles/theme.css`

- [ ] **Step 1: Create the theme CSS file**

Create `src/styles/theme.css` with `:root` (light) and `html.dark` (dark) blocks defining all 13 `--app-*` CSS variables:

```css
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

- [ ] **Step 2: Commit**

```bash
git add src/styles/theme.css
git commit -m "feat(gui): add pure CSS theme variables for light/dark mode"
```

---

### Task 2: Create components.css — shared utility CSS classes

**Files:**

- Create: `src/styles/components.css`

- [ ] **Step 1: Create the components CSS file**

Create `src/styles/components.css` containing shared classes that replace NaiveUI components. Include all of the following sections (≤300 lines total):

**Buttons**: `.btn`, `.btn-primary`, `.btn-danger`, `.btn-ghost`, `.btn-sm`, `.btn-icon` — padding 6px 14px, border-radius 6px, transitions on bg/border/color.

**Tags**: `.tag`, `.tag-success`, `.tag-warning`, `.tag-error`, `.tag-info` — inline-flex, padding 1px 8px, border-radius 4px, font-size 12px. Status variants use `color-mix(in srgb, var(--app-xxx-color) 15%, transparent)` for background.

**Cards**: `.card`, `.card-header`, `.card-body` — border-radius 8px, border 1px solid var(--app-border-color).

**Alerts**: `.alert`, `.alert-info`, `.alert-warning`, `.alert-error`, `.alert-success` — border-left 3px solid, background uses `color-mix(in srgb, ... 8%, transparent)`.

**Empty state**: `.empty-state` — flex column centered, padding 32px, color var(--app-text-color-3).

**Divider**: `.divider` — border-top 1px solid var(--app-border-color), margin 8px 0.

**Spinner**: `.spinner` — 16px circle, border-top-color primary, `spin` animation 0.6s.

**Truncate**: `.truncate` — overflow hidden, text-overflow ellipsis, white-space nowrap.

**List**: `.list` — no list-style; `.list > li` — padding 8px 12px, border-bottom.

**Description list**: `.desc-list` — grid 2-col; `dt` in text-color-3, `dd` in text-color.

**Tabs**: `.tabs` — flex row, border-bottom; `.tab-btn` — padding 8px 16px, no border except bottom 2px transparent; `[aria-selected="true"]` — primary color + border.

**Dialog styles**: `dialog` — border-radius 10px, max-width 480px, box-shadow; `dialog::backdrop` — rgba(0,0,0,0.4); `dialog.drawer` — position fixed right, 100vh, slide-in animation.

**Form elements**: `select, input, textarea` — padding 6px 10px, border-radius 6px, focus outline primary.

**Status dot**: `.status-dot` — 8px circle; variants `--ok`, `--warn`, `--err`, `--idle`.

- [ ] **Step 2: Commit**

```bash
git add src/styles/components.css
git commit -m "feat(gui): add shared CSS component classes replacing NaiveUI"
```

---

### Task 3: Create Toast notification system

**Files:**

- Modify: `src/stores/ui.ts` — add toast state + actions
- Create: `src/composables/useToast.ts`
- Create: `src/components/ToastContainer.vue`

- [ ] **Step 1: Add toast state to ui.ts**

Read `src/stores/ui.ts`. Add after the `dismissNotification` function:

1. A `ToastItem` interface: `{ id: string; message: string; type: "success"|"error"|"info"|"warning"; duration: number }`
2. `const toasts = ref<ToastItem[]>([])` + counter
3. `addToast(message, type?, duration?)` — pushes to `toasts`, returns id
4. `removeToast(id)` — filters out by id
5. Export `ToastItem` type and add `toasts`, `addToast`, `removeToast` to return object

- [ ] **Step 2: Create useToast.ts composable**

Create `src/composables/useToast.ts`:

```ts
import { useUiStore } from "@/stores/ui";

export function useToast() {
  const ui = useUiStore();
  return {
    success: (message: string, duration?: number) => ui.addToast(message, "success", duration),
    error: (message: string, duration?: number) => ui.addToast(message, "error", duration ?? 8000),
    info: (message: string, duration?: number) => ui.addToast(message, "info", duration),
    warning: (message: string, duration?: number) => ui.addToast(message, "warning", duration)
  };
}
```

- [ ] **Step 3: Create ToastContainer.vue**

Create `src/components/ToastContainer.vue`:

- `<Teleport to="body">` wrapper
- `<TransitionGroup name="toast">` iterating `toasts` from store
- Each toast: `.toast.toast--{type}` div with icon, message, close button
- `@vue:mounted` triggers `setTimeout(removeToast, duration)`
- Icon map: success→✓, error→✕, warning→⚠, info→ℹ
- Scoped CSS: fixed top-right, slide-in/out animations, border-left colored by type

- [ ] **Step 4: Commit**

```bash
git add src/stores/ui.ts src/composables/useToast.ts src/components/ToastContainer.vue
git commit -m "feat(gui): add self-built toast notification system"
```

---

### Task 4: Create ConfirmDialog system

**Files:**

- Create: `src/composables/useConfirm.ts`
- Create: `src/components/ConfirmDialog.vue`

- [ ] **Step 1: Create useConfirm.ts**

Create `src/composables/useConfirm.ts`:

```ts
import { inject, type InjectionKey } from "vue";

export interface ConfirmOptions {
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  type?: "info" | "warning" | "error";
}

export interface ConfirmAPI {
  confirm: (options: ConfirmOptions) => Promise<boolean>;
}

export const confirmDialogKey: InjectionKey<ConfirmAPI> = Symbol("confirmDialog");

export function useConfirm(): ConfirmAPI {
  const api = inject(confirmDialogKey);
  if (!api) {
    throw new Error("useConfirm() requires <ConfirmDialog /> to be mounted in a parent component");
  }
  return api;
}
```

- [ ] **Step 2: Create ConfirmDialog.vue**

Create `src/components/ConfirmDialog.vue`:

- Uses native `<dialog>` element with `showModal()`/`close()`
- `provide(confirmDialogKey, { confirm })` — exposes `confirm(options): Promise<boolean>`
- Internal state: `currentOptions` ref + `resolvePromise` closure
- Template: dialog header (optional title), body (message), footer (cancel + confirm buttons)
- `<slot />` after dialog to render children
- Scoped CSS: header padding 16px 20px, body 16px 20px, footer flex justify-end gap 8px

- [ ] **Step 3: Commit**

```bash
git add src/composables/useConfirm.ts src/components/ConfirmDialog.vue
git commit -m "feat(gui): add self-built confirm dialog system"
```

---

### Task 5: Wire infrastructure into main.ts

**Files:**

- Modify: `src/main.ts`

- [ ] **Step 1: Add CSS imports**

Read `src/main.ts`. Add these two lines after `import "./assets/main.css"`:

```ts
import "./styles/theme.css";
import "./styles/components.css";
```

- [ ] **Step 2: Commit**

```bash
git add src/main.ts
git commit -m "feat(gui): import theme and component CSS in main entry"
```

---

### Task 6: Rewrite useNotifications — drop NaiveUI

**Files:**

- Modify: `src/composables/useNotifications.ts`

- [ ] **Step 1: Rewrite useNotifications.ts**

Read `src/composables/useNotifications.ts`. Replace entire content with:

```ts
import { useUiStore, type NotificationLevel } from "@/stores/ui";

export function useNotifications() {
  const ui = useUiStore();

  function notify(level: NotificationLevel, content: string) {
    ui.pushNotification(level, content);
  }

  return { notify };
}
```

This removes the `useMessage` import from `naive-ui` and the try/catch fallback logic.

- [ ] **Step 2: Commit**

```bash
git add src/composables/useNotifications.ts
git commit -m "refactor(gui): remove NaiveUI useMessage from useNotifications"
```

---

### Task 7: Delete NotificationToast + bridge notifications→toasts

**Files:**

- Delete: `src/components/NotificationToast.vue`
- Modify: `src/stores/ui.ts`

- [ ] **Step 1: Delete NotificationToast.vue**

```bash
git rm src/components/NotificationToast.vue
```

- [ ] **Step 2: Bridge pushNotification to addToast**

Read `src/stores/ui.ts`. In the `pushNotification` function, add a call to `addToast(message, level)` after pushing to the notifications array. This ensures existing `useNotifications().notify()` calls automatically produce visible toasts.

- [ ] **Step 3: Commit**

```bash
git add src/stores/ui.ts
git commit -m "refactor(gui): bridge pushNotification to toast system, delete NotificationToast"
```

---

## Phase 2: Low-Complexity Migrations (Tasks 8–9)

### Task 8: Migrate low-complexity components

**Files:**

- Modify: `src/components/McpStatusIndicator.vue`
- Modify: `src/components/TraceEntry.vue`
- Modify: `src/components/TraceTimeline.vue`
- Modify: `src/components/TaskSteps.vue`
- Modify: `src/components/marketplace/RuntimeMissingHint.vue`

- [ ] **Step 1: Read each component file**

Read all five files to identify exact NaiveUI usage.

- [ ] **Step 2: Replace NaiveUI elements in each component**

Apply these replacements in each file:

- `<NTag type="xxx">` → `<span class="tag tag-xxx">`
- `<NButton ...>` → `<button class="btn ...">`
- `<NEllipsis>` → `<span class="truncate">`
- `<NScrollbar>` → parent element with `overflow-y: auto` CSS
- `<NEmpty description="...">` → `<div class="empty-state">...</div>`
- `<NText>` → `<span>`
- Remove all `import { ... } from "naive-ui"` lines
- Remove all `import type { ... } from "naive-ui"` lines

- [ ] **Step 3: Run lint to verify**

```bash
cd ~/AIProjects/kairox/.worktrees/feat-gui-polish-and-marketplace-fix/apps/agent-gui
pnpm run lint 2>&1 | head -50
```

- [ ] **Step 4: Commit**

```bash
git add src/components/McpStatusIndicator.vue src/components/TraceEntry.vue \
  src/components/TraceTimeline.vue src/components/TaskSteps.vue \
  src/components/marketplace/RuntimeMissingHint.vue
git commit -m "refactor(gui): migrate low-complexity components from NaiveUI to native HTML"
```

---

### Task 9: Rewrite AppLayout.vue — remove NaiveUI Provider stack

**Files:**

- Modify: `src/layouts/AppLayout.vue`

- [ ] **Step 1: Read current AppLayout.vue**

- [ ] **Step 2: Rewrite AppLayout.vue**

Remove all NaiveUI imports and the 5-layer Provider stack. The new version:

1. **Remove imports**: `darkTheme`, `useThemeVars`, `type GlobalTheme` from `naive-ui`; `lightThemeOverrides`, `darkThemeOverrides` from `@/styles/naive-theme`; `NotificationToast` component
2. **Remove template Provider stack**: `NConfigProvider` → `NLoadingBarProvider` → `NMessageProvider` → `NDialogProvider` → `NNotificationProvider` → `NGlobalStyle`
3. **Remove `:style` binding**: the 13 `--app-*` vars are now defined in `theme.css`
4. **Add imports**: `ToastContainer` from `@/components/ToastContainer.vue`, `ConfirmDialog` from `@/components/ConfirmDialog.vue`
5. **New template**: `<ConfirmDialog>` wrapping `<div class="app-shell">` containing `<nav>`, `<RouterView />`, then `<ToastContainer />` after the div
6. **Keep** existing `.app-shell` and `.app-nav` scoped CSS, add `background: var(--app-body-color)` and `color: var(--app-text-color)` to `.app-shell`

- [ ] **Step 3: Commit**

```bash
git add src/layouts/AppLayout.vue
git commit -m "refactor(gui): remove NaiveUI Provider stack from AppLayout"
```

---

## Phase 3: Medium/High-Complexity Migrations (Tasks 10–17)

### Task 10: Rewrite SettingsView.vue — native tabs and selects

**Files:**

- Modify: `src/views/SettingsView.vue`

- [ ] **Step 1: Read current SettingsView.vue**

- [ ] **Step 2: Rewrite with native HTML**

1. Remove `import { NSelect, NTabs, NTabPane } from "naive-ui"`
2. Replace `<NTabs type="line" animated>` with `<div class="tabs" role="tablist">` + `<button class="tab-btn" role="tab" :aria-selected="activeTab === 'general'" @click="activeTab = 'general'">` for each tab
3. Add `const activeTab = ref<"general" | "marketplace">("general")` in `<script setup>`
4. Replace `<NTabPane name="general">` with `<div v-show="activeTab === 'general'" role="tabpanel">`
5. Replace `<NSelect>` with native `<select>` elements:
   - `<select :value="locale" @change="ui.setLocale(($event.target as HTMLSelectElement).value as SupportedLocale)">`
   - Each option: `<option v-for="opt in locales" :key="opt.value" :value="opt.value">{{ t(opt.labelKey) }}</option>`
6. Same pattern for theme select

- [ ] **Step 3: Commit**

```bash
git add src/views/SettingsView.vue
git commit -m "refactor(gui): replace NaiveUI tabs and selects in SettingsView"
```

---

### Task 11: Migrate SessionsSidebar.vue — useDialog → useConfirm

**Files:**

- Modify: `src/components/SessionsSidebar.vue`

- [ ] **Step 1: Read current file**

- [ ] **Step 2: Replace useDialog with useConfirm**

1. Remove `import { useDialog } from "naive-ui"` and `const dialog = useDialog()`
2. Add `import { useConfirm } from "@/composables/useConfirm"` and `const { confirm } = useConfirm()`
3. Replace `dialog.warning({ title, content, positiveText, negativeText, onPositiveClick })` with:
   ```ts
   const confirmed = await confirm({
     title,
     message: content,
     confirmText: positiveText,
     cancelText: negativeText,
     type: "warning"
   });
   if (confirmed) {
     /* original onPositiveClick body */
   }
   ```
4. Fix any hard-coded colors: replace hex values with `var(--app-*)` CSS variables

- [ ] **Step 3: Commit**

```bash
git add src/components/SessionsSidebar.vue
git commit -m "refactor(gui): migrate SessionsSidebar from NaiveUI, fix hard-coded colors"
```

---

### Task 12: Migrate ChatPanel.vue — remove NaiveUI types, fix input width

**Files:**

- Modify: `src/components/ChatPanel.vue`

- [ ] **Step 1: Read current file**

- [ ] **Step 2: Apply changes**

1. Remove `import { type ScrollbarInst } from "naive-ui"`
2. Replace `const scrollbar = ref<ScrollbarInst | null>(null)` with `const scrollbar = ref<HTMLElement | null>(null)`
3. Replace any `scrollbar.value?.scrollTo()` calls with `scrollbar.value?.scrollTo({ top: scrollbar.value.scrollHeight, behavior: 'smooth' })`
4. Fix input container width: ensure the chat input area uses `width: 100%` and doesn't overflow
5. Fix any hard-coded colors with CSS variable references

- [ ] **Step 3: Commit**

```bash
git add src/components/ChatPanel.vue
git commit -m "refactor(gui): migrate ChatPanel from NaiveUI, fix input width"
```

---

### Task 13: Migrate PermissionCenter + PermissionPrompt

**Files:**

- Modify: `src/components/PermissionCenter.vue`
- Modify: `src/components/PermissionPrompt.vue`

- [ ] **Step 1: Read both files**

- [ ] **Step 2: Migrate PermissionCenter**

Replace any NaiveUI components (`NButton`, `NCard`, etc.) with native HTML + CSS classes:

- `<NButton>` → `<button class="btn">`
- `<NCard>` → `<div class="card">`
- Remove any `import { ... } from "naive-ui"` lines

- [ ] **Step 3: Verify PermissionPrompt**

Confirm `PermissionPrompt.vue` has no NaiveUI imports or component usage. Based on research it's already NaiveUI-free — no changes needed.

- [ ] **Step 4: Commit**

```bash
git add src/components/PermissionCenter.vue src/components/PermissionPrompt.vue
git commit -m "refactor(gui): migrate permission components from NaiveUI"
```

---

### Task 14: Migrate TaskNode.vue

**Files:**

- Modify: `src/components/TaskNode.vue`

- [ ] **Step 1: Read current file**

- [ ] **Step 2: Replace NaiveUI components**

- `<NCard>` → `<div class="card">`
- `<NSpace>` → `<div style="display: flex; gap: 8px; align-items: center">`
- `<NTag type="xxx">` → `<span class="tag tag-xxx">`
- `<NDivider>` → `<hr class="divider">`
- Remove all NaiveUI imports

- [ ] **Step 3: Commit**

```bash
git add src/components/TaskNode.vue
git commit -m "refactor(gui): migrate TaskNode from NaiveUI"
```

---

### Task 15: Migrate MemoryBrowser.vue

**Files:**

- Modify: `src/components/MemoryBrowser.vue`

- [ ] **Step 1: Read current file**

- [ ] **Step 2: Apply changes**

1. Remove `import { useDialog, type SelectOption } from "naive-ui"`
2. Add `import { useConfirm } from "@/composables/useConfirm"`
3. Replace `useDialog()` confirm calls with `useConfirm().confirm()` (same pattern as Task 11)
4. Replace `SelectOption` type usage with inline `{ label: string; value: string }` type
5. Replace any NaiveUI components with native HTML + CSS classes

- [ ] **Step 3: Commit**

```bash
git add src/components/MemoryBrowser.vue
git commit -m "refactor(gui): migrate MemoryBrowser from NaiveUI"
```

---

### Task 16: Migrate McpServerManager.vue

**Files:**

- Modify: `src/components/McpServerManager.vue`

- [ ] **Step 1: Read current file**

- [ ] **Step 2: Replace all NaiveUI components with native HTML + CSS classes**

Apply standard replacements (NButton→button.btn, NCard→div.card, NTag→span.tag, etc.) and remove all NaiveUI imports.

- [ ] **Step 3: Commit**

```bash
git add src/components/McpServerManager.vue
git commit -m "refactor(gui): migrate McpServerManager from NaiveUI"
```

---

### Task 17: Migrate Marketplace components + fix catalog fetch

**Files:**

- Modify: `src/components/MarketplacePane.vue`
- Modify: `src/components/marketplace/CatalogList.vue`
- Modify: `src/components/marketplace/CatalogCard.vue`
- Modify: `src/components/marketplace/CatalogDetail.vue`
- Modify: `src/components/marketplace/InstalledList.vue`
- Modify: `src/components/marketplace/InstallProgress.vue`
- Modify: `src/components/CatalogSourcesSettings.vue`

- [ ] **Step 1: Read each file**

- [ ] **Step 2: Replace NaiveUI in all marketplace components**

For each file, apply standard replacements:

- Remove `import type { SelectOption } from "naive-ui"` → use `{ label: string; value: string }`
- Replace any NaiveUI components with native HTML + CSS classes
- Replace NaiveUI type references with plain TS types

- [ ] **Step 3: Fix marketplace data fetch**

In `MarketplacePane.vue`, ensure `onMounted` calls both `fetchSources()` AND `fetchCatalog()`:

```ts
onMounted(() => {
  void catalog.fetchSources();
  void catalog.fetchCatalog();
});
```

This fixes the issue where marketplace entries don't appear because `fetchCatalog()` was never called.

- [ ] **Step 4: Commit**

```bash
git add src/components/MarketplacePane.vue \
  src/components/marketplace/CatalogList.vue \
  src/components/marketplace/CatalogCard.vue \
  src/components/marketplace/CatalogDetail.vue \
  src/components/marketplace/InstalledList.vue \
  src/components/marketplace/InstallProgress.vue \
  src/components/CatalogSourcesSettings.vue
git commit -m "refactor(gui): migrate marketplace components from NaiveUI, fix catalog fetch"
```

---

## Phase 4: Cleanup (Tasks 18–22)

### Task 18: Remove NaiveUI from build config

**Files:**

- Modify: `vite.config.ts`
- Modify: `vitest.config.ts`
- Delete: `src/styles/naive-theme.ts`

- [ ] **Step 1: Read vite.config.ts**

- [ ] **Step 2: Remove NaiveUiResolver from vite.config.ts**

1. Remove the import line: `import { NaiveUiResolver } from "unplugin-vue-components/resolvers"`
2. Remove `NaiveUiResolver()` from the `resolvers` array in `Components({ resolvers: [...] })`
3. If the `resolvers` array becomes empty, remove the `resolvers` key entirely

- [ ] **Step 3: Read vitest.config.ts and apply same changes**

- [ ] **Step 4: Delete naive-theme.ts**

```bash
cd ~/AIProjects/kairox/.worktrees/feat-gui-polish-and-marketplace-fix/apps/agent-gui
git rm src/styles/naive-theme.ts
```

- [ ] **Step 5: Commit**

```bash
git add vite.config.ts vitest.config.ts
git commit -m "refactor(gui): remove NaiveUI from build configuration"
```

---

### Task 19: Update test-utils/mount.ts

**Files:**

- Modify: `src/test-utils/mount.ts`

- [ ] **Step 1: Read current mount.ts**

- [ ] **Step 2: Remove NaiveUI Provider harness**

1. Remove the NaiveUI imports:
   ```ts
   import {
     NConfigProvider,
     NMessageProvider,
     NDialogProvider,
     NNotificationProvider,
     NLoadingBarProvider
   } from "naive-ui";
   ```
2. Add confirm mock import:
   ```ts
   import { confirmDialogKey, type ConfirmAPI } from "@/composables/useConfirm";
   ```
3. Remove `wrapInNConfigProvider` and `withNaiveProviders` from `MountWithPluginsOptions` interface
4. Remove the `shouldWrap` logic and the `defineComponent("NaiveProviderHarness", ...)` block
5. Always use `comp` directly as the `target` (no wrapping)
6. Add a `confirmMock` to `global.provide`:
   ```ts
   const confirmMock: ConfirmAPI = { confirm: () => Promise.resolve(true) };
   ```
   Pass it via `mountOpts.global.provide = { [confirmDialogKey as symbol]: confirmMock, ...mountOpts.global?.provide }`

- [ ] **Step 3: Commit**

```bash
git add src/test-utils/mount.ts
git commit -m "refactor(gui): remove NaiveUI Provider harness from test utils"
```

---

### Task 20: Uninstall naive-ui dependency

**Files:**

- Modify: `package.json`
- Modify: `pnpm-lock.yaml`

- [ ] **Step 1: Uninstall naive-ui**

```bash
cd ~/AIProjects/kairox/.worktrees/feat-gui-polish-and-marketplace-fix/apps/agent-gui
pnpm remove naive-ui
```

- [ ] **Step 2: Verify no remaining references**

```bash
grep -rn "naive-ui" src/ vite.config.ts vitest.config.ts
```

Expected: zero results.

- [ ] **Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "chore(deps): remove naive-ui dependency"
```

---

### Task 21: Update AGENTS.md

**Files:**

- Modify: `AGENTS.md` (repo root)

- [ ] **Step 1: Read AGENTS.md**

Read the relevant sections that mention NaiveUI.

- [ ] **Step 2: Update documentation**

Update these sections in `AGENTS.md`:

1. **"UI library"** under TypeScript / Vue conventions: replace NaiveUI description with:

   > **UI approach**: No component library. Native HTML elements + shared CSS classes (`src/styles/components.css`). Theme via CSS custom properties (`src/styles/theme.css`) with `useDark()` from `@vueuse/core`. Self-built services: `useToast()` for notifications, `useConfirm()` for confirmation dialogs.

2. **"Auto-imports"** paragraph: remove mention of "NaiveUI components are auto-registered" and "useMessage/useDialog/useNotification/useLoadingBar are functions and must still be imported explicitly"

3. **"When modifying the GUI"** bullet: replace "Prefer NaiveUI components over hand-rolled markup; reach for `<NCard>`, `<NButton>`, `<NList>`, `<NModal>`, etc. before writing new CSS" with:

   > Use native HTML elements with shared CSS classes from `src/styles/components.css` (`.btn`, `.card`, `.tag`, `.alert`, etc.). Use `useToast()` for notifications and `useConfirm()` for confirmation dialogs. Use native `<dialog>` for modals.

4. **"Theme"** bullet: replace `src/styles/naive-theme.ts` reference with `src/styles/theme.css`

5. **Common pitfalls**: remove 5 NaiveUI-specific pitfalls:
   - "Don't reach for `useMessage()` outside a component..."
   - "Don't import NaiveUI components for templates..."
   - NaiveUI resolver mentions
     Add new pitfall:
     > **Don't use `useConfirm()` outside a component wrapped by `<ConfirmDialog>`** — it returns null and crashes. The provider lives in `AppLayout.vue`.

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs(gui): update AGENTS.md for NaiveUI removal"
```

---

### Task 22: Final verification

- [ ] **Step 1: Lint check**

```bash
cd ~/AIProjects/kairox/.worktrees/feat-gui-polish-and-marketplace-fix/apps/agent-gui
pnpm run lint 2>&1 | tail -20
```

Expected: zero errors.

- [ ] **Step 2: Run GUI tests**

```bash
cd ~/AIProjects/kairox/.worktrees/feat-gui-polish-and-marketplace-fix
just test-gui 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 3: Verify zero NaiveUI references**

```bash
cd ~/AIProjects/kairox/.worktrees/feat-gui-polish-and-marketplace-fix/apps/agent-gui
grep -rn "naive-ui" src/ vite.config.ts vitest.config.ts
grep -rn "NaiveUiResolver" .
pnpm ls naive-ui
```

All three commands should return empty/no results.

- [ ] **Step 4: Dev server smoke test**

```bash
cd ~/AIProjects/kairox/.worktrees/feat-gui-polish-and-marketplace-fix
just gui-dev
```

Manual checks:

- All pages render correctly
- Dark/light toggle works via Settings → Theme
- Toast notifications appear on actions
- Confirm dialogs appear for destructive actions (e.g., delete session)
- Marketplace tab shows catalog sources
- No console errors related to missing NaiveUI components
