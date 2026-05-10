# GUI Interaction Polish Design

## Summary

Kairox GUI will use the confirmed combined direction: apply the incremental sidebar polish from visual Option A and the stronger MCP consolidation from visual Option B. The goal is to make the desktop UI more focused, consistent, and predictable while staying within the existing Vue 3 + Pinia + Tauri IPC architecture.

## Scope

This design covers these GUI areas:

- `SessionsSidebar.vue`: project list, chat list, creation actions, hover actions, inline delete confirmation, project import.
- `ChatPanel.vue`, `StatusBar.vue`, `ContextMeter.vue`, and `session.ts`: context/model display so the UI shows provider and model name instead of only profile alias.
- `McpSettingsPane.vue`, `MarketplacePane.vue`, `SettingsView.vue`, marketplace components, and related tests: MCP settings and marketplace consolidation.
- GUI validation with unit tests, Playwright where appropriate, and tauri-pilot for real desktop inspection.

Out of scope:

- Redesigning the main chat, trace, task graph, memory, or runtime architecture.
- Replacing the native CSS component system with a new UI library.
- Removing backend catalog or MCP install commands that remain useful for the consolidated UI.

## Design Principles

- Keep the workbench local-first and data-dense, matching the existing Kairox product style.
- Use native HTML controls, CSS custom properties, and existing shared classes.
- Use SVG/text icons and consistent button sizing; avoid emoji-style action icons.
- Ensure hover-only actions also appear on keyboard focus via `:focus-within`.
- Prefer inline confirmation for destructive list-row actions to avoid page-top confirmation cards.
- Preserve existing stores and IPC where possible; add only the missing folder-selection capability for project import.

## Sidebar Design

The sidebar will only expose two top-level content groups: Projects and Chats.

### Projects

- Remove the current top `Sessions + New` header from `SessionsSidebar.vue`.
- Keep the Projects section at the top.
- Keep the project creation menu, but make it visually consistent and scoped to projects.
- Add an `Import Existing Folder` action in the project menu.
- Reuse the existing `projects.addExistingProject(path)` store method and `add_existing_project` Tauri command.
- Add the missing folder picker capability through Tauri dialog support, then pass the selected path into `addExistingProject(path)`.
- Project rename should be available consistently with session rename if the existing store method `renameProject()` is wired into this sidebar.
- Project row actions are hidden by default and shown on row hover or focus.
- Project removal uses a two-click inline confirmation: first click changes the delete button into a confirm state; second click performs removal.

### Chats

- Move the new chat button from the removed header into the Chats section heading.
- Reuse the existing new-session dialog and `session.createSession()` flow.
- Keep inline session rename.
- Hide chat row actions by default and reveal them on hover or focus.
- Replace global delete confirmation with a row-level two-click confirmation.
- Delete confirmation state clears when another row action starts, the row loses focus, or after a short timeout.

## Context and Model Display Design

The chat composer and status bar should distinguish model identity from context usage.

### Model identity

- Add a profile alias to provider/model mapping in `session.ts` or a small focused helper.
- Load profile details from existing GUI commands such as `get_profile_info` or `list_profiles_with_limits`.
- Expose a computed display value with this priority:
  1. `provider · model_id` when profile detail is known.
  2. `provider · model_id (alias)` where the alias adds useful context.
  3. Existing `currentProfile` as fallback.
- Update `ChatPanel.vue` composer metadata and `StatusBar.vue` to show provider/model, not only alias.
- Update `ContextMeter.vue` profile picker metadata to include provider, model id, and context window.

### Context usage

- Keep `ContextMeter.vue` responsible for token usage, context budget, and context-window status.
- Avoid mixing context budget text into the provider/model badge when it would reduce readability.
- If current context usage is unavailable, show a stable fallback instead of misleading or empty data.

## MCP Settings and Marketplace Design

MCP settings become the single place for MCP servers and market/catalog installation.

### Settings navigation

- Remove the separate top-level Marketplace tab from `SettingsView.vue`.
- Keep any legacy `/marketplace` route redirect only if needed for compatibility.
- Remove the standalone Marketplace display shell from active navigation.

### MCP page layout

- Make installed/configured servers the first visible content in `McpSettingsPane.vue`.
- Move the add-server form out of the default page flow.
- Add a right-aligned `Add server` button in the MCP page header.
- Clicking `Add server` opens a focused card/dialog.

### Add server flow

The add card/dialog has two modes:

- `Git / Catalog install`: browse or select catalog/git install entries and reuse catalog install progress components.
- `Manual config`: fill server name, transport, command/url, args, env, and enabled state.

The Marketplace `Installed` page/tab is removed. Installed server visibility belongs to the MCP servers list at the top of the page.

### State and command retention

Retain MCP and catalog capabilities that the consolidated UI still needs:

- MCP settings and runtime actions from `mcp.ts`.
- Catalog listing, source refresh, install, and install progress from `catalog.ts`.
- Installed-only UI state may be removed or simplified when no longer rendered.

## Testing and Verification

Implementation should follow test-first changes where practical:

- Component or store tests for sidebar create/import/delete-confirm interactions.
- Component/store tests for provider/model display fallback and profile mapping.
- Component tests for MCP settings page layout, add dialog modes, and removed Marketplace/Installed entry points.
- Update affected Playwright tests and mocks if visible selectors or commands change.
- Run focused GUI tests before broader checks.
- Use tauri-pilot after implementation to inspect the real desktop UI:
  - Verify sidebar contains only Projects and Chats groups.
  - Verify row actions appear on hover/focus and delete confirms inline.
  - Verify chat/status model display shows provider and model id.
  - Verify MCP page shows servers first and add flow opens from the header.
  - Check browser/console logs for errors.

## Acceptance Criteria

- The sidebar no longer shows the top `Sessions + New` header.
- Project and chat row actions are hidden by default and visible on hover/focus.
- Project and chat delete actions use inline two-click confirmation.
- Project list supports importing an existing folder through a native folder picker.
- Chat composer and status bar show model provider and model name when profile details are available.
- Context usage remains visible and stable through `ContextMeter.vue`.
- Settings has no separate Marketplace tab or standalone Marketplace screen in active navigation.
- MCP servers list appears before add-server controls.
- Add server UI is hidden by default and opens from the MCP page header.
- Add server UI supports catalog/git install and manual config modes.
- Marketplace Installed tab/page is removed from the rendered UI.
- Updated tests and tauri-pilot inspection pass without new lint or runtime errors.
