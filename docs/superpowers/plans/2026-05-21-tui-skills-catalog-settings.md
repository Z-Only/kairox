# TUI Skills Catalog Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the TUI skills overlay from session activation only into a tabbed skills manager that can browse installed settings, search/list catalog entries, install catalog skills, update/delete installed skills, and toggle skill catalog sources.

**Architecture:** Reuse the existing `PluginOverlay` tab pattern in `crates/agent-tui/src/components/plugin_overlay.rs`, but keep all new commands and DTOs skill-specific. `dispatch_commands` builds a single `SkillOverlaySnapshot` from the `AppFacade` skills methods and refreshes the overlay after mutations. Existing discovered-skill session activation/deactivation remains in the `Discovered` tab.

**Tech Stack:** Rust, ratatui, crossterm, `agent_core::facade` skill DTOs, `agent-runtime` facade methods, `agent-tui` component tests and integration command-dispatch tests.

---

### Task 1: Add Failing Command Dispatch Tests

**Files:**

- Modify: `crates/agent-tui/tests/app_logic.rs`
- Modify later: `crates/agent-tui/src/components/mod.rs`
- Modify later: `crates/agent-tui/src/app/commands.rs`

- [x] **Step 1: Write failing tests**

Add tests near the existing skill command tests:

```rust
#[test]
fn colon_skill_catalog_input_dispatches_open_skills_overlay_command() {
    let commands = chat_commands_for_input(":skill catalog");
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenSkillsOverlay)),
        "expected Command::OpenSkillsOverlay; got {commands:?}"
    );
}

#[test]
fn skill_catalog_command_variant_can_carry_query() {
    let command = Command::ListSkillCatalog {
        keyword: Some("review".to_string()),
    };
    assert!(matches!(
        command,
        Command::ListSkillCatalog {
            keyword: Some(keyword),
        } if keyword == "review"
    ));
}

#[test]
fn skill_mutation_command_variants_carry_payloads() {
    let install = Command::InstallRemoteSkill {
        request: agent_core::facade::InstallRemoteSkillRequest {
            package: "review".to_string(),
            source: "skillhub".to_string(),
            target: agent_core::facade::SkillInstallTarget::User,
            package_url: Some("https://example.test/review.zip".to_string()),
        },
    };
    let update = Command::UpdateSkillSettings {
        skill_id: "review".to_string(),
    };
    let delete = Command::DeleteSkillSettings {
        skill_id: "review".to_string(),
    };

    assert!(matches!(install, Command::InstallRemoteSkill { request } if request.package == "review"));
    assert!(matches!(update, Command::UpdateSkillSettings { skill_id } if skill_id == "review"));
    assert!(matches!(delete, Command::DeleteSkillSettings { skill_id } if skill_id == "review"));
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui skill
```

Expected: compile failure because `ListSkillCatalog`, `InstallRemoteSkill`, `UpdateSkillSettings`, and `DeleteSkillSettings` do not exist, and `:skill catalog` is not parsed.

- [x] **Step 3: Add minimal command variants and chat parsing**

Add command variants in `crates/agent-tui/src/components/mod.rs`:

```rust
ListSkillCatalog {
    keyword: Option<String>,
},
InstallRemoteSkill {
    request: agent_core::facade::InstallRemoteSkillRequest,
},
UpdateSkillSettings {
    skill_id: String,
},
DeleteSkillSettings {
    skill_id: String,
},
SetSkillEnabled {
    skill_id: String,
    enabled: bool,
},
SetSkillSourceEnabled {
    source_id: String,
    enabled: bool,
},
RefreshSkillCatalog,
```

Parse `:skill catalog` and `:skill catalog <keyword>` in `crates/agent-tui/src/components/chat/input.rs`, mapping both to `Command::OpenSkillsOverlay` or `Command::ListSkillCatalog { keyword }` once overlay support exists.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same `cargo ... test -p agent-tui skill` command and keep only these new assertions passing before moving on.

### Task 2: Replace Skills Snapshot With Tabbed Manager Data

**Files:**

- Modify: `crates/agent-tui/src/components/mod.rs`
- Modify: `crates/agent-tui/src/app/commands.rs`
- Modify: `crates/agent-tui/src/components/skills_overlay.rs`

- [x] **Step 1: Write failing overlay tests**

Add component tests proving the tabbed overlay can render and dispatch from installed/catalog/source tabs:

```rust
#[test]
fn installed_tab_dispatches_enable_update_and_delete_commands() { /* build snapshot, press e/u/x */ }

#[test]
fn catalog_tab_installs_selected_entry_to_selected_target() { /* Tab to catalog, press i, press t, press i */ }

#[test]
fn sources_tab_toggles_selected_skill_source() { /* BackTab to sources, press e */ }

#[test]
fn discovered_tab_keeps_session_activation_commands() { /* existing a/d behavior still emits ActivateSkill/DeactivateSkill */ }
```

- [x] **Step 2: Run overlay tests to verify RED**

Run:

```bash
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui skills_overlay
```

Expected: compile/test failures because tabs and snapshot fields are not implemented.

- [x] **Step 3: Implement snapshot and overlay tabs**

Create `SkillOverlaySnapshot` in `components/mod.rs`:

```rust
pub struct SkillOverlaySnapshot {
    pub discovered: Vec<SkillEntry>,
    pub installed: Vec<agent_core::facade::SkillSettingsView>,
    pub catalog: Vec<agent_core::facade::SkillCatalogEntry>,
    pub sources: Vec<agent_core::facade::SkillSourceView>,
    pub install_target: agent_core::facade::SkillInstallTarget,
}
```

Change `CrossPanelEffect::ShowSkillsOverlay(Vec<SkillEntry>)` to `ShowSkillsOverlay(SkillOverlaySnapshot)`.

In `skills_overlay.rs`, add `SkillTab::{Discovered, Installed, Catalog, Sources}` and mirror the plugin overlay controls:

```text
Tab/BackTab: switch tab
j/k: navigate current tab
r: refresh overlay
Discovered: a/d activate or deactivate current session skill, Enter body
Installed: e enable/disable, u update, x/Delete delete
Catalog: i install selected catalog entry, t toggle user/project install target
Sources: e enable/disable source
```

- [x] **Step 4: Refresh snapshot from facade**

Update `refresh_skills_overlay` in `app/commands.rs` to call:

```rust
list_skills()
list_active_skills(session_id)
list_skill_settings()
list_skill_catalog(SkillCatalogQuery { keyword: None, sources: None, limit: Some(50) })
list_skill_sources()
```

On catalog/source errors, push a visible status message and keep the overlay usable with empty catalog/source lists.

- [x] **Step 5: Run focused tests to verify GREEN**

Run:

```bash
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui skills_overlay
```

### Task 3: Dispatch Skill Catalog And Mutation Commands

**Files:**

- Modify: `crates/agent-tui/src/app/commands.rs`
- Modify: `crates/agent-tui/tests/app_logic.rs`
- Modify: `crates/agent-tui/src/components/command_palette.rs`

- [x] **Step 1: Write failing integration test**

Extend `tui_skill_commands_call_facade_and_render_visible_messages` or add a new runtime-backed test using `LocalRuntime::with_skill_catalog(temp_dir)` plus a fake package manager where needed, then dispatch:

```rust
Command::ListSkillCatalog { keyword: None }
Command::InstallRemoteSkill { request }
Command::UpdateSkillSettings { skill_id }
Command::DeleteSkillSettings { skill_id }
Command::RefreshSkillCatalog
```

Assert visible status messages include catalog/mutation feedback and the overlay refresh path does not panic.

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui skill
```

- [x] **Step 3: Implement dispatch branches**

Add branches in `dispatch_commands`:

```rust
Command::ListSkillCatalog { keyword } => list_skill_catalog(...)
Command::InstallRemoteSkill { request } => install_remote_skill(...)
Command::UpdateSkillSettings { skill_id } => update_skill(...)
Command::DeleteSkillSettings { skill_id } => delete_skill_settings(...)
Command::SetSkillEnabled { skill_id, enabled } => set_skill_enabled(...)
Command::SetSkillSourceEnabled { source_id, enabled } => set_skill_source_enabled(...)
Command::RefreshSkillCatalog => refresh_skill_catalog(...)
```

Each branch refreshes the open skills overlay after success; outside the overlay, each branch pushes a concise visible status message.

- [x] **Step 4: Update command palette**

Add palette entries for:

```text
:skill catalog
:skill install <package>
:skill update <id>
:skill delete <id>
```

Argument-taking entries prefill chat input.

- [x] **Step 5: Run focused tests to verify GREEN**

Run:

```bash
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui skill
```

### Task 4: Final Verification And PR

**Files:**

- Check all modified Rust files
- Check `docs/superpowers/plans/2026-05-21-tui-skills-catalog-settings.md`

- [x] **Step 1: Format**

Run:

```bash
cargo fmt --all --check
bun run format:check
```

- [x] **Step 2: Required focused gates**

Run:

```bash
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui skill
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' check -p agent-tui
```

- [x] **Step 3: Broad Rust/Repo gates**

Run:

```bash
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test --workspace --all-targets
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' clippy --workspace --all-targets --all-features -- -D warnings
bun run lint:web
```

If bare `bun run lint` is still blocked by the user-level USTC mirror missing newly locked crates, record the exact failure and the equivalent `cargo --config ... clippy` command in the PR body.

- [ ] **Step 4: Commit and PR**

Commit:

```bash
git add docs/superpowers/plans/2026-05-21-tui-skills-catalog-settings.md crates/agent-tui/src crates/agent-tui/tests
git commit -m "feat(tui): add skills catalog manager"
```

Rebase onto `origin/iter/tui-parity`, push `feat/tui-skills-marketplace`, open PR against `iter/tui-parity`, enable squash auto-merge, watch CI, and clean up after merge.
