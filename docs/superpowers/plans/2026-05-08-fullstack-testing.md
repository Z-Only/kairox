# Full-Stack Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce tauri-pilot for real full-stack E2E testing and GitHub Models for live AI integration tests, with CI automation.

**Architecture:** Four-layer test pyramid — existing Layer 0 (unit/FakeModelClient) and Layer 2 (Playwright) are preserved. New Layer 1 adds Rust integration tests with live GitHub Models behind a feature flag. New Layer 3 adds tauri-pilot declarative TOML scenarios that exercise the real Tauri window + Rust backend + IPC.

**Tech Stack:** tauri-plugin-pilot (v0.5.0), tauri-pilot-cli, GitHub Models (OpenAI-compatible API), Rust feature flags, GitHub Actions CI with xvfb.

**Spec:** `docs/superpowers/specs/2026-05-08-fullstack-testing-design.md`

---

### Task 1: Upgrade Rust Toolchain to 1.95.0

**Files:**

- Modify: `rust-toolchain.toml`

- [ ] **Step 1: Update rust-toolchain.toml**

Replace the content of `rust-toolchain.toml` with:

```toml
[toolchain]
channel = "1.95.0"
components = ["clippy", "rustfmt"]
```

- [ ] **Step 2: Verify the toolchain installs and workspace compiles**

Run:

```bash
rustup install 1.95.0
cargo check --workspace 2>&1 | tail -5
```

Expected: compilation succeeds with no errors.

- [ ] **Step 3: Run clippy to ensure no new warnings**

Run:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -5
```

Expected: no warnings or errors.

- [ ] **Step 4: Commit**

```bash
git add rust-toolchain.toml
git commit -m "chore(deps): upgrade Rust toolchain to 1.95.0 for tauri-pilot compatibility"
```

---

### Task 2: Add tauri-plugin-pilot Dependency

**Files:**

- Modify: `apps/agent-gui/src-tauri/Cargo.toml`

- [ ] **Step 1: Add pilot dependency and feature to Cargo.toml**

Add at the end of `[dependencies]` section (before `[build-dependencies]`):

```toml
tauri-plugin-pilot = { git = "https://github.com/mpiton/tauri-pilot", optional = true }
```

Add a new `[features]` section after `[build-dependencies]`:

```toml
[features]
pilot = ["dep:tauri-plugin-pilot"]
```

- [ ] **Step 2: Verify it compiles without the feature**

Run:

```bash
cargo check -p agent-gui-tauri 2>&1 | tail -5
```

Expected: compiles successfully (pilot not included).

- [ ] **Step 3: Verify it compiles with the feature**

Run:

```bash
cargo check -p agent-gui-tauri --features pilot 2>&1 | tail -5
```

Expected: compiles successfully (pilot included).

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src-tauri/Cargo.toml
git commit -m "feat(gui): add tauri-plugin-pilot as optional dependency"
```

---

### Task 3: Register Pilot Plugin in lib.rs

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/lib.rs`

- [ ] **Step 1: Add pilot plugin registration**

In the `run()` function, find the line:

```rust
        .invoke_handler(tauri::generate_handler![
```

Insert the following block **before** `.invoke_handler(...)`:

```rust
        .setup(move |app| {
```

Wait — the `setup` closure already exists. Instead, find the end of the `setup` closure's `tauri::async_runtime::block_on` block, right after `});` that closes the cleanup background task block, and before `Ok(())`. Add pilot plugin registration inside the existing `setup` closure.

Actually, the pilot plugin must be registered on the **Builder chain**, not inside `setup`. Find:

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
```

Add after the updater plugin line:

```rust
        .plugin({
            #[cfg(all(debug_assertions, feature = "pilot"))]
            {
                tauri_plugin_pilot::init()
            }
            #[cfg(not(all(debug_assertions, feature = "pilot")))]
            {
                // No-op: pilot is only available in debug builds with the `pilot` feature
                tauri::plugin::Builder::new("pilot-noop").build()
            }
        })
```

Hmm, that's overly complex. A cleaner approach: use a conditional builder. Find the line:

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
```

Replace with:

```rust
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build());

    #[cfg(all(debug_assertions, feature = "pilot"))]
    {
        builder = builder.plugin(tauri_plugin_pilot::init());
    }

    builder
        .setup(move |app| {
```

- [ ] **Step 2: Verify it compiles without pilot**

Run:

```bash
cargo check -p agent-gui-tauri 2>&1 | tail -5
```

- [ ] **Step 3: Verify it compiles with pilot**

Run:

```bash
cargo check -p agent-gui-tauri --features pilot 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src-tauri/src/lib.rs
git commit -m "feat(gui): register tauri-pilot plugin (debug + feature-gated)"
```

---

### Task 4: Add Pilot Capability Permission

**Files:**

- Modify: `apps/agent-gui/src-tauri/capabilities/default.json`

- [ ] **Step 1: Add pilot:default permission**

Replace the permissions array in `default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:event:default",
    "core:event:allow-emit",
    "core:event:allow-listen",
    "pilot:default"
  ]
}
```

- [ ] **Step 2: Verify it compiles with pilot feature**

Run:

```bash
cargo check -p agent-gui-tauri --features pilot 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src-tauri/capabilities/default.json
git commit -m "feat(gui): add pilot:default capability permission"
```

---

### Task 5: Create Pilot E2E Test Scenarios

**Files:**

- Create: `apps/agent-gui/e2e-pilot/app-bootstrap.toml`
- Create: `apps/agent-gui/e2e-pilot/chat-flow.toml`
- Create: `apps/agent-gui/e2e-pilot/session-lifecycle.toml`

- [ ] **Step 1: Create app-bootstrap.toml**

```toml
[connect]
timeout_ms = 10000

[scenario]
name = "App bootstrap"
fail_fast = true
global_timeout_ms = 30000

[[step]]
name = "wait for sidebar"
action = "wait"
selector = "[data-test='sessions-sidebar']"
timeout_ms = 15000

[[step]]
name = "assert sidebar visible"
action = "assert-visible"
target = "[data-test='sessions-sidebar']"

[[step]]
name = "assert message input exists"
action = "assert-visible"
target = "textarea[data-test='message-input']"
```

- [ ] **Step 2: Create chat-flow.toml**

```toml
[connect]
timeout_ms = 10000

[scenario]
name = "Chat flow"
fail_fast = true
global_timeout_ms = 60000

[[step]]
name = "wait for app ready"
action = "wait"
selector = "[data-test='sessions-sidebar']"
timeout_ms = 15000

[[step]]
name = "type message"
action = "fill"
target = "textarea[data-test='message-input']"
value = "Hello from pilot test"

[[step]]
name = "send message"
action = "press"
key = "Enter"

[[step]]
name = "wait for user message"
action = "wait"
selector = ".message-user"
timeout_ms = 10000

[[step]]
name = "assert user message displayed"
action = "assert-text"
target = ".message-user"
expected = "Hello from pilot test"
```

- [ ] **Step 3: Create session-lifecycle.toml**

```toml
[connect]
timeout_ms = 10000

[scenario]
name = "Session lifecycle"
fail_fast = true
global_timeout_ms = 60000

[[step]]
name = "wait for app ready"
action = "wait"
selector = "[data-test='sessions-sidebar']"
timeout_ms = 15000

[[step]]
name = "assert initial session exists"
action = "assert-visible"
target = "[data-test='sessions-sidebar']"

[[step]]
name = "screenshot initial state"
action = "screenshot"
path = "tauri-pilot-failures/session-lifecycle-init.png"
```

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/e2e-pilot/
git commit -m "test(gui): add tauri-pilot declarative E2E test scenarios"
```

---

### Task 6: Create Pilot Test Runner Script

**Files:**

- Create: `scripts/run-pilot-tests.sh`

- [ ] **Step 1: Create the script**

```bash
#!/usr/bin/env bash
# Start debug Tauri app with pilot, run TOML tests, cleanup.
# Pre-condition: debug binary already built with `cargo tauri build --debug --no-bundle --features pilot`
set -euo pipefail

APP_BIN="${1:-apps/agent-gui/src-tauri/target/debug/agent-gui-tauri}"

# Start app in background
"$APP_BIN" &
APP_PID=$!
trap 'kill $APP_PID 2>/dev/null || true' EXIT

# Wait for pilot socket (up to 30s)
for i in $(seq 1 30); do
    if tauri-pilot ping 2>/dev/null; then
        echo "tauri-pilot connected"
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "ERROR: tauri-pilot ping timed out after 30s"
        exit 1
    fi
    sleep 1
done

# Run all TOML scenarios
tauri-pilot run apps/agent-gui/e2e-pilot/*.toml --junit pilot-results.xml
```

- [ ] **Step 2: Make it executable**

Run:

```bash
chmod +x scripts/run-pilot-tests.sh
```

- [ ] **Step 3: Commit**

```bash
git add scripts/run-pilot-tests.sh
git commit -m "test(gui): add tauri-pilot test runner script"
```

---

### Task 7: Add live-model-tests Feature Flag

**Files:**

- Modify: `crates/agent-runtime/Cargo.toml`

- [ ] **Step 1: Add feature flag**

Add at the end of `crates/agent-runtime/Cargo.toml`:

```toml

[features]
live-model-tests = []
```

- [ ] **Step 2: Verify compilation**

Run:

```bash
cargo check -p agent-runtime --features live-model-tests 2>&1 | tail -3
```

Expected: compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime/Cargo.toml
git commit -m "feat(runtime): add live-model-tests feature flag"
```

---

### Task 8: Create Test Profile Configs

**Files:**

- Create: `fixtures/test-profiles/github-models.toml`
- Create: `fixtures/test-profiles/fake-model.toml`

- [ ] **Step 1: Create github-models.toml**

```toml
# GitHub Models profile for CI integration tests.
# Uses GITHUB_TOKEN (auto-provided in GitHub Actions with `permissions: models: read`).
# Model: gpt-4o-mini (Low tier — 15 RPM / 150 RPD, generous for CI).

[profiles.github-gpt4o-mini]
provider = "openai_compatible"
model_id = "openai/gpt-4o-mini"
base_url = "https://models.github.ai/inference"
api_key_env = "GITHUB_TOKEN"
```

- [ ] **Step 2: Create fake-model.toml**

This config is used by tauri-pilot E2E tests so the app starts with a deterministic FakeModelClient (no real API calls).

```toml
# Fake model profile for deterministic tauri-pilot E2E tests.
# FakeModelClient echoes input or returns canned responses — no network needed.

[profiles.fake]
provider = "fake"
model_id = "fake"
```

- [ ] **Step 3: Commit**

```bash
git add fixtures/test-profiles/github-models.toml fixtures/test-profiles/fake-model.toml
git commit -m "test(runtime): add GitHub Models and fake model test profile configs"
```

---

### Task 9: Write Live Model Integration Tests

**Files:**

- Create: `crates/agent-runtime/tests/live_model_tests.rs`

- [ ] **Step 1: Create the test file**

```rust
//! Integration tests that call a real AI model (GitHub Models).
//!
//! Gated behind the `live-model-tests` feature flag:
//!   cargo test -p agent-runtime --features live-model-tests -- --test-threads=1
//!
//! Requires GITHUB_TOKEN env var with `models:read` scope.
//! Tests skip gracefully if the token is absent.
#![cfg(feature = "live-model-tests")]

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use std::time::Duration;
use tokio::time::timeout;

/// Load GitHub Models config from fixtures and build a runtime.
async fn make_live_runtime(
) -> Option<LocalRuntime<SqliteEventStore, agent_models::ModelRouter>> {
    let token = std::env::var("GITHUB_TOKEN").ok()?;
    if token.is_empty() {
        return None;
    }

    let config_str =
        std::fs::read_to_string("fixtures/test-profiles/github-models.toml").ok()?;
    let config = agent_config::load_from_str(&config_str, "github-models.toml").ok()?;
    let router = config.build_router();

    let store = SqliteEventStore::in_memory().await.ok()?;
    let runtime = LocalRuntime::new(store, router)
        .with_permission_mode(PermissionMode::Autonomous)
        .with_context_limit(8_000);
    Some(runtime)
}

/// Helper: skip test if no token available.
macro_rules! require_live_runtime {
    () => {
        match make_live_runtime().await {
            Some(rt) => rt,
            None => {
                eprintln!("SKIP: GITHUB_TOKEN not set or config not found");
                return;
            }
        }
    };
}

#[tokio::test]
async fn live_simple_chat() {
    let runtime = require_live_runtime!();

    let ws = timeout(Duration::from_secs(10), runtime.open_workspace())
        .await
        .expect("timeout opening workspace")
        .expect("failed to open workspace");

    let session = timeout(
        Duration::from_secs(10),
        runtime.start_session(StartSessionRequest {
            workspace_id: ws.id.clone(),
            title: None,
        }),
    )
    .await
    .expect("timeout starting session")
    .expect("failed to start session");

    let result = timeout(
        Duration::from_secs(60),
        runtime.send_message(SendMessageRequest {
            session_id: session.id.clone(),
            content: "Reply with exactly: PONG".to_string(),
        }),
    )
    .await
    .expect("timeout sending message");

    assert!(result.is_ok(), "send_message failed: {:?}", result.err());
}

#[tokio::test]
async fn live_streaming_fidelity() {
    let runtime = require_live_runtime!();

    let ws = timeout(Duration::from_secs(10), runtime.open_workspace())
        .await
        .expect("timeout")
        .expect("failed to open workspace");

    let session = timeout(
        Duration::from_secs(10),
        runtime.start_session(StartSessionRequest {
            workspace_id: ws.id.clone(),
            title: None,
        }),
    )
    .await
    .expect("timeout")
    .expect("failed to start session");

    // Send a request that should produce multiple streaming tokens
    let result = timeout(
        Duration::from_secs(60),
        runtime.send_message(SendMessageRequest {
            session_id: session.id.clone(),
            content: "Count from 1 to 5, one number per line.".to_string(),
        }),
    )
    .await
    .expect("timeout sending message");

    assert!(result.is_ok(), "send_message failed: {:?}", result.err());

    // Verify trace contains token events
    let trace = runtime.get_trace(&session.id).await.expect("get_trace failed");
    let has_tokens = trace
        .iter()
        .any(|e| e.event_type == "ModelTokenDelta");
    assert!(has_tokens, "Expected ModelTokenDelta events in trace");
}

#[tokio::test]
async fn live_tool_calling() {
    let runtime = require_live_runtime!();

    let cwd = std::env::current_dir().expect("cwd");
    let runtime = runtime.with_builtin_tools(cwd).await;

    let ws = timeout(Duration::from_secs(10), runtime.open_workspace())
        .await
        .expect("timeout")
        .expect("failed to open workspace");

    let session = timeout(
        Duration::from_secs(10),
        runtime.start_session(StartSessionRequest {
            workspace_id: ws.id.clone(),
            title: None,
        }),
    )
    .await
    .expect("timeout")
    .expect("failed to start session");

    // Ask the model to read a known file — triggers fs.read tool call
    let result = timeout(
        Duration::from_secs(90),
        runtime.send_message(SendMessageRequest {
            session_id: session.id.clone(),
            content: "Use the fs.read tool to read the file 'fixtures/test-profiles/github-models.toml' and tell me what provider it uses.".to_string(),
        }),
    )
    .await
    .expect("timeout sending message");

    assert!(result.is_ok(), "send_message failed: {:?}", result.err());

    // Verify trace contains tool-related events
    let trace = runtime.get_trace(&session.id).await.expect("get_trace failed");
    let has_tool_call = trace
        .iter()
        .any(|e| e.event_type == "ToolCallRequested" || e.event_type == "ToolResult");
    assert!(has_tool_call, "Expected tool call events in trace");
}
```

- [ ] **Step 2: Verify test compiles**

Run:

```bash
cargo test -p agent-runtime --features live-model-tests --no-run 2>&1 | tail -5
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime/tests/live_model_tests.rs
git commit -m "test(runtime): add live model integration tests (GitHub Models)"
```

---

### Task 10: Add justfile Commands

**Files:**

- Modify: `justfile`

- [ ] **Step 1: Add new test commands**

Append to the end of the justfile (after the `test-mcp` recipe):

```just

# Run tauri-pilot full-stack E2E tests (requires debug app with pilot feature)
test-pilot:
    cargo tauri build --debug --no-bundle --features pilot
    scripts/run-pilot-tests.sh

# Run live model integration tests (requires GITHUB_TOKEN env var)
test-live:
    cargo test -p agent-runtime --features live-model-tests -- --test-threads=1
```

- [ ] **Step 2: Verify just lists new commands**

Run:

```bash
just --list 2>&1 | grep -E "test-pilot|test-live"
```

Expected: both commands appear in the list.

- [ ] **Step 3: Commit**

```bash
git add justfile
git commit -m "chore: add just commands for tauri-pilot and live model tests"
```

---

### Task 11: Add CI Workflows

**Files:**

- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add live-model-tests job**

Add before the `ci-success` job in `ci.yml`:

```yaml
live-model-tests:
  name: Live Model Tests
  runs-on: ubuntu-latest
  timeout-minutes: 10
  permissions:
    contents: read
    models: read
  steps:
    - name: Checkout
      uses: actions/checkout@v6

    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Cache cargo
      uses: Swatinem/rust-cache@v2
      with:
        cache-on-failure: true
        shared-key: rust-ci

    - name: Install Linux system deps for Tauri crates
      run: |
        sudo apt-get update
        sudo apt-get install -y \
          libglib2.0-dev \
          libgtk-3-dev \
          libwebkit2gtk-4.1-dev \
          libappindicator3-dev \
          librsvg2-dev \
          patchelf

    - name: Run live model tests
      env:
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      run: cargo test -p agent-runtime --features live-model-tests -- --test-threads=1
```

- [ ] **Step 2: Add tauri-pilot-e2e job**

Add after the `live-model-tests` job:

```yaml
tauri-pilot-e2e:
  name: Tauri Pilot E2E
  runs-on: ubuntu-latest
  timeout-minutes: 25
  steps:
    - name: Checkout
      uses: actions/checkout@v6

    - name: Setup pnpm
      uses: pnpm/action-setup@v6

    - name: Setup Node.js
      uses: actions/setup-node@v6
      with:
        node-version: 22
        cache: pnpm

    - name: Install repo tooling deps
      run: pnpm install --frozen-lockfile

    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Cache cargo
      uses: Swatinem/rust-cache@v2
      with:
        cache-on-failure: true
        shared-key: rust-ci

    - name: Install Linux system deps
      run: |
        sudo apt-get update
        sudo apt-get install -y \
          libglib2.0-dev \
          libgtk-3-dev \
          libwebkit2gtk-4.1-dev \
          libappindicator3-dev \
          librsvg2-dev \
          patchelf \
          xvfb

    - name: Install tauri-pilot CLI
      run: cargo install tauri-pilot-cli --locked

    - name: Install Tauri CLI
      run: cargo install tauri-cli --locked

    - name: Build debug app with pilot
      run: cargo tauri build --debug --no-bundle --features pilot

    - name: Run pilot E2E tests
      run: xvfb-run --auto-servernum scripts/run-pilot-tests.sh

    - name: Upload pilot results
      uses: actions/upload-artifact@v7
      if: ${{ !cancelled() }}
      with:
        name: pilot-results
        path: pilot-results.xml
        retention-days: 7
```

- [ ] **Step 3: Update ci-success needs array**

Find the `ci-success` job's `needs` array and add the two new jobs:

```yaml
ci-success:
  name: CI
  if: ${{ always() }}
  needs:
    [
      format,
      lint-rust,
      lint-web,
      test,
      test-e2e,
      type-sync,
      build-tui,
      build-gui,
      live-model-tests,
      tauri-pilot-e2e
    ]
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add live model tests and tauri-pilot E2E CI jobs"
```

---

### Task 12: Update AGENTS.md Documentation

**Files:**

- Modify: `AGENTS.md`

- [ ] **Step 1: Add tauri-pilot section**

After the existing "E2E testing with Playwright" section, add a new section:

````markdown
### Full-stack E2E testing with tauri-pilot

[tauri-pilot](https://github.com/mpiton/tauri-pilot) provides real full-stack E2E testing by injecting a JS bridge via a Tauri plugin and communicating over Unix socket. Unlike the Playwright tests (which mock IPC), tauri-pilot tests exercise the real Tauri window, real Rust backend, and real IPC.

**Setup**: The `tauri-plugin-pilot` dependency is optional and feature-gated:

- Cargo feature: `pilot` in `apps/agent-gui/src-tauri/Cargo.toml`
- Runtime gate: `#[cfg(all(debug_assertions, feature = "pilot"))]` — never included in release builds

**Test scenarios** live in `apps/agent-gui/e2e-pilot/*.toml` (declarative TOML format).

**Running locally**:

```bash
# Start app with pilot enabled
cargo tauri dev --features pilot

# In another terminal
tauri-pilot ping
tauri-pilot snapshot -i
tauri-pilot run apps/agent-gui/e2e-pilot/chat-flow.toml

# Or run all scenarios via just
just test-pilot
```
````

**CI**: Runs on Linux with `xvfb-run` for virtual display.

````

- [ ] **Step 2: Add live model tests section**

After the "TUI and runtime integration tests" section, add:

```markdown
### Live model integration tests

Integration tests that call a real AI model API (GitHub Models) to verify streaming, tool calling, and end-to-end model interaction.

**Feature flag**: `live-model-tests` in `crates/agent-runtime/Cargo.toml`. Tests are gated behind this flag AND a runtime check for `GITHUB_TOKEN`.

**Config**: `fixtures/test-profiles/github-models.toml` — uses `openai/gpt-4o-mini` via GitHub Models API.

**Running**:
```bash
export GITHUB_TOKEN="ghp_your_token"  # needs models:read scope
just test-live
# Or directly:
cargo test -p agent-runtime --features live-model-tests -- --test-threads=1
````

**CI**: Runs on Linux only with `permissions: models: read` to auto-provide `GITHUB_TOKEN`.

````

- [ ] **Step 3: Update just command reference table**

Add to the command reference table:

```markdown
| `just test-pilot`         | Run tauri-pilot full-stack E2E tests              |
| `just test-live`          | Run live model integration tests (needs API key)  |
````

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md
git commit -m "docs: document tauri-pilot E2E and live model tests"
```

---

### Task 13: Verify Everything Works Locally

- [ ] **Step 1: Run existing tests to ensure no regressions**

```bash
cargo test --workspace --all-targets 2>&1 | tail -10
```

Expected: all existing tests pass.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -5
```

Expected: zero warnings.

- [ ] **Step 3: Verify live model tests compile**

```bash
cargo test -p agent-runtime --features live-model-tests --no-run 2>&1 | tail -5
```

Expected: compiles successfully.

- [ ] **Step 4: Verify pilot feature compiles**

```bash
cargo check -p agent-gui-tauri --features pilot 2>&1 | tail -5
```

Expected: compiles successfully.
