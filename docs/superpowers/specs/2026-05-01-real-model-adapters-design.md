# Real Model Adapters Design

Date: 2026-05-01
Status: Approved
Scope: New `agent-config` crate, `agent-models` (ModelRouter impl ModelClient), `agent-tui`, `apps/agent-gui/src-tauri`

## Context

Kairox v0.5.0 has a working GUI with streaming chat, session management, and event forwarding — but it only works with `FakeModelClient`. The `agent-models` crate already has fully implemented `OpenAiCompatibleClient` and `OllamaClient` adapters with tests, plus a `ModelRouter` that maps profile aliases to clients — but `ModelRouter` doesn't implement the `ModelClient` trait, so it can't be used as the `M` type parameter in `LocalRuntime<S, M>`. Both TUI and GUI hardcode `FakeModelClient` and use string-based `detect_profiles()` with no configuration file support.

## Goals

1. Create an `agent-config` crate that discovers, loads, and validates `kairox.toml` configuration files
2. Support profile definitions with `openai_compatible`, `ollama`, and `fake` providers
3. Support both direct API keys (`api_key`) and environment variable references (`api_key_env`) with mixed-mode priority
4. Generate sensible default profiles from environment variables when no config file exists
5. Build a `ModelRouter` from config that implements `ModelClient`, enabling `LocalRuntime<SqliteEventStore, ModelRouter>` everywhere
6. Update TUI and GUI to use `Config::load()` + `Config::build_router()` instead of hardcoded `FakeModelClient`
7. Expose profile metadata to the GUI frontend for richer status display

## Non-Goals

- Configuration hot-reload or filesystem watch
- GUI Settings UI for editing profiles (future v0.7.0)
- Encrypted key storage or keychain integration
- MCP server configuration (future)
- Multi-agent orchestration configuration

## Architecture

### Data Flow

```
Config::load()
  ├── discovery::find_config()
  │     ├── ./kairox.toml → ConfigSource::ProjectFile
  │     ├── ~/.kairox/config.toml → ConfigSource::UserFile
  │     └── none → ConfigSource::Defaults
  ├── loader::resolve_api_keys()   → api_key or api_key_env lookup
  ├── loader::validate()           → required fields, known providers
  └── Config

Config::build_router()
  ├── ModelRouter::new()
  ├── for each (alias, ProfileDef):
  │     ├── "openai_compatible" → OpenAiCompatibleClient + Profile
  │     ├── "ollama"            → OllamaClient + Profile
  │     └── "fake"              → FakeModelClient + Profile
  └── ModelRouter (impl ModelClient)

TUI/GUI startup:
  let config = Config::load()?;
  let router = config.build_router();
  let runtime = LocalRuntime::new(store, router)...
```

### Key Decisions

| Decision          | Choice                                                           | Rationale                                                                                               |
| ----------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Config format     | TOML                                                             | Consistent with Rust ecosystem, simple, human-readable                                                  |
| Config discovery  | CWD first, then home dir                                         | Project-level overrides user-level; familiar pattern                                                    |
| API key handling  | Mixed mode (`api_key` direct OR `api_key_env` env var reference) | Matches existing `OpenAiCompatibleConfig.api_key_env` pattern; flexible for both interactive and CI use |
| Default profiles  | Generated from env vars when no config file                      | Zero-config experience: set OPENAI_API_KEY and it just works                                            |
| Crate location    | New `crates/agent-config`                                        | Clean separation of concerns; doesn't bloat agent-models with I/O                                       |
| ModelClient impl  | `impl ModelClient for ModelRouter`                               | Minimal change; LocalRuntime already generic over M: ModelClient                                        |
| Profile selection | Same priority: fast > local-code > fake                          | Backward compatible with existing behavior                                                              |

## Configuration File Format

```toml
# ~/.kairox/config.toml or ./kairox.toml

[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4.1-mini"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
context_window = 128_000
output_limit = 16_384

[profiles.deep]
provider = "openai_compatible"
model_id = "gpt-4.1"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
context_window = 1_000_000
output_limit = 32_768

[profiles.local-code]
provider = "ollama"
model_id = "devstral"
base_url = "http://localhost:11434"
context_window = 128_000
output_limit = 16_384

[profiles.fake]
provider = "fake"
response = "hello from Kairox"
```

### API Key Resolution

Per profile:

1. `api_key` present → use that literal value (highest priority)
2. `api_key_env` present → read the named environment variable
3. Both present → `api_key` wins
4. Neither → no key (valid for Ollama/fake; error for openai_compatible if key is needed at runtime)

### Default Profiles (no config file)

| Profile      | Condition            | Provider          | Model        |
| ------------ | -------------------- | ----------------- | ------------ |
| `fast`       | `OPENAI_API_KEY` set | openai_compatible | gpt-4.1-mini |
| `local-code` | always               | ollama            | devstral     |
| `fake`       | always               | fake              | (built-in)   |

Default profile selection: `fast` > `local-code` > `fake`

## Crate Structure

```
crates/agent-config/
├── Cargo.toml
└── src/
    ├── lib.rs          # Public API: Config, ConfigError, ProfileDef, ProfileInfo
    ├── discovery.rs    # find_config() — CWD then home dir
    ├── loader.rs       # parse_toml(), resolve_api_keys(), validate()
    └── builder.rs      # build_router() — Config → ModelRouter
```

### Core Types

```rust
// lib.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDef {
    pub provider: String,           // "openai_compatible" | "ollama" | "fake"
    pub model_id: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,    // Direct key, takes priority
    pub api_key_env: Option<String>,// Env var name for key
    #[serde(default = "default_context_window")]
    pub context_window: u64,
    #[serde(default = "default_output_limit")]
    pub output_limit: u64,
    pub response: Option<String>,   // fake provider only
}

#[derive(Debug, Clone)]
pub struct Config {
    pub profiles: Vec<(String, ProfileDef)>,
    pub source: ConfigSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    ProjectFile,
    UserFile,
    Defaults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config parse error in {path}: {message}")]
    Parse { path: String, message: String },
    #[error("profile '{profile}' has unknown provider '{provider}'")]
    UnknownProvider { profile: String, provider: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
```

### Config Public API

```rust
impl Config {
    /// Load config from discovered file, or generate defaults.
    pub fn load() -> Result<Self, ConfigError>;

    /// Generate default config from environment variables.
    pub fn defaults() -> Self;

    /// Build a ModelRouter from this config, registering all profiles.
    pub fn build_router(&self) -> ModelRouter;

    /// Get profile names in order.
    pub fn profile_names(&self) -> Vec<String>;

    /// Get the default profile name (fast > local-code > fake).
    pub fn default_profile(&self) -> &str;

    /// Get profile metadata for UI display.
    pub fn profile_info(&self) -> Vec<ProfileInfo>;
}
```

## ModelRouter Implementation

```rust
// agent-models/src/router.rs — NEW impl block

#[async_trait]
impl ModelClient for ModelRouter {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> crate::Result<BoxStream<'static, crate::Result<ModelEvent>>> {
        // Delegate to existing ModelRouter::stream() method
        self.stream(request).await
    }
}
```

This ~8-line impl makes `ModelRouter` usable as the `M` type in `LocalRuntime<S, M>`.

## TUI Changes

### Before

```rust
let model = FakeModelClient::new(vec!["hello from fake model".into()]);
let runtime = Arc::new(LocalRuntime::new(store, model)...);
let profiles = detect_profiles();  // hardcoded
let profile = choose_profile(&profiles);  // hardcoded
```

### After

```rust
let config = Config::load().unwrap_or_else(|e| {
    eprintln!("Config warning: {e}, using defaults");
    Config::defaults()
});
let router = config.build_router();
let runtime = Arc::new(LocalRuntime::new(store, router)...);
let profiles = config.profile_names();
let profile = config.default_profile();
eprintln!("Available profiles: {:?}", profiles);
eprintln!("Using profile: {profile}");
```

Remove `detect_profiles()` and `choose_profile()` functions from `main.rs`.

Add `agent-config` to `crates/agent-tui/Cargo.toml` dependencies.

## GUI Changes

### app_state.rs

```rust
pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    pub config: Arc<Config>,  // NEW
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub sessions: Mutex<HashMap<String, WorkspaceSession>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
}
```

### commands.rs

- Remove `detect_profiles()` and `choose_default_profile()`
- `list_profiles` reads from `state.config.profile_names()`
- `initialize_workspace` uses `state.config.default_profile()`
- New `get_profile_info` command returns `Vec<ProfileInfo>` for frontend

### event_forwarder.rs

- Type signature: `FakeModelClient` → `ModelRouter`

### lib.rs

- `build_runtime()` uses `Config::load()` and `config.build_router()`
- `GuiState::new()` accepts `Config` parameter

### Frontend

- `StatusBar.vue`: Display `profile (model_id)` format
- `SessionsSidebar.vue`: Profile dropdown uses `list_profiles` (already dynamic)
- No structural component changes needed

## Testing Strategy

### agent-config Unit Tests

- TOML parsing: valid config, missing fields, unknown provider
- API key resolution: `api_key` priority, `api_key_env` lookup, missing env var
- Default profiles: with/without `OPENAI_API_KEY`
- `build_router()`: each provider type produces correct client
- Profile metadata: `profile_names()`, `default_profile()`, `profile_info()`
- Config discovery: order (project > user > defaults)

### ModelRouter ModelClient Tests

- `ModelRouter` with two fake profiles routes to correct client
- Unknown profile returns error via `ModelClient::stream()`

### Integration

- `cargo test --workspace --all-targets` passes
- TUI starts with config from environment variables
- GUI starts and shows correct profiles in sidebar

## Acceptance Criteria

1. `agent-config` crate compiles and all tests pass
2. `ModelRouter` implements `ModelClient` trait
3. TUI starts with `Config::load()` instead of hardcoded `FakeModelClient`
4. GUI starts with `Config::load()` instead of hardcoded `FakeModelClient`
5. Setting `OPENAI_API_KEY` env var makes `fast` profile available in both TUI and GUI
6. A `~/.kairox/config.toml` with custom profiles is loaded and used
7. API key from `api_key` field takes priority over `api_key_env`
8. `list_profiles` Tauri command returns profiles from config, not hardcoded list
9. `cargo test --workspace --all-targets` passes with no regressions
10. `pnpm run format:check && pnpm run lint` passes

## Version Target

This design targets **v0.6.0** of Kairox.
