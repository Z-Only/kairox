# Model Settings Polish — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 10 UX/functionality issues in model settings: test coverage, ordering, form UI, filtering, API key detection, reordering, source toggle, config dir button, button styling, and enable/disable toggle.

**Architecture:** Backend fixes in `profile_settings.rs` + new facade methods for ordering and config dir; frontend rework of `ModelSettingsPane.vue` with grid form layout, toolbar, move up/down, and source toggle; new test file following existing patterns.

**Tech Stack:** Rust (tokio, toml_edit, serde), Vue 3 + TypeScript (Pinia, vue-i18n), Vitest + @vue/test-utils

---

### Task 1: Fix `has_api_key` detection and filter disabled defaults (Issues #4, #5)

**Files:**

- Modify: `crates/agent-runtime/src/profile_settings.rs:104-140`

- [ ] **Step 1: Fix `has_api_key` to use row data directly**

In `list_profile_settings`, change the `has_api_key` check from `config.get_profile()` (which only sees config.toml profiles) to use the row's own `api_key_env`:

```rust
// Before (line 107-116):
let has_api_key = config
    .get_profile(&alias)
    .map(|def| {
        def.api_key.is_some()
            || def
                .api_key_env
                .as_ref()
                .is_some_and(|v| std::env::var(v).is_ok())
    })
    .unwrap_or(false);

// After:
let has_api_key = row.api_key_env
    .as_ref()
    .is_some_and(|v| std::env::var(v).is_ok());
```

Apply this change at line 107 in the `map` closure.

- [ ] **Step 2: Filter out disabled default profiles**

After the `rows.into_iter().map(...)` block but before `.collect()`, add a filter:

```rust
.filter(|view| !(view.source == "defaults" && !view.enabled))
```

This removes built-in disabled profiles like `local-code` from the list.

- [ ] **Step 3: Run existing tests to verify no regressions**

```bash
cargo test -p agent-runtime -- profile_settings
```

Expected: all existing tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime/src/profile_settings.rs
git commit -m "fix(runtime): use row api_key_env for has_api_key, filter disabled defaults

- has_api_key now checks row.api_key_env directly instead of
  config.get_profile(), which only looked at config.toml profiles
- Disabled built-in default profiles (source=defaults, enabled=false)
  are now filtered from list_profile_settings output

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: Fix enable/disable toggle to preserve full profile definition (Issue #10)

**Files:**

- Modify: `crates/agent-runtime/src/profile_settings.rs:230-241`

- [ ] **Step 1: Update `set_profile_enabled_in_file` to copy full profile on first toggle**

When the profile doesn't exist in profiles.toml yet, the current code creates a partial entry with only `enabled`. Fix by cloning the full definition from the merged Config:

```rust
pub async fn set_profile_enabled_in_file(
    config_path: &Path,
    alias: &str,
    enabled: bool,
    config: &Config,
) -> agent_core::Result<()> {
    mutate_profiles_config(config_path, |document| {
        // If the profile doesn't exist yet in profiles.toml, seed it with
        // the full definition from the merged Config so we don't override
        // defaults with an empty table.
        let profiles = document["profiles"].as_table()
            .map(|t| t.contains_key(alias))
            .unwrap_or(false);
        if !profiles {
            if let Some(def) = config.get_profile(alias) {
                let table = ensure_profile_table(document, alias);
                seed_profile_table(table, def);
            }
        }
        let profile_table = ensure_profile_table(document, alias);
        profile_table["enabled"] = value(enabled);
        Ok(())
    })
    .await
}

fn seed_profile_table(table: &mut Table, def: &ProfileDef) {
    table["provider"] = value(def.provider.clone());
    table["model_id"] = value(def.model_id.clone());
    table["enabled"] = value(def.enabled);
    if let Some(v) = def.context_window { table["context_window"] = value(v as i64); }
    if let Some(v) = def.output_limit { table["output_limit"] = value(v as i64); }
    if let Some(v) = def.temperature { table["temperature"] = value(v as f64); }
    if let Some(v) = def.top_p { table["top_p"] = value(v as f64); }
    if let Some(v) = def.top_k { table["top_k"] = value(v as i64); }
    if let Some(v) = def.max_tokens { table["max_tokens"] = value(v as i64); }
    if let Some(ref v) = def.base_url { if !v.is_empty() { table["base_url"] = value(v.clone()); } }
    if let Some(ref v) = def.api_key_env { if !v.is_empty() { table["api_key_env"] = value(v.clone()); } }
}
```

Add `use agent_config::ProfileDef;` at the top of the file.

- [ ] **Step 2: Update callers that pass `config`**

In `facade_runtime.rs:1288`, update the call:

```rust
crate::profile_settings::set_profile_enabled_in_file(&config_path, &alias, enabled, &self.config).await
```

The `AppFacade` trait default in `facade.rs` stays unchanged (it's a no-op default).

- [ ] **Step 3: Run tests**

```bash
cargo test -p agent-runtime -- profile_settings
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime/src/profile_settings.rs crates/agent-runtime/src/facade_runtime.rs
git commit -m "fix(runtime): seed full profile definition on first enable/disable toggle

When toggling a profile that doesn't exist in profiles.toml yet, clone
the full ProfileDef from merged Config before setting enabled. This
prevents creating a partial entry that overrides defaults with empty
provider/model_id fields.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: Add display ordering support (Issue #6 — backend)

**Files:**

- Modify: `crates/agent-runtime/src/profile_settings.rs:35-140` (list + new functions)
- Modify: `crates/agent-runtime/src/facade_runtime.rs` (new facade methods)
- Modify: `crates/agent-core/src/facade.rs` (new trait methods)

- [ ] **Step 1: Add `load_display_order` and `save_display_order` helpers**

Add to `profile_settings.rs`:

```rust
fn load_display_order(document: &DocumentMut) -> Vec<String> {
    document
        .get("display_order")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn save_display_order(document: &mut DocumentMut, order: &[String]) {
    let array = toml_edit::Array::from_iter(order.iter().map(|s| toml_edit::Value::from(s.clone())));
    document["display_order"] = toml_edit::Item::Value(toml_edit::Value::Array(array));
}

fn sort_by_display_order(views: &mut Vec<ProfileSettingsView>, display_order: &[String]) {
    views.sort_by(|a, b| {
        let pos_a = display_order.iter().position(|s| s == &a.alias);
        let pos_b = display_order.iter().position(|s| s == &b.alias);
        match (pos_a, pos_b) {
            (Some(pa), Some(pb)) => pa.cmp(&pb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.alias.cmp(&b.alias),
        }
    });
}
```

- [ ] **Step 2: Wire display_order into `list_profile_settings`**

After building the `views` Vec and applying the filter, load display_order from profiles.toml and sort:

```rust
// After the .collect::<Vec<_>>() call:
views.retain(|v| !(v.source == "defaults" && !v.enabled));

// Load display_order from profiles.toml for sort
if let Some(path) = profiles_toml_path {
    if path.exists() {
        if let Ok(raw) = tokio::fs::read_to_string(path).await {
            if let Ok(doc) = raw.parse::<DocumentMut>() {
                let display_order = load_display_order(&doc);
                sort_by_display_order(&mut views, &display_order);
                // return early with sorted views if display_order exists
                return Ok(views);
            }
        }
    }
}
// If no display_order, sort alphabetically as before
views.sort_by(|a, b| a.alias.cmp(&b.alias));
Ok(views)
```

Wait — restructure more cleanly. Replace the existing sort at line 138 with display_order-based sort:

```rust
// After filtering, before returning:
let mut display_order: Vec<String> = Vec::new();
if let Some(path) = profiles_toml_path {
    if path.exists() {
        if let Ok(raw) = tokio::fs::read_to_string(path).await {
            if let Ok(doc) = raw.parse::<DocumentMut>() {
                display_order = load_display_order(&doc);
            }
        }
    }
}
sort_by_display_order(&mut views, &display_order);
Ok(views)
```

Replace the existing `views.sort_by(|a, b| a.alias.cmp(&b.alias));` line with this block.

- [ ] **Step 3: Add `move_profile_in_order` function**

```rust
pub async fn move_profile_in_order(
    config_path: &Path,
    alias: &str,
    direction: i32, // -1 for up, +1 for down
) -> agent_core::Result<()> {
    mutate_profiles_config(config_path, |document| {
        let mut order = load_display_order(document);
        if let Some(pos) = order.iter().position(|s| s == alias) {
            let new_pos = if direction < 0 {
                pos.saturating_sub(1)
            } else {
                (pos + 1).min(order.len() - 1)
            };
            if new_pos != pos {
                order.swap(pos, new_pos);
                save_display_order(document, &order);
            }
        } else {
            // Profile not in order yet — add it at the end
            order.push(alias.to_string());
            save_display_order(document, &order);
        }
        Ok(())
    })
    .await
}
```

- [ ] **Step 4: Add trait methods to `AppFacade`**

In `crates/agent-core/src/facade.rs`, add after `delete_profile_settings`:

```rust
/// Move a profile up or down in display order.
async fn move_profile_in_order(
    &self,
    alias: String,
    direction: i32,
) -> crate::Result<()> {
    let _ = (alias, direction);
    Err(crate::CoreError::InvalidState(
        "profile ordering not supported".into(),
    ))
}

/// Open the config directory in the system file manager.
async fn open_config_dir(&self) -> crate::Result<Option<String>> {
    Ok(None)
}
```

- [ ] **Step 5: Implement trait methods in `facade_runtime.rs`**

```rust
async fn move_profile_in_order(
    &self,
    alias: String,
    direction: i32,
) -> agent_core::Result<()> {
    let config_path = crate::profile_settings::writable_profiles_config_path(
        self.marketplace_dir.as_deref(),
    )?
    .ok_or_else(|| {
        agent_core::CoreError::InvalidState(
            "config dir not configured; cannot reorder profiles".into(),
        )
    })?;
    crate::profile_settings::move_profile_in_order(&config_path, &alias, direction).await
}

async fn open_config_dir(&self) -> agent_core::Result<Option<String>> {
    Ok(self.marketplace_dir.as_ref().map(|p| p.display().to_string()))
}
```

- [ ] **Step 6: Add Tauri commands**

In `apps/agent-gui/src-tauri/src/commands.rs`, add after `delete_profile_settings`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn move_profile_in_order(
    state: State<'_, GuiState>,
    alias: String,
    direction: i32,
) -> Result<(), String> {
    state
        .runtime
        .move_profile_in_order(alias, direction)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn open_config_dir(
    state: State<'_, GuiState>,
) -> Result<Option<String>, String> {
    let Some(config_dir) = state
        .runtime
        .open_config_dir()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let config_dir = std::path::PathBuf::from(config_dir);
    open_path_in_system_file_manager(&config_dir)?;
    Ok(Some(config_dir.display().to_string()))
}
```

- [ ] **Step 7: Register commands in `lib.rs` and `specta.rs`**

In `apps/agent-gui/src-tauri/src/lib.rs`, add after `delete_profile_settings`:

```rust
crate::commands::move_profile_in_order,
crate::commands::open_config_dir,
```

In `apps/agent-gui/src-tauri/src/specta.rs`, add to `collect_commands!`:

```rust
move_profile_in_order,
open_config_dir,
```

- [ ] **Step 8: Build to verify compilation**

```bash
cargo build -p agent-gui
```

Expected: clean build.

- [ ] **Step 9: Run tests**

```bash
cargo test -p agent-runtime -- profile_settings
cargo test -p agent-core
```

- [ ] **Step 10: Commit**

```bash
git add crates/agent-runtime/src/profile_settings.rs \
        crates/agent-runtime/src/facade_runtime.rs \
        crates/agent-core/src/facade.rs \
        apps/agent-gui/src-tauri/src/commands.rs \
        apps/agent-gui/src-tauri/src/lib.rs \
        apps/agent-gui/src-tauri/src/specta.rs
git commit -m "feat(runtime): add display ordering and config dir commands

- display_order array in profiles.toml controls model list order
- move_profile_in_order command for up/down reordering
- open_config_dir command opens config directory in file manager
- Profiles not in display_order sort alphabetically at end

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: Add source filter to `list_profile_settings` (Issue #7 — backend)

**Files:**

- Modify: `crates/agent-runtime/src/profile_settings.rs:35-140`
- Modify: `crates/agent-runtime/src/facade_runtime.rs:1252`
- Modify: `crates/agent-core/src/facade.rs:745`
- Modify: `apps/agent-gui/src-tauri/src/commands.rs:1216`

- [ ] **Step 1: Add `source_filter` parameter to `list_profile_settings`**

Change the function signature:

```rust
pub async fn list_profile_settings(
    config: &Config,
    profiles_toml_path: Option<&Path>,
    user_config_path: Option<&Path>,
    project_config_path: Option<&Path>,
    source_filter: Option<&str>,
) -> agent_core::Result<Vec<ProfileSettingsView>> {
```

Skip layers based on filter value. When `source_filter == "user"`, skip layer 4 (project_config). When `source_filter == "project"`, skip layer 3 (user_config). When `None`, merge all layers as before.

```rust
// Layer 3: user config.toml (skip when source_filter == "project")
if source_filter != Some("project") {
    if let Some(path) = user_config_path {
        // ... existing layer 3 code ...
    }
}

// Layer 4: project config.toml (skip when source_filter == "user")
if source_filter != Some("user") {
    if let Some(path) = project_config_path {
        // ... existing layer 4 code ...
    }
}
```

- [ ] **Step 2: Update trait and implementations**

In `facade.rs`:

```rust
async fn list_profile_settings(
    &self,
    source_filter: Option<String>,
) -> crate::Result<Vec<ProfileSettingsView>> {
    let _ = source_filter;
    Ok(Vec::new())
}
```

In `facade_runtime.rs`:

```rust
async fn list_profile_settings(
    &self,
    source_filter: Option<String>,
) -> agent_core::Result<Vec<ProfileSettingsView>> {
    // ... same as before but pass source_filter.as_deref()
    crate::profile_settings::list_profile_settings(
        &self.config,
        profiles_toml_path.as_deref(),
        user_config_path.as_deref(),
        project_config_path.as_deref(),
        source_filter.as_deref(),
    )
    .await
}
```

In `commands.rs`:

```rust
pub async fn list_profile_settings(
    state: State<'_, GuiState>,
    source_filter: Option<String>,
) -> Result<Vec<ProfileSettingsView>, String> {
    state
        .runtime
        .list_profile_settings(source_filter)
        .await
        .map_err(|error| error.to_string())
}
```

- [ ] **Step 3: Build and test**

```bash
cargo build -p agent-gui
cargo test -p agent-runtime -- profile_settings
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime/src/profile_settings.rs \
        crates/agent-runtime/src/facade_runtime.rs \
        crates/agent-core/src/facade.rs \
        apps/agent-gui/src-tauri/src/commands.rs
git commit -m "feat(runtime): add source_filter to list_profile_settings

Supports filtering by 'user' or 'project' config source to enable
the settings UI toggle between user-level and project-level views.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: Regenerate TypeScript types and update test mock

**Files:**

- Regenerate: `apps/agent-gui/src/generated/commands.ts`
- Modify: `apps/agent-gui/e2e/tauri-mock.js`

- [ ] **Step 1: Regenerate types**

```bash
just gen-types
```

- [ ] **Step 2: Update Playwright mock**

In `apps/agent-gui/e2e/tauri-mock.js`, add mock implementations for new commands:

```javascript
// Find the existing listProfileSettings mock and update/add:
moveProfileInOrder: async () => ({ status: "ok", data: null }),
openConfigDir: async () => ({ status: "ok", data: "/mock/path/to/config" }),
```

If the mock file uses a registry pattern, find the right place to add these.

- [ ] **Step 3: Verify generated types include new commands**

```bash
grep -n "moveProfileInOrder\|openConfigDir\|sourceFilter" apps/agent-gui/src/generated/commands.ts
```

Expected: matching entries found.

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/generated/commands.ts \
        apps/agent-gui/e2e/tauri-mock.js
git commit -m "chore(gui): regenerate types and update mock for profile commands

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: Add i18n keys

**Files:**

- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

- [ ] **Step 1: Add English i18n keys**

Add to the `models` section in `en.json` after `sourceProjectConfig`:

```json
"moveUp": "Move Up",
"moveDown": "Move Down",
"sortAlpha": "Sort A-Z",
"showUserConfig": "User Config",
"showProjectConfig": "Project Config",
"openConfigDir": "Open Config Dir",
"advancedOptions": "Advanced",
"basicOptions": "Basic",
"connectionOptions": "Connection",
"sortBy": "Sort by"
```

- [ ] **Step 2: Add Chinese i18n keys**

Add to the `models` section in `zh-CN.json`:

```json
"moveUp": "上移",
"moveDown": "下移",
"sortAlpha": "按字母排序",
"showUserConfig": "用户配置",
"showProjectConfig": "项目配置",
"openConfigDir": "打开配置目录",
"advancedOptions": "高级选项",
"basicOptions": "基本",
"connectionOptions": "连接",
"sortBy": "排序"
```

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/locales/en.json \
        apps/agent-gui/src/locales/zh-CN.json
git commit -m "feat(gui): add model settings i18n keys for new features

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Write ModelSettingsPane unit tests (Issue #1)

**Files:**

- Create: `apps/agent-gui/src/components/ModelSettingsPane.test.ts`

- [ ] **Step 1: Create the test file with mock setup**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import { commands } from "@/generated/commands";
import ModelSettingsPane from "./ModelSettingsPane.vue";

vi.mock("@/generated/commands", () => ({
  commands: {
    listProfileSettings: vi.fn(),
    upsertProfileSettings: vi.fn(),
    setProfileEnabled: vi.fn(),
    deleteProfileSettings: vi.fn(),
    moveProfileInOrder: vi.fn(),
    openConfigDir: vi.fn()
  }
}));

const mockedCommands = vi.mocked(commands);

const writableProfile = {
  alias: "my-model",
  provider: "openai_compatible",
  model_id: "gpt-4.1",
  enabled: true,
  context_window: 128000,
  output_limit: 16384,
  temperature: 0.7,
  top_p: null,
  top_k: null,
  max_tokens: null,
  base_url: "https://api.openai.com/v1",
  api_key_env: "OPENAI_API_KEY",
  has_api_key: true,
  writable: true,
  config_path: "/tmp/profiles.toml",
  source: "profiles_toml"
};

const readOnlyProfile = {
  alias: "fast",
  provider: "openai_compatible",
  model_id: "gpt-4.1-mini",
  enabled: true,
  context_window: 128000,
  output_limit: 16384,
  temperature: null,
  top_p: null,
  top_k: null,
  max_tokens: null,
  base_url: "https://api.openai.com/v1",
  api_key_env: "OPENAI_API_KEY",
  has_api_key: true,
  writable: false,
  config_path: null,
  source: "user_config"
};

const disabledProfile = {
  alias: "slow-model",
  provider: "anthropic",
  model_id: "claude-opus-4-7",
  enabled: false,
  context_window: 200000,
  output_limit: 32000,
  temperature: null,
  top_p: null,
  top_k: null,
  max_tokens: null,
  base_url: null,
  api_key_env: null,
  has_api_key: false,
  writable: true,
  config_path: "/tmp/profiles.toml",
  source: "profiles_toml"
};

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function mountPane() {
  const mountOptions: MountWithPluginsOptions<typeof ModelSettingsPane> = {
    reusePinia: true
  };
  return mountWithPlugins(ModelSettingsPane, mountOptions).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listProfileSettings.mockResolvedValue(
    ok([writableProfile, readOnlyProfile, disabledProfile])
  );
  mockedCommands.upsertProfileSettings.mockResolvedValue(ok(writableProfile));
  mockedCommands.setProfileEnabled.mockResolvedValue(ok(null));
  mockedCommands.deleteProfileSettings.mockResolvedValue(ok(null));
  mockedCommands.moveProfileInOrder.mockResolvedValue(ok(null));
  mockedCommands.openConfigDir.mockResolvedValue(ok("/tmp/config"));
});
```

- [ ] **Step 2: Test — renders profile list**

```typescript
describe("ModelSettingsPane", () => {
  it("renders profile list with correct data", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="model-row-my-model"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-row-fast"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-row-slow-model"]').exists()).toBe(true);

    // Enabled/disabled tags
    const myModelRow = wrapper.find('[data-test="model-row-my-model"]');
    expect(myModelRow.text()).toContain("Enabled");
    expect(myModelRow.text()).toContain("Has API Key");

    const slowRow = wrapper.find('[data-test="model-row-slow-model"]');
    expect(slowRow.text()).toContain("Disabled");
  });
```

- [ ] **Step 3: Test — refresh button**

```typescript
it("refresh button reloads profiles", async () => {
  const wrapper = mountPane();
  await flushPromises();
  expect(mockedCommands.listProfileSettings).toHaveBeenCalledTimes(1);

  await wrapper.find('[data-test="model-refresh"]').trigger("click");
  expect(mockedCommands.listProfileSettings).toHaveBeenCalledTimes(2);
});
```

- [ ] **Step 4: Test — add dialog**

```typescript
it("add dialog opens, validates required fields, and saves", async () => {
  const wrapper = mountPane();
  await flushPromises();

  // Open dialog
  await wrapper.find('[data-test="model-add-profile"]').trigger("click");
  expect(wrapper.find('[data-test="model-add-dialog"]').isVisible()).toBe(true);

  // Save with empty fields should not call upsert
  await wrapper.find('[data-test="model-save-button"]').trigger("click");
  expect(mockedCommands.upsertProfileSettings).not.toHaveBeenCalled();

  // Fill required fields
  await wrapper.find('[data-test="model-form-alias"]').setValue("new-model");
  await wrapper.find('[data-test="model-form-provider"]').setValue("ollama");
  await wrapper.find('[data-test="model-form-model-id"]').setValue("llama3");

  await wrapper.find('[data-test="model-save-button"]').trigger("click");
  await flushPromises();

  expect(mockedCommands.upsertProfileSettings).toHaveBeenCalledWith(
    expect.objectContaining({
      alias: "new-model",
      provider: "ollama",
      model_id: "llama3",
      enabled: true
    })
  );
});
```

- [ ] **Step 5: Test — edit dialog**

```typescript
it("edit dialog opens pre-filled and saves changes", async () => {
  const wrapper = mountPane();
  await flushPromises();

  await wrapper.find('[data-test="model-edit-my-model"]').trigger("click");
  expect(wrapper.find('[data-test="model-edit-dialog"]').isVisible()).toBe(true);

  // Alias should be readonly with correct value
  const aliasInput = wrapper.find('[data-test="model-edit-alias"]');
  expect((aliasInput.element as HTMLInputElement).value).toBe("my-model");
  expect((aliasInput.element as HTMLInputElement).readOnly).toBe(true);

  // Change provider
  await wrapper.find('[data-test="model-edit-provider"]').setValue("anthropic");
  await wrapper.find('[data-test="model-edit-save-button"]').trigger("click");
  await flushPromises();

  expect(mockedCommands.upsertProfileSettings).toHaveBeenCalledWith(
    expect.objectContaining({ provider: "anthropic" })
  );
});
```

- [ ] **Step 6: Test — toggle enable/disable**

```typescript
it("toggle button disables an enabled profile", async () => {
  const wrapper = mountPane();
  await flushPromises();

  const myModelRow = wrapper.find('[data-test="model-row-my-model"]');
  await myModelRow.find('[data-test="model-enable-my-model"]').trigger("click");

  expect(mockedCommands.setProfileEnabled).toHaveBeenCalledWith("my-model", false);
});
```

- [ ] **Step 7: Test — delete (writable only)**

```typescript
it("delete button only appears for writable profiles", async () => {
  const wrapper = mountPane();
  await flushPromises();

  expect(wrapper.find('[data-test="model-delete-my-model"]').exists()).toBe(true);
  expect(wrapper.find('[data-test="model-delete-fast"]').exists()).toBe(false);
});

it("delete button removes profile", async () => {
  const wrapper = mountPane();
  await flushPromises();

  await wrapper.find('[data-test="model-delete-my-model"]').trigger("click");
  expect(mockedCommands.deleteProfileSettings).toHaveBeenCalledWith("my-model");
});
```

- [ ] **Step 8: Test — move up/down**

```typescript
it("move up/down buttons call moveProfileInOrder", async () => {
  const wrapper = mountPane();
  await flushPromises();

  await wrapper.find('[data-test="model-move-up-my-model"]').trigger("click");
  expect(mockedCommands.moveProfileInOrder).toHaveBeenCalledWith("my-model", -1);

  await wrapper.find('[data-test="model-move-down-my-model"]').trigger("click");
  expect(mockedCommands.moveProfileInOrder).toHaveBeenCalledWith("my-model", 1);
});
```

- [ ] **Step 9: Test — source toggle**

```typescript
it("source toggle switches between user and project views", async () => {
  const wrapper = mountPane();
  await flushPromises();

  const select = wrapper.find('[data-test="model-source-filter"]');
  expect(select.exists()).toBe(true);

  await select.setValue("project");
  expect(mockedCommands.listProfileSettings).toHaveBeenCalledWith("project");
});
```

- [ ] **Step 10: Test — open config dir**

```typescript
it("open config dir button calls openConfigDir", async () => {
  const wrapper = mountPane();
  await flushPromises();

  await wrapper.find('[data-test="model-open-config-dir"]').trigger("click");
  expect(mockedCommands.openConfigDir).toHaveBeenCalled();
});
```

- [ ] **Step 11: Test — error and loading states**

```typescript
  it("shows error message on fetch failure", async () => {
    mockedCommands.listProfileSettings.mockRejectedValue(new Error("fetch failed"));
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="model-page-error"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-page-error"]').text()).toContain("fetch failed");
  });

  it("shows empty state when no profiles", async () => {
    mockedCommands.listProfileSettings.mockResolvedValue(ok([]));
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.text()).toContain("No model profiles configured");
  });
});
```

- [ ] **Step 12: Run tests to verify they fail (no implementation yet)**

```bash
pnpm --filter agent-gui run test -- ModelSettingsPane
```

Expected: test file parsed but tests fail because the new component features aren't implemented yet.

- [ ] **Step 13: Commit**

```bash
git add apps/agent-gui/src/components/ModelSettingsPane.test.ts
git commit -m "test(gui): add ModelSettingsPane unit tests

Tests cover: render, refresh, add/edit dialog, toggle, delete,
move up/down, source filter, open config dir, error/loading states.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: Rework ModelSettingsPane.vue — script and toolbar (Issues #2, #6, #7, #8, #9)

**Files:**

- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue`

- [ ] **Step 1: Update script — add new state and methods**

Replace the script section. Keep existing imports and add new state:

```typescript
<script setup lang="ts">
import type { ProfileSettingsView } from "@/generated/commands";
import { commands } from "@/generated/commands";

const { t } = useI18n();
const profiles = ref<ProfileSettingsView[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const busyAlias = ref<string | null>(null);
const addDialogOpen = ref(false);
const editDialogOpen = ref(false);
const editingProfile = ref<ProfileSettingsView | null>(null);
const sourceFilter = ref<string | null>(null); // null = all, "user", "project"
const advancedOpen = ref(false); // for collapsible advanced section in form
const editAdvancedOpen = ref(false);
// Form fields (same as before)
const formAlias = ref("");
const formProvider = ref("");
const formModelId = ref("");
const formContextWindow = ref("");
const formOutputLimit = ref("");
const formTemperature = ref("");
const formTopP = ref("");
const formTopK = ref("");
const formMaxTokens = ref("");
const formBaseUrl = ref("");
const formApiKeyEnv = ref("");
```

- [ ] **Step 2: Update `fetchProfiles` to pass source_filter**

```typescript
async function fetchProfiles(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    profiles.value = await unwrapCommandResult(commands.listProfileSettings(sourceFilter.value));
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    loading.value = false;
  }
}
```

- [ ] **Step 3: Add `changeSourceFilter` and `moveProfile` methods**

```typescript
function changeSourceFilter(value: string): void {
  sourceFilter.value = value || null;
  void fetchProfiles();
}

async function moveProfile(alias: string, direction: number): Promise<void> {
  busyAlias.value = alias;
  error.value = null;
  try {
    await unwrapCommandResult(commands.moveProfileInOrder(alias, direction));
    await fetchProfiles();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    busyAlias.value = null;
  }
}

async function openConfigDir(): Promise<void> {
  try {
    await unwrapCommandResult(commands.openConfigDir());
  } catch (caughtError) {
    error.value = formatError(caughtError);
  }
}
```

- [ ] **Step 4: Keep all existing methods unchanged** (`formatError`, `isCommandResult`, `unwrapCommandResult`, `resetForm`, `openAddDialog`, `closeAddDialog`, `openEditDialog`, `closeEditDialog`, `parseOptionalNumber`, `saveNewProfile`, `saveEditProfile`, `toggleProfile`, `deleteProfile`)

- [ ] **Step 5: Update form open methods to reset advanced state**

In `resetForm()`: add `advancedOpen.value = false; editAdvancedOpen.value = false;`

In `openAddDialog()`: call `resetForm()` (already does), keep `advancedOpen.value = false`

In `closeEditDialog()`: add `editAdvancedOpen.value = false`

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/components/ModelSettingsPane.vue
git commit -m "feat(gui): add model settings script logic for ordering, filter, dir

- source_filter param on fetchProfiles
- moveProfile and openConfigDir actions
- Advanced section collapse state for add/edit forms

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 9: Rework ModelSettingsPane.vue — toolbar template (Issues #7, #8, #9)

**Files:**

- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue` (template section)

- [ ] **Step 1: Replace the existing toolbar with new layout**

Replace the existing `<div class="mcp-toolbar">` block (lines 240-258) with:

```html
<div class="model-toolbar">
  <div class="model-toolbar__left">
    <select
      v-model="sourceFilter"
      class="model-source-select"
      data-test="model-source-filter"
      @change="changeSourceFilter(($event.target as HTMLSelectElement).value)"
    >
      <option value="">{{ t("models.showUserConfig") }}</option>
      <option value="project">{{ t("models.showProjectConfig") }}</option>
    </select>
  </div>
  <div class="model-toolbar__actions">
    <button
      class="btn btn-sm"
      type="button"
      :disabled="loading"
      data-test="model-refresh"
      @click="fetchProfiles()"
    >
      {{ loading ? t("common.loading") : t("common.refresh") }}
    </button>
    <button
      class="btn btn-sm btn-primary"
      type="button"
      data-test="model-add-profile"
      @click="openAddDialog()"
    >
      {{ t("models.addProfile") }}
    </button>
    <button
      class="btn btn-sm"
      type="button"
      data-test="model-open-config-dir"
      @click="openConfigDir()"
    >
      {{ t("models.openConfigDir") }}
    </button>
  </div>
</div>
```

- [ ] **Step 2: Update the empty state message to keep it consistent**

Keep `<p v-if="profiles.length === 0">` unchanged.

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/components/ModelSettingsPane.vue
git commit -m "feat(gui): add model settings toolbar with source filter, consistent buttons

- Source toggle dropdown for user/project config views
- All toolbar buttons use consistent btn-sm sizing
- Open config dir button added
- Proper horizontal gap between toolbar elements

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 10: Rework ModelSettingsPane.vue — list items with move buttons (Issues #6, #8)

**Files:**

- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue` (list template)

- [ ] **Step 1: Add move up/down buttons to each list item and remove source tag**

Replace the existing list item template (lines 268-330). Keep the article/card structure but add move buttons and remove the source tag:

```html
<div v-else class="model-settings__list" role="list" aria-label="Configured model profiles">
  <article
    v-for="(profile, index) in profiles"
    :key="profile.alias"
    class="card model-settings__profile"
    role="listitem"
    :data-test="`model-row-${profile.alias}`"
  >
    <div class="card-body model-settings__profile-body">
      <div class="model-settings__profile-main">
        <h3>{{ profile.alias }}</h3>
        <p>{{ profile.provider }} / {{ profile.model_id }}</p>
        <div class="mcp-settings__tags" aria-label="Profile metadata">
          <span :class="['tag', profile.enabled ? 'tag-success' : 'tag-warning']">
            {{ profile.enabled ? t("models.enabled") : t("models.disabled") }}
          </span>
          <span :class="['tag', profile.has_api_key ? 'tag-success' : 'tag-warning']">
            {{ profile.has_api_key ? t("models.hasApiKey") : t("models.noApiKey") }}
          </span>
          <span v-if="profile.context_window" class="tag">
            {{ t("models.contextWindow") }}: {{ profile.context_window.toLocaleString() }}
          </span>
          <span v-if="profile.output_limit" class="tag">
            {{ t("models.outputLimit") }}: {{ profile.output_limit.toLocaleString() }}
          </span>
          <span v-if="profile.temperature != null" class="tag">
            {{ t("models.temperature") }}: {{ profile.temperature }}
          </span>
        </div>
      </div>

      <div class="model-settings__actions" aria-label="Profile actions">
        <div class="model-settings__reorder">
          <button
            class="btn btn-sm btn-icon"
            type="button"
            :disabled="busyAlias === profile.alias || index === 0"
            :data-test="`model-move-up-${profile.alias}`"
            :title="t('models.moveUp')"
            @click="moveProfile(profile.alias, -1)"
          >
            ▲
          </button>
          <button
            class="btn btn-sm btn-icon"
            type="button"
            :disabled="busyAlias === profile.alias || index === profiles.length - 1"
            :data-test="`model-move-down-${profile.alias}`"
            :title="t('models.moveDown')"
            @click="moveProfile(profile.alias, 1)"
          >
            ▼
          </button>
        </div>
        <button
          class="btn btn-sm"
          type="button"
          :disabled="busyAlias === profile.alias"
          :data-test="`model-edit-${profile.alias}`"
          @click="openEditDialog(profile)"
        >
          {{ t("common.edit") }}
        </button>
        <button
          class="btn btn-sm"
          type="button"
          :disabled="busyAlias === profile.alias"
          :data-test="`model-enable-${profile.alias}`"
          @click="toggleProfile(profile)"
        >
          {{ profile.enabled ? t("models.disable") : t("models.enable") }}
        </button>
        <button
          v-if="profile.writable"
          class="btn btn-danger btn-sm"
          type="button"
          :disabled="busyAlias === profile.alias"
          :data-test="`model-delete-${profile.alias}`"
          @click="deleteProfile(profile)"
        >
          {{ t("common.delete") }}
        </button>
      </div>
    </div>
  </article>
</div>
```

Key changes from original:

1. Added `(profile, index)` to `v-for`
2. Added move up/down arrow buttons with disabled logic for first/last
3. Removed the `<span class="tag tag-muted">{{ sourceLabel(profile.source) }}</span>` source tag
4. `index === 0` disables ▲ on first item; `index === profiles.length - 1` disables ▼ on last

Also remove the now-unused `sourceLabel` function (lines 218-231 in original).

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/ModelSettingsPane.vue
git commit -m "feat(gui): add move up/down buttons, remove source tags from model list

Each profile row now has ▲/▼ reorder buttons. Source tags removed
since the toolbar toggle already communicates which config level is
shown. Removed unused sourceLabel function.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 11: Rework ModelSettingsPane.vue — add/edit form with grid layout (Issue #3)

**Files:**

- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue` (form template + styles)

- [ ] **Step 1: Replace the Add Profile dialog form**

Replace the existing Add Profile form (lines 341-425) with a grid layout:

```html
<form class="model-form" data-test="model-add-form" @submit.prevent="saveNewProfile">
  <fieldset class="model-form__section">
    <legend>{{ t("models.basicOptions") }}</legend>
    <div class="model-form__grid model-form__grid--2col">
      <label>
        <span>{{ t("models.alias") }} *</span>
        <input id="model-add-alias" v-model="formAlias" data-test="model-form-alias" required />
      </label>
      <label>
        <span>{{ t("models.provider") }} *</span>
        <input
          id="model-add-provider"
          v-model="formProvider"
          data-test="model-form-provider"
          required
        />
      </label>
    </div>
    <label>
      <span>{{ t("models.modelId") }} *</span>
      <input
        id="model-add-model-id"
        v-model="formModelId"
        data-test="model-form-model-id"
        required
      />
    </label>
  </fieldset>

  <fieldset class="model-form__section">
    <legend>{{ t("models.connectionOptions") }}</legend>
    <label>
      <span>{{ t("models.baseUrl") }}</span>
      <input id="model-add-base-url" v-model="formBaseUrl" data-test="model-form-base-url" />
    </label>
    <label>
      <span>{{ t("models.apiKeyEnv") }}</span>
      <input
        id="model-add-api-key-env"
        v-model="formApiKeyEnv"
        data-test="model-form-api-key-env"
      />
    </label>
  </fieldset>

  <fieldset class="model-form__section">
    <legend>
      <button
        type="button"
        class="btn btn-sm model-form__toggle"
        @click="advancedOpen = !advancedOpen"
      >
        {{ advancedOpen ? "▾" : "▸" }} {{ t("models.advancedOptions") }}
      </button>
    </legend>
    <div v-if="advancedOpen" class="model-form__grid model-form__grid--3col">
      <label>
        <span>{{ t("models.contextWindow") }}</span>
        <input
          id="model-add-ctx"
          v-model="formContextWindow"
          type="number"
          data-test="model-form-ctx"
        />
      </label>
      <label>
        <span>{{ t("models.outputLimit") }}</span>
        <input
          id="model-add-out"
          v-model="formOutputLimit"
          type="number"
          data-test="model-form-out"
        />
      </label>
      <label>
        <span>{{ t("models.temperature") }}</span>
        <input
          id="model-add-temp"
          v-model="formTemperature"
          type="number"
          step="0.1"
          min="0"
          max="2"
          data-test="model-form-temp"
        />
      </label>
      <label>
        <span>{{ t("models.topP") }}</span>
        <input
          id="model-add-top-p"
          v-model="formTopP"
          type="number"
          step="0.1"
          min="0"
          max="1"
          data-test="model-form-top-p"
        />
      </label>
      <label>
        <span>{{ t("models.topK") }}</span>
        <input
          id="model-add-top-k"
          v-model="formTopK"
          type="number"
          min="0"
          data-test="model-form-top-k"
        />
      </label>
      <label>
        <span>{{ t("models.maxTokens") }}</span>
        <input
          id="model-add-max-tokens"
          v-model="formMaxTokens"
          type="number"
          data-test="model-form-max-tokens"
        />
      </label>
    </div>
  </fieldset>
</form>
```

- [ ] **Step 2: Replace the Edit Profile dialog form similarly**

Follow the same grid structure as the add form, but using `model-edit-*` data-test IDs and the `editAdvancedOpen` ref for the collapsible section. The alias field should remain readonly.

- [ ] **Step 3: Replace scoped styles**

Replace the existing `<style scoped>` section:

```css
<style scoped>
.model-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.model-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
}

.model-toolbar__left {
  display: flex;
  align-items: center;
}

.model-toolbar__actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.model-source-select {
  padding: 4px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-surface);
  color: var(--color-text);
  font-size: 0.85rem;
}

.model-settings__list {
  display: grid;
  gap: 12px;
}

.model-settings__profile-body {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
}

.model-settings__profile-main {
  min-width: 0;
  display: grid;
  gap: 8px;
  flex: 1;
}

.model-settings__profile h3 {
  margin: 0 0 4px;
}

.model-settings__actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.model-settings__reorder {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-right: 4px;
}

.btn-icon {
  padding: 2px 6px;
  line-height: 1;
  font-size: 0.7rem;
}

/* Form styles */
.model-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.model-form__section {
  border: none;
  padding: 0;
  margin: 0;
}

.model-form__section legend {
  font-weight: 600;
  font-size: 0.9rem;
  margin-bottom: 8px;
  color: var(--color-text-muted);
  width: 100%;
}

.model-form__toggle {
  all: unset;
  cursor: pointer;
  font-weight: 600;
  font-size: 0.9rem;
  color: var(--color-text-muted);
}

.model-form__toggle:hover {
  color: var(--color-text);
}

.model-form__grid {
  display: grid;
  gap: 8px;
}

.model-form__grid--2col {
  grid-template-columns: 1fr 1fr;
}

.model-form__grid--3col {
  grid-template-columns: 1fr 1fr 1fr;
}

.model-form label {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.model-form label > span {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--color-text-muted);
}

.model-form input {
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-surface);
  color: var(--color-text);
  font-size: 0.85rem;
}

.model-form input:focus {
  border-color: var(--color-primary);
  outline: none;
}
</style>
```

- [ ] **Step 4: Run the unit tests to check what passes**

```bash
pnpm --filter agent-gui run test -- ModelSettingsPane
```

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/components/ModelSettingsPane.vue
git commit -m "feat(gui): rework model add/edit form with grid layout and sections

- Basic/Connection/Advanced sections with CSS grid layout
- Advanced section is collapsible, default collapsed
- Form fields organized into 2-col and 3-col grids

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 12: Fix ChatPanel model switcher order (Issue #2)

**Files:**

- Modify: `apps/agent-gui/src/components/ChatPanel.vue:186`

- [ ] **Step 1: Sort modelOptions to match settings page ordering**

Replace line 186:

```typescript
const modelOptions = computed<ProfileInfo[]>(() => session.profileInfos);
```

With:

```typescript
const modelOptions = computed<ProfileInfo[]>(() =>
  [...session.profileInfos].sort((a, b) => a.alias.localeCompare(b.alias))
);
```

This sorts alphabetically by alias, which matches the settings page ordering when no custom `display_order` is set. If we want to sync with the actual display_order from profiles.toml, we'd need the backend to return it. For now, alphabetical is consistent and predictable.

- [ ] **Step 2: Verify with existing tests**

```bash
pnpm --filter agent-gui run test -- ChatPanel
```

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/components/ChatPanel.vue
git commit -m "fix(gui): sort chat model switcher options alphabetically

Matches the settings page default ordering for consistency.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 13: Run full test suite and fix issues

**Files:**

- All modified files

- [ ] **Step 1: Run all Rust tests**

```bash
cargo test --workspace --all-targets
```

- [ ] **Step 2: Run all frontend tests**

```bash
pnpm --filter agent-gui run test
```

- [ ] **Step 3: Run lint and format**

```bash
pnpm run lint
pnpm run format:check
```

- [ ] **Step 4: Fix any failures**

Iterate on any test/lint failures.

- [ ] **Step 5: Regenerate types if needed**

```bash
just gen-types
```

- [ ] **Step 6: Commit any fixes**

```bash
git add -A
git commit -m "chore: fix test/lint issues from model settings changes

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 14: Final verification

- [ ] **Step 1: Confirm all 10 issues are addressed**

| #   | Issue                 | How verified                                           |
| --- | --------------------- | ------------------------------------------------------ |
| 1   | Test coverage         | `ModelSettingsPane.test.ts` exists with 10+ test cases |
| 2   | Chat panel order      | `ChatPanel.vue` sorts options alphabetically           |
| 3   | Form UI               | Grid layout with collapsible sections                  |
| 4   | local-code showing    | Filtered in `list_profile_settings`                    |
| 5   | api_key_env detection | Fixed `has_api_key` to use row data                    |
| 6   | Reordering            | Move up/down buttons + display_order                   |
| 7   | User/project toggle   | Source filter dropdown in toolbar                      |
| 8   | Config dir button     | `openConfigDir` button, source tags removed            |
| 9   | Button styling        | Consistent btn-sm with gap in toolbar                  |
| 10  | Enable/disable toggle | Seeds full ProfileDef on first toggle                  |

- [ ] **Step 2: Run full suite one final time**

```bash
cargo test --workspace --all-targets && pnpm --filter agent-gui run test && pnpm run lint
```

- [ ] **Step 3: Final commit if any cleanup needed**

```bash
git add -A && git commit -m "chore: final cleanup for model settings polish"
```
