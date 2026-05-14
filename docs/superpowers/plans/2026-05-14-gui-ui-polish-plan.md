# GUI UI Polish — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish agent-gui UI with improved color contrast, typography, component consistency, empty states, accessibility, and micro-interactions — all CSS-level changes, no behavior modifications.

**Architecture:** Foundation tasks (theme tokens, typography, shared CSS) land first. Component tasks are independent of each other and can run in parallel after foundation. Each task modifies only its target file(s) and is self-contained.

**Tech Stack:** Vue 3 SFC with scoped CSS, CSS custom properties, no CSS framework.

---

### Task 1: Theme color tokens + radius + type scale (`theme.css`)

**Files:**

- Modify: `apps/agent-gui/src/styles/theme.css`

- [ ] **Step 1: Update theme.css with new color tokens, radius tokens, and type scale**

Read the current file at `apps/agent-gui/src/styles/theme.css`. Replace its entire content with:

```css
/* theme.css — Light/dark theme via CSS custom properties.
 * Toggled by @vueuse/core's useDark() which adds/removes html.dark.
 */

:root {
  /* === Surface === */
  --app-body-color: #ffffff;
  --app-card-color: #f9fafb;
  --app-border-color: #e5e7eb;
  --app-hover-color: #f3f4f6;
  --app-muted-surface-color: #f3f4f6;

  /* === Text === */
  --app-text-color: #1f2937;
  --app-text-color-2: #475569;
  --app-text-color-3: #64748b;
  --app-muted-text-color: #475569;

  /* === Semantic === */
  --app-primary-color: #3b82f6;
  --app-primary-contrast-color: #fff;
  --app-success-color: #16a34a;
  --app-warning-color: #f59e0b;
  --app-error-color: #ef4444;
  --app-info-color: #3b82f6;

  /* === Code === */
  --app-code-bg: #f1f5f9;

  /* === Radius === */
  --app-radius-sm: 4px;
  --app-radius-md: 6px;
  --app-radius-lg: 8px;
  --app-radius-xl: 12px;

  /* === Type scale === */
  --app-text-xs: 11px;
  --app-text-sm: 12px;
  --app-text-base: 13px;
  --app-text-lg: 14px;
  --app-text-xl: 16px;

  /* Context meter — per-source colours (light) */
  --src-system: #64748b;
  --src-tools: #2563eb;
  --src-memory: #7c3aed;
  --src-history: #16a34a;
  --src-tool-result: #f59e0b;
  --src-selected-file: #db2777;
  --src-compaction-summary: #94a3b8;
  --src-request: #0ea5e9;
}

html.dark {
  /* === Surface === */
  --app-body-color: #0f172a;
  --app-card-color: #1e293b;
  --app-border-color: #334155;
  --app-hover-color: #334155;
  --app-muted-surface-color: #1e293b;

  /* === Text === */
  --app-text-color: #f0f4f8;
  --app-text-color-2: #8896ab;
  --app-text-color-3: #7c8da2;
  --app-muted-text-color: #8896ab;

  /* === Semantic === */
  --app-primary-color: #60a5fa;
  --app-primary-contrast-color: #0f172a;
  --app-success-color: #22c55e;
  --app-warning-color: #fbbf24;
  --app-error-color: #f87171;
  --app-info-color: #60a5fa;

  /* === Code === */
  --app-code-bg: #0f172a;

  /* === Radius (same as light) === */
  --app-radius-sm: 4px;
  --app-radius-md: 6px;
  --app-radius-lg: 8px;
  --app-radius-xl: 12px;

  /* === Type scale (same as light) === */
  --app-text-xs: 11px;
  --app-text-sm: 12px;
  --app-text-base: 13px;
  --app-text-lg: 14px;
  --app-text-xl: 16px;

  /* Context meter — per-source colours (dark, higher contrast) */
  --src-system: #94a3b8;
  --src-tools: #60a5fa;
  --src-memory: #a78bfa;
  --src-history: #4ade80;
  --src-tool-result: #fbbf24;
  --src-selected-file: #f472b6;
  --src-compaction-summary: #64748b;
  --src-request: #38bdf8;
}
```

- [ ] **Step 2: Verify no typos**

```bash
grep -c 'var(--' apps/agent-gui/src/styles/theme.css
```

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/styles/theme.css
git commit -m "feat(gui): refine theme color tokens, add radius and type scale tokens"
```

---

### Task 2: Typography — fonts + body styles

**Files:**

- Modify: `apps/agent-gui/index.html`
- Modify: `apps/agent-gui/src/App.vue`

- [ ] **Step 1: Add Google Fonts link to index.html**

Read `apps/agent-gui/index.html` first. Add the Google Fonts `<link>` inside `<head>`, before the existing `<link>` for stylesheet:

```html
<link rel="preconnect" href="https://fonts.googleapis.com" />
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
<link
  href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600;700&display=swap"
  rel="stylesheet"
/>
```

- [ ] **Step 2: Apply font-family in App.vue**

Read `apps/agent-gui/src/App.vue` first. In the `<style>` section (not scoped, or add an unscoped `<style>` block), apply body font. If there's already a global style block, add to it. Otherwise add a new unscoped `<style>` block at the end:

```css
body {
  font-family:
    "IBM Plex Sans",
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    system-ui,
    sans-serif;
  line-height: 1.5;
}

code,
pre,
.markdown-body code,
.markdown-body pre {
  font-family: "JetBrains Mono", "SF Mono", "Fira Code", "Cascadia Code", monospace;
}

h1,
h2,
h3,
h4,
h5,
h6 {
  line-height: 1.3;
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/index.html apps/agent-gui/src/App.vue
git commit -m "feat(gui): add IBM Plex Sans + JetBrains Mono font stack"
```

---

### Task 3: Shared components.css polish

**Files:**

- Modify: `apps/agent-gui/src/styles/components.css`

- [ ] **Step 1: Replace button hover effects (remove layout shift)**

Find `.btn-primary:hover` and `.btn-danger:hover` in `components.css`. Replace `transform: translateY(-1px)` with `filter: brightness(1.1)`:

Current `.btn-primary:hover` has:

```css
.btn-primary:hover {
  background: color-mix(in srgb, var(--app-primary-color) 85%, #000);
  border-color: color-mix(in srgb, var(--app-primary-color) 85%, #000);
  transform: translateY(-1px);
  box-shadow: 0 2px 8px color-mix(in srgb, var(--app-primary-color) 30%, transparent);
}
```

Change to:

```css
.btn-primary:hover {
  background: color-mix(in srgb, var(--app-primary-color) 85%, #000);
  border-color: color-mix(in srgb, var(--app-primary-color) 85%, #000);
  filter: brightness(1.1);
  box-shadow: 0 2px 8px color-mix(in srgb, var(--app-primary-color) 30%, transparent);
}
```

Same for `.btn-danger:hover`: replace `transform: translateY(-1px)` with `filter: brightness(1.1)`.

- [ ] **Step 2: Add webkit scrollbar styling**

Append to the end of `components.css`:

```css
/* === Scrollbar === */
::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: var(--app-border-color);
  border-radius: 3px;
}

::-webkit-scrollbar-thumb:hover {
  background: var(--app-text-color-3);
}
```

- [ ] **Step 3: Add cursor:pointer to dropdown items and interactive elements**

In `.kx-dropdown-item`, add `cursor: pointer`. In `.kx-icon-button`, ensure `cursor: pointer` is present. In `.tab-btn`, add `cursor: pointer`.

- [ ] **Step 4: Wrap transitions in reduced-motion media query**

Wrap ALL `transition` properties in `.btn`, `.btn:hover`, `.tab-btn`, `.tab-btn:hover`, `.kx-icon-button`, `.kx-dropdown-item` in:

```css
@media (prefers-reduced-motion: no-preference) {
  /* transition declarations */
}
```

Example for `.btn`:

```css
.btn {
  /* ... other properties, keep transition out */
}

@media (prefers-reduced-motion: no-preference) {
  .btn {
    transition:
      background 0.15s,
      border-color 0.15s,
      color 0.15s;
  }
}
```

Apply same pattern to: `.btn:hover`, `.tab-btn`, `.tab-btn:hover`, `.kx-icon-button`, `.kx-icon-button:hover`, `.kx-dropdown-item`, `.kx-dropdown-item:hover`.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/styles/components.css
git commit -m "feat(gui): polish buttons, scrollbar, cursor pointers, reduced-motion"
```

---

### Task 4: AppLayout nav polish + SettingsLayout tabs

**Files:**

- Modify: `apps/agent-gui/src/layouts/AppLayout.vue`
- Modify: `apps/agent-gui/src/layouts/SettingsLayout.vue`

- [ ] **Step 1: AppLayout nav — richer active state, cursor polish**

Read `apps/agent-gui/src/layouts/AppLayout.vue`. In the scoped `<style>` block:

- Change `.app-nav a` padding from `4px 8px` to `6px 12px`
- Change `.app-nav a:hover` to remove `transform: translateY(-1px)`
- Change `.app-nav a.router-link-active` to use `--app-radius-md` for border-radius
- Wrap transitions in `@media (prefers-reduced-motion: no-preference)`

Updated `.app-nav a` and `.app-nav a:hover` styles:

```css
.app-nav a {
  text-decoration: none;
  color: var(--app-text-color-2);
  padding: 6px 12px;
  border-radius: var(--app-radius-md);
  cursor: pointer;
}
@media (prefers-reduced-motion: no-preference) {
  .app-nav a {
    transition:
      color 0.2s,
      background 0.2s;
  }
}
.app-nav a:hover {
  color: var(--app-text-color);
  background: var(--app-hover-color);
}
.app-nav a.router-link-active {
  color: var(--app-primary-color);
  font-weight: 600;
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}
```

- [ ] **Step 2: SettingsLayout tabs — larger active indicator, better gap**

Read `apps/agent-gui/src/layouts/SettingsLayout.vue`. In the scoped `<style>` block:

- Change `.tabs` gap from `4px` to `8px`
- Change `.tab-btn[aria-selected="true"]` border-bottom from `2px` to `3px`
- Change `.tab-btn` border-radius to use `var(--app-radius-md) var(--app-radius-md) 0 0`
- Wrap transitions in reduced-motion

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/layouts/AppLayout.vue apps/agent-gui/src/layouts/SettingsLayout.vue
git commit -m "feat(gui): polish AppLayout nav and SettingsLayout tabs"
```

---

### Task 5: ChatPanel polish

**Files:**

- Modify: `apps/agent-gui/src/components/ChatPanel.vue`

- [ ] **Step 1: Change message bubble radius from 16px to 12px**

Read `apps/agent-gui/src/components/ChatPanel.vue`. In the scoped style, find `.message-content` and change `border-radius: 16px` to `border-radius: var(--app-radius-xl)` (12px).

- [ ] **Step 2: Add empty state for chat**

In the template, inside `.message-list-inner`, the empty state is already handled by the `data-test="chat-empty-state"` conditional. Add a visual empty state using the `.empty-state` class from `components.css`. Find the `message-list-inner` div and ensure it shows the empty state when there are no messages and no token stream:

```html
<div
  v-if="session.projection.messages.length === 0 && !session.projection.token_stream"
  class="empty-state"
  data-test="chat-empty-state"
>
  <svg
    width="48"
    height="48"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    aria-hidden="true"
  >
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
  </svg>
  <p>{{ t("chat.emptyState") }}</p>
</div>
```

This replaces the current invisible state (the `data-test` attribute on the inner div). Add the i18n key `chat.emptyState` with value "Start a conversation" (EN) and "开始对话" (zh-CN) in the locale files.

- [ ] **Step 3: Add model popover fade-in animation**

In the scoped style, add animation for the popover panel:

```css
@media (prefers-reduced-motion: no-preference) {
  .chat-model-popover-panel {
    animation: popover-in 0.15s ease;
  }
}
@keyframes popover-in {
  from {
    opacity: 0;
    transform: scale(0.97);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
```

- [ ] **Step 4: Improve input focus ring consistency**

Find `.message-input:focus` and change to:

```css
.message-input:focus {
  border-color: var(--app-primary-color);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--app-primary-color) 25%, transparent);
}
```

- [ ] **Step 5: Wrap scoped transitions in reduced-motion**

Wrap the `transition` properties in `.message-content`, `.chat-model-trigger`, `.chat-model-option`, `.attachment-remove:hover` etc. in `@media (prefers-reduced-motion: no-preference)`.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/components/ChatPanel.vue apps/agent-gui/src/locales/en.json apps/agent-gui/src/locales/zh-CN.json
git commit -m "feat(gui): polish ChatPanel bubbles, empty state, popover animation"
```

---

### Task 6: SessionsSidebar polish

**Files:**

- Modify: `apps/agent-gui/src/components/SessionsSidebar.vue`

- [ ] **Step 1: Add transitions to session/project list items**

Read `apps/agent-gui/src/components/SessionsSidebar.vue`. Find the scoped `<style>` block. Add transition to `.session-item` or equivalent clickable rows:

```css
@media (prefers-reduced-motion: no-preference) {
  .session-item,
  .project-title-btn {
    transition:
      background 0.15s,
      color 0.15s;
  }
}
```

Exact class names depend on what's in the template. Search for clickable row elements and add transitions.

- [ ] **Step 2: Add cursor:pointer to interactive elements**

Ensure these elements have `cursor: pointer`:

- `.project-expand-btn`
- `.project-title-btn`
- `.session-item` (or whatever the session row class is)
- All buttons inside the sidebar

- [ ] **Step 3: Add empty state for sessions**

Find where sessions are listed. When there are no sessions/projects, show:

```html
<div
  v-if="projects.activeProjects.length === 0 && session.sessions.length === 0"
  class="empty-state"
  style="padding: 24px 12px;"
>
  <p style="color: var(--app-text-color-3); font-size: var(--app-text-sm); text-align: center;">
    {{ t("sessions.emptyState") }}
  </p>
</div>
```

Add i18n key `sessions.emptyState` = "No sessions yet" (EN) / "暂无会话" (zh-CN).

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/components/SessionsSidebar.vue apps/agent-gui/src/locales/en.json apps/agent-gui/src/locales/zh-CN.json
git commit -m "feat(gui): polish SessionsSidebar transitions, cursors, empty state"
```

---

### Task 7: ToastContainer animation

**Files:**

- Modify: `apps/agent-gui/src/components/ToastContainer.vue`

- [ ] **Step 1: Add enter/exit animation to toasts**

Read `apps/agent-gui/src/components/ToastContainer.vue`. Add animation styles in the scoped `<style>` block:

```css
@media (prefers-reduced-motion: no-preference) {
  .toast {
    animation: toast-in 0.25s ease;
  }
}
@keyframes toast-in {
  from {
    opacity: 0;
    transform: translateX(16px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
```

The toast element class may be named differently — check the template for the actual class applied to toast items.

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/ToastContainer.vue
git commit -m "feat(gui): add slide-in animation to toast notifications"
```

---

### Task 8: StatusBar + ContextMeter polish

**Files:**

- Modify: `apps/agent-gui/src/components/StatusBar.vue`
- Modify: `apps/agent-gui/src/components/ContextMeter.vue`

- [ ] **Step 1: StatusBar — minor spacing and font polish**

Read `apps/agent-gui/src/components/StatusBar.vue`. In the scoped style:

- Change `.status-bar` font-size to `var(--app-text-xs)`
- Change `.status-label` opacity from `0.7` to `0.8`
- Ensure status items have consistent gap

- [ ] **Step 2: ContextMeter — focus ring and cursor**

Read `apps/agent-gui/src/components/ContextMeter.vue`. Ensure the clickable ring element has:

- `cursor: pointer`
- `:focus-visible` with `outline: 2px solid var(--app-primary-color); outline-offset: 2px;`

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/components/StatusBar.vue apps/agent-gui/src/components/ContextMeter.vue
git commit -m "feat(gui): polish StatusBar typography and ContextMeter accessibility"
```

---

### Task 9: Settings panes polish

**Files:**

- Modify: `apps/agent-gui/src/views/settings/GeneralSettings.vue`
- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue`
- Modify: `apps/agent-gui/src/components/McpSettingsPane.vue`
- Modify: `apps/agent-gui/src/components/SkillSettingsPane.vue`
- Modify: `apps/agent-gui/src/components/ArchiveSettingsPane.vue`

- [ ] **Step 1: GeneralSettings — row dividers**

Read `apps/agent-gui/src/views/settings/GeneralSettings.vue`. Add `border-bottom: 1px solid var(--app-border-color)` to `.settings__row`:

```css
.settings__row {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-block: 0;
  padding: 12px 0;
  border-bottom: 1px solid var(--app-border-color);
}
.settings__row:last-child {
  border-bottom: none;
}
```

- [ ] **Step 2: ModelSettingsPane — card padding consistency**

Read `apps/agent-gui/src/components/ModelSettingsPane.vue`. Ensure consistent `padding: 16px` on card-like containers. Use `var(--app-radius-lg)` for border-radius. Add focus rings to interactive elements.

- [ ] **Step 3: McpSettingsPane — card padding consistency**

Read `apps/agent-gui/src/components/McpSettingsPane.vue`. Apply same padding/border-radius consistency.

- [ ] **Step 4: SkillSettingsPane — card padding consistency**

Read `apps/agent-gui/src/components/SkillSettingsPane.vue`. Apply same padding/border-radius consistency.

- [ ] **Step 5: ArchiveSettingsPane — card padding consistency**

Read `apps/agent-gui/src/components/ArchiveSettingsPane.vue`. Apply same padding/border-radius consistency.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/views/settings/GeneralSettings.vue apps/agent-gui/src/components/ModelSettingsPane.vue apps/agent-gui/src/components/McpSettingsPane.vue apps/agent-gui/src/components/SkillSettingsPane.vue apps/agent-gui/src/components/ArchiveSettingsPane.vue
git commit -m "feat(gui): polish settings panes with consistent padding and row dividers"
```

---

### Task 10: Verification — lint, format, tests, visual check

- [ ] **Step 1: Run lint**

```bash
pnpm run lint
```

Expected: PASS (no new errors)

- [ ] **Step 2: Run format check**

```bash
pnpm run format:check
```

Expected: PASS

- [ ] **Step 3: Run Rust tests**

```bash
cargo test --workspace --all-targets
```

Expected: PASS

- [ ] **Step 4: Run GUI Vitest tests**

```bash
pnpm --filter agent-gui run test
```

Expected: PASS

- [ ] **Step 5: Launch GUI for visual check**

```bash
pnpm --filter agent-gui run tauri dev
```

Manually verify:

- [ ] Light mode: text contrast improved, colors correct
- [ ] Dark mode: slate tones visible, borders visible
- [ ] Nav: active state prominent, hover smooth
- [ ] Settings tabs: 3px active indicator visible
- [ ] Chat: message bubbles 12px radius, empty state shows
- [ ] Sidebar: transitions smooth, cursor pointers correct
- [ ] Toast: slide-in animation plays
- [ ] Scrollbar: styled thin scrollbar appears
- [ ] Focus rings: visible on Tab through UI
- [ ] Reduced motion: enable in OS settings, animations should stop

- [ ] **Step 6: Commit any final fixes**

```bash
git add -A
git commit -m "chore(gui): final polish fixes from visual review"
```
