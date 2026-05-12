# Model Configuration Enhancement — Design Spec

**Date**: 2026-05-12
**Status**: Draft

## Problem Statement

1. Model profiles from project-level `.kairox/config.toml` (e.g. deepseek) may not appear in
   the GUI model switcher because `Config::load()` relies on `std::env::current_dir()` which
   can be unreliable in Tauri dev/production contexts.
2. The provider whitelist in `validate()` rejects provider names beyond `openai_compatible`,
   `anthropic`, `ollama`, `fake`. Providers like `deepseek`, `groq`, `xai` that expose
   OpenAI-compatible APIs must either lie about their provider name or be silently treated as
   `fake` by `build_client()`.
3. `ProfileDef` supports only `provider`, `model_id`, `base_url`, `api_key`, `api_key_env`,
   `context_window`, `output_limit`, `response`. Users cannot configure sampling parameters,
   custom headers, capability overrides, or provider-specific options.

## Design Decisions

- **Any provider name + auto-detect protocol**: Instead of a hardcoded provider whitelist,
  accept any provider name and map it to the correct client based on a known alias table
  (`anthropic` → AnthropicClient, `ollama` → OllamaClient, `fake` → FakeModelClient,
  everything else → OpenAiCompatibleClient).
- **Config directory upward search**: When `current_dir()` doesn't contain `.kairox/`,
  walk up to 5 parent directories to find it, similar to how `git` finds `.git/`.
- **Full parameter set**: Support `max_tokens`, `temperature`, `top_p`, `top_k`, custom
  `headers`, capability overrides (`supports_tools`, `supports_vision`, `supports_reasoning`),
  and an `extra_params` catch-all for provider-specific options.

## Architecture

```
.kairox/config.toml  ──→  loader.rs  ──→  Config { profiles: Vec<(String, ProfileDef)> }
                                                 │
                                                 ▼
                                         builder.rs  ──→  ModelRouter
                                           │
                                           ├─ provider_family() → auto-detect client type
                                           ├─ build_profile()   → ModelProfile + capabilities
                                           └─ build_client()    → Box<dyn ModelClient>
                                                 │
                                                 ▼
                              OpenAiCompatibleClient / AnthropicClient / OllamaClient / FakeModelClient
```

## File Changes

### 1. `crates/agent-config/src/discovery.rs` — Upward search

Add `find_config_upward(start_dir: &Path) -> Option<(PathBuf, ConfigSource)>` that walks
from `start_dir` up to the filesystem root (max 5 levels) looking for `.kairox/config.toml`.

### 2. `crates/agent-config/src/lib.rs` — `ProfileDef` extension + `Config::load()` fix

- Add 9 new fields to `ProfileDef`: `max_tokens`, `temperature`, `top_p`, `top_k`, `headers`,
  `supports_tools`, `supports_vision`, `supports_reasoning`, `extra_params`.
- `Config::load_inner()`: use `find_config_upward` as fallback when the direct
  `project_root/.kairox/config.toml` path doesn't exist.
- `ProfileInfo`: add `provider_display` and `model_display` fields for richer UI display.
- Remove `profile_order_key` hardcoded sort — keep sort stable by insertion order.

### 3. `crates/agent-config/src/loader.rs` — TOML parsing + validation

- `ProfileToml`: mirror new fields from `ProfileDef`.
- `validate()`: remove `known_providers` check entirely. Keep only structural validations
  (`openai_compatible` without `base_url` is still an error for now; in the future
  auto-detection could fill in well-known base URLs).

### 4. `crates/agent-config/src/builder.rs` — Auto-detect + new params

- Add `provider_family(provider: &str) -> &str` mapping.
- `build_profile()`: capability overrides (`supports_tools`, `supports_vision`,
  `supports_reasoning`) from `ProfileDef` take precedence over provider-default
  capabilities; when unset, provider defaults are used.
- `build_client()`: pass `temperature`, `top_p`, `top_k`, `headers`, `extra_params` to
  client config structs. `extra_params` is stored as `toml::Value` in `ProfileDef` and
  converted to `serde_json::Value` during client build for HTTP serialization.

### 5. `crates/agent-models/src/types.rs` — Client config structs

Add fields to `OpenAiCompatibleConfig` and `AnthropicConfig`:

- `temperature: Option<f32>`
- `top_p: Option<f32>`
- `headers: Vec<(String, String)>`
- `extra_params: Option<serde_json::Value>`

`AnthropicConfig` additionally: `top_k: Option<u32>`.

### 6. `crates/agent-models/src/openai_compatible.rs` — Wire new params

Pass `temperature`, `top_p`, custom `headers`, and `extra_params` into the HTTP request body
and headers during `chat/completions` calls.

### 7. `crates/agent-models/src/anthropic.rs` — Wire new params

Pass `temperature`, `top_k`, `top_p`, custom `headers` into Anthropic messages API calls.

### 8. `kairox.toml.example` — Update

- Remove "Supported providers: openai_compatible, anthropic, ollama, fake" restriction.
- Add deepseek example using `provider = "deepseek"`.
- Document all new fields with usage examples.
- Add section on auto-detection behavior.

### 9. `crates/agent-config/src/limits.rs` — No changes

Existing three-layer fallback (UserConfig → BuiltinRegistry → Fallback) works as-is for
new provider names. Unknown providers fall through to `FALLBACK_GENERIC` (128k/16k).

## Data Flow

### Startup

```
App::setup()
  → Config::load()
    → find .kairox/config.toml (cwd → upward search → ~/.kairox → defaults)
    → load_from_str() → merge profiles (project > user > app)
    → resolve_api_keys()
  → config.build_router()
    → for each profile: provider_family() → build_client() → register
  → GuiState::new(runtime, config, mem_store)
```

### Model switch in GUI

```
ChatPanel.vue: selectModelProfile(alias)
  → invoke("switch_model", { sessionId, profileAlias })
  → runtime.switch_model(session_id, profile_alias)
  → next send_message uses new profile
```

### get_profile_info

```
ChatPanel.vue onMounted → session.loadProfileInfo()
  → invoke("get_profile_info")
  → state.config.read().profile_info() → Vec<ProfileInfo>
  → displayed in model popover
```

## Error Handling

- If `.kairox/config.toml` parse fails: log warning, fall back to defaults.
- If `provider_family` returns `openai_compatible` but `base_url` is missing:
  return a clear `ConfigError` at router-build time (not at load time).
- Unknown fields in `extra_params` are silently passed through to the API;
  the provider is responsible for rejecting invalid params.

## Testing

- Unit: `ProfileToml` parses all new fields with defaults.
- Unit: `provider_family()` maps known and unknown providers correctly.
- Unit: `find_config_upward()` finds `.kairox/` in parent directories.
- Integration: `Config::load()` merges project/user/default profiles correctly.
- Integration: `build_router()` creates correct client types for `deepseek`, `groq`, etc.
- GUI Vitest: `get_profile_info` returns all profiles including custom providers.
