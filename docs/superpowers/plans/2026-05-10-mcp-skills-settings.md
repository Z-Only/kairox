# MCP and Skills Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build first-class MCP and Skills settings management across Rust runtime, Tauri commands, generated TypeScript bindings, Pinia stores, and Vue settings UI.

**Architecture:** Add stable settings DTOs and facade methods in `agent-core`, implement orchestration in `agent-runtime`, keep local Skill discovery/state in `agent-skills`, and expose GUI workflows through `specta`-generated Tauri commands. Remote Skills operations use a replaceable `SkillPackageManager` trait with an initial `NpxSkillsPackageManager` adapter.

**Tech Stack:** Rust, Tokio, async-trait, serde, toml, tempfile, Tauri 2, tauri-specta, Vue 3 Composition API, Pinia, Vitest, Playwright mock IPC.

---

## File Structure

- Create `crates/agent-skills/src/state.rs`: read/write `skills-state.toml`, activation overrides, remote install records, and update metadata.
- Create `crates/agent-skills/src/settings.rs`: build effective Skill settings views, preserve shadowed entries, and surface invalid `SKILL.md` parse errors.
- Create `crates/agent-runtime/src/skill_package.rs`: define `SkillPackageManager`, fake package manager for tests, `NpxSkillsPackageManager`, and CLI output parsing.
- Create `crates/agent-runtime/src/mcp_settings.rs`: writable MCP config settings workflows for list/upsert/delete/enable/open-config.
- Create `crates/agent-runtime/src/skill_settings.rs`: runtime Skills settings workflows that merge local state with package manager operations.
- Modify `crates/agent-core/src/facade.rs`: add settings DTOs and `AppFacade` methods.
- Modify `crates/agent-skills/src/lib.rs`: export new modules and error variants.
- Modify `crates/agent-runtime/src/lib.rs` and `crates/agent-runtime/src/facade_runtime.rs`: wire new modules into `LocalRuntime` and facade methods.
- Modify `apps/agent-gui/src-tauri/src/commands.rs`: add Tauri commands and request/response bridge types.
- Modify `apps/agent-gui/src-tauri/src/specta.rs`, `apps/agent-gui/src-tauri/src/bin/export_specta.rs`, and `apps/agent-gui/src-tauri/src/lib.rs`: register new commands and types.
- Modify `apps/agent-gui/src/stores/mcp.ts` and `apps/agent-gui/src/stores/skills.ts`: use generated command types and expose settings actions.
- Create `apps/agent-gui/src/components/McpSettingsPane.vue` and `apps/agent-gui/src/components/SkillSettingsPane.vue`: focused settings panes.
- Modify `apps/agent-gui/src/views/SettingsView.vue`: keep only `General`, `MCP`, and `Skills` top-level tabs.
- Modify `apps/agent-gui/e2e/tauri-mock.js`: mock new settings commands for Playwright.

---

### Task 1: Core Settings DTOs and Facade Boundary

**Files:**

- Modify: `crates/agent-core/src/facade.rs`
- Test: `cargo test -p agent-core facade_settings_dtos --lib`

- [ ] **Step 1: Write the failing core DTO serde test**

Add this test module near existing `facade.rs` tests or create one if none exists:

```rust
#[cfg(test)]
mod facade_settings_dtos {
    use super::*;

    #[test]
    fn mcp_settings_input_serializes_stdio_transport() {
        let input = McpServerSettingsInput {
            name: "filesystem".to_string(),
            transport: McpServerSettingsTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
                env: BTreeMap::from([("ROOT".to_string(), "/tmp".to_string())]),
            },
            enabled: true,
            description: Some("Local files".to_string()),
        };

        let encoded = serde_json::to_string(&input).expect("input should serialize");
        assert!(encoded.contains("filesystem"));
        assert!(encoded.contains("stdio"));
    }

    #[test]
    fn skill_settings_view_distinguishes_scope_and_update_state() {
        let view = SkillSettingsView {
            id: "review".to_string(),
            name: "review".to_string(),
            description: "Review code".to_string(),
            version: Some("1.2.3".to_string()),
            scope: SkillSettingsScope::Project,
            path: "/workspace/.kairox/skills/review/SKILL.md".to_string(),
            enabled: true,
            activation_mode: "suggest".to_string(),
            install_source: SkillInstallSource::Registry,
            update_state: SkillUpdateState::UpdateAvailable,
            effective: true,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            editable: true,
            deletable: true,
        };

        assert_eq!(view.scope, SkillSettingsScope::Project);
        assert_eq!(view.update_state, SkillUpdateState::UpdateAvailable);
        assert!(view.editable);
    }
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
cargo test -p agent-core facade_settings_dtos --lib
```

Expected: compile fails with missing types such as `McpServerSettingsInput`, `SkillSettingsView`, and `SkillUpdateState`.

- [ ] **Step 3: Add DTOs and facade method signatures**

Add these public DTO families in `crates/agent-core/src/facade.rs` after existing Skill DTOs:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerSettingsTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    },
    Sse {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpServerSettingsInput {
    pub name: String,
    pub transport: McpServerSettingsTransport,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillSettingsScope {
    Project,
    User,
    Builtin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillInstallSource {
    Local,
    Registry,
    Github,
    Builtin,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillUpdateState {
    Unknown,
    UpToDate,
    UpdateAvailable,
    CheckFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillSettingsView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub scope: SkillSettingsScope,
    pub path: String,
    pub enabled: bool,
    pub activation_mode: String,
    pub install_source: SkillInstallSource,
    pub update_state: SkillUpdateState,
    pub effective: bool,
    pub shadowed_by: Option<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub editable: bool,
    pub deletable: bool,
}
```

Also add `SkillSettingsDetail`, `RemoteSkillSearchResult`, `SkillInstallTarget`, `InstallRemoteSkillRequest`, `InstallGithubSkillRequest`, and facade default methods:

```rust
async fn list_mcp_server_settings(&self) -> crate::Result<Vec<McpServerSettingsView>> { Ok(Vec::new()) }
async fn upsert_mcp_server_settings(&self, input: McpServerSettingsInput) -> crate::Result<McpServerSettingsView> { let _ = input; Err(crate::CoreError::InvalidState("MCP settings mutation not supported".into())) }
async fn delete_mcp_server_settings(&self, server_id: String) -> crate::Result<()> { let _ = server_id; Err(crate::CoreError::InvalidState("MCP settings deletion not supported".into())) }
async fn set_mcp_server_enabled(&self, server_id: String, enabled: bool) -> crate::Result<()> { let _ = (server_id, enabled); Err(crate::CoreError::InvalidState("MCP settings enablement not supported".into())) }
async fn open_mcp_config_file(&self) -> crate::Result<Option<String>> { Ok(None) }
async fn list_skill_settings(&self) -> crate::Result<Vec<SkillSettingsView>> { Ok(Vec::new()) }
async fn get_skill_settings_detail(&self, skill_id: String) -> crate::Result<Option<SkillSettingsDetail>> { let _ = skill_id; Ok(None) }
async fn set_skill_enabled(&self, skill_id: String, enabled: bool) -> crate::Result<()> { let _ = (skill_id, enabled); Err(crate::CoreError::InvalidState("Skill settings enablement not supported".into())) }
async fn delete_skill_settings(&self, skill_id: String) -> crate::Result<()> { let _ = skill_id; Err(crate::CoreError::InvalidState("Skill deletion not supported".into())) }
async fn search_remote_skills(&self, query: String) -> crate::Result<Vec<RemoteSkillSearchResult>> { let _ = query; Ok(Vec::new()) }
async fn install_remote_skill(&self, request: InstallRemoteSkillRequest) -> crate::Result<SkillSettingsView> { let _ = request; Err(crate::CoreError::InvalidState("Skill install not supported".into())) }
async fn install_github_skill(&self, request: InstallGithubSkillRequest) -> crate::Result<SkillSettingsView> { let _ = request; Err(crate::CoreError::InvalidState("GitHub Skill install not supported".into())) }
async fn update_skill(&self, skill_id: String) -> crate::Result<SkillSettingsView> { let _ = skill_id; Err(crate::CoreError::InvalidState("Skill update not supported".into())) }
```

- [ ] **Step 4: Run the test and verify GREEN**

Run:

```bash
cargo test -p agent-core facade_settings_dtos --lib
```

Expected: the new DTO tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-core/src/facade.rs
git commit -m "feat(core): add settings facade types"
```

---

### Task 2: Local Skill State and Settings Projection

**Files:**

- Create: `crates/agent-skills/src/state.rs`
- Create: `crates/agent-skills/src/settings.rs`
- Modify: `crates/agent-skills/src/lib.rs`
- Modify: `crates/agent-skills/src/registry.rs`
- Test: `crates/agent-skills/src/state.rs`, `crates/agent-skills/src/settings.rs`

- [ ] **Step 1: Write failing state and projection tests**

Create tests that prove state is separate from `SKILL.md`, invalid files degrade, and workspace shadows user/builtin:

```rust
#[tokio::test]
async fn state_file_persists_disabled_skill_without_touching_skill_markdown() {
    let root = tempfile::tempdir().expect("root should exist");
    let skill_directory = root.path().join("review");
    std::fs::create_dir_all(&skill_directory).expect("skill directory should exist");
    let skill_path = skill_directory.join("SKILL.md");
    std::fs::write(&skill_path, "---\nname: review\ndescription: Review code\n---\nBody\n")
        .expect("skill should be written");

    let state_path = root.path().join("skills-state.toml");
    let mut state = SkillsStateFile::default();
    state.set_enabled("review", false);
    write_skills_state(&state_path, &state).await.expect("state should write");

    let reloaded = read_skills_state(&state_path).await.expect("state should read");
    assert_eq!(reloaded.skill("review").and_then(|entry| entry.enabled), Some(false));
    let markdown = std::fs::read_to_string(skill_path).expect("skill markdown should remain");
    assert!(markdown.contains("description: Review code"));
}

#[tokio::test]
async fn settings_projection_keeps_shadowed_entries_visible() {
    let builtin_root = tempfile::tempdir().expect("builtin root");
    let user_root = tempfile::tempdir().expect("user root");
    let workspace_root = tempfile::tempdir().expect("workspace root");
    write_skill(builtin_root.path(), "builtin-review", "review", "Builtin review", "Builtin body\n");
    write_skill(user_root.path(), "user-review", "review", "User review", "User body\n");
    write_skill(workspace_root.path(), "workspace-review", "review", "Workspace review", "Workspace body\n");

    let projection = discover_skill_settings(vec![
        SkillRoot::new(SkillSourceKind::Builtin, builtin_root.path()),
        SkillRoot::new(SkillSourceKind::User, user_root.path()),
        SkillRoot::new(SkillSourceKind::Workspace, workspace_root.path()),
    ]).await.expect("settings should discover");

    assert_eq!(projection.skills.len(), 3);
    assert_eq!(projection.skills.iter().filter(|skill| skill.effective).count(), 1);
    assert!(projection.skills.iter().any(|skill| skill.scope == SkillSourceKind::Workspace && skill.effective));
    assert!(projection.skills.iter().any(|skill| skill.scope == SkillSourceKind::User && skill.shadowed_by.as_deref() == Some("workspace")));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p agent-skills state settings --lib
```

Expected: compile fails because `SkillsStateFile`, `write_skills_state`, `read_skills_state`, and `discover_skill_settings` do not exist.

- [ ] **Step 3: Implement state file support**

In `crates/agent-skills/src/state.rs`, add serde structs and atomic write helpers:

```rust
#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillsStateFile {
    #[serde(default)]
    pub skills: BTreeMap<String, SkillStateEntry>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct SkillStateEntry {
    pub enabled: Option<bool>,
    pub activation_mode: Option<SkillActivationMode>,
    pub install_source: Option<String>,
    pub remote: Option<String>,
    pub version: Option<String>,
    pub last_update_check: Option<String>,
    pub update_available: Option<bool>,
}

pub async fn read_skills_state(path: &Path) -> Result<SkillsStateFile> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(SkillsStateFile::default());
    }
    let raw = tokio::fs::read_to_string(path).await?;
    toml::from_str(&raw).map_err(|error| SkillError::InvalidStateFile(error.to_string()))
}

pub async fn write_skills_state(path: &Path, state: &SkillsStateFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let encoded = toml::to_string_pretty(state).map_err(|error| SkillError::InvalidStateFile(error.to_string()))?;
    let temporary_path = path.with_extension("toml.tmp");
    tokio::fs::write(&temporary_path, encoded).await?;
    tokio::fs::rename(temporary_path, path).await?;
    Ok(())
}
```

Add `SkillError::InvalidStateFile(String)` and export `state` from `lib.rs`.

- [ ] **Step 4: Implement settings projection**

In `crates/agent-skills/src/settings.rs`, add projection types that preserve every discovered candidate:

```rust
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SkillSettingsProjection {
    pub skills: Vec<LocalSkillSettingsView>,
    pub state_errors: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LocalSkillSettingsView {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub scope: SkillSourceKind,
    pub path: PathBuf,
    pub enabled: bool,
    pub activation_mode: SkillActivationMode,
    pub install_source: String,
    pub update_available: Option<bool>,
    pub effective: bool,
    pub shadowed_by: Option<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
}
```

Update discovery to collect invalid `SKILL.md` entries as invalid views instead of silently skipping them for settings projection. Keep `FileSkillRegistry::discover` behavior compatible for session activation.

- [ ] **Step 5: Run tests and verify GREEN**

Run:

```bash
cargo test -p agent-skills --lib
```

Expected: all `agent-skills` unit tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-skills/src/lib.rs crates/agent-skills/src/state.rs crates/agent-skills/src/settings.rs crates/agent-skills/src/registry.rs
git commit -m "feat(skills): add local settings state"
```

---

### Task 3: Skill Package Manager Trait and `npx skills` Adapter

**Files:**

- Create: `crates/agent-runtime/src/skill_package.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Modify: `crates/agent-runtime/Cargo.toml` only if a required dependency is not already available through workspace dependencies.
- Test: `crates/agent-runtime/src/skill_package.rs`

- [ ] **Step 1: Write failing adapter tests**

Add tests for parsing and error classification:

```rust
#[test]
fn parses_skills_find_lines_into_remote_results() {
    let output = "code-review\tReview code changes\tobra/superpowers\t1200\n";
    let results = parse_npx_skills_find_output(output).expect("output should parse");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "code-review");
    assert_eq!(results[0].repository.as_deref(), Some("obra/superpowers"));
    assert_eq!(results[0].install_count, Some(1200));
}

#[test]
fn missing_npx_is_classified_as_runtime_missing() {
    let error = classify_npx_spawn_error(std::io::Error::from(std::io::ErrorKind::NotFound));
    assert!(error.to_string().contains("npx"));
    assert!(error.to_string().contains("not found"));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p agent-runtime skill_package --lib
```

Expected: compile fails for missing `skill_package` module and parser functions.

- [ ] **Step 3: Implement trait, request types, fake manager, and parser**

In `skill_package.rs`, define:

```rust
#[async_trait::async_trait]
pub trait SkillPackageManager: Send + Sync {
    async fn search(&self, query: &str) -> agent_core::Result<Vec<RemoteSkillSearchResult>>;
    async fn install_from_registry(&self, request: &InstallRemoteSkillRequest) -> agent_core::Result<()>;
    async fn install_from_github(&self, request: &InstallGithubSkillRequest) -> agent_core::Result<()>;
    async fn check_updates(&self, skill_id: &str) -> agent_core::Result<SkillUpdateState>;
    async fn update(&self, skill_id: &str) -> agent_core::Result<()>;
}

#[derive(Default)]
pub struct FakeSkillPackageManager {
    pub search_results: tokio::sync::Mutex<Vec<RemoteSkillSearchResult>>,
}

pub struct NpxSkillsPackageManager;
```

Implement `NpxSkillsPackageManager` with `tokio::process::Command::new("npx")` and arguments `skills find`, `skills add`, `skills check`, and `skills update`. The adapter returns normalized `agent_core` DTOs and maps `std::io::ErrorKind::NotFound` to an actionable `CoreError::InvalidState` message.

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```bash
cargo test -p agent-runtime skill_package --lib
```

Expected: parser and fake-manager tests pass without network access.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime/src/skill_package.rs crates/agent-runtime/src/lib.rs crates/agent-runtime/Cargo.toml
git commit -m "feat(runtime): add skill package manager"
```

---

### Task 4: MCP Settings Runtime Workflows

**Files:**

- Create: `crates/agent-runtime/src/mcp_settings.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/src/mcp_manager.rs`
- Test: `crates/agent-runtime/src/mcp_settings.rs`

- [ ] **Step 1: Write failing MCP settings tests**

Add tests proving server-first settings views and enablement behavior:

```rust
#[tokio::test]
async fn disabling_running_server_stops_before_marking_disabled() {
    let config_path = write_mcp_config_fixture("[mcp_servers.files]\ncommand = \"npx\"\nargs = [\"server\"]\nenabled = true\n");
    let mut fake_manager = FakeMcpSettingsLifecycle::running("files");

    set_mcp_server_enabled_in_file(&config_path, &mut fake_manager, "files", false)
        .await
        .expect("server should be disabled");

    assert_eq!(fake_manager.stopped_servers(), vec!["files".to_string()]);
    let raw = tokio::fs::read_to_string(config_path).await.expect("config should read");
    assert!(raw.contains("enabled = false"));
}

#[tokio::test]
async fn enabling_server_does_not_start_it() {
    let config_path = write_mcp_config_fixture("[mcp_servers.files]\ncommand = \"npx\"\nargs = [\"server\"]\nenabled = false\n");
    let mut fake_manager = FakeMcpSettingsLifecycle::stopped("files");

    set_mcp_server_enabled_in_file(&config_path, &mut fake_manager, "files", true)
        .await
        .expect("server should be enabled");

    assert!(fake_manager.started_servers().is_empty());
    let raw = tokio::fs::read_to_string(config_path).await.expect("config should read");
    assert!(raw.contains("enabled = true"));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p agent-runtime mcp_settings --lib
```

Expected: compile fails for missing `mcp_settings` module and helper functions.

- [ ] **Step 3: Implement MCP settings helpers**

Implement functions that:

- Locate the writable MCP config path from runtime config or default to the user config path already used by marketplace TOML helpers.
- Parse TOML into a mutable `toml_edit::DocumentMut`.
- Upsert `[mcp_servers.<name>]` with transport-specific fields.
- Set `enabled` without starting the server.
- Stop a running server before disabling or deleting.
- Return `McpServerSettingsView` by merging config rows with `McpServerManager::server_statuses()` and `McpServerManager::is_trusted()`.

Expose small pure functions for TOML mutation so tests do not require live MCP processes.

- [ ] **Step 4: Add facade methods in `LocalRuntime`**

In `facade_runtime.rs`, implement:

```rust
async fn list_mcp_server_settings(&self) -> agent_core::Result<Vec<McpServerSettingsView>> {
    let manager = self.mcp_manager();
    mcp_settings::list_mcp_server_settings(&self.config, manager).await
}

async fn upsert_mcp_server_settings(
    &self,
    input: McpServerSettingsInput,
) -> agent_core::Result<McpServerSettingsView> {
    mcp_settings::upsert_mcp_server_settings(&self.config, input).await?;
    let manager = self.mcp_manager();
    let views = mcp_settings::list_mcp_server_settings(&self.config, manager).await?;
    views
        .into_iter()
        .find(|view| view.name == input.name)
        .ok_or_else(|| agent_core::CoreError::InvalidState("saved MCP server was not reloaded".into()))
}

async fn delete_mcp_server_settings(&self, server_id: String) -> agent_core::Result<()> {
    let manager = self.mcp_manager();
    mcp_settings::delete_mcp_server_settings(&self.config, manager, &server_id).await
}

async fn set_mcp_server_enabled(
    &self,
    server_id: String,
    enabled: bool,
) -> agent_core::Result<()> {
    let manager = self.mcp_manager();
    mcp_settings::set_mcp_server_enabled(&self.config, manager, &server_id, enabled).await
}

async fn open_mcp_config_file(&self) -> agent_core::Result<Option<String>> {
    mcp_settings::writable_mcp_config_path(&self.config)
        .map(|path| Some(path.display().to_string()))
}
```

For `open_mcp_config_file`, return the path string first. GUI shell-opening can be added later with a focused Tauri command if needed.

- [ ] **Step 5: Run tests and verify GREEN**

Run:

```bash
cargo test -p agent-runtime mcp_settings --lib
```

Expected: MCP settings tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime/src/mcp_settings.rs crates/agent-runtime/src/lib.rs crates/agent-runtime/src/facade_runtime.rs crates/agent-runtime/src/mcp_manager.rs
git commit -m "feat(runtime): add mcp settings workflows"
```

---

### Task 5: Skills Settings Runtime Workflows

**Files:**

- Create: `crates/agent-runtime/src/skill_settings.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/src/skills.rs`
- Test: `crates/agent-runtime/src/skill_settings.rs`

- [ ] **Step 1: Write failing runtime Skills settings tests**

Add tests with temporary roots and `FakeSkillPackageManager`:

```rust
#[tokio::test]
async fn list_skill_settings_maps_project_skill_to_editable_view() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    write_skill(workspace_root.path(), "review", "review", "Review code", "Body\n");

    let views = list_skill_settings_from_roots(SkillSettingsRoots {
        workspace_root: Some(workspace_root.path().to_path_buf()),
        user_root: None,
        builtin_root: None,
    }).await.expect("settings should list");

    let review = views.iter().find(|view| view.id == "review").expect("review skill");
    assert_eq!(review.scope, SkillSettingsScope::Project);
    assert!(review.editable);
    assert!(review.deletable);
}

#[tokio::test]
async fn installing_remote_skill_refreshes_installed_view() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let package_manager = FakeSkillPackageManager::default();

    let request = InstallRemoteSkillRequest {
        package: "obra/superpowers@brainstorming".to_string(),
        target: SkillInstallTarget::Project,
    };

    let installed = install_remote_skill_into_root(&package_manager, workspace_root.path(), request)
        .await
        .expect("remote skill should install");

    assert_eq!(installed.install_source, SkillInstallSource::Registry);
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p agent-runtime skill_settings --lib
```

Expected: compile fails for missing runtime settings workflow functions.

- [ ] **Step 3: Implement Skills settings orchestration**

Implement `skill_settings.rs` functions that:

- Build roots using existing runtime Skills root conventions from `crates/agent-runtime/src/skills.rs`.
- Convert `agent_skills::LocalSkillSettingsView` to `agent_core::SkillSettingsView`.
- Write enablement and activation mode changes to `skills-state.toml`.
- Reject edit/delete/update for built-in Skills with `CoreError::InvalidState`.
- Delete project/user Skill directories only when they are under the configured root.
- Call `SkillPackageManager` for search, registry install, GitHub install, check updates, and update.
- Refresh the settings list after install/update and return the matching `SkillSettingsView`.

- [ ] **Step 4: Wire facade methods**

In `facade_runtime.rs`, implement:

```rust
async fn list_skill_settings(&self) -> agent_core::Result<Vec<SkillSettingsView>> {
    skill_settings::list_skill_settings(self.skill_settings_roots()).await
}

async fn get_skill_settings_detail(
    &self,
    skill_id: String,
) -> agent_core::Result<Option<SkillSettingsDetail>> {
    skill_settings::get_skill_settings_detail(self.skill_settings_roots(), &skill_id).await
}

async fn set_skill_enabled(&self, skill_id: String, enabled: bool) -> agent_core::Result<()> {
    skill_settings::set_skill_enabled(self.skill_settings_roots(), &skill_id, enabled).await
}

async fn delete_skill_settings(&self, skill_id: String) -> agent_core::Result<()> {
    skill_settings::delete_skill(self.skill_settings_roots(), &skill_id).await
}

async fn search_remote_skills(
    &self,
    query: String,
) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
    self.skill_package_manager.search(&query).await
}

async fn install_remote_skill(
    &self,
    request: InstallRemoteSkillRequest,
) -> agent_core::Result<SkillSettingsView> {
    skill_settings::install_remote_skill(
        self.skill_settings_roots(),
        self.skill_package_manager.as_ref(),
        request,
    )
    .await
}

async fn install_github_skill(
    &self,
    request: InstallGithubSkillRequest,
) -> agent_core::Result<SkillSettingsView> {
    skill_settings::install_github_skill(
        self.skill_settings_roots(),
        self.skill_package_manager.as_ref(),
        request,
    )
    .await
}

async fn update_skill(&self, skill_id: String) -> agent_core::Result<SkillSettingsView> {
    skill_settings::update_skill(
        self.skill_settings_roots(),
        self.skill_package_manager.as_ref(),
        &skill_id,
    )
    .await
}
```

- [ ] **Step 5: Run tests and verify GREEN**

Run:

```bash
cargo test -p agent-runtime skill_settings --lib
```

Expected: Skills runtime tests pass without network access.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime/src/skill_settings.rs crates/agent-runtime/src/lib.rs crates/agent-runtime/src/facade_runtime.rs crates/agent-runtime/src/skills.rs
git commit -m "feat(runtime): add skill settings workflows"
```

---

### Task 6: Tauri Commands, Specta Registration, and Generated Bindings

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/bin/export_specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Generated: `apps/agent-gui/src/generated/commands.ts`
- Test: `just gen-types`, `cargo test -p agent-gui-tauri --lib`

- [ ] **Step 1: Add command compile test expectation**

Before adding commands to every registry, add one command such as `list_mcp_server_settings` only to `commands.rs` and run the specta export command to verify it is missing from generated bindings. This creates the failure mode the project warns about.

Run:

```bash
just gen-types
```

Expected: generated command file does not include the new command until it is registered in both `specta.rs` and `export_specta.rs`.

- [ ] **Step 2: Add Tauri command functions**

Add commands in `commands.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_mcp_server_settings(
    state: State<'_, GuiState>,
) -> Result<Vec<agent_core::McpServerSettingsView>, String> {
    state
        .runtime
        .list_mcp_server_settings()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_mcp_server_settings(
    state: State<'_, GuiState>,
    input: agent_core::McpServerSettingsInput,
) -> Result<agent_core::McpServerSettingsView, String> {
    state
        .runtime
        .upsert_mcp_server_settings(input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_mcp_server_enabled(
    state: State<'_, GuiState>,
    server_id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_mcp_server_enabled(server_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_mcp_server_settings(
    state: State<'_, GuiState>,
    server_id: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_mcp_server_settings(server_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn open_mcp_config_file(
    state: State<'_, GuiState>,
) -> Result<Option<String>, String> {
    state
        .runtime
        .open_mcp_config_file()
        .await
        .map_err(|error| error.to_string())
}
```

Add these Skills commands:

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_skill_settings(
    state: State<'_, GuiState>,
) -> Result<Vec<agent_core::SkillSettingsView>, String> {
    state
        .runtime
        .list_skill_settings()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_skill_settings_detail(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<agent_core::SkillSettingsDetail, String> {
    state
        .runtime
        .get_skill_settings_detail(skill_id.clone())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Skill not found: {skill_id}"))
}

#[tauri::command]
#[specta::specta]
pub async fn set_skill_enabled(
    state: State<'_, GuiState>,
    skill_id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_skill_enabled(skill_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_skill_settings(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_skill_settings(skill_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn search_remote_skills(
    state: State<'_, GuiState>,
    query: String,
) -> Result<Vec<agent_core::RemoteSkillSearchResult>, String> {
    state
        .runtime
        .search_remote_skills(query)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_remote_skill(
    state: State<'_, GuiState>,
    request: agent_core::InstallRemoteSkillRequest,
) -> Result<agent_core::SkillSettingsView, String> {
    state
        .runtime
        .install_remote_skill(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_github_skill(
    state: State<'_, GuiState>,
    request: agent_core::InstallGithubSkillRequest,
) -> Result<agent_core::SkillSettingsView, String> {
    state
        .runtime
        .install_github_skill(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_skill(
    state: State<'_, GuiState>,
    skill_id: String,
) -> Result<agent_core::SkillSettingsView, String> {
    state
        .runtime
        .update_skill(skill_id)
        .await
        .map_err(|error| error.to_string())
}
```

Every command maps facade errors with `.map_err(|error| error.to_string())`.

- [ ] **Step 3: Register every command and type**

Add each command to:

- `apps/agent-gui/src-tauri/src/specta.rs` `collect_commands![]`
- `apps/agent-gui/src-tauri/src/bin/export_specta.rs` `collect_commands![]`
- `apps/agent-gui/src-tauri/src/lib.rs` `tauri::generate_handler![]`

Add these type registrations in both Specta builders if they are not already inferred from command signatures:

```rust
.typ::<agent_core::McpServerSettingsView>()
.typ::<agent_core::McpServerSettingsInput>()
.typ::<agent_core::McpServerSettingsTransport>()
.typ::<agent_core::SkillSettingsView>()
.typ::<agent_core::SkillSettingsDetail>()
.typ::<agent_core::SkillSettingsScope>()
.typ::<agent_core::SkillInstallSource>()
.typ::<agent_core::SkillUpdateState>()
.typ::<agent_core::RemoteSkillSearchResult>()
.typ::<agent_core::SkillInstallTarget>()
.typ::<agent_core::InstallRemoteSkillRequest>()
.typ::<agent_core::InstallGithubSkillRequest>()
```

- [ ] **Step 4: Generate TypeScript bindings and verify GREEN**

Run:

```bash
just gen-types
cargo test -p agent-gui-tauri --lib
```

Expected: `apps/agent-gui/src/generated/commands.ts` contains new functions and Rust command registration tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/specta.rs apps/agent-gui/src-tauri/src/bin/export_specta.rs apps/agent-gui/src-tauri/src/lib.rs apps/agent-gui/src/generated/commands.ts
git commit -m "feat(gui): expose settings commands"
```

---

### Task 7: Pinia Settings Stores

**Files:**

- Modify: `apps/agent-gui/src/stores/mcp.ts`
- Modify: `apps/agent-gui/src/stores/skills.ts`
- Test: `apps/agent-gui/src/stores/mcp.test.ts`, `apps/agent-gui/src/stores/skills.test.ts`

- [ ] **Step 1: Write failing store tests**

Add tests that call generated command names through mocked `invoke`:

```ts
it("loads MCP settings servers from the settings command", async () => {
  mockInvoke.mockResolvedValueOnce([
    {
      id: "files",
      name: "files",
      transport: "stdio",
      enabled: true,
      runtime_status: "stopped",
      trusted: false,
      tool_count: null,
      last_error: null,
      writable: true,
      config_path: "/tmp/mcp.toml",
      description: null
    }
  ]);
  const store = useMcpStore();

  await store.fetchSettingsServers();

  expect(mockInvoke).toHaveBeenCalledWith("list_mcp_server_settings");
  expect(store.settingsServers[0].id).toBe("files");
});

it("does not optimistically keep failed skill enablement", async () => {
  mockInvoke.mockRejectedValueOnce(new Error("state file is read-only"));
  const store = useSkillsStore();
  store.skillSettings = [
    {
      id: "review",
      name: "review",
      description: "Review",
      version: null,
      scope: "user",
      path: "/tmp/SKILL.md",
      enabled: false,
      activation_mode: "manual",
      install_source: "local",
      update_state: "unknown",
      effective: true,
      shadowed_by: null,
      valid: true,
      validation_error: null,
      editable: true,
      deletable: true
    }
  ];

  await store.setSkillEnabled("review", true);

  expect(store.skillSettings[0].enabled).toBe(false);
  expect(store.error).toContain("state file is read-only");
});
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
pnpm --filter agent-gui test -- src/stores/mcp.test.ts src/stores/skills.test.ts --run
```

Expected: tests fail because store actions and state fields are missing.

- [ ] **Step 3: Implement store actions**

In `mcp.ts`, add:

- `settingsServers`
- `settingsLoading`
- `settingsError`
- `fetchSettingsServers()`
- `saveServerSettings(input)`
- `setServerEnabled(serverId, enabled)`
- `deleteServerSettings(serverId)`
- `openConfigFile()`

Use generated command types from `@/generated/commands` and keep existing status-popover methods.

In `skills.ts`, add:

- `skillSettings`
- `remoteResults`
- `settingsLoading`
- `remoteLoading`
- `loadSkillSettings()`
- `setSkillEnabled(skillId, enabled)`
- `deleteSkill(skillId)`
- `searchRemoteSkills(query)`
- `installRemoteSkill(package, target)`
- `installGithubSkill(source, target)`
- `updateSkill(skillId)`

On mutation failure, leave existing state unchanged and set `error`.

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```bash
pnpm --filter agent-gui test -- src/stores/mcp.test.ts src/stores/skills.test.ts --run
```

Expected: store tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/stores/mcp.ts apps/agent-gui/src/stores/mcp.test.ts apps/agent-gui/src/stores/skills.ts apps/agent-gui/src/stores/skills.test.ts
git commit -m "feat(gui): add settings stores"
```

---

### Task 8: Vue Settings Panes

**Files:**

- Create: `apps/agent-gui/src/components/McpSettingsPane.vue`
- Create: `apps/agent-gui/src/components/McpSettingsPane.test.ts`
- Create: `apps/agent-gui/src/components/SkillSettingsPane.vue`
- Create: `apps/agent-gui/src/components/SkillSettingsPane.test.ts`
- Modify: `apps/agent-gui/src/views/SettingsView.vue`
- Modify: `apps/agent-gui/src/views/SettingsView.test.ts`

- [ ] **Step 1: Write failing component tests**

Add `SettingsView` test:

```ts
it("shows General, MCP, and Skills tabs without a top-level Marketplace tab", () => {
  const wrapper = mountWithPlugins(SettingsView);
  const tabs = wrapper.findAll('[role="tab"]').map((tab) => tab.text());
  expect(tabs).toContain("General");
  expect(tabs).toContain("MCP");
  expect(tabs).toContain("Skills");
  expect(tabs).not.toContain("Marketplace");
});
```

Add `McpSettingsPane` and `SkillSettingsPane` tests for server rows, embedded marketplace, shadowed Skill state, invalid Skill state, and built-in read-only buttons.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
pnpm --filter agent-gui test -- src/views/SettingsView.test.ts src/components/McpSettingsPane.test.ts src/components/SkillSettingsPane.test.ts --run
```

Expected: tests fail because components and new tabs are missing.

- [ ] **Step 3: Implement `McpSettingsPane.vue`**

Create a server-first pane with:

- Search/filter controls.
- Default `Servers` sub-tab.
- Secondary `Marketplace` sub-tab embedding `<MarketplacePane />`.
- Add/edit form for stdio and SSE.
- Per-row buttons for enable/disable, start/stop, refresh tools, trust/revoke, edit, delete.
- Alerts for row-level and page-level errors.

Use native HTML, `.btn`, `.card`, `.tag`, and project CSS variables. All interactive buttons need accessible text and `data-test` selectors.

- [ ] **Step 4: Implement `SkillSettingsPane.vue`**

Create sections for:

- `Installed`
- `Discover`
- `Install from GitHub`

Render scope tags, enabled toggles, activation mode, effective/shadowed tags, invalid messages, update state, and read-only built-in actions. Disable edit/delete/update when the backend view marks them unavailable.

- [ ] **Step 5: Simplify `SettingsView.vue`**

Remove inline Skills list and top-level marketplace tab. Import and render:

```vue
<McpSettingsPane v-if="activeTab === 'mcp'" />
<SkillSettingsPane v-if="activeTab === 'skills'" />
```

Set:

```ts
const activeTab = ref<"general" | "mcp" | "skills">("general");
```

- [ ] **Step 6: Run component tests and verify GREEN**

Run:

```bash
pnpm --filter agent-gui test -- src/views/SettingsView.test.ts src/components/McpSettingsPane.test.ts src/components/SkillSettingsPane.test.ts --run
```

Expected: component tests pass.

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src/components/McpSettingsPane.vue apps/agent-gui/src/components/McpSettingsPane.test.ts apps/agent-gui/src/components/SkillSettingsPane.vue apps/agent-gui/src/components/SkillSettingsPane.test.ts apps/agent-gui/src/views/SettingsView.vue apps/agent-gui/src/views/SettingsView.test.ts
git commit -m "feat(gui): add mcp and skills settings panes"
```

---

### Task 9: Playwright Mock and Final Verification

**Files:**

- Modify: `apps/agent-gui/e2e/tauri-mock.js`
- Modify: existing MCP or settings E2E spec if the settings tab assertions need new selectors.
- Test: full focused verification commands below.

- [ ] **Step 1: Add failing E2E mock assertions**

Update or add a settings E2E assertion that navigates to settings, opens `MCP`, sees configured servers, opens embedded marketplace, opens `Skills`, searches remote Skills, and sees a missing `npx` error when the mock is configured to fail.

Run:

```bash
just test-e2e
```

Expected: the relevant scenario fails until `tauri-mock.js` handles the new command names.

- [ ] **Step 2: Update `tauri-mock.js`**

Handle these command names:

```js
"list_mcp_server_settings";
"upsert_mcp_server_settings";
"set_mcp_server_enabled";
"delete_mcp_server_settings";
"open_mcp_config_file";
"list_skill_settings";
"get_skill_settings_detail";
"set_skill_enabled";
"delete_skill_settings";
"search_remote_skills";
"install_remote_skill";
"install_github_skill";
"update_skill";
```

Return deterministic mock data for project, user, built-in, shadowed, invalid, registry-installed, and GitHub-installed Skills.

- [ ] **Step 3: Run focused verification**

Run:

```bash
cargo test -p agent-core facade_settings_dtos --lib
cargo test -p agent-skills --lib
cargo test -p agent-runtime mcp_settings skill_settings skill_package --lib
just gen-types
pnpm --filter agent-gui test -- src/stores/mcp.test.ts src/stores/skills.test.ts src/views/SettingsView.test.ts src/components/McpSettingsPane.test.ts src/components/SkillSettingsPane.test.ts --run
just test-e2e
```

Expected: every command exits 0. If `just test-e2e` fails for an unrelated pre-existing issue, capture the failing spec and run the focused settings spec after confirming with the user.

- [ ] **Step 4: Run lint diagnostics for modified files**

Use IDE diagnostics for modified files and fix new syntax or type errors:

```text
read_lints on modified Rust, TypeScript, and Vue files
```

Expected: no new lint errors in files changed by this plan.

- [ ] **Step 5: Commit final mock and verification updates**

```bash
git add apps/agent-gui/e2e/tauri-mock.js apps/agent-gui/e2e
git commit -m "test(gui): cover mcp and skills settings flows"
```

---

## Self-Review Checklist

- Spec coverage: Tasks 1-6 cover Rust facade/runtime/Tauri, Tasks 7-9 cover Pinia/Vue/E2E, and every MCP/Skills operation from the spec has a command and store action.
- TDD coverage: each task starts with a failing test or failing generation check before implementation.
- Type consistency: DTO names match `McpServerSettingsView`, `McpServerSettingsInput`, `SkillSettingsView`, `SkillSettingsDetail`, `RemoteSkillSearchResult`, `SkillInstallTarget`, `InstallRemoteSkillRequest`, and `InstallGithubSkillRequest`.
- Verification: Task 9 includes Rust, generated types, GUI unit tests, E2E, and IDE lint diagnostics.
