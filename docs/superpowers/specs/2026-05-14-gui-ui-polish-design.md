# GUI UI Polish — Design Spec

**Date**: 2026-05-14
**Branch**: `feat/gui-ui-polish`
**Scope**: CSS-level UI polish for agent-gui (Tauri 2 + Vue 3 + TypeScript). No behavior changes.

## Design System (from ui-ux-pro-max)

- **Style**: Professional developer tool — dark/light, precise, functional
- **Colors**: Slate-based dark palette, blue primary (#3b82f6 light / #60a5fa dark), green CTA (#16a34a)
- **Typography**: IBM Plex Sans (body), JetBrains Mono (code). System font fallback.
- **Effects**: 150-300ms transitions, no layout-shifting hovers, `prefers-reduced-motion` respected

## Category 1: Theme & Color Refinement

### 1.1 Light mode contrast fixes

Current `--app-text-color-2: #6b7280` (gray-500) has ~3.4:1 contrast on white — fails WCAG AA.

| Token                    | Current   | New       | Ratio |
| ------------------------ | --------- | --------- | ----- |
| `--app-text-color-2`     | `#6b7280` | `#475569` | 4.7:1 |
| `--app-text-color-3`     | `#9ca3af` | `#64748b` | 4.5:1 |
| `--app-muted-text-color` | `#6b7280` | `#475569` | 4.7:1 |

### 1.2 Dark mode palette adjustment

Shift from near-black to richer slate tones (matches design system "Code dark" recommendation).

| Token                       | Current   | New       |
| --------------------------- | --------- | --------- |
| `--app-body-color`          | `#0b0f19` | `#0f172a` |
| `--app-card-color`          | `#141b2d` | `#1e293b` |
| `--app-hover-color`         | `#1a2235` | `#334155` |
| `--app-muted-surface-color` | `#1a2235` | `#1e293b` |

### 1.3 Success color

| Token                         | Current   | New       |
| ----------------------------- | --------- | --------- |
| `--app-success-color` (light) | `#22c55e` | `#16a34a` |
| `--app-success-color` (dark)  | `#4ade80` | `#22c55e` |

### 1.4 New semantic radius tokens

Add to `theme.css`:

- `--app-radius-sm: 4px`
- `--app-radius-md: 6px`
- `--app-radius-lg: 8px`
- `--app-radius-xl: 12px`

## Category 2: Typography

### 2.1 Font stack

Add Google Fonts import for IBM Plex Sans + JetBrains Mono in `index.html`. Apply to `body`:

```css
body {
  font-family:
    "IBM Plex Sans",
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    system-ui,
    sans-serif;
}

code,
pre,
.markdown-body code {
  font-family: "JetBrains Mono", "SF Mono", "Fira Code", "Cascadia Code", monospace;
}
```

### 2.2 Type scale tokens

Add to `theme.css`:

- `--app-text-xs: 11px`
- `--app-text-sm: 12px`
- `--app-text-base: 13px`
- `--app-text-lg: 14px`
- `--app-text-xl: 16px`

### 2.3 Line-height

Body text `1.5`, headings `1.3`, code blocks `1.5`.

## Category 3: Component Polish

### 3.1 Buttons — no layout shift on hover

Replace `transform: translateY(-1px)` in `.btn-primary:hover` and `.btn-danger:hover` with:

```css
.btn-primary:hover {
  box-shadow: 0 2px 8px color-mix(in srgb, var(--app-primary-color) 30%, transparent);
  filter: brightness(1.1);
}
```

### 3.2 Message bubbles — unify radius

Change `.message-content` border-radius from `16px` to `12px` (consistent with `--app-radius-xl`).

### 3.3 Chat model trigger

Refine pill button: better horizontal padding (`4px 12px`), consistent focus ring. Already mostly good.

### 3.4 Tabs

SettingsLayout tabs: active indicator `2px → 3px` for prominence. Better `gap: 8px` (was `4px`).

### 3.5 Cards / settings panes

All settings panes: ensure consistent `padding: 16px`, add subtle `background` on hover for interactive cards.

### 3.6 Inputs & selects

Unify `border-radius: var(--app-radius-md)` (6px). Add consistent `box-shadow` focus ring (`0 0 0 2px color-mix(in srgb, var(--app-primary-color) 25%, transparent)`).

### 3.7 Scrollbar styling

Add webkit scrollbar styling to `components.css`:

```css
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

## Category 4: Empty States & Visual Polish

### 4.1 Empty chat state

Currently: blank area. Add centered icon + "Start a conversation" text using existing `.empty-state` class.

### 4.2 Empty session list

Add helper text when no sessions exist: "No sessions yet. Create one to start."

### 4.3 Settings rows

Add subtle `border-bottom: 1px solid var(--app-border-color)` between settings rows in GeneralSettings.

## Category 5: Accessibility

### 5.1 Focus rings

Audit all interactive elements. Ensure every button, input, select, and clickable element has:

```css
:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
```

### 5.2 cursor:pointer

Audit: dropdown items, clickable rows, expand buttons, model options. Add `cursor: pointer` where missing.

### 5.3 Reduced motion

Wrap all `transition` declarations:

```css
@media (prefers-reduced-motion: no-preference) {
  .element {
    transition: ...;
  }
}
```

For elements that already have transitions in scoped styles, add the media query wrapper. For `components.css` shared classes, wrap the transition properties.

## Category 6: Micro-interactions

### 6.1 Sidebar items

Add `transition: background 0.15s, color 0.15s` to session list items (already partially done, audit gaps).

### 6.2 Toast enter/exit

Add slide-in + fade animation to ToastContainer toasts:

```css
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

### 6.3 Model popover

Add `animation: fade-in 0.15s ease` to popover content appearance (already has visibility toggle, add subtle scale/opacity).

## Files Changed

| File                                                    | Changes                                                          |
| ------------------------------------------------------- | ---------------------------------------------------------------- |
| `apps/agent-gui/src/styles/theme.css`                   | Color tokens, radius tokens, type scale tokens                   |
| `apps/agent-gui/src/styles/components.css`              | Button hovers, scrollbar, focus rings, cursor, reduced-motion    |
| `apps/agent-gui/index.html`                             | Google Fonts link                                                |
| `apps/agent-gui/src/App.vue`                            | Body font-family                                                 |
| `apps/agent-gui/src/layouts/AppLayout.vue`              | Nav polish                                                       |
| `apps/agent-gui/src/layouts/SettingsLayout.vue`         | Tab spacing, active indicator                                    |
| `apps/agent-gui/src/components/ChatPanel.vue`           | Bubble radius, empty state, input focus, model popover animation |
| `apps/agent-gui/src/components/SessionsSidebar.vue`     | Item transitions, cursor pointers, empty state                   |
| `apps/agent-gui/src/components/ToastContainer.vue`      | Enter/exit animation                                             |
| `apps/agent-gui/src/components/StatusBar.vue`           | Minor polish                                                     |
| `apps/agent-gui/src/components/ContextMeter.vue`        | Focus ring, cursor                                               |
| `apps/agent-gui/src/views/settings/GeneralSettings.vue` | Row dividers                                                     |
| Various settings panes                                  | Card padding consistency, focus rings                            |

## Out of Scope

- New components or features
- Behavior/logic changes
- Mobile responsive design (desktop app)
- Icon set replacement (stays with inline SVGs)
- i18n string changes
- Playwright test changes (visual-only, tests verify behavior)

## Verification

1. `pnpm run lint` passes
2. `pnpm run format:check` passes
3. `cargo test --workspace --all-targets` passes
4. `pnpm --filter agent-gui run test` passes (Vitest)
5. Manual visual check: launch GUI with `pnpm --filter agent-gui run tauri dev`, verify light + dark modes
6. Use tauri-pilot to inspect elements and verify styles applied correctly
