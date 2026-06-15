# Model Stream Timeouts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Anthropic streaming model sessions avoid an implicit 300 second total HTTP timeout while preserving configurable timeout controls for deployments that need them.

**Architecture:** Move timeout policy into `AnthropicConfig`: a short default connect timeout protects dead ports, while total request timeout defaults to disabled so long-running SSE streams are governed by runtime stream idle handling. Expose `connect_timeout_secs` and `request_timeout_secs` on `ProfileDef` and propagate them through `agent-config`'s builder.

**Tech Stack:** Rust, `reqwest`, `serde`, `agent-models`, `agent-config`, Kairox Dev App pilot.

---

## File Structure

- Modify `crates/agent-models/src/anthropic/config.rs`
  - Add timeout fields and defaults to `AnthropicConfig`.
- Modify `crates/agent-models/src/anthropic/client.rs`
  - Build `reqwest::Client` from config using connect timeout and optional total request timeout.
- Modify `crates/agent-models/src/anthropic/config_tests.rs`
  - TDD coverage for default timeout policy and serde round trip.
- Modify `crates/agent-models/src/anthropic/tests.rs`
  - Update Anthropic test config helper for new fields.
- Modify `crates/agent-models/src/anthropic/request_tests.rs`
  - Update literal config construction for new fields.
- Modify `crates/agent-config/src/types/profile.rs`
  - Add optional TOML profile fields.
- Modify `crates/agent-config/src/builder.rs`
  - Propagate profile timeout settings to `AnthropicConfig`.
- Modify `crates/agent-config/src/builder_tests.rs`
  - TDD coverage for builder propagation and update `ProfileDef` literals.
- Modify other `ProfileDef` literals in Rust tests only as needed for compilation.

Forbidden:

- Do not change model request payload schema.
- Do not change runtime stream idle timeout in `agent-runtime`.
- Do not edit generated GUI bindings.
- Do not modify user config files.

## Task 1: Add Failing Model Config Tests

**Files:**

- Modify: `crates/agent-models/src/anthropic/config_tests.rs`

- [ ] **Step 1: Write failing tests for default timeout policy**

Add assertions to `default_config_has_expected_values`:

```rust
assert_eq!(config.connect_timeout_secs, 15);
assert!(config.request_timeout_secs.is_none());
```

Extend `serde_round_trip` to include explicit timeout values:

```rust
let config = AnthropicConfig {
    temperature: Some(0.7),
    top_k: Some(40),
    connect_timeout_secs: 10,
    request_timeout_secs: Some(900),
    extra_params: Some(serde_json::json!({"foo": "bar"})),
    ..AnthropicConfig::default()
};
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p agent-models anthropic::config::tests -- --nocapture
```

Expected: FAIL to compile because `AnthropicConfig` has no `connect_timeout_secs` or `request_timeout_secs` fields.

## Task 2: Implement Anthropic Timeout Config

**Files:**

- Modify: `crates/agent-models/src/anthropic/config.rs`
- Modify: `crates/agent-models/src/anthropic/client.rs`
- Modify: `crates/agent-models/src/anthropic/tests.rs`
- Modify: `crates/agent-models/src/anthropic/request_tests.rs`

- [ ] **Step 1: Add config fields and defaults**

In `AnthropicConfig`, add:

```rust
#[serde(default = "default_connect_timeout_secs")]
pub connect_timeout_secs: u64,
#[serde(default)]
pub request_timeout_secs: Option<u64>,
```

Add helper:

```rust
pub fn default_connect_timeout_secs() -> u64 {
    15
}
```

Set defaults in `Default::default()`:

```rust
connect_timeout_secs: default_connect_timeout_secs(),
request_timeout_secs: None,
```

- [ ] **Step 2: Use timeout config when building HTTP client**

In `AnthropicClient::new`, replace the hard-coded total timeout builder with:

```rust
let mut builder = reqwest::Client::builder()
    .connect_timeout(std::time::Duration::from_secs(config.connect_timeout_secs));

if let Some(timeout_secs) = config.request_timeout_secs {
    builder = builder.timeout(std::time::Duration::from_secs(timeout_secs));
}

let http = builder.build().expect("failed to build reqwest client");
```

- [ ] **Step 3: Update Anthropic test constructors**

Where tests construct `AnthropicConfig` without `..Default::default()`, add:

```rust
connect_timeout_secs: default_connect_timeout_secs(),
request_timeout_secs: None,
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p agent-models anthropic::config::tests -- --nocapture
cargo test -p agent-models anthropic:: -- --nocapture
```

Expected: PASS.

## Task 3: Add Failing Profile Propagation Tests

**Files:**

- Modify: `crates/agent-config/src/builder_tests.rs`

- [ ] **Step 1: Write failing builder test**

Add a test that builds an anthropic profile with explicit timeouts and verifies the generated client configuration through an exposed test-only helper:

```rust
#[test]
fn anthropic_profile_propagates_timeout_settings() {
    let mut def = test_profile("anthropic", Some("https://api.anthropic.com"));
    def.connect_timeout_secs = Some(7);
    def.request_timeout_secs = Some(900);

    let config = build_anthropic_config("fast", &def);

    assert_eq!(config.connect_timeout_secs, 7);
    assert_eq!(config.request_timeout_secs, Some(900));
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
cargo test -p agent-config anthropic_profile_propagates_timeout_settings -- --nocapture
```

Expected: FAIL to compile because `ProfileDef` and `build_anthropic_config` do not yet expose these fields.

## Task 4: Implement Profile Timeout Propagation

**Files:**

- Modify: `crates/agent-config/src/types/profile.rs`
- Modify: `crates/agent-config/src/builder.rs`
- Modify: `crates/agent-config/src/builder_tests.rs`
- Modify: any Rust tests with `ProfileDef { ... }` literals that need the new fields.

- [ ] **Step 1: Add optional profile fields**

Add to `ProfileDef`:

```rust
#[serde(default)]
pub connect_timeout_secs: Option<u64>,
#[serde(default)]
pub request_timeout_secs: Option<u64>,
```

- [ ] **Step 2: Extract Anthropic config builder**

In `builder.rs`, add:

```rust
fn build_anthropic_config(alias: &str, def: &ProfileDef) -> AnthropicConfig {
    let base_url = def
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    let api_key_env = resolve_api_key_env(alias, def);
    let headers = profile_headers(def);
    let extra_params: Option<serde_json::Value> = def.extra_params.as_ref().map(|v| {
        let json_str = serde_json::to_string(v).unwrap_or_else(|_| "null".to_string());
        serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null)
    });

    AnthropicConfig {
        base_url,
        api_key_env,
        default_model: def.model_id.clone(),
        max_tokens: def
            .output_limit
            .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
        headers,
        capability_overrides: None,
        temperature: def.temperature,
        top_p: def.top_p,
        top_k: def.top_k,
        extra_params,
        connect_timeout_secs: def
            .connect_timeout_secs
            .unwrap_or_else(agent_models::anthropic::config::default_connect_timeout_secs),
        request_timeout_secs: def.request_timeout_secs,
    }
}
```

Then use it in the `"anthropic"` branch:

```rust
Box::new(AnthropicClient::new(build_anthropic_config(alias, def)))
```

- [ ] **Step 3: Update profile literals**

For each `ProfileDef { ... }` literal, add:

```rust
connect_timeout_secs: None,
request_timeout_secs: None,
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p agent-config anthropic_profile_propagates_timeout_settings -- --nocapture
cargo test -p agent-config
```

Expected: PASS.

## Task 5: Quality Gates and Dev App Verification

**Files:**

- No new source files expected.

- [ ] **Step 1: Run Rust quality gates**

Run:

```bash
cargo fmt --all --check
cargo clippy -p agent-models --all-targets -- -D warnings
cargo clippy -p agent-config --all-targets -- -D warnings
cargo test -p agent-models
cargo test -p agent-config
```

Expected: PASS.

- [ ] **Step 2: Run common lint gates**

Run:

```bash
bun run format:check
bun run lint
```

Expected: PASS.

- [ ] **Step 3: Dev App verification**

Start:

```bash
bun --filter agent-gui tauri dev --features pilot
```

Verify:

```bash
tauri-pilot ping
tauri-pilot snapshot -i
tauri-pilot logs --level error
```

Expected: app starts with pilot enabled and no JS errors. This is a Rust model transport behavior change; no GUI interaction flow changes, so no settings/UI mutation is required.

## Task 6: PR and Cleanup

**Files:**

- PR body temp file only.

- [ ] **Step 1: Commit**

Run:

```bash
git add crates/agent-models/src/anthropic/config.rs \
  crates/agent-models/src/anthropic/client.rs \
  crates/agent-models/src/anthropic/config_tests.rs \
  crates/agent-models/src/anthropic/tests.rs \
  crates/agent-models/src/anthropic/request_tests.rs \
  crates/agent-config/src/types/profile.rs \
  crates/agent-config/src/builder.rs \
  crates/agent-config/src/builder_tests.rs \
  docs/superpowers/plans/2026-06-15-model-stream-timeouts.md
git commit -m "fix(models): relax anthropic stream timeout"
```

- [ ] **Step 2: Push and create PR**

Run:

```bash
git fetch origin main
git rebase origin/main
git push -u origin fix/model-stream-timeouts
gh pr create --base main --head fix/model-stream-timeouts --title "fix(models): relax anthropic stream timeout" --body-file /tmp/kairox-model-stream-timeouts-pr.md
gh pr merge <pr-number> --auto --squash --delete-branch
```

- [ ] **Step 3: Watch until merged and clean up**

Run:

```bash
PR=<pr-number> bash .agents/skills/kairox-github-pr-ops/scripts/pr-watcher.sh
```

After merge, from main checkout remove:

```bash
git -C /Users/chanyu/AIProjects/kairox worktree remove /Users/chanyu/AIProjects/kairox/.worktrees/fix-model-stream-timeouts
git -C /Users/chanyu/AIProjects/kairox worktree remove /Users/chanyu/AIProjects/kairox/.worktrees/eval-kairox-control-sandbox-tests
git -C /Users/chanyu/AIProjects/kairox worktree remove /Users/chanyu/AIProjects/kairox/.worktrees/eval-kairox-project-sandbox-tests
git -C /Users/chanyu/AIProjects/kairox worktree prune
git -C /Users/chanyu/AIProjects/kairox branch -D fix/model-stream-timeouts eval/kairox-control-sandbox-tests eval/kairox-project-sandbox-tests
git -C /Users/chanyu/AIProjects/kairox push origin --delete fix/model-stream-timeouts || true
```
