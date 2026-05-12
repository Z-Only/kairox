# Model Configuration Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow any provider name with auto-detected protocol, add upward config search, and support full model parameters (temperature, top_p, top_k, headers, capability overrides, extra_params).

**Architecture:** Extend `ProfileDef` with 9 new fields, remove provider whitelist, add `provider_family()` auto-detection, add `find_config_upward()` for reliable `.kairox/` discovery, and wire new params into OpenAI-compatible and Anthropic HTTP clients.

**Tech Stack:** Rust (serde, toml, reqwest), Tauri 2, Vue 3/TypeScript

---

### Task 1: Extend `ProfileDef` and `ProfileToml` with new fields

**Files:**

- Modify: `crates/agent-config/src/lib.rs:23-41`
- Modify: `crates/agent-config/src/loader.rs:18-33`

- [ ] **Step 1: Add new fields to `ProfileDef` in `lib.rs`**

```rust
// In crates/agent-config/src/lib.rs, replace the ProfileDef struct:

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDef {
    pub provider: String,
    pub model_id: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub output_limit: Option<u64>,
    #[serde(default)]
    pub response: Option<String>,
    // -- new fields --
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub supports_tools: Option<bool>,
    #[serde(default)]
    pub supports_vision: Option<bool>,
    #[serde(default)]
    pub supports_reasoning: Option<bool>,
    #[serde(default)]
    pub extra_params: Option<toml::Value>,
}
```

- [ ] **Step 2: Add new fields to `ProfileToml` in `loader.rs`**

```rust
// In crates/agent-config/src/loader.rs, replace the ProfileToml struct:

#[derive(Debug, serde::Deserialize)]
struct ProfileToml {
    provider: String,
    model_id: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default)]
    context_window: Option<u64>,
    #[serde(default)]
    output_limit: Option<u64>,
    #[serde(default)]
    response: Option<String>,
    // -- new fields --
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    top_k: Option<u32>,
    #[serde(default)]
    headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    supports_tools: Option<bool>,
    #[serde(default)]
    supports_vision: Option<bool>,
    #[serde(default)]
    supports_reasoning: Option<bool>,
    #[serde(default)]
    extra_params: Option<toml::Value>,
}
```

- [ ] **Step 3: Wire new fields in `load_from_str()` ProfileDef construction**

In `crates/agent-config/src/loader.rs`, update the `ProfileDef` construction inside `load_from_str()` (around line 51-60):

```rust
let profile_def = ProfileDef {
    provider: profile_toml.provider,
    model_id: profile_toml.model_id,
    base_url: profile_toml.base_url,
    api_key: profile_toml.api_key,
    api_key_env: profile_toml.api_key_env,
    context_window: profile_toml.context_window,
    output_limit: profile_toml.output_limit,
    response: profile_toml.response,
    max_tokens: profile_toml.max_tokens,
    temperature: profile_toml.temperature,
    top_p: profile_toml.top_p,
    top_k: profile_toml.top_k,
    headers: profile_toml.headers,
    supports_tools: profile_toml.supports_tools,
    supports_vision: profile_toml.supports_vision,
    supports_reasoning: profile_toml.supports_reasoning,
    extra_params: profile_toml.extra_params,
};
```

- [ ] **Step 4: Update `Config::defaults()` to include new fields**

In `crates/agent-config/src/lib.rs`, update all 3 `ProfileDef` literals in `Config::defaults()` to include the new fields. Each `ProfileDef` literal needs the 9 new fields set to `None`:

For the `fake` entry, add after `response: Some(...)`:

```rust
max_tokens: None,
temperature: None,
top_p: None,
top_k: None,
headers: None,
supports_tools: None,
supports_vision: None,
supports_reasoning: None,
extra_params: None,
```

For the `local-code` entry, add the same 9 fields after `response: None`.

For the `fast` entry (conditional on `OPENAI_API_KEY`), add the same 9 fields after `response: None`.

- [ ] **Step 5: Update `ProfileDef` literals in builder tests**

In `crates/agent-config/src/builder.rs`, the test `build_profile_sets_capabilities_per_provider` constructs two `ProfileDef` structs directly (`fast_def` and `ollama_def`). Add the 9 new fields set to `None` to each.

- [ ] **Step 6: Run tests to verify compilation and existing tests pass**

```bash
cargo test -p agent-config --lib
```

Expected: all existing tests pass (new fields have `#[serde(default)]` so they don't break existing TOML parsing).

- [ ] **Step 7: Commit**

```bash
git add crates/agent-config/src/lib.rs crates/agent-config/src/loader.rs crates/agent-config/src/builder.rs
git commit -m "feat(config): add sampling params, headers, capability overrides, and extra_params to ProfileDef"
```

---

### Task 2: Add `find_config_upward()` to discovery

**Files:**

- Modify: `crates/agent-config/src/discovery.rs`

- [ ] **Step 1: Add `find_config_upward` function**

Add after the existing `find_config_from` function (before `#[cfg(test)]`):

```rust
/// Walk up from `start_dir` to at most 5 parent directories looking for
/// `.kairox/config.toml`. Returns the path and `ConfigSource::ProjectFile`
/// when found, or `None`.
pub fn find_config_upward(start_dir: &Path) -> Option<(PathBuf, ConfigSource)> {
    let mut current = Some(start_dir);
    for _ in 0..=5 {
        let dir = current?;
        let candidate = dir.join(CONFIG_DIR).join(CONFIG_FILENAME);
        if candidate.is_file() {
            return Some((candidate, ConfigSource::ProjectFile));
        }
        current = dir.parent();
    }
    None
}
```

- [ ] **Step 2: Update `pub use` export in `lib.rs`**

In `crates/agent-config/src/lib.rs` line 9, update the public export:

```rust
pub use discovery::{find_config, find_config_upward};
```

- [ ] **Step 3: Write unit test for upward search**

Add to the test module in `discovery.rs`:

```rust
#[test]
fn find_config_upward_discovers_in_parent() {
    let project_dir = TempDir::new().expect("project temp dir");
    let nested_dir = project_dir.path().join("a").join("b").join("c");
    std::fs::create_dir_all(&nested_dir).expect("create nested dirs");
    let config_path = project_dir.path().join(".kairox/config.toml");
    write_config(&config_path);

    let (path, source) = find_config_upward(&nested_dir)
        .expect("config found via upward search");
    assert_eq!(path, config_path);
    assert_eq!(source, ConfigSource::ProjectFile);
}

#[test]
fn find_config_upward_stops_after_5_levels() {
    let project_dir = TempDir::new().expect("project temp dir");
    // Create config 6 levels above — should NOT be found
    let deep_dir = (0..=6).fold(project_dir.path().to_path_buf(), |p, i| {
        let d = p.join(format!("d{i}"));
        std::fs::create_dir_all(&d).expect("create dir");
        d
    });
    let config_path = project_dir.path().join(".kairox/config.toml");
    write_config(&config_path);

    let result = find_config_upward(&deep_dir);
    assert!(result.is_none(), "should not find config beyond 5 levels");
}

#[test]
fn find_config_upward_returns_none_when_no_config() {
    let project_dir = TempDir::new().expect("project temp dir");
    let nested = project_dir.path().join("a").join("b");
    std::fs::create_dir_all(&nested).expect("create nested dirs");

    let result = find_config_upward(&nested);
    assert!(result.is_none());
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p agent-config -- discovery
```

Expected: 3 new tests pass + all existing discovery tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-config/src/discovery.rs crates/agent-config/src/lib.rs
git commit -m "feat(config): add find_config_upward for reliable .kairox/ discovery"
```

---

### Task 3: Fix `Config::load_inner()` to use upward search fallback

**Files:**

- Modify: `crates/agent-config/src/lib.rs:227-248`

- [ ] **Step 1: Update `load_inner()` to use `find_config_upward`**

Replace the `load_inner` method:

```rust
fn load_inner(project_root: Option<&std::path::Path>) -> Result<Self, ConfigError> {
    let mut base = Self::defaults();

    // Layer 1: merge user-level config if present
    if let Some(home_dir) = dirs::home_dir() {
        let user_path = home_dir.join(".kairox").join("config.toml");
        if user_path.is_file() {
            base = Self::merge_config(base, &user_path)?;
        }
    }

    // Layer 2: merge project-level config if present (highest priority)
    if let Some(root) = project_root {
        let project_path = root.join(".kairox").join("config.toml");
        if project_path.is_file() {
            base = Self::merge_config(base, &project_path)?;
            base.source = ConfigSource::ProjectFile;
        } else {
            // Fallback: walk up from project_root looking for .kairox/config.toml
            if let Some((found_path, _)) = discovery::find_config_upward(root) {
                base = Self::merge_config(base, &found_path)?;
                base.source = ConfigSource::ProjectFile;
            }
        }
    }

    Ok(base)
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p agent-config --lib
```

Expected: all existing tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-config/src/lib.rs
git commit -m "fix(config): use upward search as fallback when project_root lacks .kairox/"
```

---

### Task 4: Remove provider whitelist from `validate()` and add auto-detection

**Files:**

- Modify: `crates/agent-config/src/loader.rs:185-212`
- Modify: `crates/agent-config/src/builder.rs:149-217`

- [ ] **Step 1: Remove `known_providers` check from `validate()`**

Replace the `validate()` function in `loader.rs`:

```rust
/// Validate the configuration: check for missing required fields, etc.
pub fn validate(config: &Config) -> Result<(), ConfigError> {
    for (alias, profile) in &config.profiles {
        // openai_compatible requires base_url
        if profile.provider == "openai_compatible" && profile.base_url.is_none() {
            return Err(ConfigError::Parse {
                path: "config".to_string(),
                message: format!(
                    "profile '{}' uses 'openai_compatible' provider but missing 'base_url'",
                    alias
                ),
            });
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Update the `rejects_unknown_provider` test**

In `loader.rs` test module, replace the `rejects_unknown_provider` test — it should now accept unknown providers:

```rust
#[test]
fn accepts_any_provider_name() {
    let toml = r#"
[profiles.custom]
provider = "deepseek"
model_id = "deepseek-chat"
base_url = "https://api.deepseek.com/v1"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    let result = validate(&config);
    assert!(result.is_ok(), "any provider name should be accepted");
}
```

Also remove the `UnknownProvider` variant from `ConfigError` in `lib.rs`:

```rust
// Remove this variant:
#[error("profile '{profile}' has unknown provider '{provider}'")]
UnknownProvider { profile: String, provider: String },
```

- [ ] **Step 3: Add `provider_family()` and update `build_client()` in `builder.rs`**

Add `provider_family()` helper:

```rust
/// Map a provider name to a client family.
/// Known providers map to their specific client; everything else maps to
/// `openai_compatible` since most third-party APIs follow the OpenAI protocol.
fn provider_family(provider: &str) -> &str {
    match provider {
        "anthropic" => "anthropic",
        "ollama" => "ollama",
        "fake" => "fake",
        "openai_compatible" => "openai_compatible",
        _ => "openai_compatible",
    }
}
```

Replace the `build_client()` function:

```rust
fn build_client(alias: &str, def: &ProfileDef) -> Box<dyn ModelClient> {
    match provider_family(&def.provider) {
        "openai_compatible" => {
            let base_url = def
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let api_key_env = resolve_api_key_env(alias, def);

            let config = OpenAiCompatibleConfig {
                base_url,
                api_key_env,
                default_model: def.model_id.clone(),
                headers: Vec::new(),
                capability_overrides: None,
            };
            Box::new(OpenAiCompatibleClient::new(config))
        }
        "anthropic" => {
            let base_url = def
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            let api_key_env = resolve_api_key_env(alias, def);

            let config = AnthropicConfig {
                base_url,
                api_key_env,
                default_model: def.model_id.clone(),
                max_tokens: def
                    .output_limit
                    .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
                headers: Vec::new(),
                capability_overrides: None,
            };
            Box::new(AnthropicClient::new(config))
        }
        "ollama" => {
            let config = OllamaConfig {
                base_url: def
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                default_model: def.model_id.clone(),
                context_window: def
                    .context_window
                    .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            };
            Box::new(OllamaClient::new(config))
        }
        "fake" => {
            let response = def
                .response
                .clone()
                .unwrap_or_else(|| "hello from Kairox".to_string());
            Box::new(FakeModelClient::new(vec![response]))
        }
        _ => unreachable!("provider_family always returns a known family"),
    }
}
```

- [ ] **Step 4: Update `build_profile()` to use `provider_family()` and capability overrides**

Replace the `build_profile()` function:

```rust
fn build_profile(alias: &str, def: &ProfileDef) -> ModelProfile {
    let family = provider_family(&def.provider);

    let mut capabilities = match family {
        "openai_compatible" => ModelCapabilities {
            streaming: true,
            tool_calling: true,
            json_schema: true,
            vision: false,
            reasoning_controls: false,
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            output_limit: def
                .output_limit
                .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
            local_model: false,
        },
        "anthropic" => ModelCapabilities {
            streaming: true,
            tool_calling: true,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            output_limit: def
                .output_limit
                .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
            local_model: false,
        },
        "ollama" => ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            output_limit: def
                .output_limit
                .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
            local_model: true,
        },
        "fake" => ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: def
                .context_window
                .unwrap_or_else(|| crate::resolve_limits(def).context_window),
            output_limit: def
                .output_limit
                .unwrap_or_else(|| crate::resolve_limits(def).output_limit),
            local_model: true,
        },
        _ => unreachable!("provider_family always returns a known family"),
    };

    // Apply capability overrides from ProfileDef
    if let Some(v) = def.supports_tools {
        capabilities.tool_calling = v;
    }
    if let Some(v) = def.supports_vision {
        capabilities.vision = v;
    }
    if let Some(v) = def.supports_reasoning {
        capabilities.reasoning_controls = v;
    }

    ModelProfile {
        alias: alias.to_string(),
        provider: def.provider.clone(),
        model_id: def.model_id.clone(),
        capabilities,
    }
}
```

- [ ] **Step 5: Write unit test for `provider_family()`**

Add to the test module in `builder.rs`:

```rust
#[test]
fn provider_family_maps_correctly() {
    assert_eq!(provider_family("anthropic"), "anthropic");
    assert_eq!(provider_family("ollama"), "ollama");
    assert_eq!(provider_family("fake"), "fake");
    assert_eq!(provider_family("openai_compatible"), "openai_compatible");
    assert_eq!(provider_family("deepseek"), "openai_compatible");
    assert_eq!(provider_family("groq"), "openai_compatible");
    assert_eq!(provider_family("xai"), "openai_compatible");
    assert_eq!(provider_family("unknown-thing"), "openai_compatible");
}
```

- [ ] **Step 6: Write test for capability overrides**

```rust
#[test]
fn capability_overrides_from_profile_def() {
    let def = ProfileDef {
        provider: "deepseek".into(),
        model_id: "deepseek-chat".into(),
        base_url: Some("https://api.deepseek.com/v1".into()),
        api_key: None,
        api_key_env: Some("DEEPSEEK_API_KEY".into()),
        context_window: Some(128_000),
        output_limit: Some(32_768),
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        supports_tools: Some(false),
        supports_vision: Some(true),
        supports_reasoning: None,
        extra_params: None,
    };
    let profile = build_profile("deepseek", &def);
    // Overridden
    assert!(!profile.capabilities.tool_calling);
    assert!(profile.capabilities.vision);
    // Not overridden — uses provider default (openai_compatible defaults)
    assert!(!profile.capabilities.reasoning_controls);
}
```

- [ ] **Step 7: Run tests**

```bash
cargo test -p agent-config --lib
```

Expected: all tests pass, including new provider_family and capability override tests. The old `rejects_unknown_provider` test is replaced.

- [ ] **Step 8: Update `crates/agent-config/src/lib.rs` to remove `UnknownProvider` from exports if needed**

Check that nothing else references `UnknownProvider`. Search for usages:

```bash
grep -r "UnknownProvider" crates/
```

If only found in the `ConfigError` definition (which we updated), proceed.

- [ ] **Step 9: Commit**

```bash
git add crates/agent-config/src/loader.rs crates/agent-config/src/builder.rs crates/agent-config/src/lib.rs
git commit -m "feat(config): remove provider whitelist, add provider_family() auto-detection and capability overrides"
```

---

### Task 5: Add `temperature`, `top_p`, `top_k`, `extra_params` to client configs

**Files:**

- Modify: `crates/agent-models/src/types.rs` (no changes — `OpenAiCompatibleConfig` and `AnthropicConfig` are in their own files)
- Modify: `crates/agent-models/src/openai_compatible.rs:10-17`
- Modify: `crates/agent-models/src/anthropic.rs:15-25`

- [ ] **Step 1: Add `temperature`, `top_p`, `extra_params` to `OpenAiCompatibleConfig`**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub headers: Vec<(String, String)>,
    pub capability_overrides: Option<crate::ModelCapabilities>,
    // -- new fields --
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub extra_params: Option<serde_json::Value>,
}
```

- [ ] **Step 2: Add `temperature`, `top_p`, `top_k`, `extra_params` to `AnthropicConfig`**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub max_tokens: u64,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub capability_overrides: Option<crate::ModelCapabilities>,
    // -- new fields --
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default)]
    pub extra_params: Option<serde_json::Value>,
}
```

- [ ] **Step 3: Update `AnthropicConfig::default()` to include new fields**

```rust
impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.anthropic.com".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            default_model: "claude-sonnet-4-20250514".into(),
            max_tokens: 16_384,
            headers: Vec::new(),
            capability_overrides: None,
            temperature: None,
            top_p: None,
            top_k: None,
            extra_params: None,
        }
    }
}
```

- [ ] **Step 4: Run tests to verify compilation**

```bash
cargo test -p agent-models --lib
```

Expected: compilation succeeds, existing tests pass (new fields have defaults).

- [ ] **Step 5: Commit**

```bash
git add crates/agent-models/src/openai_compatible.rs crates/agent-models/src/anthropic.rs
git commit -m "feat(models): add temperature, top_p, top_k, extra_params to client configs"
```

---

### Task 6: Wire new params into `OpenAiCompatibleClient` HTTP requests

**Files:**

- Modify: `crates/agent-models/src/openai_compatible.rs:59-134`

- [ ] **Step 1: Inject `temperature`, `top_p`, `extra_params` into `build_chat_request()`**

In `build_chat_request()`, after the tools block (before `Ok(body)`), add:

```rust
// After the tools block (around line 131), before Ok(body):

if let Some(temperature) = self.config.temperature {
    body["temperature"] = serde_json::json!(temperature);
}
if let Some(top_p) = self.config.top_p {
    body["top_p"] = serde_json::json!(top_p);
}
if let Some(ref extra) = self.config.extra_params {
    if let Some(obj) = extra.as_object() {
        for (key, value) in obj {
            body[key] = value.clone();
        }
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p agent-models --lib
```

Expected: all existing tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-models/src/openai_compatible.rs
git commit -m "feat(models): wire temperature, top_p, extra_params into OpenAI-compatible requests"
```

---

### Task 7: Wire new params into `AnthropicClient` HTTP requests

**Files:**

- Modify: `crates/agent-models/src/anthropic.rs:77-214`

- [ ] **Step 1: Inject `temperature`, `top_p`, `top_k` into `build_messages_request()`**

In `build_messages_request()`, after the tools block (around line 211), before the `body` return:

```rust
// After tools block, before the implicit return of body:

if let Some(temperature) = self.config.temperature {
    body["temperature"] = serde_json::json!(temperature);
}
if let Some(top_p) = self.config.top_p {
    body["top_p"] = serde_json::json!(top_p);
}
if let Some(top_k) = self.config.top_k {
    body["top_k"] = serde_json::json!(top_k);
}
if let Some(ref extra) = self.config.extra_params {
    if let Some(obj) = extra.as_object() {
        for (key, value) in obj {
            body[key] = value.clone();
        }
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p agent-models --lib
```

Expected: all existing tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-models/src/anthropic.rs
git commit -m "feat(models): wire temperature, top_p, top_k, extra_params into Anthropic requests"
```

---

### Task 8: Wire new `ProfileDef` fields through `build_client()`

**Files:**

- Modify: `crates/agent-config/src/builder.rs:149-217`

- [ ] **Step 1: Pass `temperature`, `top_p`, `top_k`, `headers`, `extra_params` from `ProfileDef` to client configs**

Update the `openai_compatible` arm in `build_client()`:

```rust
"openai_compatible" => {
    let base_url = def
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let api_key_env = resolve_api_key_env(alias, def);

    let headers: Vec<(String, String)> = def
        .headers
        .as_ref()
        .map(|h| h.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    let extra_params: Option<serde_json::Value> = def
        .extra_params
        .as_ref()
        .map(|v| {
            // Convert toml::Value → serde_json::Value
            let json_str = serde_json::to_string(v)
                .unwrap_or_else(|_| "null".to_string());
            serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null)
        });

    let config = OpenAiCompatibleConfig {
        base_url,
        api_key_env,
        default_model: def.model_id.clone(),
        headers,
        capability_overrides: None,
        temperature: def.temperature,
        top_p: def.top_p,
        extra_params,
    };
    Box::new(OpenAiCompatibleClient::new(config))
}
```

Update the `anthropic` arm:

```rust
"anthropic" => {
    let base_url = def
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    let api_key_env = resolve_api_key_env(alias, def);

    let headers: Vec<(String, String)> = def
        .headers
        .as_ref()
        .map(|h| h.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    let extra_params: Option<serde_json::Value> = def
        .extra_params
        .as_ref()
        .map(|v| {
            let json_str = serde_json::to_string(v)
                .unwrap_or_else(|_| "null".to_string());
            serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null)
        });

    let config = AnthropicConfig {
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
    };
    Box::new(AnthropicClient::new(config))
}
```

- [ ] **Step 2: Run full workspace tests**

```bash
cargo test -p agent-config -p agent-models --lib
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-config/src/builder.rs
git commit -m "feat(config): wire new ProfileDef params through build_client() to model clients"
```

---

### Task 9: Update `Config::defaults()`, `ProfileInfo`, and remove `profile_order_key`

**Files:**

- Modify: `crates/agent-config/src/lib.rs`

- [ ] **Step 1: Add `provider_display` and `model_display` to `ProfileInfo`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
    #[serde(default)]
    pub provider_display: String,
    #[serde(default)]
    pub model_display: String,
}
```

Update `profile_info()` to populate the new fields:

```rust
pub fn profile_info(&self) -> Vec<ProfileInfo> {
    self.profiles
        .iter()
        .map(|(alias, def)| {
            let local = def.provider == "ollama" || def.provider == "fake";
            let has_api_key = def.api_key.is_some()
                || def
                    .api_key_env
                    .as_ref()
                    .is_some_and(|v| std::env::var(v).is_ok());
            ProfileInfo {
                alias: alias.clone(),
                provider: def.provider.clone(),
                model_id: def.model_id.clone(),
                local,
                has_api_key,
                provider_display: def.provider.clone(),
                model_display: def.model_id.clone(),
            }
        })
        .collect()
}
```

- [ ] **Step 2: Update `Config::defaults()` to include new fields in constructed `ProfileDef`s**

The `ProfileDef` struct's new fields all have `#[serde(default)]` so the existing defaults construction still compiles. No change needed — the new fields default to `None`.

- [ ] **Step 3: Remove `profile_order_key` and the stable sort in `merge_config()`**

Remove the `profile_order_key` function (lines 169-175 in lib.rs).

In `merge_config()`, replace the sort block:

```rust
// Before (remove):
// Stable sort: keep "fake" first, then "fast", then others
merged_profiles.sort_by(|a, b| {
    let ap = profile_order_key(&a.0);
    let bp = profile_order_key(&b.0);
    ap.cmp(&bp)
});

// After: preserve insertion order (HashMap insertion order is not stable,
// but we collect into Vec without sorting — profiles appear in merge order:
// defaults first, then user overrides, then project overrides).
// No sort call.
```

The profiles will appear in merge order: defaults first, then user-level overrides appended, then project-level overrides last. This is a reasonable stable order.

- [ ] **Step 4: Run tests**

```bash
cargo test -p agent-config --lib
```

Expected: all tests pass (the test `profile_names_returns_ordered_list` checks length equality, not specific order).

- [ ] **Step 5: Run `just gen-types` to regenerate TypeScript bindings**

```bash
just gen-types
```

- [ ] **Step 6: Commit**

```bash
git add crates/agent-config/src/lib.rs apps/agent-gui/src/generated/
git commit -m "feat(config): add provider_display/model_display to ProfileInfo, remove profile_order_key sort"
```

---

### Task 10: Update `kairox.toml.example` with new fields and auto-detection docs

**Files:**

- Modify: `kairox.toml.example`

- [ ] **Step 1: Rewrite `kairox.toml.example`**

Replace the current content with the updated example:

```toml
# ═══════════════════════════════════════════════════════════════════════════
# Kairox Configuration Example
# ═══════════════════════════════════════════════════════════════════════════
#
# Copy this file to `.kairox/config.toml` (project root) or `~/.kairox/config.toml`
# and fill in your profiles. The `.kairox/config.toml` file is git-ignored.
#
# Discovery order:
#   1. ./.kairox/config.toml  (project-level, takes priority; walks up 5 parents)
#   2. ~/.kairox/config.toml  (user-level fallback)
#   3. Built-in defaults      (fake + local-code, plus "fast" if OPENAI_API_KEY is set)
#
# Provider auto-detection:
#   Any provider name is accepted. Known providers (anthropic, ollama, fake) map
#   to their native clients; everything else (deepseek, groq, xai, openai, ...)
#   uses the OpenAI-compatible client. Use `provider = "deepseek"` directly —
#   no need to pretend it's "openai_compatible".
#
# Profile fields:
#   provider          — (required) any provider name; auto-detected client type
#   model_id          — (required) model identifier sent to the API
#   base_url          — API base URL (optional; defaults depend on provider)
#   api_key           — direct API key string (takes priority over api_key_env)
#   api_key_env       — environment variable name holding the API key
#   context_window    — (optional) max context tokens, resolved via 3-layer fallback
#   output_limit      — (optional) max output tokens, resolved via 3-layer fallback
#   max_tokens        — (optional) max tokens for the response (Anthropic: overrides output_limit)
#   temperature       — (optional) sampling temperature (0.0–2.0)
#   top_p             — (optional) nucleus sampling parameter (0.0–1.0)
#   top_k             — (optional) top-k sampling (Anthropic only)
#   headers           — (optional) custom HTTP headers sent with every request
#   supports_tools    — (optional) override auto-detected tool calling capability
#   supports_vision   — (optional) override auto-detected vision capability
#   supports_reasoning — (optional) override auto-detected reasoning capability
#   extra_params      — (optional) provider-specific parameters passed through to the API
#   response          — static response text (only used by the "fake" provider)
# ═══════════════════════════════════════════════════════════════════════════

# ---------------------------------------------------------------------------
# DeepSeek (auto-detected as OpenAI-compatible)
# ---------------------------------------------------------------------------

[profiles.deepseek]
provider = "deepseek"
model_id = "deepseek-chat"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"

# ---------------------------------------------------------------------------
# OpenAI-compatible providers (OpenAI, Groq, xAI, etc.)
# ---------------------------------------------------------------------------

[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4.1-mini"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"

[profiles.gpt4]
provider = "openai_compatible"
model_id = "gpt-4.1"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
context_window = 1_047_576
output_limit = 32_768

# ---------------------------------------------------------------------------
# Anthropic Claude
# ---------------------------------------------------------------------------
# If api_key and api_key_env are both unset, the Anthropic provider will
# auto-resolve the API key from ~/.claude/settings.json (ANTHROPIC_AUTH_TOKEN).

[profiles.claude]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
# temperature = 0.7
# top_p = 0.9
# top_k = 50

# Anthropic with extended thinking (provider-specific extra_params)
[profiles.claude-thinking]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
temperature = 1.0
max_tokens = 32_768

[profiles.claude-thinking.extra_params]
thinking = { type = "enabled", budget_tokens = 16_000 }

# ---------------------------------------------------------------------------
# Local Ollama
# ---------------------------------------------------------------------------

[profiles.local]
provider = "ollama"
model_id = "devstral"
base_url = "http://localhost:11434"
# temperature = 0.7

[profiles.local-qwen]
provider = "ollama"
model_id = "qwen3:8b"
base_url = "http://localhost:11434"

# ---------------------------------------------------------------------------
# Fake provider (for testing / offline development)
# ---------------------------------------------------------------------------

[profiles.fake]
provider = "fake"
model_id = "fake"
response = "Hello from the Kairox fake provider!"

# ---------------------------------------------------------------------------
# Example with capability overrides
# ---------------------------------------------------------------------------
# Use this when the auto-detected capabilities for a provider are wrong.
# [proffiles.custom-vision]
# provider = "custom-provider"
# model_id = "vision-model-v1"
# base_url = "https://api.example.com/v1"
# supports_tools = false
# supports_vision = true

# ---------------------------------------------------------------------------
# Example with custom headers
# ---------------------------------------------------------------------------
# [profiles.enterprise]
# provider = "openai_compatible"
# model_id = "enterprise-model"
# base_url = "https://internal-gateway.example.com/v1"
# api_key_env = "ENTERPRISE_KEY"
# [profiles.enterprise.headers]
# X-Organization = "my-org"
# X-Project = "kairox"

# ─────────────────────────────────────────────────────────────────────────────
# MCP Server Configuration
# ─────────────────────────────────────────────────────────────────────────────
# (unchanged — see previous version for full MCP docs)
# ─────────────────────────────────────────────────────────────────────────────

# -----------------------------------------------------------------------------
# Session compaction & context budgeting (optional; safe defaults shown).
# -----------------------------------------------------------------------------
[context]
auto_compact_threshold = 0.85
# compactor_profile = "fast"
# max_tool_definition_tokens = 25000
```

- [ ] **Step 2: Commit**

```bash
git add kairox.toml.example
git commit -m "docs(config): update kairox.toml.example with new fields, auto-detection, and provider examples"
```

---

### Task 11: Integration test — deepseek profile loads and builds correctly

**Files:**

- Modify: `crates/agent-config/src/builder.rs` (add test)

- [ ] **Step 1: Write integration test for deepseek profile**

Add to the test module in `builder.rs`:

```rust
#[test]
fn deepseek_profile_builds_as_openai_compatible_client() {
    let toml = r#"
[profiles.deepseek]
provider = "deepseek"
model_id = "deepseek-chat"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"
temperature = 0.6
top_p = 0.95

[profiles.deepseek.extra_params]
frequency_penalty = 0.1
"#;
    let config = crate::loader::load_from_str(toml, "test.toml").unwrap();
    let router = build_router(&config);
    let profiles = router.list_profiles();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].alias, "deepseek");
    assert_eq!(profiles[0].provider, "deepseek");
    // Should have openai_compatible default capabilities
    assert!(profiles[0].capabilities.tool_calling);
}
```

- [ ] **Step 2: Run the integration test**

```bash
cargo test -p agent-config -- deepseek
```

Expected: test passes.

- [ ] **Step 3: Run full workspace test suite**

```bash
cargo test --workspace --all-targets
```

Expected: no regressions.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-config/src/builder.rs
git commit -m "test(config): add integration test for deepseek profile auto-detection"
```

---

### Task 12: Final verification — format, lint, full test suite

- [ ] **Step 1: Run format check**

```bash
pnpm run format:check
```

Fix any formatting issues:

```bash
pnpm run format
```

- [ ] **Step 2: Run lint**

```bash
pnpm run lint:rust && pnpm run lint:web
```

- [ ] **Step 3: Run full test suite**

```bash
just test-all
```

Expected: all tests pass, no regressions.

- [ ] **Step 4: Verify TypeScript types are regenerated**

```bash
just gen-types
git diff --stat apps/agent-gui/src/generated/
```

If there are changes to generated files, commit them.

- [ ] **Step 5: Commit any final changes**

```bash
git add -A
git commit -m "chore(config): format, lint, and regenerate types after model config enhancement"
```
