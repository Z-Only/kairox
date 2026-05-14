# MCP / Skills / Models 配置重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify MCP/Skills/Models config layers, add effective-config merged view with source annotations, marketplace installed-state detection, and MCP connectivity testing.

**Architecture:** New `EffectiveItem<T>` + `ConfigScope` types in `agent-core`, four-tier merge in `agent-config` (Local > Project > User > Builtin), new Tauri `get_effective_*` commands, unified Pinia stores, and consistent Vue component patterns across all three panes.

**Tech Stack:** Rust (workspace crates), Tauri 2 IPC (specta), Vue 3 + Pinia + TypeScript

---

### Task 1: ConfigScope enum in agent-core

**Files:**

- Create: `crates/agent-core/src/config_scope.rs`
- Modify: `crates/agent-core/src/lib.rs`

- [ ] **Step 1: Create ConfigScope module**

Write `crates/agent-core/src/config_scope.rs`:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Type)]
pub enum ConfigScope {
    Builtin = 0,
    User = 1,
    Project = 2,
    Local = 3,
}

impl ConfigScope {
    pub fn priority(self) -> u8 {
        self as u8
    }

    pub fn label(self) -> &'static str {
        match self {
            ConfigScope::Builtin => "builtin",
            ConfigScope::User => "user",
            ConfigScope::Project => "project",
            ConfigScope::Local => "local",
        }
    }
}

impl std::fmt::Display for ConfigScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Edit `crates/agent-core/src/lib.rs` — add `pub mod config_scope;` and re-export:

```rust
pub mod config_scope;
pub use config_scope::ConfigScope;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo build -p agent-core 2>&1 | tail -5
```

Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core/src/config_scope.rs crates/agent-core/src/lib.rs
git commit -m "feat(core): add ConfigScope enum for four-tier config layering"
```

---

### Task 2: EffectiveItem type in agent-core

**Files:**

- Create: `crates/agent-core/src/effective.rs`
- Modify: `crates/agent-core/src/lib.rs`

- [ ] **Step 1: Write EffectiveItem**

Write `crates/agent-core/src/effective.rs`:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::config_scope::ConfigScope;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EffectiveItem<T: Serialize + for<'de> Deserialize<'de> + Clone> {
    pub value: T,
    pub source: ConfigScope,
    pub overrides: Option<ConfigScope>,
    pub enabled: bool,
    pub disabled_by: Option<ConfigScope>,
    pub writable: bool,
    pub deletable: bool,
}

impl<T: Serialize + for<'de> Deserialize<'de> + Clone> EffectiveItem<T> {
    pub fn new(value: T, source: ConfigScope) -> Self {
        Self {
            value,
            source,
            overrides: None,
            enabled: true,
            disabled_by: None,
            writable: source >= ConfigScope::User,
            deletable: source >= ConfigScope::User,
        }
    }

    pub fn with_disabled(mut self, by: ConfigScope) -> Self {
        self.enabled = false;
        self.disabled_by = Some(by);
        self
    }

    pub fn with_override(mut self, by: ConfigScope) -> Self {
        self.overrides = Some(by);
        self
    }
}
```

- [ ] **Step 2: Register module**

Edit `crates/agent-core/src/lib.rs`:

```rust
pub mod effective;
pub use effective::EffectiveItem;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo build -p agent-core 2>&1 | tail -5
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core/src/effective.rs crates/agent-core/src/lib.rs
git commit -m "feat(core): add EffectiveItem<T> for merged-config views"
```

---

### Task 3: EffectiveMergedConfig in agent-config

**Files:**

- Create: `crates/agent-config/src/effective.rs`
- Modify: `crates/agent-config/src/lib.rs`

- [ ] **Step 1: Write effective config merger**

Write `crates/agent-config/src/effective.rs`:

```rust
use agent_core::config_scope::ConfigScope;
use agent_core::EffectiveItem;
use crate::Config;

pub fn build_effective_mcp_servers(config: &Config) -> Vec<EffectiveItem<agent_mcp::McpServerDef>> {
    let mut result: Vec<EffectiveItem<agent_mcp::McpServerDef>> = Vec::new();

    for (name, def) in &config.mcp_servers {
        let server_def = def.to_server_def(name);
        let source = match config.source {
            crate::ConfigSource::ProjectFile => ConfigScope::Project,
            crate::ConfigSource::UserFile => ConfigScope::User,
            crate::ConfigSource::Defaults => ConfigScope::Builtin,
        };
        let item = EffectiveItem::new(server_def, source);
        result.push(item);
    }

    result
}

pub fn build_effective_profiles(config: &Config) -> Vec<EffectiveItem<crate::ProfileDef>> {
    let source = match config.source {
        crate::ConfigSource::ProjectFile => ConfigScope::Project,
        crate::ConfigSource::UserFile => ConfigScope::User,
        crate::ConfigSource::Defaults => ConfigScope::Builtin,
    };

    config.profiles.iter().map(|(alias, profile)| {
        let mut p = profile.clone();
        let item = EffectiveItem::new(p, source);
        item
    }).collect()
}
```

- [ ] **Step 2: Create unit test**

Write test in `crates/agent-config/src/effective.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_mcp_empty_config() {
        let config = Config::defaults();
        let servers = build_effective_mcp_servers(&config);
        assert!(servers.is_empty());
    }
}
```

- [ ] **Step 3: Register and verify**

Edit `crates/agent-config/src/lib.rs` — add `pub mod effective;` after existing modules.

```bash
cargo test -p agent-config --lib effective 2>&1 | tail -5
```

Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-config/src/effective.rs crates/agent-config/src/lib.rs
git commit -m "feat(config): add effective config merger for MCP servers and profiles"
```

---

### Task 4: Four-tier Config loading (Local scope support)

**Files:**

- Modify: `crates/agent-config/src/lib.rs:240-267`
- Modify: `crates/agent-config/src/discovery.rs:14-17`

- [ ] **Step 1: Add local config path to discovery**

Edit `crates/agent-config/src/discovery.rs` — add function after `find_config_upward`:

```rust
pub fn find_local_config(project_root: Option<&Path>) -> Option<PathBuf> {
    let root = project_root?;
    let local = root.join(".kairox").join("config.local.toml");
    if local.exists() {
        Some(local)
    } else {
        None
    }
}
```

- [ ] **Step 2: Update load_inner to include Local tier**

Edit `crates/agent-config/src/lib.rs:240`, replace `load_inner`:

```rust
fn load_inner(project_root: Option<&Path>) -> Self {
    let mut base = Self::defaults();

    // Layer 1: User config (~/.kairox/config.toml)
    if let Some(user_path) = discovery::find_config(None) {
        if let Ok(user_config) = Self::load_from_file(&user_path) {
            base.merge_config(user_config, ConfigSource::UserFile);
        }
    }

    // Layer 2: Project config (.kairox/config.toml)
    if let Some(root) = project_root {
        if let Some(proj_path) = discovery::find_config_upward(root).or_else(|| {
            let p = root.join(".kairox").join("config.toml");
            if p.exists() { Some(p) } else { None }
        }) {
            if let Ok(proj_config) = Self::load_from_file(&proj_path) {
                base.merge_config(proj_config, ConfigSource::ProjectFile);
            }
        }
    }

    // Layer 3: Local config (.kairox/config.local.toml, gitignored)
    if let Some(local_path) = discovery::find_local_config(project_root) {
        if let Ok(local_config) = Self::load_from_file(&local_path) {
            base.merge_config(local_config, ConfigSource::LocalFile);
        }
    }

    base
}
```

- [ ] **Step 3: Add LocalFile variant to ConfigSource**

Edit `crates/agent-config/src/lib.rs:80`:

```rust
pub enum ConfigSource {
    ProjectFile,
    UserFile,
    LocalFile,
    Defaults,
}
```

- [ ] **Step 4: Update merge_config to track source per-item**

Edit `crates/agent-config/src/lib.rs:271`, update `merge_config` to accept `ConfigSource`:

```rust
fn merge_config(&mut self, other: Config, source: ConfigSource) {
    for (name, profile) in other.profiles {
        self.profiles.retain(|(n, _)| n != &name);
        self.profiles.push((name, profile));
    }
    for (name, server) in other.mcp_servers {
        self.mcp_servers.retain(|(n, _)| n != &name);
        self.mcp_servers.push((name, server));
    }
    self.source = source;
}
```

- [ ] **Step 5: Run existing tests to verify no regression**

```bash
cargo test -p agent-config 2>&1 | tail -10
```

Expected: All existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-config/src/lib.rs crates/agent-config/src/discovery.rs
git commit -m "feat(config): add four-tier config loading with Local scope support"
```

---

### Task 5: get*effective*\* Tauri commands (MCP)

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs` (or equivalent command definition file)
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`

Note: exact command file paths depend on current Tauri command registration pattern. Verify with grep first.

- [ ] **Step 1: Find current MCP command registration**

```bash
grep -rn "list_mcp_server_settings\|listMcpServerSettings" apps/agent-gui/src-tauri/src/ | head -10
```

- [ ] **Step 2: Add get_effective_mcp_servers command**

In the MCP commands file, add:

```rust
use agent_core::config_scope::ConfigScope;
use agent_core::EffectiveItem;
use agent_config::effective::build_effective_mcp_servers;

#[tauri::command]
#[specta::specta]
fn get_effective_mcp_servers(state: tauri::State<'_, AppState>) -> Result<Vec<EffectiveItem<McpServerSettingsView>>, String> {
    let config = state.config_loader.load_merged()?;
    let effective = build_effective_mcp_servers(&config);
    Ok(effective.into_iter().map(|item| {
        let view = McpServerSettingsView {
            id: item.value.name.clone(),
            name: item.value.name.clone(),
            transport: transport_to_string(&item.value.transport),
            enabled: item.enabled,
            runtime_status: "unknown".into(),
            trusted: false,
            tool_count: 0,
            last_error: None,
            writable: item.writable,
            config_path: None,
            description: None,
            source: item.source.to_string(),
        };
        EffectiveItem {
            value: view,
            source: item.source,
            overrides: item.overrides,
            enabled: item.enabled,
            disabled_by: item.disabled_by,
            writable: item.writable,
            deletable: item.deletable,
        }
    }).collect())
}
```

- [ ] **Step 3: Register command in generate_handler! and collect_commands!**

Per CLAUDE.md: "Register new Tauri commands in BOTH `generate_handler!` (in `lib.rs`) AND `collect_commands!` (in `src/specta.rs`)."

```rust
// In generate_handler! macro
get_effective_mcp_servers,

// In collect_commands! macro
get_effective_mcp_servers,
```

- [ ] **Step 4: Run just gen-types to regenerate TypeScript bindings**

```bash
just gen-types 2>&1 | tail -10
```

Expected: No errors, `commands.ts` updated.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src-tauri/src/ apps/agent-gui/src/generated/
git commit -m "feat(gui): add get_effective_mcp_servers Tauri command"
```

---

### Task 6: get_effective_skills and get_effective_models commands

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/` (MCP/skills/models command files)
- Modify: `apps/agent-gui/src-tauri/src/lib.rs` (register)
- Modify: `apps/agent-gui/src-tauri/src/specta.rs` (collect)

- [ ] **Step 1: Add get_effective_skills command**

```rust
#[tauri::command]
#[specta::specta]
fn get_effective_skills(state: tauri::State<'_, AppState>) -> Result<Vec<EffectiveItem<SkillSettingsView>>, String> {
    let settings = state.skills_facade.get_effective_skills()?;
    Ok(settings)
}
```

- [ ] **Step 2: Add get_effective_model_profiles command**

```rust
#[tauri::command]
#[specta::specta]
fn get_effective_model_profiles(state: tauri::State<'_, AppState>) -> Result<Vec<EffectiveItem<ProfileSettingsView>>, String> {
    let config = state.config_loader.load_merged()?;
    let profiles = build_effective_profiles(&config);
    Ok(profiles.into_iter().map(|item| {
        let p = &item.value;
        EffectiveItem {
            value: ProfileSettingsView {
                alias: /* map from p */,
                provider: p.provider.clone(),
                model_id: p.model_id.clone(),
                enabled: item.enabled,
                context_window: p.context_window,
                output_limit: p.output_limit,
                temperature: p.temperature,
                top_p: p.top_p,
                top_k: p.top_k,
                max_tokens: p.max_tokens,
                base_url: p.base_url.clone(),
                api_key_env: p.api_key_env.clone(),
                has_api_key: p.api_key.is_some(),
                writable: item.writable,
                config_path: None,
                source: item.source.to_string(),
            },
            source: item.source,
            overrides: item.overrides,
            enabled: item.enabled,
            disabled_by: item.disabled_by,
            writable: item.writable,
            deletable: item.deletable,
        }
    }).collect())
}
```

- [ ] **Step 3: Register both commands and regenerate types**

```bash
just gen-types 2>&1 | tail -10
```

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src-tauri/src/ apps/agent-gui/src/generated/
git commit -m "feat(gui): add get_effective_skills and get_effective_model_profiles commands"
```

---

### Task 7: Update Pinia stores for effective views

**Files:**

- Modify: `apps/agent-gui/src/stores/mcp.ts`
- Modify: `apps/agent-gui/src/stores/skills.ts`

- [ ] **Step 1: Add effective servers to MCP store**

Edit `apps/agent-gui/src/stores/mcp.ts` — add state and action:

```typescript
import type { EffectiveItem } from '../generated/commands';

// In store state:
effectiveServers: Ref<EffectiveItem<McpServerSettingsView>[]> = ref([]),

// In store actions:
async fetchEffectiveServers(): Promise<void> {
  try {
    const result = await commands.getEffectiveMcpServers();
    this.effectiveServers = result;
  } catch (e) {
    console.error('Failed to fetch effective MCP servers', e);
  }
},
```

- [ ] **Step 2: Add effective skills to Skills store**

Edit `apps/agent-gui/src/stores/skills.ts`:

```typescript
effectiveSkills: Ref<EffectiveItem<SkillSettingsView>[]> = ref([]),

async fetchEffectiveSkills(): Promise<void> {
  try {
    const result = await commands.getEffectiveSkills();
    this.effectiveSkills = result;
  } catch (e) {
    console.error('Failed to fetch effective skills', e);
  }
},
```

- [ ] **Step 3: Add unit tests for store actions**

Edit `apps/agent-gui/src/stores/mcp.test.ts` — add test:

```typescript
it("fetchEffectiveServers populates effectiveServers", async () => {
  const mock = {
    value: {
      /* ... */
    },
    source: "user",
    enabled: true,
    writable: true,
    deletable: true
  };
  vi.mocked(commands.getEffectiveMcpServers).mockResolvedValue([mock]);
  const store = useMcpStore();
  await store.fetchEffectiveServers();
  expect(store.effectiveServers).toHaveLength(1);
  expect(store.effectiveServers[0].source).toBe("user");
});
```

- [ ] **Step 4: Run GUI tests**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -15
```

Expected: Tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/stores/
git commit -m "feat(gui): add effective-view actions to MCP and Skills Pinia stores"
```

---

### Task 8: Unified list view with Source column — McpSettingsPane

**Files:**

- Modify: `apps/agent-gui/src/components/McpSettingsPane.vue`

- [ ] **Step 1: Replace settingsServers with effectiveServers**

In `McpSettingsPane.vue` `<script setup>`:

```typescript
const mcp = useMcpStore();

// Replace: settingsServers → effectiveServers
const effectiveServers = computed(() => mcp.effectiveServers);

// Fetch on mount and source change
onMounted(async () => {
  await mcp.fetchEffectiveServers();
});

watch([configSource, configProjectId], async () => {
  await mcp.fetchEffectiveServers();
});
```

- [ ] **Step 2: Add Source column to the table/list template**

In the server list `<article>` template, add source tag:

```html
<div class="server__tags">
  <span class="tag tag--source" :class="`tag--source-${server.source}`"> {{ server.source }} </span>
  <span v-if="server.overrides" class="tag tag--override"> 覆盖{{ server.overrides }} </span>
  <span v-if="server.disabled_by" class="tag tag--disabled-by">
    · {{ server.disabled_by }}已禁用
  </span>
  <!-- existing transport, tool count, enabled tags -->
</div>
```

- [ ] **Step 3: Add CSS for source tags**

```css
.tag--source {
  font-weight: 600;
}
.tag--source-project {
  background: var(--color-primary-light);
  color: var(--color-primary);
}
.tag--source-user {
  background: var(--color-secondary-light);
  color: var(--color-secondary);
}
.tag--source-builtin {
  background: var(--color-muted);
  color: var(--color-text-muted);
}
.tag--override {
  background: var(--color-warning-light);
  color: var(--color-warning);
}
.tag--disabled-by {
  background: var(--color-danger-light);
  color: var(--color-danger);
}
```

- [ ] **Step 4: Run GUI tests**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -15
```

Expected: Existing tests still pass (update snapshots if needed).

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/components/McpSettingsPane.vue
git commit -m "feat(gui): add unified effective view with Source column to MCP pane"
```

---

### Task 9: Unified list view — SkillSettingsPane and ModelSettingsPane

**Files:**

- Modify: `apps/agent-gui/src/components/SkillSettingsPane.vue`
- Modify: `apps/agent-gui/src/components/ModelSettingsPane.vue`

- [ ] **Step 1: Update SkillSettingsPane to use effective view**

Replace `loadSkillSettings()` call + `filteredSkills` computed with `fetchEffectiveSkills()`:

```typescript
const effectiveSkills = computed(() => skillsStore.effectiveSkills);

onMounted(async () => {
  await skillsStore.fetchEffectiveSkills();
});
```

Keep existing scope tag rendering (already shows scope), add `disabled_by` and `overrides` indicators.

- [ ] **Step 2: Update ModelSettingsPane to use effective view**

Replace `listProfileSettings(sourceFilter)` with `getEffectiveModelProfiles()`:

```typescript
const profiles = ref<EffectiveItem<ProfileSettingsView>[]>([]);

async function loadProfiles() {
  profiles.value = await commands.getEffectiveModelProfiles();
}
```

Add Source column tag to profile card template.

- [ ] **Step 3: Run GUI tests**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -15
```

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/components/SkillSettingsPane.vue apps/agent-gui/src/components/ModelSettingsPane.vue
git commit -m "feat(gui): add unified effective view to Skills and Models panes"
```

---

### Task 10: Marketplace installed-state detection

**Files:**

- Modify: `apps/agent-gui/src/stores/catalog.ts`
- Modify: `apps/agent-gui/src/components/marketplace/CatalogDetail.vue`

- [ ] **Step 1: Add installed-check to catalog store**

Edit `apps/agent-gui/src/stores/catalog.ts` — add to state:

```typescript
installedServerNames: Ref<Set<string>> = ref(new Set()),

async checkInstalledStatus(): Promise<void> {
  const installed = await commands.listInstalledEntries();
  this.installedServerNames = new Set(installed.map(e => e.name));
},
```

- [ ] **Step 2: Expose isInstalled computed from store**

```typescript
function isServerInstalled(name: string): boolean {
  return this.installedServerNames.has(name);
}
```

- [ ] **Step 3: Update CatalogDetail.vue install button logic**

Edit `apps/agent-gui/src/components/marketplace/CatalogDetail.vue`:

```typescript
const catalog = useCatalogStore();
const mcp = useMcpStore();

const installState = computed<"none" | "installed_same" | "installed_other" | "update">(() => {
  const name = props.entry.name;
  const effective = mcp.effectiveServers.find((s) => s.value.name === name);
  if (!effective) return "none";
  if (effective.source === "project" && configSource === "project") return "installed_same";
  if (effective.source === "user" && configSource === "project") return "installed_other";
  // version check for update
  return "installed_same";
});
```

Update template:

```html
<button v-if="installState === 'none'" @click="install">安装</button>
<button v-else-if="installState === 'installed_other'" @click="install" class="btn--warn">
  安装到项目 (用户级已安装)
</button>
<span v-else class="badge badge--installed">已安装 ✓</span>
```

- [ ] **Step 4: Run GUI tests**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -15
```

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/stores/catalog.ts apps/agent-gui/src/components/marketplace/CatalogDetail.vue
git commit -m "feat(gui): add installed-state detection to marketplace catalog"
```

---

### Task 11: MCP connectivity test — backend

**Files:**

- Create or modify: MCP command file in `apps/agent-gui/src-tauri/src/`
- Modify: `crates/agent-mcp/src/lifecycle.rs` (add test_connectivity method)

- [ ] **Step 1: Add test_connectivity to ServerLifecycle**

Edit `crates/agent-mcp/src/lifecycle.rs`, add method to `impl ServerLifecycle`:

```rust
pub async fn test_connectivity(&mut self, timeout: Duration) -> ConnectivityResult {
    use crate::types::ConnectivityResult;

    match self.ensure_running().await {
        Ok(()) => {
            match tokio::time::timeout(timeout, self.discover_tools()).await {
                Ok(Ok(tools)) => {
                    if tools.is_empty() {
                        ConnectivityResult::Failed {
                            reason: "tools/list returned empty".into(),
                        }
                    } else {
                        ConnectivityResult::Connected {
                            tool_count: tools.len() as u32,
                        }
                    }
                }
                Ok(Err(e)) => ConnectivityResult::Failed {
                    reason: format!("discovery failed: {}", e),
                },
                Err(_) => ConnectivityResult::Failed {
                    reason: "timeout".into(),
                },
            }
        }
        Err(e) => ConnectivityResult::Failed {
            reason: format!("start failed: {}", e),
        },
    }
}
```

- [ ] **Step 2: Add ConnectivityResult type**

Edit `crates/agent-mcp/src/types.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum ConnectivityResult {
    Connected { tool_count: u32 },
    Failed { reason: String },
}
```

- [ ] **Step 3: Add test_connectivity Tauri command**

```rust
#[tauri::command]
#[specta::specta]
async fn test_mcp_connectivity(
    state: tauri::State<'_, AppState>,
    server_id: String,
) -> Result<ConnectivityResult, String> {
    let mut lifecycle = state
        .mcp_runtime
        .get_or_create(&server_id)
        .map_err(|e| e.to_string())?;
    let result = lifecycle
        .test_connectivity(std::time::Duration::from_secs(15))
        .await;
    Ok(result)
}
```

- [ ] **Step 4: Register and regenerate types**

```bash
just gen-types 2>&1 | tail -10
```

- [ ] **Step 5: Run MCP tests**

```bash
cargo test -p agent-mcp 2>&1 | tail -10
```

- [ ] **Step 6: Commit**

```bash
git add crates/agent-mcp/src/ apps/agent-gui/src-tauri/src/ apps/agent-gui/src/generated/
git commit -m "feat(mcp): add connectivity testing to server lifecycle"
```

---

### Task 12: MCP connectivity test — frontend

**Files:**

- Modify: `apps/agent-gui/src/stores/mcp.ts`
- Modify: `apps/agent-gui/src/components/McpSettingsPane.vue`
- Modify: `apps/agent-gui/src/components/marketplace/CatalogDetail.vue`

- [ ] **Step 1: Add connectivity state to MCP store**

Edit `apps/agent-gui/src/stores/mcp.ts`:

```typescript
connectivityResults: Ref<Record<string, ConnectivityResult>> = ref({}),

async testConnectivity(serverId: string): Promise<void> {
  this.connectivityResults[serverId] = { status: 'checking' };
  try {
    const result = await commands.testMcpConnectivity(serverId);
    this.connectivityResults[serverId] = result;
  } catch (e) {
    this.connectivityResults[serverId] = { status: 'failed', reason: String(e) };
  }
},

async testAllConnectivity(): Promise<void> {
  for (const server of this.effectiveServers) {
    if (server.value.transport !== 'builtin') {
      await this.testConnectivity(server.value.id);
    }
  }
},
```

- [ ] **Step 2: Add test button + status indicator to server list rows**

Edit `McpSettingsPane.vue` — add to each server card action area:

```html
<button
  class="btn btn--sm"
  @click="mcp.testConnectivity(server.value.id)"
  :disabled="mcp.connectivityResults[server.value.id]?.status === 'checking'"
>
  <span v-if="mcp.connectivityResults[server.value.id]?.status === 'checking'">检测中...</span>
  <span v-else-if="mcp.connectivityResults[server.value.id]?.status === 'connected'">
    ● {{ mcp.connectivityResults[server.value.id].tool_count }} tools
  </span>
  <span v-else>测试连通性</span>
</button>
```

- [ ] **Step 3: Add test button to CatalogDetail before install**

Edit `CatalogDetail.vue` — add test button before install button:

```html
<button class="btn btn--outline" @click="testBeforeInstall" :disabled="testing">
  {{ testing ? '检测中...' : '测试连通性' }}
</button>
```

```typescript
const testing = ref(false);
async function testBeforeInstall() {
  testing.value = true;
  // Temporarily install to test, or test spec before install
  const result = await commands.testMcpConnectivityWithSpec(props.entry.installSpec);
  connectivityResult.value = result;
  testing.value = false;
}
```

- [ ] **Step 4: Run GUI tests**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -15
```

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/stores/mcp.ts apps/agent-gui/src/components/McpSettingsPane.vue apps/agent-gui/src/components/marketplace/CatalogDetail.vue
git commit -m "feat(gui): add MCP connectivity test UI to server list and catalog detail"
```

---

### Task 13: Install target scope selector — all forms

**Files:**

- Modify: `apps/agent-gui/src/components/McpSettingsPane.vue` (add server dialog)
- Modify: `apps/agent-gui/src/components/SkillSettingsPane.vue` (GitHub install form)
- Modify: `apps/agent-gui/src/components/marketplace/CatalogDetail.vue` (install)

- [ ] **Step 1: Create reusable ScopeSelector component**

Create `apps/agent-gui/src/components/ScopeSelector.vue`:

```vue
<template>
  <div class="scope-selector">
    <label class="scope-selector__label">安装到</label>
    <div class="scope-selector__options">
      <label
        v-for="opt in options"
        :key="opt.value"
        class="scope-selector__option"
        :class="{ active: modelValue === opt.value }"
      >
        <input type="radio" :value="opt.value" v-model="modelValue" />
        <span class="scope-selector__name">{{ opt.label }}</span>
        <span class="scope-selector__hint">{{ opt.hint }}</span>
      </label>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ConfigScope } from "../generated/commands";

const props = defineProps<{
  modelValue: ConfigScope;
  showLocal?: boolean;
}>();

const emit = defineEmits<{ "update:modelValue": [value: ConfigScope] }>();

const options = computed(() => {
  const opts = [
    { value: "user" as ConfigScope, label: "用户 (全局)", hint: "所有项目生效" },
    { value: "project" as ConfigScope, label: "项目", hint: "仅当前项目生效" }
  ];
  if (props.showLocal) {
    opts.push({
      value: "local" as ConfigScope,
      label: "本地覆盖",
      hint: "个人临时配置，不提交 git"
    });
  }
  return opts;
});
</script>
```

- [ ] **Step 2: Add ScopeSelector to MCP add-server dialog**

In `McpSettingsPane.vue` add-server form, insert before name field:

```html
<ScopeSelector v-model="installTarget" :show-local="true" />
```

Update `saveServerSettings` call to include scope:

```typescript
const installTarget = ref<ConfigScope>("project");

async function save() {
  await commands.upsertMcpServerSettings({
    ...formData,
    scope: installTarget.value
  });
}
```

- [ ] **Step 3: Add ScopeSelector to SkillSettingsPane GitHub install**

Replace existing `<select>` with `<ScopeSelector v-model="installTarget" />`.

- [ ] **Step 4: Add ScopeSelector to CatalogDetail install flow**

Add `<ScopeSelector v-model="installTarget" />` above the install button.

- [ ] **Step 5: Run GUI tests**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -15
```

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/components/ScopeSelector.vue apps/agent-gui/src/components/McpSettingsPane.vue apps/agent-gui/src/components/SkillSettingsPane.vue apps/agent-gui/src/components/marketplace/CatalogDetail.vue
git commit -m "feat(gui): add reusable ScopeSelector component to all install forms"
```

---

### Task 14: Project disable/override of user config — backend

**Files:**

- Modify: `crates/agent-config/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/` (commands file)

- [ ] **Step 1: Add disabled_servers tracking to Config**

Edit `crates/agent-config/src/lib.rs` — add to `Config` struct:

```rust
pub struct Config {
    // ... existing fields
    pub disabled_mcp_servers: Vec<String>,  // project-level disabled user servers
}
```

- [ ] **Step 2: Add disable/enable commands**

```rust
#[tauri::command]
#[specta::specta]
fn disable_mcp_server_at_scope(
    state: tauri::State<'_, AppState>,
    server_name: String,
    scope: String,  // "project"
) -> Result<(), String> {
    let mut config = state.config_loader.load_project_config()?;
    if !config.disabled_mcp_servers.contains(&server_name) {
        config.disabled_mcp_servers.push(server_name);
    }
    state.config_loader.save_project_config(&config)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn enable_mcp_server_at_scope(
    state: tauri::State<'_, AppState>,
    server_name: String,
    scope: String,
) -> Result<(), String> {
    let mut config = state.config_loader.load_project_config()?;
    config.disabled_mcp_servers.retain(|n| n != &server_name);
    state.config_loader.save_project_config(&config)?;
    Ok(())
}
```

- [ ] **Step 3: Reflect disabled state in effective view**

Update `crates/agent-config/src/effective.rs` — set `disabled_by`:

```rust
if config.disabled_mcp_servers.contains(&name) {
    item = item.with_disabled(ConfigScope::Project);
}
```

- [ ] **Step 4: Register, regenerate, commit**

```bash
just gen-types && cargo test -p agent-config 2>&1 | tail -5
git add -A && git commit -m "feat(config): add project-level disable/override of user MCP servers"
```

---

### Task 15: Project disable/override — frontend context menu

**Files:**

- Modify: `apps/agent-gui/src/components/McpSettingsPane.vue`

- [ ] **Step 1: Add right-click context menu to server rows**

```html
<div class="server__context-menu" v-if="contextMenu.serverId === server.value.id">
  <button @click="disableAtProject(server)">在项目中禁用</button>
  <button @click="overrideAtProject(server)">在项目中覆盖</button>
</div>
```

- [ ] **Step 2: Implement disable/override handlers**

```typescript
async function disableAtProject(item: EffectiveItem<McpServerSettingsView>) {
  if (item.source === "user") {
    await commands.disableMcpServerAtScope(item.value.name, "project");
    await mcp.fetchEffectiveServers();
  }
}

async function overrideAtProject(item: EffectiveItem<McpServerSettingsView>) {
  // Open add dialog pre-filled with user values, target = project
  openAddDialogWithPrefill(item.value, "project");
}
```

- [ ] **Step 3: Run tests and commit**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -5
git add -A && git commit -m "feat(gui): add project-level disable/override context menu to MCP list"
```

---

### Task 16: Skill source management — remove skills.sh, keep skillhub

**Files:**

- Remove or feature-gate: `crates/agent-mcp/src/catalog/skills/skills_sh.rs`
- Modify: `crates/agent-mcp/src/catalog/skills/mod.rs`

- [ ] **Step 1: Remove skills.sh catalog source variant**

Delete `skills_sh.rs` module and remove from `mod.rs`:

```rust
// In catalog/skills/mod.rs — remove:
// pub mod skills_sh;
```

- [ ] **Step 2: Verify skillhub is functional**

Check `skillhub.rs` has zip download + extraction logic. If not, add:

```rust
// In catalog/skills/skillhub.rs
pub async fn download_and_install(
    entry: &SkillCatalogEntry,
    target_dir: &Path,
) -> Result<(), SkillInstallError> {
    let url = format!(
        "https://skillhub-1388575217.cos.accelerate.myqcloud.com/skills/{}/{}.zip",
        entry.name, entry.version,
    );
    let response = reqwest::get(&url).await?;
    let bytes = response.bytes().await?;

    // Extract to temp dir
    let temp_dir = tempfile::tempdir()?;
    let zip_path = temp_dir.path().join("skill.zip");
    tokio::fs::write(&zip_path, &bytes).await?;

    // Use zip crate to extract
    let file = std::fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    archive.extract(temp_dir.path())?;

    // Verify SKILL.md
    if !temp_dir.path().join("SKILL.md").exists() {
        return Err(SkillInstallError::MissingSkillMd);
    }

    // Move to target
    let dest = target_dir.join(&entry.name);
    tokio::fs::create_dir_all(&dest).await?;
    copy_dir_all(temp_dir.path(), &dest)?;

    Ok(())
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p agent-mcp 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent-mcp/src/catalog/skills/
git commit -m "feat(skills): remove skills.sh source, implement skillhub zip download"
```

---

### Task 17: Built-in MCP availability marking

**Files:**

- Modify: `crates/agent-mcp/src/catalog/data/builtin-catalog.json`
- Modify: `crates/agent-mcp/src/catalog/builtin.rs`

- [ ] **Step 1: Add verified field to builtin-catalog.json entries**

For each entry, add `"verified": true` or `"verified": false`:

```json
{
  "name": "git",
  "display_name": "Git",
  "verified": false,
  "...": "..."
}
```

- [ ] **Step 2: Parse verified field in BuiltinCatalogProvider**

Edit `builtin.rs`:

```rust
struct BuiltinServerEntry {
    name: String,
    // ...
    verified: Option<bool>,
}
```

Propagate to `ServerEntry`:

```rust
ServerEntry {
    // ...
    verified: entry.verified.unwrap_or(false),
}
```

- [ ] **Step 3: Show warning in effective list for unverified builtins**

In McpSettingsPane, for source=builtin + not verified:

```html
<span v-if="server.source === 'builtin' && !server.value.verified" class="tag tag--unverified">
  ⚠ 未验证
</span>
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent-mcp/src/catalog/
git commit -m "feat(mcp): add verified field to builtin catalog entries"
```

---

### Task 18: End-to-end verification

**Files:** None (verification only)

- [ ] **Step 1: Run full Rust test suite**

```bash
cargo test --workspace --all-targets 2>&1 | tail -20
```

Expected: All tests pass (pre-existing lifecycle failures excluded).

- [ ] **Step 2: Run GUI tests**

```bash
pnpm --filter agent-gui run test 2>&1 | tail -10
```

Expected: All tests pass.

- [ ] **Step 3: Run format and lint**

```bash
pnpm run format:check && pnpm run lint 2>&1 | tail -10
```

Expected: No errors.

- [ ] **Step 4: Regenerate types and verify no stale files**

```bash
just gen-types
git diff --name-only apps/agent-gui/src/generated/
```

Expected: Only intentional changes.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore: final verification — all tests pass, types regenerated"
```

---

### Task 19 (Optional, P2): Effective config viewer panel

**Files:**

- Create: `apps/agent-gui/src/components/EffectiveConfigViewer.vue`

- [ ] **Step 1: Build expandable field-source inspector**

```vue
<template>
  <div class="effective-config">
    <h3>生效配置</h3>
    <details v-for="item in effectiveServers" :key="item.value.id">
      <summary>{{ item.value.name }} ({{ item.source }})</summary>
      <dl>
        <div v-for="(val, key) in flattenFields(item.value)" :key="key">
          <dt>{{ key }}</dt>
          <dd>
            {{ val }} <span class="source-tag">{{ fieldSource(item, key) }}</span>
          </dd>
        </div>
      </dl>
    </details>
  </div>
</template>
```

- [ ] **Step 2: Run tests and commit**

```bash
pnpm --filter agent-gui run test && git add -A && git commit -m "feat(gui): add effective config viewer panel"
```

---

## Summary

| Task  | Component                                       | Priority |
| ----- | ----------------------------------------------- | -------- |
| 1-2   | Core types (ConfigScope, EffectiveItem)         | P0       |
| 3-4   | Config layer (effective merger, 4-tier loading) | P0       |
| 5-6   | Tauri commands (get*effective*\*)               | P0       |
| 7     | Pinia stores (effective views)                  | P0       |
| 8-9   | Vue components (unified list + Source column)   | P0       |
| 10    | Marketplace installed detection                 | P0       |
| 11-12 | MCP connectivity test (backend + frontend)      | P0       |
| 13    | ScopeSelector component                         | P1       |
| 14-15 | Project disable/override (backend + UI)         | P1       |
| 16    | skillhub zip + skills.sh removal                | P1       |
| 17    | Built-in MCP availability                       | P1       |
| 18    | E2E verification                                | —        |
| 19    | Effective config viewer                         | P2       |
