# Model Settings Polish — Design Spec

**Date**: 2026-05-12
**Status**: approved (pending spec review)

## Problem Statement

The model settings page (`ModelSettingsPane`) has 10 issues that degrade UX and code quality:

1. No unit tests for `ModelSettingsPane.vue`
2. Chat panel model switcher order differs from settings page
3. Add/edit form fields are stacked without visual organization
4. Built-in disabled default `local-code` appears in the list
5. `api_key_env` profiles (e.g. DeepSeek) incorrectly show "no API key"
6. Model list has no reordering capability (sort, move up/down)
7. No way to toggle between user-level and project-level config views
8. No button to open config directory in file manager; unnecessary source tags on list items
9. Refresh and Add buttons have inconsistent styling and no horizontal gap
10. Enable/disable toggle has no effect (writes partial entries to profiles.toml)

## Architecture

```
profiles.toml  <── move/upsert/delete/toggle writes
      │
      ▼
list_profile_settings(source_filter?) ──→ ProfileSettingsView[]
      │                                        │
      │                              ┌─────────┘
      │                              ▼
      │                    ModelSettingsPane.vue
      │                      ├─ source toggle (user/project)
      │                      ├─ sorted list (display_order → alphabetical)
      │                      ├─ move up/down
      │                      └─ add/edit/delete/toggle
      │
get_profile_info() ──→ ProfileInfo[] (enabled only)
      │
      ▼
ChatPanel.vue model popover (same sort order)
```

## Design Decisions

### 1. Filtering disabled built-in defaults (Issue #4)

In `list_profile_settings`, after the 4-layer merge, remove entries where `source == "defaults" && !enabled`. Profiles explicitly configured by the user in any layer (even if disabled) remain visible.

### 2. Fixing `has_api_key` detection (Issue #5)

`Config.get_profile()` only searches profiles loaded from `config.toml` (user/project), not `profiles.toml`. Fix: check `has_api_key` from the row data directly using `std::env::var(row.api_key_env).is_ok()` instead of routing through Config.

### 3. Fixing enable/disable toggle (Issue #10)

When `set_profile_enabled_in_file` creates a new entry in `profiles.toml`, it currently only sets `enabled`, leaving provider/model_id empty. The next `list_profile_settings` call then sees the profiles.toml layer overriding the defaults with empty fields.

Fix: when the profile doesn't exist in profiles.toml yet, clone the full ProfileDef from the merged Config into profiles.toml before toggling `enabled`.

### 4. Display ordering (Issue #6)

Store a top-level `display_order` array in `profiles.toml`:

```toml
display_order = ["fast", "my-model", "local-code"]
```

Profiles not in the array sort alphabetically at the end. Two new commands:

- `move_profile_up(alias)`: swap with previous entry in display_order
- `move_profile_down(alias)`: swap with next entry

Frontend shows ▲/▼ buttons per list item; first item's ▲ and last item's ▼ are disabled.

### 5. User/Project config toggle (Issue #7)

Add optional `source_filter: Option<String>` parameter to `list_profile_settings`:

- `"user"`: defaults + user config.toml overlay (no project layer)
- `"project"`: defaults + project config.toml overlay (highest priority)
- `None`: all layers merged (current behavior)

Frontend renders a dropdown/select at the top of the list. Default selection: "project" if a project-level config exists, otherwise "user".

### 6. Open config directory (Issue #8)

New Tauri command `open_config_dir` that uses the `open` crate to open the config directory in the system file manager. Returns the opened path.

Remove source tags (`sourceLabel`) from list items — the toggle already communicates which config level is shown. Also remove the `v-if="profile.writable"` guard from the delete button, but keep it for the enable/disable button (see fix #3).

Wait — reconsider: the enable/disable toggle should now work for all profiles (fix #3 handles the profiles.toml creation). Delete should only work for profiles in profiles.toml (writable).

### 7. Form UI (Issue #3)

Reorganize the add/edit form with a 2-column CSS grid and collapsible sections:

- **Basic** (always visible): alias, provider, model_id — 2-column layout
- **Connection** (always visible): base_url, api_key_env — full-width
- **Advanced** (collapsible, default collapsed): context_window, output_limit, temperature, top_p, top_k, max_tokens — 2-column or 3-column grid

### 8. Toolbar styling (Issues #8, #9)

```
[User Config ▾]  [Refresh]  [+ Add Profile]  [📂 Open Dir]
```

- Source toggle: left-aligned dropdown
- Action buttons: uniform styling with 8px horizontal gap
- `Open Dir` button opens the config directory in file manager

### 9. Chat panel order consistency (Issue #2)

Sort `profileInfos` in `ChatPanel.vue` using the same order as the settings page: by `display_order` position, then alphabetically for unlisted profiles.

### 10. Tests (Issue #1)

Create `ModelSettingsPane.test.ts` covering:

- Renders profile list with data
- Refresh reload
- Add dialog: open, validate required fields, save
- Edit dialog: open pre-filled, save changes
- Toggle enable/disable
- Delete (writable profiles)
- Move up/down
- Source toggle switching between user/project views
- Open config dir
- Error and loading states

## Files Changed

| File                                                      | Change                                                            |
| --------------------------------------------------------- | ----------------------------------------------------------------- |
| `crates/agent-runtime/src/profile_settings.rs`            | Filter defaults, fix has_api_key, fix toggle, add ordering writes |
| `crates/agent-runtime/src/facade_runtime.rs`              | Add move_up/down, open_dir, source_filter passthrough             |
| `crates/agent-core/src/facade.rs`                         | Add new facade methods if needed                                  |
| `apps/agent-gui/src-tauri/src/commands.rs`                | New/updated commands                                              |
| `apps/agent-gui/src-tauri/src/lib.rs`                     | Register new commands                                             |
| `apps/agent-gui/src-tauri/src/specta.rs`                  | Collect new commands                                              |
| `apps/agent-gui/src/generated/commands.ts`                | Auto-generated (`just gen-types`)                                 |
| `apps/agent-gui/src/components/ModelSettingsPane.vue`     | Major UI rework                                                   |
| `apps/agent-gui/src/components/ChatPanel.vue`             | Sort model options                                                |
| `apps/agent-gui/src/components/ModelSettingsPane.test.ts` | **New**                                                           |
| `apps/agent-gui/src/locales/en.json`                      | New i18n keys                                                     |
| `apps/agent-gui/src/locales/zh-CN.json`                   | New i18n keys                                                     |
