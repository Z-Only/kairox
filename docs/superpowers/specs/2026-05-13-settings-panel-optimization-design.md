# Settings Panel Optimization — Design Spec

**Date**: 2026-05-13
**Status**: approved

## Overview

Optimize the settings panel with four workstreams:

1. Add an Archive tab showing archived sessions with restore/permanent-delete
2. Unified config source system: project-level priority + user-level fallback + same-name override
3. Config source selector UI (user/project toggle) on MCP/Skills/Models tabs
4. Fix `has_api_key` detection to check both `api_key` and `api_key_env`

## Architecture

### Route Restructuring

Replace `SettingsView.vue` flat-tab approach with nested routing:

```
/settings              → redirect to /settings/general
/settings/general      → GeneralSettings
/settings/archive       → ArchiveSettingsPane
/settings/mcp           → McpSettingsPane
/settings/skills        → SkillSettingsPane
/settings/models        → ModelSettingsPane
```

New component: `SettingsLayout` — renders tab navigation bar + `<router-view>` for the active settings page. On MCP/Skills/Models routes, also renders `ConfigSourceBar` above the page content.

### Tab Order

General → MCP → Skills → Models → Archive (Archive last)

## Feature 1: Archive Tab

**Component**: `ArchiveSettingsPane.vue`
**Route**: `/settings/archive`
**Data**: `useProjectStore.archivedSessions` + `loadArchivedSessions()`

**Layout**:

- Stats banner: total archived count, most recent archive date, count per project
- Card list: each card shows session title, project name, archive date, branch
- Actions per card: Restore, Permanent Delete (with confirmation dialog)

**Rust additions**:

- `restore_archived_session(session_id)` — sets visibility from `archived` back to `visible`, returns the associated project info so the sidebar can refresh
- `permanently_delete_session(session_id)` — hard-deletes the session and all associated events/messages from the event store

## Feature 2: Config Source System

### Design: Project-level priority + user-level fallback + same-name override

Given a user-level config and a project-level config, the effective config is:

1. Start with all user-level entries
2. Override with any project-level entry sharing the same id/alias/name (same-name override)
3. Add any project-level entry not present in user config
4. Each entry carries a `source` marker: `"user_config"` or `"project_config"`

### ConfigSourceBar Component

Shared component rendered by `SettingsLayout` on MCP/Skills/Models routes.

**Props**: `currentTab: "mcp" | "skills" | "models"`

**Structure**:

```
[用户配置 | 项目配置]   [项目: <select>]   [打开配置]
```

- Segmented button: `用户配置` (default) / `项目配置`
- Project selector dropdown: visible only when `项目配置` is selected
  - Lists `useProjectStore().activeProjects`
  - Projects with non-existent paths show ⚠ icon; hover tooltip shows original path and removal time
- If any project paths are missing, a yellow banner appears above the project selector showing the count and a "view details" link
- "打开配置" button: opens the relevant config file/directory for the current tab:
  - Models: `open_config_dir()` → `~/.kairox/` directory
  - MCP: `open_mcp_config_file()` → MCP config file directory
  - Skills: `open_skills_dir()` → skills directory (new command)

**Events**: `@source-change(source: "user" | "project", projectId?: string)`

### Per-Tab Integration

Each pane listens for `source-change` events from ConfigSourceBar and reloads data accordingly:

- **User mode**: load only user-level config
- **Project mode**: load merged config (project overriding user) for the selected project

### Source Display

Each list entry shows a source tag:

- `用户级` (blue) — from user config
- `项目级` (purple) — from project config
- `内置` (gray) — builtin (skills only)

## Feature 3: MCP/Skills/Models Refinements

### MCP

- Add `source: String` field to `McpServerSettingsView` (Rust)
- In `mcp_settings.rs`, tag each server with its config origin path
- Store: add `fetchSettingsServers(source?, projectId?)` method
- Sub-tabs (installed/marketplace) unchanged

### Skills

- `SkillSettingsView` already has `scope` field — reuse for source display
- Store: add `loadSkillSettings(source?, projectId?)` method
- New Rust command: `open_skills_dir()` — opens the skills config directory
- Sub-tabs (installed/discover) unchanged

### Models

- Extract `useModelStore` Pinia store from `ModelSettingsPane.vue` local state
- Store manages: `profiles`, `loading`, `error`, `fetchProfiles(source, projectId?)`, CRUD actions
- `ProfileSettingsView` already has `source` field
- Sub-tabs not applicable (no marketplace for models)
- Fix `has_api_key` (see Feature 4)

## Feature 4: Fix `has_api_key` Detection

**Bug**: `crates/agent-runtime/src/profile_settings.rs` line 115-119 checks only `api_key_env` env var existence. Profiles with a direct `api_key` value (not via env var) show "未配置 API Key".

**Fix**: Change the check to also test `api_key`:

```rust
let has_api_key = row.api_key.is_some()
    || row.api_key_env.as_ref().is_some_and(|v| std::env::var(v).is_ok());
```

This aligns with `Config::profile_info()` logic.

## Feature 5: Project Path Existence Check

On app startup (when `loadProjects()` is called), check each project's `rootPath` exists on disk.

- Add `pathExists: boolean` field to `ProjectInfo`
- In the project list dropdown (ConfigSourceBar), non-existent projects show ⚠
- Banner warning if any projects have missing paths

The Rust `list_projects` command already returns `root_path`; we can add a `path_exists` boolean computed via `std::path::Path::exists()`.

## Rust Changes Summary

| Change                                  | Crate/File                                                              | Notes                                        |
| --------------------------------------- | ----------------------------------------------------------------------- | -------------------------------------------- |
| Fix `has_api_key`                       | `agent-runtime/profile_settings.rs`                                     | Check `api_key` OR `api_key_env`             |
| Add `source` to `McpServerSettingsView` | `agent-core/facade.rs`                                                  | String: `"user_config"` / `"project_config"` |
| Tag MCP servers with source             | `agent-runtime/mcp_settings.rs`                                         | Distinguish user vs project config files     |
| Add `restore_archived_session`          | `agent-core/facade/` + `agent-runtime/facade_mcp.rs` + `commands.rs`    | Restore visibility from archived             |
| Add `permanently_delete_session`        | `agent-core/facade/` + `agent-runtime/` + `commands.rs`                 | Hard-delete session + events                 |
| Add `open_skills_dir`                   | `agent-core/facade/` + `agent-runtime/facade_skills.rs` + `commands.rs` | Open skills config dir in file manager       |
| Add `path_exists` to project response   | `commands.rs`                                                           | Check `root_path` existence                  |

## Frontend Changes Summary

| Change              | File                                       | Notes                                          |
| ------------------- | ------------------------------------------ | ---------------------------------------------- |
| Route restructuring | `router/routes.ts`                         | Nested routes `/settings/:tab`                 |
| SettingsLayout      | `layouts/SettingsLayout.vue` (new)         | Tab nav + ConfigSourceBar + router-view        |
| ConfigSourceBar     | `components/ConfigSourceBar.vue` (new)     | Shared config source selector                  |
| GeneralSettings     | `views/settings/GeneralSettings.vue` (new) | Extracted from SettingsView                    |
| ArchiveSettingsPane | `components/ArchiveSettingsPane.vue` (new) | Archived sessions list                         |
| ModelSettingsPane   | Refactor                                   | Use `useModelStore`, listen to ConfigSourceBar |
| McpSettingsPane     | Modify                                     | Listen to ConfigSourceBar events               |
| SkillSettingsPane   | Modify                                     | Listen to ConfigSourceBar events               |
| useModelStore       | `stores/models.ts` (new)                   | Pinia store for model profiles                 |
| useProjectStore     | Modify                                     | Add `pathExists` field, check on startup       |
| SettingsView        | Remove or redirect                         | Replaced by SettingsLayout + nested routes     |
| Type generation     | Run `just gen-types`                       | Update generated commands/events               |

## Error Handling

- Archive restore failure: show error toast, keep item in list
- Permanent delete failure: show error toast, keep item in list
- Config load failure (project mode): show error banner in pane, fallback to empty list
- Project path not found: non-blocking warning, allowed to select but config may be empty
- File manager open failure: silent (best-effort, OS-dependent)

## Testing Plan

- Unit tests: `has_api_key` fix in `profile_settings` tests
- Integration tests: new Rust commands (`restore_archived_session`, `permanently_delete_session`, `open_skills_dir`)
- Vitest: `useModelStore`, `useProjectStore.pathExists`
- E2E (Playwright): archive tab navigation, config source switching, project selector interaction
- Manual: project path missing warning, source tags visibility across all three tabs
