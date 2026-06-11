# Model Profile Source Consistency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure the GUI model selector, selected alias, session profile metadata, and runtime model router all use the same effective profile source after project config refresh.

**Architecture:** Add a focused regression test around the GUI `refresh_config` command delegate that reproduces a user-level and active project-level profile alias collision. Fix the command path so the installed `state.config`, `runtime.config()`, and refreshed model router are built from the same active config and expose the project override for the colliding alias.

**Tech Stack:** Rust, Tauri command state, `agent-config`, `agent-runtime`, `cargo test`.

---

### Task 1: Lock the Alias Collision Regression

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/app_state.rs`

- [x] **Step 1: Write the failing test**

Add a Tokio test in `app_state.rs` under the existing `#[cfg(test)]` module:

```rust
#[tokio::test]
async fn refresh_config_keeps_profile_info_and_runtime_router_in_sync_for_alias_overrides() {
    let _env_lock = HOME_ENV_LOCK.lock().await;
    let home_dir = unique_home();
    let config_dir = home_dir.join(".kairox");
    let workspace_root = home_dir.join("workspace");
    fs::create_dir_all(&config_dir).expect("config dir should be created");
    fs::create_dir_all(workspace_root.join(".kairox")).expect("project config dir should be created");
    fs::write(
        config_dir.join("config.toml"),
        r#"
[profiles.kairox-live]
provider = "anthropic"
model_id = "glink/claude-opus-4-6[1m]"
api_key_env = "ANTHROPIC_AUTH_TOKEN"
"#,
    )
    .expect("user config should be written");
    fs::write(
        workspace_root.join(".kairox").join("config.toml"),
        r#"
[profiles.kairox-live]
provider = "ali-idealab"
model_id = "gpt-5.4-0305-global"
"#,
    )
    .expect("project config should be written");

    let initial_config = Config::defaults();
    let router = initial_config.build_router();
    let runtime = LocalRuntime::new(SqliteEventStore::in_memory().await.unwrap(), router)
        .with_config(Arc::new(initial_config.clone()));
    let mut state = GuiState::new(
        runtime,
        initial_config,
        Arc::new(NoopMemoryStore) as Arc<dyn MemoryStore>,
    );
    state.home_dir = config_dir.clone();
    state.workspace_root = workspace_root.clone();
    let _home_guard = HomeEnvGuard::set(&home_dir);

    let _cwd_guard = CwdGuard::set(&workspace_root);

    state
        .refresh_active_config()
        .await
        .expect("config command delegate should refresh");

    let profile_info = state
        .config
        .read()
        .unwrap()
        .profile_info()
        .into_iter()
        .find(|profile| profile.alias == "kairox-live")
        .expect("profile info should include project alias");
    assert_eq!(profile_info.provider, "ali-idealab");
    assert_eq!(profile_info.model_id, "gpt-5.4-0305-global");

    let runtime_profile = state
        .runtime
        .config()
        .profiles
        .into_iter()
        .find(|(alias, _)| alias == "kairox-live")
        .expect("runtime config should include project alias")
        .1;
    assert_eq!(runtime_profile.provider, "ali-idealab");
    assert_eq!(runtime_profile.model_id, "gpt-5.4-0305-global");

    fs::remove_dir_all(home_dir).ok();
}
```

- [x] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -p agent-gui-tauri refresh_config_keeps_profile_info_and_runtime_router_in_sync_for_alias_overrides -- --nocapture`

Expected before fix: FAIL showing the colliding alias resolves to the user Anthropic profile in either `state.config` or `runtime.config()`.

### Task 2: Make Command Refresh Use One Effective Config Source

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/app_state.rs`
- Possibly Modify: `crates/agent-runtime/src/ui_bootstrap.rs` if the correct shared helper belongs there after inspection.

- [x] **Step 1: Implement the minimal fix**

Update the `refresh_config` command delegate so it loads the same active user + project + GUI profile overlay config that backs `get_profile_info` and the runtime router. Preserve current `profiles.toml` overlay behavior and knowledge-base root handling.

- [x] **Step 2: Run the focused test to verify it passes**

Run: `cargo test -p agent-gui-tauri refresh_config_keeps_profile_info_and_runtime_router_in_sync_for_alias_overrides -- --nocapture`

Expected after fix: PASS.

### Task 3: Regression and Quality Gates

**Files:**

- No further source changes unless tests reveal a directly related issue.

- [x] **Step 1: Run related Rust tests**

Run: `cargo test -p agent-gui-tauri app_state::tests:: -- --nocapture`

- [x] **Step 2: Run formatting**

Run: `cargo fmt --all --check`

- [x] **Step 3: Dev App validation if practical**

Follow `references/dev-app-verification.md` for the model selector flow using an isolated HOME. Verify `chat-model-option-kairox-live`, `aria-current`, session profile info, and send path do not cross from Ali/project to Anthropic/user for the same alias.
