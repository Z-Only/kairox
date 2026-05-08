# Full-Stack Testing with tauri-pilot + GitHub Models

**Date**: 2026-05-08
**Status**: Draft
**Scope**: Introduce tauri-pilot for full-stack E2E testing and GitHub Models for live AI integration tests

## Problem Statement

Kairox currently has:

- **Playwright E2E** (11 specs) — tests Vue frontend with IPC mock, no real Rust backend
- **Rust integration tests** (full_stack.rs etc.) — tests LocalRuntime with FakeModelClient, no real UI
- **Vitest unit tests** — Vue component logic only

**Gap**: No tests exercise the complete stack: real Tauri window → real Rust backend → real IPC → real (or fake) model. No tests verify that the system works with a real AI model API.

## Decisions

| Decision                   | Choice                          | Rationale                                                                    |
| -------------------------- | ------------------------------- | ---------------------------------------------------------------------------- |
| Full-stack E2E tool        | tauri-pilot                     | Cross-platform (incl. macOS), plugin-based (no WebDriver), AI-agent friendly |
| Live model provider        | GitHub Models                   | Free GITHUB_TOKEN in CI, OpenAI-compatible API, zero code changes            |
| Model control              | Feature flag `live-model-tests` | Flexible: off by default, explicit opt-in locally, auto-enabled in CI        |
| CI platform for live tests | Linux only                      | Model calls are platform-independent; saves CI resources                     |
| Existing Playwright E2E    | Keep as-is                      | Proven, fast, great for frontend-only regression                             |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Layer 3: tauri-pilot E2E (real full-stack)              │  NEW
│  Real Tauri window + real Rust backend + real IPC        │
│  Model: FakeModelClient via test config OR GitHub Models │
│  Platforms: Linux CI (xvfb) + local macOS/Linux/Win     │
├─────────────────────────────────────────────────────────┤
│  Layer 2: Playwright E2E (frontend + IPC mock)           │  EXISTING
│  Vue SPA + tauri-mock.js, no Rust backend               │
│  Platforms: all + CI                                     │
├─────────────────────────────────────────────────────────┤
│  Layer 1: Rust integration tests (live model smoke)      │  NEW
│  LocalRuntime + GitHub Models (tool calling, streaming)  │
│  Control: feature flag `live-model-tests`                │
│  Platforms: Linux CI only, local opt-in                  │
├─────────────────────────────────────────────────────────┤
│  Layer 0: Unit / integration tests (FakeModelClient)     │  EXISTING
│  All Rust crates + Vue components (Vitest)               │
│  Platforms: all + CI                                     │
└─────────────────────────────────────────────────────────┘
```

## Part 1: tauri-pilot Integration

### What is tauri-pilot

[tauri-pilot](https://github.com/mpiton/tauri-pilot) is an interactive testing CLI for Tauri v2 apps. It lets AI agents (Claude Code) and developers inspect, interact with, and debug Tauri applications in real-time. Unlike WebDriver-based solutions (tauri-driver), it works by injecting a JS bridge via a Tauri plugin and communicating over Unix socket / Named Pipe — no external WebDriver binary needed.

**Key capabilities**:

- Snapshot accessibility tree, click/fill/type/press elements
- Assert text/visibility/value/URL
- Declarative TOML test scenarios with JUnit output
- Record/replay user interactions
- Execute JS (`eval`) and call Tauri IPC commands (`ipc`)
- Works on Linux, macOS, and Windows (macOS works because it bypasses WebDriver entirely)

### Integration changes

#### 1. Rust toolchain upgrade

tauri-pilot requires Rust 1.95.0+ (edition 2024). Current: 1.93.1.

**File**: `rust-toolchain.toml`

```toml
[toolchain]
channel = "1.95.0"    # was: "stable" (resolved to 1.93.1)
components = ["clippy", "rustfmt"]
```

> Pinning to 1.95.0 instead of "stable" avoids surprises. Can move to "stable" once 1.95+ is the default.

#### 2. Cargo dependency

**File**: `apps/agent-gui/src-tauri/Cargo.toml`

```toml
[dependencies]
tauri-plugin-pilot = { git = "https://github.com/mpiton/tauri-pilot", optional = true }

[features]
pilot = ["dep:tauri-plugin-pilot"]
```

Using `optional = true` + feature flag so the plugin is never compiled into release builds.

#### 3. Plugin registration

**File**: `apps/agent-gui/src-tauri/src/lib.rs`

```rust
// Inside the builder chain, after other plugins:
#[cfg(all(debug_assertions, feature = "pilot"))]
{
    builder = builder.plugin(tauri_plugin_pilot::init());
}
```

Double-gated: `debug_assertions` (never in release) AND `pilot` feature (explicit opt-in).

#### 4. Capability permission

**File**: `apps/agent-gui/src-tauri/capabilities/default.json`

Add `"pilot:default"` to the permissions array. Since the plugin only loads with `#[cfg(debug_assertions)]` + `feature = "pilot"`, this permission is harmless in release builds.

#### 5. Test scenario files

**Directory**: `apps/agent-gui/e2e-pilot/`

TOML declarative test scenarios:

| File                     | Tests                                                     |
| ------------------------ | --------------------------------------------------------- |
| `chat-flow.toml`         | Send message → wait for response → verify message display |
| `session-lifecycle.toml` | Create → switch → rename → delete sessions                |
| `app-bootstrap.toml`     | App launches → sidebar visible → workspace initialized    |

Each scenario uses a test-specific config (`fixtures/test-profiles/fake-model.toml`) that configures `FakeModelClient` so tests are deterministic and fast.

#### 6. Test runner script

**File**: `scripts/run-pilot-tests.sh`

Expects the debug binary to be pre-built. The caller (justfile or CI) handles the build step.

```bash
#!/usr/bin/env bash
# Start debug Tauri app with pilot, run TOML tests, cleanup.
# Pre-condition: `cargo tauri build --debug --no-bundle --features pilot` already run.
set -euo pipefail

APP_BIN="${1:-apps/agent-gui/src-tauri/target/debug/agent-gui}"
TIMEOUT="${PILOT_TIMEOUT:-120}"

$APP_BIN &
APP_PID=$!
trap 'kill $APP_PID 2>/dev/null || true' EXIT

# Wait for pilot socket to become ready (up to 30s)
for i in $(seq 1 30); do
    tauri-pilot ping 2>/dev/null && break
    sleep 1
done

tauri-pilot run apps/agent-gui/e2e-pilot/*.toml --junit pilot-results.xml
```

### tauri-pilot in CI

Linux CI requires `xvfb-run` because Tauri needs a display server:

```yaml
tauri-pilot-e2e:
  name: Tauri Pilot E2E
  runs-on: ubuntu-latest
  timeout-minutes: 20
  steps:
    - uses: actions/checkout@v6
    - name: Install system deps
      run: |
        sudo apt-get update
        sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
          libayatana-appindicator3-dev librsvg2-dev xvfb
    - uses: dtolnay/rust-toolchain@stable
      with: { toolchain: "1.95.0" }
    - name: Install tauri-pilot CLI
      run: cargo install tauri-pilot-cli --locked
    - name: Build debug app with pilot
      run: cargo tauri build --debug --no-bundle --features pilot
    - name: Run E2E tests
      run: xvfb-run --auto-servernum scripts/run-pilot-tests.sh
    - name: Upload results
      uses: actions/upload-artifact@v7
      if: ${{ !cancelled() }}
      with:
        name: pilot-results
        path: pilot-results.xml
        retention-days: 7
```

### Local development

```bash
# macOS / Linux — run app with pilot enabled
cargo tauri dev --features pilot

# In another terminal — interact
tauri-pilot ping
tauri-pilot snapshot -i
tauri-pilot click "#send-button"
tauri-pilot run apps/agent-gui/e2e-pilot/chat-flow.toml
```

## Part 2: GitHub Models Integration (Live Model Tests)

### Zero code changes

GitHub Models provides an OpenAI-compatible API at `https://models.github.ai/inference/chat/completions`. Kairox's `OpenAiCompatibleClient` already supports configurable `base_url` + `api_key_env`, so integration requires only a config file:

**File**: `fixtures/test-profiles/github-models.toml`

```toml
[profiles.github-gpt4o-mini]
provider = "openai_compatible"
model_id = "openai/gpt-4o-mini"
base_url = "https://models.github.ai/inference"
api_key_env = "GITHUB_TOKEN"
```

### Feature flag design

**File**: `crates/agent-runtime/Cargo.toml`

```toml
[features]
live-model-tests = []
```

**Test file**: `crates/agent-runtime/tests/live_model_tests.rs`

```rust
#![cfg(feature = "live-model-tests")]

// Tests only compile/run when:
//   cargo test -p agent-runtime --features live-model-tests

// Each test also checks GITHUB_TOKEN env var at runtime:
fn skip_without_token() -> bool {
    std::env::var("GITHUB_TOKEN").is_err()
}
```

### Test scenarios

| Test                      | Model       | Verifies                                                             |
| ------------------------- | ----------- | -------------------------------------------------------------------- |
| `live_simple_chat`        | gpt-4o-mini | Send message → receive streaming tokens → Completed event            |
| `live_tool_calling`       | gpt-4o-mini | Model requests fs.read tool → tool executes → model summarizes       |
| `live_streaming_fidelity` | gpt-4o-mini | TokenDelta events arrive in order, usage stats present on completion |

**Model choice**: `gpt-4o-mini` (Low tier) — 15 RPM / 150 RPD, sufficient for CI. Higher-tier models have stricter limits (10 RPM / 50 RPD).

### CI workflow

```yaml
live-model-tests:
  name: Live Model Tests
  runs-on: ubuntu-latest
  timeout-minutes: 10
  permissions:
    models: read
  steps:
    - uses: actions/checkout@v6
    - uses: dtolnay/rust-toolchain@stable
      with: { toolchain: "1.95.0" }
    - uses: Swatinem/rust-cache@v2
    - name: Run live model tests
      env:
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      run: cargo test -p agent-runtime --features live-model-tests -- --test-threads=1
```

Key details:

- `permissions: models: read` — enables GITHUB_TOKEN for model inference
- `--test-threads=1` — serializes tests to respect rate limits
- `timeout-minutes: 10` — prevents runaway costs on API hangs

### Rate limit protection

- **Low model (gpt-4o-mini)**: 15 RPM / 150 RPD
- **3 tests × ~2-3 requests each** = ~9 requests per CI run, well within limits
- **Serialized execution** (`--test-threads=1`) avoids concurrent request spikes
- **Each test has 60s timeout** via `tokio::time::timeout`
- **Runtime skip**: if `GITHUB_TOKEN` is unset, tests print a skip message and pass (no panic)

## Part 3: justfile & AGENTS.md Updates

### New just commands

| Command                 | Description                                  |
| ----------------------- | -------------------------------------------- |
| `just test-pilot`       | Run tauri-pilot E2E tests locally            |
| `just test-pilot-build` | Build debug app with pilot feature           |
| `just test-live`        | Run live model tests (requires GITHUB_TOKEN) |

### Updated `just test-all`

Add `test-pilot` and optionally `test-live` to the comprehensive test suite.

### AGENTS.md updates

- Add tauri-pilot section under "E2E testing"
- Add live model testing section
- Update test layer diagram
- Add `pilot` to feature flag documentation
- Update `just` command reference table

## Risks & Mitigations

| Risk                            | Impact                       | Mitigation                                                 |
| ------------------------------- | ---------------------------- | ---------------------------------------------------------- |
| tauri-pilot is pre-1.0 (v0.5.0) | API may change               | Pin git rev in Cargo.toml; feature-gated so easy to remove |
| Rust 1.95.0 upgrade             | Potential compilation issues | Test full workspace compilation before merging             |
| GitHub Models rate limits       | Flaky CI                     | Use Low model, serialize tests, skip if no token           |
| xvfb flakiness on Linux CI      | Test failures                | Retry + screenshot on failure                              |
| tauri-pilot socket timeout      | Tests hang                   | Script-level timeout + process cleanup                     |

## Out of Scope

- Replacing existing Playwright E2E tests (they remain as-is)
- Adding tauri-pilot tests for every existing Playwright scenario (start with 3 core flows)
- Using High-tier GitHub Models in CI (cost/rate concerns)
- Windows CI for live model tests (Linux-only per user decision)
- tauri-pilot on release builds (debug-only by design)

## File Change Summary

| File                                                 | Action | Scope                              |
| ---------------------------------------------------- | ------ | ---------------------------------- |
| `rust-toolchain.toml`                                | Modify | Pin to 1.95.0                      |
| `apps/agent-gui/src-tauri/Cargo.toml`                | Modify | Add tauri-plugin-pilot dep         |
| `apps/agent-gui/src-tauri/src/lib.rs`                | Modify | Register pilot plugin              |
| `apps/agent-gui/src-tauri/capabilities/default.json` | Modify | Add pilot:default                  |
| `crates/agent-runtime/Cargo.toml`                    | Modify | Add live-model-tests feature       |
| `crates/agent-runtime/tests/live_model_tests.rs`     | Create | Live model integration tests       |
| `fixtures/test-profiles/github-models.toml`          | Create | GitHub Models config               |
| `fixtures/test-profiles/fake-model.toml`             | Create | Test config for pilot E2E          |
| `apps/agent-gui/e2e-pilot/*.toml`                    | Create | Declarative test scenarios         |
| `scripts/run-pilot-tests.sh`                         | Create | Pilot test runner                  |
| `.github/workflows/ci.yml`                           | Modify | Add 2 new CI jobs                  |
| `justfile`                                           | Modify | Add test-pilot, test-live commands |
| `AGENTS.md`                                          | Modify | Document new test layers           |
