# Settings Panel Optimization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Optimize the settings panel with archive tab, config source system (project > user priority), source selectors on MCP/Skills/Models, and fix `has_api_key` detection.

**Architecture:** Restructure `SettingsView.vue` flat-tab into nested routes under a shared `SettingsLayout` that renders a `ConfigSourceBar` component on MCP/Skills/Models routes. Extract `useModelStore` Pinia store for Models. Add `source` field to `McpServerSettingsView` in Rust. Add three new Rust commands: `restore_archived_session`, `permanently_delete_session`, `open_skills_dir`.

**Tech Stack:** Rust (agent-core, agent-runtime, agent-store) + Vue 3 / TypeScript / Pinia (Tauri 2) + SQLite

---

## File Structure

**New files:**

- `apps/agent-gui/src/layouts/SettingsLayout.vue` — tab navigation + ConfigSourceBar + `<router-view>`
- `apps/agent-gui/src/components/ConfigSourceBar.vue` — shared config source selector
- `apps/agent-gui/src/views/settings/GeneralSettings.vue` — extracted general settings
- `apps/agent-gui/src/components/ArchiveSettingsPane.vue` — archived sessions list
- `apps/agent-gui/src/stores/models.ts` — Pinia store for model profiles

**Modified files:**

- `crates/agent-core/src/facade.rs` — add `source` to `McpServerSettingsView`
- `crates/agent-runtime/src/profile_settings.rs` — fix `has_api_key`
- `crates/agent-runtime/src/mcp_settings.rs` — tag MCP servers with source
- `crates/agent-runtime/src/facade_mcp.rs` — new commands + source-aware listing
- `crates/agent-runtime/src/facade_skills.rs` — add `open_skills_dir`
- `crates/agent-runtime/src/facade_projects.rs` — add restore/permanent-delete
- `crates/agent-core/src/facade/session.rs` — trait method additions
- `crates/agent-core/src/facade/skills.rs` — trait method additions
- `crates/agent-store/src/event_store.rs` — hard-delete + restore visibility
- `apps/agent-gui/src-tauri/src/commands.rs` — new Tauri commands
- `apps/agent-gui/src-tauri/src/lib.rs` — register new commands
- `apps/agent-gui/src-tauri/src/specta.rs` — collect new commands
- `apps/agent-gui/src/router/routes.ts` — nested routes
- `apps/agent-gui/src/views/SettingsView.vue` — repurpose or remove
- `apps/agent-gui/src/components/McpSettingsPane.vue` — source-aware loading
- `apps/agent-gui/src/components/SkillSettingsPane.vue` — source-aware loading
- `apps/agent-gui/src/components/ModelSettingsPane.vue` — use `useModelStore`
- `apps/agent-gui/src/stores/project.ts` — `pathExists` field
- `apps/agent-gui/src/locales/en.json` — new i18n keys
- `apps/agent-gui/src/locales/zh-CN.json` — new i18n keys

---

### Task 1: Fix `has_api_key` detection in Rust

**Files:**

- Modify: `crates/agent-runtime/src/profile_settings.rs:115-119`

- [ ] **Step 1: Add `api_key` field to `ProfileSettingsRow`**

The struct at line 18 needs a new field to capture the direct `api_key` from config. Add it after `api_key_env`:

```rust
// Line 29, after `api_key_env`:
    api_key: Option<String>,
```

- [ ] **Step 2: Update all `ProfileSettingsRow` constructors**

In `list_profile_settings()` (line 48), add `api_key: def.api_key.clone()` to the defaults layer.

In `profile_row_from_toml_table()` (line 178), add extraction:

```rust
api_key: table
    .and_then(|t| t.get("api_key"))
    .and_then(Item::as_str)
    .map(ToString::to_string),
```

In the `has_api_key` computation at line 115-119, replace with:

```rust
let has_api_key = row.api_key.is_some()
    || row
        .api_key_env
        .as_ref()
        .is_some_and(|v| std::env::var(v).is_ok());
```

- [ ] **Step 3: Run Rust tests**

```bash
cargo test -p agent-runtime --lib profile_settings
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime/src/profile_settings.rs
git commit -m "fix(agent-runtime): check api_key field in has_api_key detection

Previously only checked api_key_env (env var name). Profiles with a
direct api_key value showed '未配置 API Key' incorrectly.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: Add `source` field to `McpServerSettingsView`

**Files:**

- Modify: `crates/agent-core/src/facade.rs:225-237`

- [ ] **Step 1: Add source field to struct**

Add `pub source: String` after `description`:

```rust
pub struct McpServerSettingsView {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub runtime_status: String,
    pub trusted: bool,
    pub tool_count: Option<usize>,
    pub last_error: Option<String>,
    pub writable: bool,
    pub config_path: Option<String>,
    pub description: Option<String>,
    pub source: String,
}
```

- [ ] **Step 2: Compile to find all construction sites**

```bash
cargo check --workspace 2>&1 | grep -A2 "McpServerSettingsView"
```

Fix each construction site by adding `source: "user_config".to_string()` as a placeholder (will be properly set in Task 4).

- [ ] **Step 3: Commit**

```bash
git add crates/agent-core/src/facade.rs
# also add any other files that needed fixing for compilation
git commit -m "feat(agent-core): add source field to McpServerSettingsView

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: Add restore / permanent-delete session commands

**Files:**

- Modify: `crates/agent-core/src/facade/session.rs` — add trait methods
- Modify: `crates/agent-core/src/facade/project.rs` — add `restore_archived_session`
- Modify: `crates/agent-store/src/event_store.rs` — add impl
- Modify: `crates/agent-runtime/src/facade_runtime.rs` — delegate to store
- Modify: `crates/agent-runtime/src/facade_projects.rs` — implement restore
- Modify: `apps/agent-gui/src-tauri/src/commands.rs` — add Tauri commands
- Modify: `apps/agent-gui/src-tauri/src/lib.rs` — register commands
- Modify: `apps/agent-gui/src-tauri/src/specta.rs` — collect commands

- [ ] **Step 1: Add trait methods to `SessionFacade`**

In `crates/agent-core/src/facade/session.rs`, add after `soft_delete_session`:

```rust
async fn permanently_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
    let _ = session_id;
    Ok(())
}

async fn restore_archived_session(&self, session_id: &SessionId) -> crate::Result<()> {
    let _ = session_id;
    Ok(())
}
```

- [ ] **Step 2: Implement in `EventStore` trait**

In `crates/agent-store/src/event_store.rs`, add to the trait after `soft_delete_session`:

```rust
async fn permanently_delete_session(&self, session_id: &str) -> crate::Result<()>;
async fn restore_archived_session(&self, session_id: &str) -> crate::Result<()>;
```

- [ ] **Step 3: Implement in `SqliteEventStore`**

In `crates/agent-store/src/event_store.rs`, add after the `soft_delete_session` implementation:

```rust
async fn permanently_delete_session(&self, session_id: &str) -> crate::Result<()> {
    sqlx::query("DELETE FROM events WHERE session_id = ?1")
        .bind(session_id)
        .execute(&self.pool)
        .await?;
    sqlx::query("DELETE FROM kairox_session_visibility WHERE session_id = ?1")
        .bind(session_id)
        .execute(&self.pool)
        .await?;
    sqlx::query("DELETE FROM kairox_project_sessions WHERE session_id = ?1")
        .bind(session_id)
        .execute(&self.pool)
        .await?;
    sqlx::query("DELETE FROM kairox_sessions WHERE session_id = ?1")
        .bind(session_id)
        .execute(&self.pool)
        .await?;
    Ok(())
}

async fn restore_archived_session(&self, session_id: &str) -> crate::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO kairox_session_visibility (session_id, visibility, updated_at)
         VALUES (?1, 'visible', ?2)
         ON CONFLICT(session_id) DO UPDATE SET visibility = 'visible', updated_at = ?2",
    )
    .bind(session_id)
    .bind(&now)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 4: Implement `AppFacade` trait methods in `facade.rs`**

In `crates/agent-core/src/facade.rs`, in the `AppFacade` impl, add after the `soft_delete_session` delegation:

```rust
async fn permanently_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
    SessionFacade::permanently_delete_session(self, session_id).await
}

async fn restore_archived_session(&self, session_id: &SessionId) -> crate::Result<()> {
    SessionFacade::restore_archived_session(self, session_id).await
}
```

- [ ] **Step 5: Implement in `LocalRuntime`**

In `crates/agent-runtime/src/facade_runtime.rs`, after `soft_delete_session` impl:

```rust
async fn permanently_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
    crate::session::permanently_delete_session(&*self.store, session_id.as_str()).await
}

async fn restore_archived_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
    crate::session::restore_archived_session(&*self.store, session_id.as_str()).await
}
```

- [ ] **Step 6: Add `crate::session` helper functions**

In `crates/agent-runtime/src/session.rs` (check file name), add after `soft_delete_session`:

```rust
pub async fn permanently_delete_session(
    store: &dyn EventStore,
    session_id: &str,
) -> agent_core::Result<()> {
    store.permanently_delete_session(session_id).await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
}

pub async fn restore_archived_session(
    store: &dyn EventStore,
    session_id: &str,
) -> agent_core::Result<()> {
    store.restore_archived_session(session_id).await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
}
```

- [ ] **Step 7: Add Tauri commands in `commands.rs`**

After the existing `delete_session` command:

```rust
#[tauri::command]
#[specta::specta]
pub async fn permanently_delete_session(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .permanently_delete_session(&sid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn restore_archived_session(
    session_id: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let sid: agent_core::SessionId = session_id.into();
    state
        .runtime
        .restore_archived_session(&sid)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 8: Register in `lib.rs`**

Add after `delete_session`:

```rust
crate::commands::permanently_delete_session,
crate::commands::restore_archived_session,
```

- [ ] **Step 9: Register in `specta.rs`**

Add after `delete_session`:

```rust
permanently_delete_session,
restore_archived_session,
```

- [ ] **Step 10: Build and verify**

```bash
cargo check --workspace
```

Expected: no errors.

- [ ] **Step 11: Commit**

```bash
git add crates/agent-core/src/facade.rs crates/agent-core/src/facade/session.rs \
        crates/agent-store/src/event_store.rs crates/agent-runtime/src/facade_runtime.rs \
        crates/agent-runtime/src/facade_projects.rs crates/agent-runtime/src/session.rs \
        apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/lib.rs \
        apps/agent-gui/src-tauri/src/specta.rs
git commit -m "feat(runtime): add restore_archived_session and permanently_delete_session commands

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: Tag MCP server settings with config source

**Files:**

- Modify: `crates/agent-runtime/src/mcp_settings.rs`

- [ ] **Step 1: Add source tracking to MCP settings rows**

Introduce a local row type to carry source info. In `mcp_settings.rs`, add a struct:

```rust
struct McpSettingsRow {
    name: String,
    transport: String,
    enabled: bool,
    description: Option<String>,
    source: String,
    writable: bool,
}
```

- [ ] **Step 2: Refactor `settings_rows_from_config`**

Return `HashMap<String, McpSettingsRow>` instead of the current tuple type. Tag entries from `Config` as source `"user_config"`.

- [ ] **Step 3: Refactor `settings_rows_from_file`**

Accept an additional `source: &str` parameter. When loading from file, tag entries with the provided source.

- [ ] **Step 4: Update `list_mcp_server_settings` signature**

Add parameters for user config path and project config path:

```rust
pub async fn list_mcp_server_settings(
    config: &Config,
    user_config_path: Option<&Path>,
    project_config_path: Option<&Path>,
    source_filter: Option<&str>,
    manager: Option<Arc<Mutex<McpServerManager>>>,
) -> agent_core::Result<Vec<McpServerSettingsView>> {
```

Layer logic: load config defaults, then user file (source = "user_config"), then project file (source = "project_config"). When `source_filter = "user"`, skip project file. When `source_filter = "project"`, skip user file (but still include config defaults for merging).

- [ ] **Step 5: Update the `McpServerSettingsView` construction**

Include `source: row.source.clone()` in the struct literal.

- [ ] **Step 6: Update `LocalRuntime::list_mcp_server_settings` in `facade_mcp.rs`**

Pass user_config_path and project_config_path:

```rust
pub(crate) async fn list_mcp_server_settings(
    &self,
    source_filter: Option<String>,
) -> agent_core::Result<Vec<McpServerSettingsView>> {
    let writable_path =
        crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?;
    let user_config_path = std::env::var("HOME").ok().map(|h| {
        std::path::PathBuf::from(h).join(".kairox").join("mcp_servers.toml")
    });
    let project_config_path = std::env::current_dir()
        .ok()
        .map(|d| d.join(".kairox").join("mcp_servers.toml"));
    crate::mcp_settings::list_mcp_server_settings(
        &self.config,
        user_config_path.as_deref(),
        project_config_path.as_deref(),
        source_filter.as_deref(),
        self.mcp_manager.clone(),
    )
    .await
}
```

- [ ] **Step 7: Update `list_mcp_server_settings` Tauri command**

Add `source_filter` parameter to the command:

```rust
pub async fn list_mcp_server_settings(
    state: State<'_, GuiState>,
    source_filter: Option<String>,
) -> Result<Vec<McpServerSettingsView>, String> {
    state.runtime.list_mcp_server_settings(source_filter).await
        .map_err(|error| error.to_string())
}
```

- [ ] **Step 8: Build and fix call sites**

```bash
cargo check --workspace
```

Fix any remaining callers.

- [ ] **Step 9: Commit**

```bash
git add crates/agent-runtime/src/mcp_settings.rs crates/agent-runtime/src/facade_mcp.rs \
        apps/agent-gui/src-tauri/src/commands.rs
git commit -m "feat(agent-runtime): tag MCP server settings with config source

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: Add `open_skills_dir` command

**Files:**

- Modify: `crates/agent-core/src/facade/skills.rs` — add trait method
- Modify: `crates/agent-runtime/src/facade_skills.rs` — implement
- Modify: `apps/agent-gui/src-tauri/src/commands.rs` — Tauri command
- Modify: `apps/agent-gui/src-tauri/src/lib.rs` — register
- Modify: `apps/agent-gui/src-tauri/src/specta.rs` — collect

- [ ] **Step 1: Add trait method**

In `crates/agent-core/src/facade/skills.rs`, add:

```rust
async fn open_skills_dir(&self) -> crate::Result<Option<String>> {
    Ok(None)
}
```

- [ ] **Step 2: Implement in `LocalRuntime`**

In `crates/agent-runtime/src/facade_skills.rs`, add:

```rust
pub(crate) async fn open_skills_dir(&self) -> agent_core::Result<Option<String>> {
    let dir = std::env::var("HOME").ok().map(|h| {
        std::path::PathBuf::from(h)
            .join(".config")
            .join("kairox")
            .join("skills")
    });
    Ok(dir.map(|d| d.display().to_string()))
}
```

- [ ] **Step 3: Add Tauri command in `commands.rs`**

After `open_config_dir`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn open_skills_dir(state: State<'_, GuiState>) -> Result<Option<String>, String> {
    let Some(skills_dir) = state
        .runtime
        .open_skills_dir()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let skills_dir = std::path::PathBuf::from(skills_dir);
    open_path_in_system_file_manager(&skills_dir)?;
    Ok(Some(skills_dir.display().to_string()))
}
```

- [ ] **Step 4: Register in `lib.rs` and `specta.rs`**

Add `crate::commands::open_skills_dir` in both `generate_handler!` and `collect_commands!`.

- [ ] **Step 5: Build**

```bash
cargo check --workspace
```

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core/src/facade/skills.rs crates/agent-runtime/src/facade_skills.rs \
        apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/lib.rs \
        apps/agent-gui/src-tauri/src/specta.rs
git commit -m "feat(skills): add open_skills_dir command

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: Project path existence check

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs` — add `path_exists` to `ProjectInfoResponse`
- Modify: `apps/agent-gui/src/stores/project.ts` — add `pathExists` to `ProjectInfo`

- [ ] **Step 1: Add `path_exists` to project list response**

In `commands.rs`, find the `list_projects` command and the `ProjectInfoResponse` struct (or the normalization). Add a `path_exists: bool` field computed from `std::path::Path::new(&project.root_path).exists()`.

Check the current `ProjectInfoResponse` struct — it's defined in `commands.rs`. Add field:

```rust
pub path_exists: bool,
```

In the command handler, check existence:

```rust
let path_exists = std::path::Path::new(&row.root_path).exists();
```

- [ ] **Step 2: Update frontend `ProjectInfo` interface**

Add `pathExists: boolean` to `ProjectInfo` in `stores/project.ts`. In the `normalizeProject` function, map `response.path_exists` to `pathExists`.

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src/stores/project.ts
git commit -m "feat(gui): add path existence check to project list

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Route restructuring

**Files:**

- Modify: `apps/agent-gui/src/router/routes.ts`
- Create: `apps/agent-gui/src/views/settings/GeneralSettings.vue`
- Create: `apps/agent-gui/src/layouts/SettingsLayout.vue`
- Modify: `apps/agent-gui/src/views/SettingsView.vue`

- [ ] **Step 1: Create `GeneralSettings.vue`**

Extract the general tab content from `SettingsView.vue` into a new file:

```vue
<script setup lang="ts">
import { useUiStore, type ThemeMode, type SupportedLocale } from "@/stores/ui";

const themes = [
  { value: "auto", labelKey: "settings.themeAuto" },
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" }
] as const satisfies ReadonlyArray<{ value: ThemeMode; labelKey: string }>;

const locales = [
  { value: "system", labelKey: "settings.localeSystem" },
  { value: "en", labelKey: "settings.localeEn" },
  { value: "zh-CN", labelKey: "settings.localeZh" }
] as const satisfies ReadonlyArray<{ value: SupportedLocale; labelKey: string }>;

const { t } = useI18n();
const ui = useUiStore();
const { locale, colorMode } = storeToRefs(ui);
const isThemeSelectFocused = ref(false);
</script>

<template>
  <div role="tabpanel">
    <div class="settings__row">
      <label for="settings-locale">{{ t("settings.locale") }}</label>
      <select
        id="settings-locale"
        :value="locale"
        data-test="settings-locale"
        @change="ui.setLocale(($event.target as HTMLSelectElement).value as SupportedLocale)"
      >
        <option v-for="opt in locales" :key="opt.value" :value="opt.value">
          {{ t(opt.labelKey) }}
        </option>
      </select>
    </div>
    <div class="settings__row" data-test="theme-toggle">
      <label for="settings-theme">{{ t("settings.theme") }}</label>
      <select
        id="settings-theme"
        :value="colorMode"
        :class="{ 'settings__select--focused': isThemeSelectFocused }"
        data-test="settings-theme"
        @focus="isThemeSelectFocused = true"
        @blur="isThemeSelectFocused = false"
        @change="ui.setTheme(($event.target as HTMLSelectElement).value as ThemeMode)"
      >
        <option v-for="opt in themes" :key="opt.value" :value="opt.value">
          {{ t(opt.labelKey) }}
        </option>
      </select>
    </div>
  </div>
</template>

<style scoped>
.settings__row {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-block: 12px;
}
.settings__row label {
  min-width: 100px;
}
</style>
```

- [ ] **Step 2: Create `SettingsLayout.vue`**

```vue
<script setup lang="ts">
import { useRoute, useRouter } from "vue-router";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const activeTab = computed(() => {
  const tab = route.path.split("/").pop();
  return tab && ["general", "mcp", "skills", "models", "archive"].includes(tab) ? tab : "general";
});

function navigateToTab(tab: string): void {
  router.push(`/settings/${tab}`);
}
</script>

<template>
  <main class="settings" data-test="view-settings">
    <h1>{{ t("settings.title") }}</h1>

    <div class="tabs" role="tablist" aria-label="Settings sections">
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'general'"
        data-test="settings-tab-general"
        @click="navigateToTab('general')"
      >
        {{ t("settings.general") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'mcp'"
        data-test="settings-tab-mcp"
        @click="navigateToTab('mcp')"
      >
        MCP
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'skills'"
        data-test="settings-tab-skills"
        @click="navigateToTab('skills')"
      >
        Skills
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'models'"
        data-test="settings-tab-models"
        @click="navigateToTab('models')"
      >
        {{ t("models.tabModels") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'archive'"
        data-test="settings-tab-archive"
        @click="navigateToTab('archive')"
      >
        {{ t("settings.archive") }}
      </button>
    </div>

    <router-view />
  </main>
</template>

<style scoped>
.settings {
  padding: 16px;
  max-width: 960px;
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.settings > :not(.tabs):not(h1) {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.tabs {
  display: flex;
  gap: 4px;
  border-bottom: 1px solid var(--app-border-color);
  margin-bottom: 12px;
}
.tab-btn {
  padding: 8px 16px;
  border: none;
  border-bottom: 2px solid transparent;
  border-radius: 6px 6px 0 0;
  background: none;
  cursor: pointer;
  font-size: inherit;
  color: var(--app-text-color-2);
  transition:
    color 0.2s,
    border-color 0.2s,
    background 0.15s;
}
.tab-btn[aria-selected="true"] {
  color: var(--app-primary-color);
  border-bottom-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}
.tab-btn:hover {
  color: var(--app-text-color);
  background: var(--app-hover-color);
}
.tab-btn:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>
```

- [ ] **Step 3: Update routes in `routes.ts`**

Replace the single `/settings` route with nested routes:

```typescript
export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: { name: "workbench" } },
  {
    path: "/workbench/:sessionId?",
    name: "workbench",
    component: () => import("@/views/WorkbenchView.vue"),
    props: true
  },
  { path: "/marketplace", redirect: { name: "settings" } },
  {
    path: "/settings",
    redirect: "/settings/general"
  },
  {
    path: "/settings",
    component: () => import("@/layouts/SettingsLayout.vue"),
    children: [
      {
        path: "general",
        name: "settings-general",
        component: () => import("@/views/settings/GeneralSettings.vue")
      },
      {
        path: "archive",
        name: "settings-archive",
        component: () => import("@/components/ArchiveSettingsPane.vue")
      },
      {
        path: "mcp",
        name: "settings-mcp",
        component: () => import("@/components/McpSettingsPane.vue")
      },
      {
        path: "skills",
        name: "settings-skills",
        component: () => import("@/components/SkillSettingsPane.vue")
      },
      {
        path: "models",
        name: "settings-models",
        component: () => import("@/components/ModelSettingsPane.vue")
      }
    ]
  },
  { path: "/:pathMatch(.*)*", redirect: { name: "workbench" } }
];
```

- [ ] **Step 4: Update `SettingsView.vue`**

Since `SettingsLayout.vue` now handles the tab navigation, `SettingsView.vue` should redirect or be empty. Keep it as a redirect proxy for backwards compatibility but mark as deprecated.

- [ ] **Step 5: Update `AppLayout.vue`**

Change the Settings `RouterLink` from `to="/settings"` to `to="/settings/general"`.

- [ ] **Step 6: Test routing**

Start the dev server and verify tab navigation works:

```bash
pnpm --filter agent-gui run dev
```

Click each tab, verify the URL changes and content loads.

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src/router/routes.ts \
        apps/agent-gui/src/layouts/SettingsLayout.vue \
        apps/agent-gui/src/views/settings/GeneralSettings.vue \
        apps/agent-gui/src/views/SettingsView.vue \
        apps/agent-gui/src/layouts/AppLayout.vue
git commit -m "feat(gui): restructure settings with nested routing and SettingsLayout

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: ConfigSourceBar component

**Files:**

- Create: `apps/agent-gui/src/components/ConfigSourceBar.vue`

- [ ] **Step 1: Create `ConfigSourceBar.vue`**

```vue
<script setup lang="ts">
import { useProjectStore } from "@/stores/project";

const props = defineProps<{
  currentTab: "mcp" | "skills" | "models";
}>();

const emit = defineEmits<{
  (e: "source-change", source: "user" | "project", projectId?: string): void;
}>();

const { t } = useI18n();
const projectStore = useProjectStore();

const source = ref<"user" | "project">("user");
const selectedProjectId = ref<string>("");

const missingProjects = computed(() => projectStore.activeProjects.filter((p) => !p.pathExists));

const projectOptions = computed(() =>
  projectStore.activeProjects.map((p) => ({
    value: p.projectId,
    label: p.displayName,
    missing: !p.pathExists
  }))
);

function onSourceChange(newSource: "user" | "project"): void {
  source.value = newSource;
  if (newSource === "user") {
    selectedProjectId.value = "";
  } else if (projectStore.activeProjects.length > 0) {
    selectedProjectId.value = projectStore.activeProjects[0].projectId;
  }
  emit("source-change", newSource, newSource === "project" ? selectedProjectId.value : undefined);
}

function onProjectChange(): void {
  emit("source-change", "project", selectedProjectId.value);
}

async function openConfigLocation(): Promise<void> {
  try {
    if (props.currentTab === "models") {
      await commands.openConfigDir();
    } else if (props.currentTab === "mcp") {
      await commands.openMcpConfigFile();
    } else if (props.currentTab === "skills") {
      await commands.openSkillsDir();
    }
  } catch {
    // best-effort
  }
}

onMounted(() => {
  void projectStore.loadProjects();
});
</script>

<template>
  <div>
    <div
      v-if="missingProjects.length > 0"
      class="config-source-banner"
      data-test="path-warning-banner"
    >
      <span>⚠️ {{ t("settings.pathWarning", { count: missingProjects.length }) }}</span>
      <button class="config-source-banner-detail" @click="/* toggle detail view */">
        {{ t("settings.viewDetails") }}
      </button>
    </div>

    <div class="config-source-bar" data-test="config-source-bar">
      <span class="config-source-bar__label">{{ t("settings.configSource") }}：</span>

      <div class="segmented" data-test="source-segmented">
        <button
          :class="['segmented__btn', { active: source === 'user' }]"
          @click="onSourceChange('user')"
        >
          {{ t("settings.userConfig") }}
        </button>
        <button
          :class="['segmented__btn', { active: source === 'project' }]"
          @click="onSourceChange('project')"
        >
          {{ t("settings.projectConfig") }}
        </button>
      </div>

      <template v-if="source === 'project'">
        <span class="config-source-bar__label">{{ t("settings.project") }}：</span>
        <div class="select-wrapper">
          <select v-model="selectedProjectId" data-test="project-select" @change="onProjectChange">
            <option
              v-for="opt in projectOptions"
              :key="opt.value"
              :value="opt.value"
              :class="{ warn: opt.missing }"
            >
              {{ opt.missing ? "⚠ " : "" }}{{ opt.label }}
            </option>
          </select>
        </div>
      </template>

      <button
        class="config-source-bar__open-btn"
        data-test="open-config-btn"
        @click="openConfigLocation"
        :title="t('settings.openConfigDir')"
      >
        📂 {{ t("settings.openConfigDir") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.config-source-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--color-surface, #282840);
  border: 1px solid var(--color-border, #3a3a5c);
  border-radius: 8px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}
.config-source-bar__label {
  font-size: 0.82rem;
  color: var(--color-text-muted);
  white-space: nowrap;
}
.segmented {
  display: inline-flex;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  overflow: hidden;
}
.segmented__btn {
  padding: 5px 14px;
  border: none;
  border-right: 1px solid var(--color-border);
  background: transparent;
  color: var(--color-text-muted);
  font-size: 0.82rem;
  cursor: pointer;
}
.segmented__btn:last-child {
  border-right: none;
}
.segmented__btn.active {
  background: var(--app-primary-color);
  color: #fff;
}
.select-wrapper select {
  padding: 5px 10px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-bg);
  color: var(--color-text);
  font-size: 0.82rem;
}
.config-source-bar__open-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: transparent;
  color: var(--color-text-muted);
  font-size: 0.82rem;
  cursor: pointer;
  margin-left: auto;
}
.config-source-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  background: #f59e0b15;
  border: 1px solid #f59e0b44;
  border-radius: 8px;
  margin-bottom: 8px;
  font-size: 0.82rem;
}
.config-source-banner-detail {
  background: none;
  border: none;
  color: var(--color-text-muted);
  font-size: 0.82rem;
  cursor: pointer;
  text-decoration: underline;
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/ConfigSourceBar.vue
git commit -m "feat(gui): add ConfigSourceBar shared component

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 9: Wire ConfigSourceBar into SettingsLayout

**Files:**

- Modify: `apps/agent-gui/src/layouts/SettingsLayout.vue`

- [ ] **Step 1: Add ConfigSourceBar to SettingsLayout**

Below the tabs div and before `<router-view>`, add:

```vue
<div v-if="['mcp', 'skills', 'models'].includes(activeTab)" class="settings__source-bar">
  <ConfigSourceBar
    :currentTab="(activeTab as 'mcp' | 'skills' | 'models')"
    @source-change="onSourceChange"
  />
</div>
```

Add the handler and provide/inject or event-based communication:

```typescript
const currentSource = ref<"user" | "project">("user");
const currentProjectId = ref<string | undefined>(undefined);

// Provide to child components
provide("configSource", currentSource);
provide("configProjectId", currentProjectId);

function onSourceChange(source: "user" | "project", projectId?: string): void {
  currentSource.value = source;
  currentProjectId.value = projectId;
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/layouts/SettingsLayout.vue
git commit -m "feat(gui): wire ConfigSourceBar into SettingsLayout

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 10: Update McpSettingsPane for source-aware loading

**Files:**

- Modify: `apps/agent-gui/src/components/McpSettingsPane.vue`
- Modify: `apps/agent-gui/src/stores/mcp.ts`

- [ ] **Step 1: Update `useMcpStore.fetchSettingsServers`**

Add optional parameters:

```typescript
async function fetchSettingsServers(source?: string, projectId?: string): Promise<void> {
  settingsLoading.value = true;
  settingsError.value = null;
  try {
    const sourceFilter = source === "project" ? "project" : null;
    settingsServers.value = await unwrapCommandResult(commands.listMcpServerSettings(sourceFilter));
  } catch (caughtError) {
    settingsError.value = formatError(caughtError);
  } finally {
    settingsLoading.value = false;
  }
}
```

- [ ] **Step 2: Watch for config source changes**

In `McpSettingsPane.vue`:

```typescript
const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

watch(
  [configSource, configProjectId],
  () => {
    if (configSource?.value && activeSubTab.value === "installed") {
      void mcp.fetchSettingsServers(configSource.value, configProjectId?.value);
    }
  },
  { deep: false }
);
```

- [ ] **Step 3: Add source tag to MCP server cards**

In the MCP server card template, add a source tag in the tags section:

```vue
<span
  :class="[
    'source-tag',
    server.source === 'project_config' ? 'source-tag--project' : 'source-tag--user'
  ]"
>
  {{ server.source === 'project_config' ? t('settings.projectLevel') : t('settings.userLevel') }}
</span>
```

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/components/McpSettingsPane.vue apps/agent-gui/src/stores/mcp.ts
git commit -m "feat(gui): add source-aware loading to MCP settings

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 11: Update SkillSettingsPane for source-aware loading

**Files:**

- Modify: `apps/agent-gui/src/components/SkillSettingsPane.vue`
- Modify: `apps/agent-gui/src/stores/skills.ts`

- [ ] **Step 1: Update `useSkillsStore.loadSkillSettings`**

Similar to MCP, add source parameters. Skills already have `scope` field in `SkillSettingsView`, so the Rust side already supports filtering. Check if `listSkillSettings` command accepts a filter; if not, we can filter on the frontend side.

For now, load all skills and filter by scope on frontend:

```typescript
async function loadSkillSettings(): Promise<void> {
  // existing code
}

// New computed for filtered view
const filteredSkillSettings = computed(() => {
  // Filter logic based on current source mode
  return skillSettings.value;
});
```

Note: If the Rust command doesn't support a source filter for skills, add one similarly to how we did for models and MCP. This would involve:

- Adding `source_filter` param to `list_skill_settings` command
- Passing it through the facade to filter `SkillSettingsView` by `scope`

- [ ] **Step 2: Watch for config source changes in `SkillSettingsPane.vue`**

Same pattern as McpSettingsPane.

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/components/SkillSettingsPane.vue apps/agent-gui/src/stores/skills.ts
git commit -m "feat(gui): add source-aware loading to skills settings

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 12: Create `useModelStore` and refactor `ModelSettingsPane`

**Files:**

- Create: `apps/agent-gui/src/stores/models.ts`
- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue`

- [ ] **Step 1: Create `useModelStore` Pinia store**

File: `apps/agent-gui/src/stores/models.ts`

```typescript
import { defineStore } from "pinia";
import { ref } from "vue";
import {
  commands,
  type ProfileSettingsInput,
  type ProfileSettingsView
} from "@/generated/commands";

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: string };

function formatError(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

async function unwrapCommandResult<T>(p: Promise<T | CommandResult<T>>): Promise<T> {
  const result = await p;
  if (typeof result === "object" && result !== null && "status" in result) {
    const r = result as CommandResult<T>;
    if (r.status === "error") throw new Error(r.error);
    return r.data;
  }
  return result;
}

export const useModelStore = defineStore("models", () => {
  const profiles = ref<ProfileSettingsView[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const busyAlias = ref<string | null>(null);

  async function fetchProfiles(source?: string, projectId?: string): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      const sourceFilter = source === "project" ? "project" : source === "user" ? "user" : null;
      profiles.value = await unwrapCommandResult(commands.listProfileSettings(sourceFilter));
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      loading.value = false;
    }
  }

  async function toggleProfile(profile: ProfileSettingsView): Promise<void> {
    busyAlias.value = profile.alias;
    error.value = null;
    try {
      await unwrapCommandResult(commands.setProfileEnabled(profile.alias, !profile.enabled));
      await fetchProfiles();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      busyAlias.value = null;
    }
  }

  async function deleteProfile(profile: ProfileSettingsView): Promise<void> {
    busyAlias.value = profile.alias;
    error.value = null;
    try {
      await unwrapCommandResult(commands.deleteProfileSettings(profile.alias));
      await fetchProfiles();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      busyAlias.value = null;
    }
  }

  async function upsertProfile(input: ProfileSettingsInput): Promise<ProfileSettingsView | null> {
    loading.value = true;
    error.value = null;
    try {
      const view = await unwrapCommandResult(commands.upsertProfileSettings(input));
      await fetchProfiles();
      return view;
    } catch (caughtError) {
      error.value = formatError(caughtError);
      return null;
    } finally {
      loading.value = false;
    }
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

  return {
    profiles,
    loading,
    error,
    busyAlias,
    fetchProfiles,
    toggleProfile,
    deleteProfile,
    upsertProfile,
    moveProfile
  };
});
```

- [ ] **Step 2: Refactor `ModelSettingsPane.vue` to use the store**

Replace local state (`profiles`, `loading`, `error`, `busyAlias`) with store usage:

```typescript
const modelStore = useModelStore();
const { profiles, loading, error, busyAlias } = storeToRefs(modelStore);
```

Replace direct command calls with store methods. Remove the local `formatError`, `isCommandResult`, `unwrapCommandResult` utilities and use the store. Wire source change listener:

```typescript
const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

watch(
  [configSource, configProjectId],
  () => {
    if (configSource?.value) {
      void modelStore.fetchProfiles(configSource.value, configProjectId?.value);
    }
  },
  { immediate: false }
);
```

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/stores/models.ts apps/agent-gui/src/components/ModelSettingsPane.vue
git commit -m "feat(gui): extract useModelStore and source-aware loading

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 13: Archive tab — `ArchiveSettingsPane.vue`

**Files:**

- Create: `apps/agent-gui/src/components/ArchiveSettingsPane.vue`
- Modify: `apps/agent-gui/src/stores/project.ts` — add restore/permanent-delete actions

- [ ] **Step 1: Add store methods to `useProjectStore`**

In `stores/project.ts`, add:

```typescript
async function restoreArchivedSession(sessionId: string): Promise<void> {
  await invoke("restore_archived_session", { sessionId });
  await loadArchivedSessions();
}

async function permanentlyDeleteSession(sessionId: string): Promise<void> {
  await invoke("permanently_delete_session", { sessionId });
  await loadArchivedSessions();
}
```

- [ ] **Step 2: Create `ArchiveSettingsPane.vue`**

```vue
<script setup lang="ts">
import { useProjectStore } from "@/stores/project";
import type { ProjectSessionInfo } from "@/stores/project";
import { commands } from "@/generated/commands";

const { t } = useI18n();
const projectStore = useProjectStore();
const loading = ref(false);
const confirmingDelete = ref<string | null>(null);

const stats = computed(() => ({
  total: projectStore.archivedSessions.length,
  projects: new Set(projectStore.archivedSessions.map((s) => s.projectId)).size
}));

async function restore(session: ProjectSessionInfo): Promise<void> {
  loading.value = true;
  try {
    await projectStore.restoreArchivedSession(session.sessionId);
  } catch {
    /* handled by store */
  } finally {
    loading.value = false;
  }
}

async function permanentlyDelete(sessionId: string): Promise<void> {
  loading.value = true;
  try {
    await projectStore.permanentlyDeleteSession(sessionId);
  } catch {
    /* handled by store */
  } finally {
    loading.value = false;
    confirmingDelete.value = null;
  }
}

onMounted(() => {
  void projectStore.loadArchivedSessions();
});
</script>

<template>
  <div class="archive-settings" data-test="archive-settings-pane">
    <div class="archive-stats">
      <div class="archive-stat">
        <span class="archive-stat__value">{{ stats.total }}</span>
        <span class="archive-stat__label">{{ t("settings.archivedSessions") }}</span>
      </div>
      <div class="archive-stat">
        <span class="archive-stat__value">{{ stats.projects }}</span>
        <span class="archive-stat__label">{{ t("settings.archivedProjects") }}</span>
      </div>
    </div>

    <p v-if="projectStore.archivedSessions.length === 0" class="empty-state">
      {{ t("settings.noArchivedSessions") }}
    </p>

    <div v-else class="archive-list">
      <article
        v-for="s in projectStore.archivedSessions"
        :key="s.sessionId"
        class="card archive-card"
        :data-test="`archive-row-${s.sessionId}`"
      >
        <div class="card-body archive-card__body">
          <h3>{{ s.title }}</h3>
          <p v-if="s.projectId">
            {{ t("settings.project") }}: {{ s.projectId }}
            <span v-if="s.branch">· {{ s.branch }}</span>
          </p>
        </div>
        <div class="archive-card__actions">
          <button class="btn btn-sm" :disabled="loading" @click="restore(s)">
            {{ t("settings.restore") }}
          </button>
          <button
            class="btn btn-sm btn-danger"
            :disabled="loading"
            @click="confirmingDelete = s.sessionId"
          >
            {{ t("common.delete") }}
          </button>
        </div>
      </article>
    </div>

    <!-- Confirm delete dialog -->
    <ModalDialog
      v-if="confirmingDelete"
      :open="true"
      :title="t('common.confirm')"
      @close="confirmingDelete = null"
    >
      <p>{{ t("settings.permanentDeleteWarning") }}</p>
      <template #footer>
        <button class="btn" @click="confirmingDelete = null">{{ t("common.cancel") }}</button>
        <button class="btn btn-danger" @click="permanentlyDelete(confirmingDelete!)">
          {{ t("common.delete") }}
        </button>
      </template>
    </ModalDialog>
  </div>
</template>
```

- [ ] **Step 3: Register archive route**

Already done in Task 7 (route points to `ArchiveSettingsPane.vue`).

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/components/ArchiveSettingsPane.vue apps/agent-gui/src/stores/project.ts
git commit -m "feat(gui): add archive tab with restore and permanent delete

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 14: i18n and `gen-types`

**Files:**

- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

- [ ] **Step 1: Add English translations**

Add to `en.json`:

```json
{
  "settings": {
    "archive": "Archive",
    "archivedSessions": "Archived Sessions",
    "archivedProjects": "Projects Archived",
    "noArchivedSessions": "No archived sessions.",
    "restore": "Restore",
    "permanentDeleteWarning": "Permanently delete this session? All data will be lost.",
    "configSource": "Config Source",
    "userConfig": "User Config",
    "projectConfig": "Project Config",
    "project": "Project",
    "openConfigDir": "Open Config Location",
    "userLevel": "User-level",
    "projectLevel": "Project-level",
    "pathWarning": "{count} project path(s) not found",
    "viewDetails": "View details"
  },
  "models": {
    "sourceTag": "Source"
  }
}
```

- [ ] **Step 2: Add Chinese translations**

Add to `zh-CN.json`:

```json
{
  "settings": {
    "archive": "归档",
    "archivedSessions": "已归档会话",
    "archivedProjects": "涉及项目",
    "noArchivedSessions": "没有已归档的会话。",
    "restore": "恢复",
    "permanentDeleteWarning": "永久删除此会话？所有数据将无法恢复。",
    "configSource": "配置来源",
    "userConfig": "用户配置",
    "projectConfig": "项目配置",
    "project": "项目",
    "openConfigDir": "打开配置位置",
    "userLevel": "用户级",
    "projectLevel": "项目级",
    "pathWarning": "有 {count} 个项目路径不存在",
    "viewDetails": "查看详情"
  },
  "models": {
    "sourceTag": "来源"
  }
}
```

- [ ] **Step 3: Run `just gen-types`**

```bash
just gen-types
```

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/locales/en.json apps/agent-gui/src/locales/zh-CN.json \
        apps/agent-gui/src/generated/
git commit -m "feat(gui): add i18n keys and regenerate types

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 15: Final integration and testing

- [ ] **Step 1: Build the entire project**

```bash
cargo check --workspace
pnpm --filter agent-gui run build
```

Fix any compilation errors.

- [ ] **Step 2: Run unit and integration tests**

```bash
cargo test --workspace --all-targets
pnpm --filter agent-gui run test
```

- [ ] **Step 3: Run lint and format checks**

```bash
pnpm run lint
pnpm run format:check
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: final integration fixes and test verification

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```
