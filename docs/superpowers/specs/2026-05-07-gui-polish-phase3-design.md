# GUI Polish Phase 3 — Screenshot-Driven Residual Fixes

**Date:** 2026-05-07
**Branch:** `feat/gui-polish-and-marketplace-fix`
**Type:** Bug fix + UX polish (incremental on top of phase-2 spec)
**Parent spec:** `2026-05-07-gui-polish-phase2-design.md`

## 1. Problem Statement

After phase-2 fixes, six residual display issues remain visible in user-provided screenshots.

| #   | Issue                                             | Severity | Root cause                                                                                                    |
| --- | ------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------- |
| P1  | **Theme does not follow system dark mode**        | 🔴 High  | `AppLayout.vue` lacks `<NGlobalStyle />` — NaiveUI cannot sync `body` bg/color to active theme                |
| P2  | **StatusBar floats above page bottom**            | 🔴 High  | Parent scoped `.status-bar { grid-column: 1 / -1 }` cannot reach child's scoped `.status-bar` class           |
| P3  | **Input textarea too narrow**                     | 🟡 Med   | `NSpace` wrapping constrains `NInput`; textarea does not fill available width                                 |
| P4  | **Marketplace tab completely blank**              | 🔴 High  | `SettingsView` uses `<RouterView />` inside `NTabPane` but tab activation doesn't navigate to the child route |
| P5  | **Marketplace lacks source management + catalog** | 🟡 Med   | `MarketplacePane.onMounted` calls `fetchSources()` but never `fetchCatalog()`; empty state not obvious        |
| P6  | **[cancelled] marker too bulky**                  | 🟢 Low   | `NAlert type="warning"` is oversized for an inline status indicator                                           |

## 2. Design

### 2.1 P1 — Add `<NGlobalStyle />` for system theme sync

NaiveUI's `NGlobalStyle` component (placed inside `NConfigProvider`) automatically sets `document.body.style.backgroundColor` and `document.body.style.color` to match the active theme's `bodyColor` and `textColorBase`. Without it, `body` retains the CSS fallback from `main.css`.

**Change:** Add `<NGlobalStyle />` as a sibling of `.app-shell` inside `NNotificationProvider`.

This also means `main.css`'s `color` and `background` declarations on `html, body, #app` become true fallbacks (pre-mount only), since `NGlobalStyle` takes over once Vue mounts.

### 2.2 P2 — Fix StatusBar grid placement

The selector `.status-bar { grid-column: 1 / -1; }` in `WorkbenchView.vue`'s scoped CSS targets a class name that exists inside `StatusBar.vue`'s own scoped styles — Vue's scoped attribute hashing prevents the match.

**Change:** Use `:deep(.status-bar)` in `WorkbenchView.vue` to pierce the child component's scope boundary.

### 2.3 P3 — Widen input textarea

The current layout uses `NSpace` with `:wrap="false"` and `align="end"`. The `NInput` has `class="message-input"` with `flex: 1`, but `NSpace` doesn't propagate flex behavior to children by default.

**Change:** Replace `NSpace` with a plain `<div class="input-row">` using `display: flex; gap: 8px; align-items: flex-end;` so that `NInput` with `flex: 1` correctly fills remaining width.

### 2.4 P4 — Fix Marketplace tab blank

The root cause: `SettingsView.vue` places `<RouterView />` inside the Marketplace `NTabPane`. When the user clicks the "Marketplace" tab, NaiveUI activates the tab pane but vue-router doesn't navigate to `/settings/marketplace` — the `<RouterView />` has no matched child route and renders nothing.

**Change:** Remove the `<RouterView />` approach. Instead, directly render `<MarketplacePane />` inside the Marketplace tab pane. Remove the `settings-marketplace` child route from `routes.ts` (keep the legacy `/marketplace` redirect pointing to `settings`). This eliminates the router ↔ tab sync complexity.

### 2.5 P5 — Fetch catalog on mount + visible empty state

`MarketplacePane.onMounted` calls `fetchSources()` but never `fetchCatalog()`. The catalog list stays empty.

**Change:** Add `void catalog.fetchCatalog()` to `onMounted` alongside the existing `fetchSources()` call.

### 2.6 P6 — Compact cancelled marker

Replace the full-width `NAlert type="warning"` with a compact inline `NTag type="warning"` that uses the same `[cancelled]` text but takes minimal space.

## 3. File Inventory

| File                                                | Changes                                          |
| --------------------------------------------------- | ------------------------------------------------ |
| `apps/agent-gui/src/layouts/AppLayout.vue`          | Add `<NGlobalStyle />`                           |
| `apps/agent-gui/src/views/WorkbenchView.vue`        | `:deep(.status-bar)` for grid placement          |
| `apps/agent-gui/src/components/ChatPanel.vue`       | Replace `NSpace` with flex div in input area     |
| `apps/agent-gui/src/views/SettingsView.vue`         | Inline `MarketplacePane`, remove `RouterView`    |
| `apps/agent-gui/src/components/MarketplacePane.vue` | Add `fetchCatalog()` to `onMounted`              |
| `apps/agent-gui/src/components/ChatPanel.vue`       | Replace `NAlert` with `NTag` for cancelled state |
| `apps/agent-gui/src/router/routes.ts`               | Remove `settings-marketplace` child route        |

## 4. Testing Strategy

- **Vitest**: Run `just test-gui` — existing tests should pass.
- **Lint**: `pnpm run lint` must pass.
- **Manual**: Verify dark mode follows system, StatusBar sticks to bottom, input fills width, Marketplace shows catalog/sources.
