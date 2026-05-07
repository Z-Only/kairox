# GUI Polish Phase 2 — Residual Display Fixes

**Date:** 2026-05-07
**Branch:** `feat/gui-polish-and-marketplace-fix`
**Type:** Bug fix + UX polish (incremental on top of the phase-1 spec)
**Parent spec:** `2026-05-07-gui-polish-and-marketplace-fix-design.md`

## 1. Problem Statement

After the phase-1 polish (Tasks 1–12 in the parent plan), seven residual display issues remain visible in the running app. Two user-provided screenshots confirm:

| #   | Issue                                         | Root cause                                                                                                                                                                                                                                                                                                        |
| --- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1  | **Nav links are unstyled blue underlines**    | `AppLayout.vue` renders `<RouterLink>` inside `<nav>` without any scoped CSS — the browser's default `<a>` styling leaks through                                                                                                                                                                                  |
| P2  | **Theme does not follow system dark mode**    | `main.css` hard-codes `background: #fff; color: #333` on `html, body, #app`, overriding NaiveUI's `bodyColor` token                                                                                                                                                                                               |
| P3  | **StatusBar floats above page bottom**        | `app-shell` in `AppLayout.vue` has no `display: flex; flex-direction: column; height: 100%` — the `WorkbenchView` grid doesn't fill the viewport, leaving whitespace below the status bar                                                                                                                         |
| P4  | **Settings "General" tab shows raw i18n key** | `SettingsView.vue` calls `t('settings.general')` but `en.json` has no `settings.general` key — falls back to the key string itself                                                                                                                                                                                |
| P5  | **StatusBar dot colors are wrong**            | `.dot-success`, `.dot-error`, `.dot-warning` all reference `--app-primary-color` instead of `--app-success-color`, `--app-error-color`, `--app-warning-color`                                                                                                                                                     |
| P6  | **SessionsSidebar hard-coded colors**         | 15+ hard-coded hex values (`#d7d7d7`, `#0077cc`, `#22a06b`, `#f0f4f8`, `#e1ecf7`, `#999`, `white`, `#777`, `#666`, etc.) break in dark mode                                                                                                                                                                       |
| P7  | **AppLayout missing extended `--app-*` vars** | Only 5 CSS custom properties are exposed; `--app-text-color-2`, `--app-text-color-3`, `--app-success-color`, `--app-warning-color`, `--app-error-color`, `--app-info-color`, `--app-hover-color`, `--app-code-bg` are all missing, causing components that reference them to fall through to hard-coded fallbacks |

## 2. Goals

- All seven issues fixed in a single commit batch.
- No regressions in existing Vitest / Playwright tests.
- Dark mode works correctly when system preference is dark.
- All component surfaces use `--app-*` CSS variables exclusively — zero remaining hard-coded color hex values in the touched files.

## 3. Non-Goals

- No new features or architectural changes.
- No changes to Rust backend or Tauri commands.
- No i18n locale additions beyond the missing `settings.general` key.

## 4. Design

### 4.1 AppLayout — nav styling + full-height flex + extended vars (P1, P3, P7)

**Nav styling:** Add scoped CSS for `.app-nav` with horizontal flex, gap, padding, bottom border using `--app-border-color`. Style `<a>` (RouterLink renders as `<a>`) with `text-decoration: none`, `color: var(--app-text-color)`, and an `.router-link-active` highlight using `--app-primary-color`.

**Full-height flex:** Add `display: flex; flex-direction: column; height: 100%` to `.app-shell` so `<RouterView>` (which renders WorkbenchView or SettingsView) fills the remaining space via `flex: 1; overflow: hidden`.

**Extended vars:** Expand the `:style` binding on `.app-shell` to expose all theme tokens:

```
--app-body-color, --app-card-color, --app-border-color,
--app-text-color, --app-text-color-2, --app-text-color-3,
--app-primary-color, --app-success-color, --app-warning-color,
--app-error-color, --app-info-color, --app-hover-color, --app-code-bg
```

`--app-code-bg` maps to `themeVars.codeColor` (NaiveUI's code background token).

### 4.2 main.css — remove hard-coded colors (P2)

Replace `color: #333; background: #fff;` with `color: var(--app-text-color, #333); background: var(--app-body-color, #fff);`. The CSS variables are set by `AppLayout.vue` on `.app-shell`, which wraps the entire app content. The fallback values ensure the page doesn't flash unstyled on first paint before Vue mounts.

### 4.3 WorkbenchView — grid row constraint (P3)

Add `grid-template-rows: 1fr auto;` so the main content occupies all available space and StatusBar sits at the bottom. StatusBar spans all 3 columns: `grid-column: 1 / -1;`. Replace hard-coded `border-left: 1px solid #d7d7d7` with `var(--app-border-color)`.

### 4.4 Settings i18n key (P4)

Add `"general": "General"` to `en.json` under `settings`, and `"general": "通用"` to `zh-CN.json`.

### 4.5 StatusBar dot fix (P5)

Replace `.dot-success { background: var(--app-primary-color, #52c41a); }` with `background: var(--app-success-color, #52c41a);`. Same for `.dot-error` → `--app-error-color` and `.dot-warning` → `--app-warning-color`.

### 4.6 SessionsSidebar theme tokens (P6)

Replace all hard-coded hex values with `--app-*` CSS variables:

| Hard-coded                           | Replacement                                            |
| ------------------------------------ | ------------------------------------------------------ |
| `#d7d7d7` (borders)                  | `var(--app-border-color)`                              |
| `#0077cc` (accent/buttons)           | `var(--app-primary-color)`                             |
| `#22a06b` (indicator)                | `var(--app-success-color)`                             |
| `#f0f4f8` (hover)                    | `var(--app-hover-color)`                               |
| `#e1ecf7` (active)                   | `var(--app-primary-color)` with 15% opacity            |
| `#999` (empty hint)                  | `var(--app-text-color-3)`                              |
| `white` (backgrounds)                | `var(--app-card-color)`                                |
| `#777` (caret)                       | `var(--app-text-color-3)`                              |
| `#666` (detail text)                 | `var(--app-text-color-2)`                              |
| `rgba(0,0,0,0.08)` (hover)           | `var(--app-hover-color)`                               |
| `rgba(0,0,0,0.15)` (shadow)          | keep as-is (opacity-based shadows work in both themes) |
| `rgba(204,51,51,0.1)` (delete hover) | keep as-is                                             |

### 4.7 SettingsView flex layout

Add `flex: 1; overflow: auto;` to `.settings` so it fills the remaining space under the nav.

## 5. File Inventory

| File                                                | Changes                                           |
| --------------------------------------------------- | ------------------------------------------------- |
| `apps/agent-gui/src/layouts/AppLayout.vue`          | Nav styling, flex layout, extended `--app-*` vars |
| `apps/agent-gui/src/assets/main.css`                | Remove hard-coded `color`/`background`            |
| `apps/agent-gui/src/views/WorkbenchView.vue`        | Grid row constraint, border var                   |
| `apps/agent-gui/src/views/SettingsView.vue`         | Add flex layout                                   |
| `apps/agent-gui/src/components/StatusBar.vue`       | Fix dot color vars                                |
| `apps/agent-gui/src/components/SessionsSidebar.vue` | Replace all hard-coded colors                     |
| `apps/agent-gui/src/locales/en.json`                | Add `settings.general` key                        |
| `apps/agent-gui/src/locales/zh-CN.json`             | Add `settings.general` key                        |

## 6. Testing Strategy

- **Vitest**: Existing component tests should pass without changes (no API/behavior changes).
- **Lint**: `pnpm run lint` must pass.
- **Manual smoke**: Switch system theme to dark, confirm all surfaces follow.
