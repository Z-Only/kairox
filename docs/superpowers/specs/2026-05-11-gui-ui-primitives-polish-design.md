# GUI UI Primitives Polish Design

**Date:** 2026-05-11
**Status:** Draft (waiting for user review)
**Target branch:** `feat/gui-ui-primitives-polish`
**Scope:** Kairox GUI UI/UX audit fixes, interaction consistency, model/context display fixes, MCP settings cleanup, and a first headless UI primitives layer.

---

## 1. Purpose and Context

Kairox GUI has grown into a desktop AI workbench with project navigation, nested sessions, chat/model controls, context budget display, MCP settings, marketplace browsing, memory, trace, task graph, confirmations, and several popup-style interactions. The current implementation relies on native HTML, CSS custom properties, shared CSS classes, and a few local composables such as `useToast()` and `useConfirm()`.

The real `tauri-pilot` audit showed that this architecture is still sound visually, but hand-written popup/menu/dialog/select behavior is now producing inconsistent UX and test fragility. Examples include mixed text/icon actions, missing project and nested-session actions, long names pushing buttons, the new-session profile dialog appearing at the wrong time, static model badges, incomplete context details, duplicate MCP Marketplace tabs, and an ineffective MCP config open button.

This design adopts the confirmed B+ direction:

- Keep Kairox's own visual system: `theme.css`, `components.css`, native HTML semantics, and product-specific dark/light styling.
- Introduce AI-friendly **headless primitives** with `reka-ui` for interaction-heavy controls.
- Add thin Kairox wrapper components so business components do not depend directly on third-party API details.
- Fix the 11 required GUI issues and encode them in unit, E2E, and pilot verification.

---

## 2. Design Principles

1. **Preserve Kairox visual identity.** Do not introduce a full visual UI kit such as Naive UI, Element Plus, Vuetify, or Tailwind-first shadcn-vue.
2. **Use headless primitives only where behavior is hard.** `DropdownMenu`, `Popover`, `Dialog`/`AlertDialog`, `Select`, and `Tooltip` are good candidates because they need keyboard, focus, escape, click-outside, and ARIA behavior.
3. **Hide dependency details behind Kairox wrappers.** Components under `components/ui/` expose stable Kairox APIs, classes, and `data-test` conventions.
4. **Make AI-generated changes safer.** Wrapper names, props, tests, and examples must be explicit and local to the repo so future coding agents can reuse existing patterns instead of inventing new menu/popover logic.
5. **Test behavior before visual polish.** All behavior changes require failing tests first, then minimal implementation, then focused verification.
6. **Keep IPC boundaries explicit.** If a Tauri command changes, update Rust command registration, specta collection, generated types, Playwright mock, and tests together.

---

## 3. UI Primitives Architecture

### 3.1 Dependency

Add `reka-ui` to `apps/agent-gui/package.json` as a GUI dependency. This is not a full UI library exception; it is an accessibility primitive dependency used to implement Kairox-owned components.

`AGENTS.md` should be updated to clarify the rule:

- The GUI still does not use a full visual UI library.
- Headless accessibility primitives are allowed behind `apps/agent-gui/src/components/ui/` wrappers.
- Business components should import wrappers, not raw `reka-ui` components, unless a design document explicitly approves an exception.

### 3.2 Wrapper Components

Create a focused first set of wrappers:

- `KxIconButton.vue`: icon-only button with fixed size, label/title, disabled/loading styling, focus ring, and stable `data-test` support.
- `KxTooltip.vue`: accessible tooltip wrapper used for icon-only actions and dense metadata.
- `KxDropdownMenu.vue`: menu wrapper for project/session more actions and new-project secondary choices.
- `KxPopover.vue`: popover wrapper for model selection and context usage details.
- `KxDialog.vue` or `KxAlertDialog.vue`: only if the existing native `ConfirmDialog.vue` cannot cover the new behavior cleanly.
- `KxProgressRing.vue`: local SVG ring for context usage. This does not need Reka UI.

The wrappers should use CSS classes from `components.css` plus new `.kx-*` classes where needed. They must support explicit `data-test` attributes on trigger/content/items so Playwright and pilot scenarios can target stable selectors.

### 3.3 Styling Contract

The wrapper layer owns shared styles for:

- 32px and 36px icon buttons.
- Visible hover, focus, active, and disabled states.
- Menu item density and destructive-action color.
- Popover surface, shadow, border, and z-index.
- Long-label truncation with `min-width: 0`, `overflow: hidden`, `text-overflow: ellipsis`, and `white-space: nowrap`.
- Dark/light color tokens using `--app-*` variables.

No Tailwind, new visual framework, or generated component artifact is introduced.

---

## 4. Required Fixes

### 4.1 Project list action consistency

`SessionsSidebar.vue` should replace mixed text/icon row actions with icon buttons. High-frequency actions remain visible when appropriate; low-frequency actions move into a `More actions` dropdown.

Project creation changes from direct creation to a secondary menu:

- `Create Blank Project`: creates a project with the default name `New Project`.
- `Import Existing Folder`: opens the folder picker and imports the selected path.

Each action has a label, tooltip, icon, and short description in the menu.

### 4.2 Default new project name

Change all runtime, GUI mock, and tests from `Untitled Project` to `New Project`.

Known locations:

- `crates/agent-runtime/src/facade_runtime.rs`
- `apps/agent-gui/e2e/tauri-mock.js`
- project store/component tests that assert the default name

### 4.3 Project rename

Use the existing project store `renameProject()` capability from the project list UI. The UX should match session rename as closely as possible:

- Rename entry in project more-actions menu.
- Inline edit state or compact rename dialog.
- Optimistic UI update after success.
- Error toast and rollback/refresh when the command fails.

### 4.4 Project nested session actions

Project-owned sessions must support the same core actions as regular sessions:

- Rename.
- Archive.
- Delete if already supported for normal sessions.

The action visual treatment, tooltip behavior, and more-menu behavior should be shared with normal session rows.

### 4.5 Long text resilience

Project names, session titles, model labels, provider names, and paths must not push action buttons out of view.

Required layout rules:

- Text containers use `min-width: 0`.
- Primary labels truncate with ellipsis.
- Full values are available through `title` or tooltip.
- Action button containers use `flex: none` and keep stable width/alignment.

### 4.6 Context usage ring

`ContextMeter.vue` should show context usage as a compact ring when used in the chat composer/header area.

The ring displays current percentage and uses state colors:

- Normal below warning threshold.
- Warning at high usage.
- Danger near exhaustion.

Hover or click opens a details popover with:

- Used tokens.
- Maximum tokens.
- Percentage.
- Current model context window.
- Fallback text when context data is unavailable.

The ring must not visually compete with the message input.

### 4.7 Project new-session model display

Project-created sessions must show the real provider and model name, not only `default`.

The display format is:

- `Provider · Model`
- Example: `OpenAI · GPT-4o`, `Anthropic · Claude Sonnet`, `Ollama · llama3`

Project path must not appear to the right of the model badge. If path context is needed, show it as secondary project metadata elsewhere.

### 4.8 Remove new-session profile dialog

Creating a new conversation should no longer show the profile selection dialog. It should directly create a session with the default model/profile.

After creation:

- The current model appears in the chat header or composer metadata.
- Clicking the model label opens a model selection popover/card.
- The card shows providers/models, current selected state, hover/focus states, disabled states when unavailable, and any useful context-window metadata.
- Switching the model updates UI state and subsequent requests.

### 4.9 MCP config open action

The MCP settings button should accurately describe and perform the action.

Target behavior:

- Rename copy to `Open config folder`, `Reveal in Finder`, or a platform-neutral equivalent.
- Click opens the config file's parent directory through Tauri shell/opener capability.
- Failure shows a clear toast.
- If IPC is changed, update `commands.rs`, `lib.rs`, `specta.rs`, generated types, mock, and tests.

### 4.10 Remove duplicate Marketplace/Browse tabs

`McpSettingsPane.vue` and `MarketplacePane.vue` should not render nested Marketplace/Browse tab layers when there is only one Browse page.

The MCP Marketplace content should appear directly inside the selected settings area with visual hierarchy matching other Settings tabs.

### 4.11 Proactive polish

While touching these areas, fix directly related issues discovered by pilot and UI/UX review:

- Replace emoji-style action glyphs with consistent SVG/text icons.
- Add missing `aria-label`, `title`, and visible focus states for icon buttons.
- Ensure hover-only actions are also available on `:focus-within`.
- Remove page-level confirmation controls from idle sidebar accessibility flow when row-level confirmation is intended.
- Stabilize pilot selectors for confirmation and nested project-session actions.
- Avoid persistent low-frequency controls that crowd dense rows.

---

## 5. Data and Command Flow

### 5.1 Project creation and rename

The project store remains the source of truth for project list state. UI actions call store methods rather than directly invoking Tauri from components.

- Blank creation calls the existing blank-project command and expects `New Project` from runtime/mock.
- Import calls folder selection first, then `addExistingProject(path)`.
- Rename calls `renameProject(projectId, name)` and updates the visible row after success.

### 5.2 Session creation and model selection

Session creation should use default profile/model without a modal. Profile details are loaded through existing profile commands where possible. A small helper in the session store or a focused utility should map profile alias to provider/model display.

Model switching should reuse existing session profile switching actions. The model selection card must not create a parallel model state that can diverge from request state.

### 5.3 MCP config folder open

Prefer adding or changing one explicit Tauri command whose name and return behavior matches the UI copy. The command should open the containing directory, not only return a path. If it returns the path for display/debugging, the UI still treats success as “folder opened”.

---

## 6. Testing Strategy

Use TDD for each behavior group.

### 6.1 Vitest / component and store tests

Add or update tests for:

- Blank project default name is `New Project`.
- Project new menu exposes blank/import choices and does not create immediately on trigger click.
- Project rename success and failure toast behavior.
- Project nested session rename and archive actions.
- Long names keep action buttons visible.
- New session creation does not render profile dialog.
- Model display uses provider/model fallback rules.
- Model label opens model selection popover and selected model state changes after switching.
- Context ring percentage, warning classes, and details popover.
- MCP config folder open success/failure behavior.
- Marketplace nested tab removal.

### 6.2 Playwright E2E and mock

Update `apps/agent-gui/e2e/tauri-mock.js` for changed defaults and any IPC changes.

Update or add Playwright coverage for:

- New project menu.
- Project rename.
- Project nested session actions.
- No profile dialog on new session.
- Model selector popover.
- MCP config folder action.
- Marketplace tab simplification.

### 6.3 Tauri pilot

After implementation, rerun real desktop checks with `tauri-pilot`:

- Workbench project/sidebar screenshot.
- New project secondary menu screenshot.
- New session flow showing no profile dialog.
- Project session showing provider/model display.
- Context ring and details popover.
- MCP Settings config folder button.
- MCP Marketplace simplified page.

Also run `tauri-pilot logs --level error` after interactions.

### 6.4 Verification commands

Minimum verification for GUI-only changes:

```bash
pnpm run format:check
pnpm run lint
just test-gui
just test-e2e
just test-pilot
```

If Rust/Tauri IPC changes:

```bash
just check-types
cargo test --workspace --all-targets
```

If environment limits prevent a command from running, report the command, failure reason, relation to this change, and alternate verification performed.

---

## 7. Acceptance Criteria

- Project list operations use consistent icon/menu actions with tooltips and accessible labels.
- New project trigger opens a two-choice menu and does not create immediately.
- All default blank projects are named `New Project`.
- Project rename is available and handles success/failure clearly.
- Project nested sessions support rename and archive.
- Long names never hide or misalign operation icons.
- Context usage appears as a ring with meaningful details on hover/click.
- Project-created sessions show provider/model names and do not show project path beside the model label.
- New conversation creation no longer shows a profile selection dialog.
- Clicking model display opens a model selector and switching affects subsequent session state.
- MCP config action opens the containing folder or shows an explicit error.
- MCP Marketplace no longer contains redundant Marketplace/Browse nesting.
- UI wrappers exist for the first headless primitive interactions and use Kairox styling.
- Unit, E2E, and pilot checks cover the changed behavior.

---

## 8. Risks and Mitigations

- **Risk:** `reka-ui` introduces unfamiliar patterns.  
  **Mitigation:** Use wrappers only, keep business components on Kairox APIs, and add tests for wrapper usage.

- **Risk:** Full UI-library migration scope creep.  
  **Mitigation:** Explicitly reject full visual libraries and Tailwind migration in this spec.

- **Risk:** Popup DOM changes break E2E selectors.  
  **Mitigation:** Add stable `data-test` attributes at wrapper trigger/content/item boundaries.

- **Risk:** Model display fixes diverge from actual request profile.  
  **Mitigation:** Derive display from the same session/profile state used by request execution.

- **Risk:** MCP folder opening differs by OS.  
  **Mitigation:** Use Tauri/opener cross-platform behavior and test command success/failure at the store/UI boundary.
