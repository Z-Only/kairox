---
title: Configuration
description: Discovery order, profile schema, MCP server schema, context budgeting, and worked examples for `kairox.toml`.
outline: [2, 3]
---

# Configuration

Kairox reads configuration from a single TOML file: `kairox.toml` (or `config.toml` inside a `.kairox/` directory). The format is shared between the TUI and the GUI; the file you write once is read by both. This page is the reference for every field that appears in that file.

The source of truth for examples is [`kairox.toml.example`](https://github.com/Z-Only/kairox/blob/main/kairox.toml.example) at the repo root. This page documents what each field means, when it applies, and what happens if you leave it out.

## Discovery order

When the runtime boots it looks for a config file in this order and uses the first one it finds:

1. **Project config.** `./.kairox/config.toml`, walking up from the current working directory through up to 5 parent directories. This is the workspace-level file; commit it to share team conventions or leave it gitignored for personal overrides.
2. **User config.** `~/.kairox/config.toml`. This is the per-user fallback. It is the right home for personal API keys and personal profile preferences.
3. **Built-in defaults.** If neither file exists, Kairox ships with sensible defaults: the `fake` provider for offline testing, a `local-code` profile pointing at Ollama, and — when `OPENAI_API_KEY` is in the environment — a `fast` profile pointing at OpenAI.

Project config does **not** merge with user config. The first file found wins. If you want both, copy what you need from the user file into the project file.

::: tip Project root vs. workspace root
"Project config" walks up from the process's current working directory looking for `.kairox/config.toml`. In the TUI that is wherever you launched `kairox`. In the GUI that is the workspace root chosen at session creation. The five-parent walk means you can `cd` into a subdirectory and still pick up the workspace config.
:::

## Profiles

A profile is a named configuration for one model. The session picks a profile by name; the profile decides which provider client to use, what model ID to pass, and which API key environment variable to consult.

### Profile schema

| Field                | Type   | Required | Default          | Notes                                                                                                       |
| -------------------- | ------ | -------- | ---------------- | ----------------------------------------------------------------------------------------------------------- |
| `provider`           | string | yes      | —                | Any provider name. Known: `anthropic`, `ollama`, `fake`. Everything else uses the OpenAI-compatible client. |
| `model_id`           | string | yes      | —                | The model identifier sent to the API (e.g. `gpt-4.1`, `claude-sonnet-4-20250514`).                          |
| `base_url`           | string | no       | provider default | API base URL. Omit for `anthropic` to use Anthropic's official endpoint.                                    |
| `api_key`            | string | no       | —                | Literal API key. Takes priority over `api_key_env`. Avoid in committed files.                               |
| `api_key_env`        | string | no       | —                | Environment variable name holding the API key. Resolved at runtime.                                         |
| `context_window`     | int    | no       | model metadata   | Max input + history tokens. Falls back through 3 layers: profile → `ModelRegistry` → provider default.      |
| `output_limit`       | int    | no       | model metadata   | Max output tokens. Same 3-layer fallback as `context_window`.                                               |
| `max_tokens`         | int    | no       | `output_limit`   | Per-response cap. Anthropic uses this to set their `max_tokens` parameter explicitly.                       |
| `temperature`        | float  | no       | provider default | Sampling temperature, 0.0–2.0.                                                                              |
| `top_p`              | float  | no       | provider default | Nucleus sampling, 0.0–1.0.                                                                                  |
| `top_k`              | int    | no       | provider default | Top-k sampling. Anthropic only.                                                                             |
| `headers`            | table  | no       | —                | Extra HTTP headers sent with every request. Useful for enterprise gateways.                                 |
| `supports_tools`     | bool   | no       | auto-detected    | Override auto-detected tool-calling capability.                                                             |
| `supports_vision`    | bool   | no       | auto-detected    | Override auto-detected vision capability.                                                                   |
| `supports_reasoning` | bool   | no       | auto-detected    | Override auto-detected reasoning capability.                                                                |
| `extra_params`       | table  | no       | —                | Provider-specific parameters passed through verbatim (e.g. Anthropic `thinking`).                           |
| `response`           | string | no       | —                | Static response. Only used by the `fake` provider.                                                          |

### Provider auto-detection

The runtime maps `provider` to a client type:

| Provider value      | Client                                                   |
| ------------------- | -------------------------------------------------------- |
| `anthropic`         | Anthropic SDK with `messages` endpoint                   |
| `ollama`            | Ollama HTTP client (`http://localhost:11434` by default) |
| `fake`              | Fixture client that returns the configured `response`    |
| `openai_compatible` | OpenAI Chat Completions client (explicit name)           |
| anything else       | OpenAI-compatible client (Groq, xAI, DeepSeek, etc.)     |

You do not need to pretend a new provider is `openai_compatible` — `provider = "deepseek"` works directly. The runtime treats any unknown provider as OpenAI-compatible.

### Worked examples

DeepSeek (auto-detected as OpenAI-compatible):

```toml
[profiles.deepseek]
provider = "deepseek"
model_id = "deepseek-chat"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"
```

OpenAI with explicit context and output limits:

```toml
[profiles.gpt4]
provider = "openai_compatible"
model_id = "gpt-4.1"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
context_window = 1_047_576
output_limit = 32_768
```

Anthropic Claude with extended thinking:

```toml
[profiles.claude-thinking]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
temperature = 1.0
max_tokens = 32_768

[profiles.claude-thinking.extra_params]
thinking = { type = "enabled", budget_tokens = 16_000 }
```

`extra_params` is passed through to the provider verbatim. For Anthropic, this is how extended thinking, beta features, and any future parameters reach the API without a Kairox release.

Local Ollama:

```toml
[profiles.local]
provider = "ollama"
model_id = "devstral"
base_url = "http://localhost:11434"
```

Fake provider (for offline testing or scripting against deterministic output):

```toml
[profiles.fake]
provider = "fake"
model_id = "fake"
response = "Hello from the Kairox fake provider!"
```

Custom headers (enterprise gateway):

```toml
[profiles.enterprise]
provider = "openai_compatible"
model_id = "enterprise-model"
base_url = "https://internal-gateway.example.com/v1"
api_key_env = "ENTERPRISE_KEY"

[profiles.enterprise.headers]
X-Organization = "my-org"
X-Project = "kairox"
```

Capability overrides (for providers with quirky auto-detection):

```toml
[profiles.custom-vision]
provider = "custom-provider"
model_id = "vision-model-v1"
base_url = "https://api.example.com/v1"
supports_tools = false
supports_vision = true
```

### Anthropic key resolution

If both `api_key` and `api_key_env` are unset and `provider = "anthropic"`, the Anthropic client falls back to `~/.claude/settings.json` and reads `ANTHROPIC_AUTH_TOKEN`. This is convenient for users who already authenticated Claude Code on the same machine — no extra config needed.

### Context window and output limit resolution

`context_window` and `output_limit` use a 3-layer fallback:

1. **Profile value** (if you set it).
2. **`ModelRegistry`** lookup keyed by `(provider, model_id)`. Known models ship with curated values.
3. **Provider default** when the registry has no entry.

This is why you can omit both fields for common models and still get correct budgeting. Set them explicitly when you are running an unusual model, a forked endpoint, or want to be conservative.

## MCP servers

`[mcp_servers.<id>]` declares a Model Context Protocol server. Each server gets a unique id — the section name — which is how the runtime, the marketplace, and the GUI refer to it.

### Common fields

| Field                  | Type   | Default | Notes                                                                     |
| ---------------------- | ------ | ------- | ------------------------------------------------------------------------- |
| `type`                 | string | —       | `"stdio"` or `"sse"`. Required.                                           |
| `keep_alive`           | bool   | `false` | If true, the server stays running even when no session uses it.           |
| `idle_timeout_secs`    | int    | `300`   | Seconds before an idle server is stopped. Ignored when `keep_alive`.      |
| `auto_restart`         | bool   | `true`  | Restart on transport failure.                                             |
| `max_restart_attempts` | int    | `3`     | Restart attempts before the manager gives up and emits `McpServerFailed`. |

### Stdio-specific fields

| Field     | Type   | Notes                                                                                     |
| --------- | ------ | ----------------------------------------------------------------------------------------- |
| `command` | string | The command to execute. Required.                                                         |
| `args`    | array  | Command-line arguments.                                                                   |
| `env`     | table  | Environment variables. An empty value `""` is resolved from the env var of the same name. |
| `cwd`     | string | Working directory for the subprocess. Defaults to the runtime's cwd.                      |

Example — local filesystem MCP server with keep-alive:

```toml
[mcp_servers.filesystem]
type = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/projects"]
keep_alive = true
```

Example — GitHub server that needs a personal access token:

```toml
[mcp_servers.github]
type = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_PERSONAL_ACCESS_TOKEN = "" }  # empty value = read env var of the same name
```

The empty-string convention means "read the same-named environment variable when the server starts." Setting `GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_xxx"` literally would commit the token; the empty value defers to the environment.

### SSE-specific fields

| Field         | Type   | Notes                                                                                      |
| ------------- | ------ | ------------------------------------------------------------------------------------------ |
| `url`         | string | The SSE endpoint. Required.                                                                |
| `headers`     | table  | HTTP headers. `${VAR}` substrings are expanded from environment variables at request time. |
| `api_key_env` | string | If set, the value is read from this env var and added as `Authorization: Bearer <value>`.  |

Example — remote search server with bearer auth:

```toml
[mcp_servers.remote-search]
type = "sse"
url = "https://mcp.example.com/search"
headers = { Authorization = "Bearer ${MCP_API_TOKEN}" }
```

Or, equivalently:

```toml
[mcp_servers.remote-search]
type = "sse"
url = "https://mcp.example.com/search"
api_key_env = "MCP_API_TOKEN"
```

The two forms are functionally equivalent for bearer auth. Use `api_key_env` when you want the runtime to construct the header for you; use `headers` when you need a non-bearer scheme or a different header name.

Example — persistent local server (long-running development server):

```toml
[mcp_servers.persistent-remote]
type = "sse"
url = "http://localhost:8080/mcp"
keep_alive = true
```

### Lifecycle and observability

Every server lifecycle change emits an event:

| Event               | When                                                           |
| ------------------- | -------------------------------------------------------------- |
| `McpServerStarting` | The manager spawns the transport and begins handshake.         |
| `McpServerReady`    | Handshake succeeds and tools are registered.                   |
| `McpServerStopped`  | Server is stopped (user action, idle timeout, shutdown).       |
| `McpServerFailed`   | Failure with a diagnostic payload. Restart counter increments. |

See [Extensibility](../concepts/extensibility) for the full server architecture and [Runtime & Sessions](../concepts/runtime-and-sessions) for how MCP tools surface in the agent loop.

## `[context]` — compaction and token budgeting

Optional. Controls when the runtime triggers automatic compaction and how it sizes tool definitions.

```toml
[context]
auto_compact_threshold = 0.85
# compactor_profile = "fast"
# max_tool_definition_tokens = 25000
```

| Field                        | Type   | Default          | Notes                                                                                                                                           |
| ---------------------------- | ------ | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `auto_compact_threshold`     | float  | `0.85`           | When the assembled context reaches this fraction of the active model's budget, the runtime triggers compaction. `1.0` disables auto-compaction. |
| `compactor_profile`          | string | (active profile) | Profile alias for the summarisation LLM call. Useful to pin a cheap fast model even when the session runs on a heavy reasoning model.           |
| `max_tool_definition_tokens` | int    | unset            | Cap on serialised MCP tool definitions. When exceeded, the assembler drops the lowest-priority tools first.                                     |

See [Memory & Context](../concepts/memory-and-context) for the compaction pipeline and the busy-state guard that prevents compaction from racing the active turn.

## Privacy defaults

There is no `[privacy]` section in `kairox.toml` today; privacy defaults are enforced in code rather than via config. The rules:

- Sessions configured with the `fake` provider and no real shell tools may enable verbose tracing for development.
- Sessions configured with a real model client or a real shell tool default to **minimal trace** in production builds. This is asserted by the runtime at boot.

If you are running a production deployment and want to relax this, you must do so through the runtime configuration in `agent-runtime` — not through the TOML file. The intent is that "I forgot to flip a config flag" cannot leak prompts or tool output into shared logs.

## Environment variable resolution

Three places consult environment variables:

1. **Profile `api_key_env`.** Read once at provider client construction.
2. **MCP stdio `env` table with empty values.** `KEY = ""` means "read the env var named `KEY` and use its value." Non-empty values are passed through literally.
3. **MCP SSE `headers` with `${VAR}`.** `${VAR}` substrings inside header values are expanded at request time, so rotating the env var rotates the header without restarting the server.

If a required env var is missing, the runtime emits a startup diagnostic and the affected profile or server is marked unavailable. The other profiles and servers keep working.

## What this page does not cover

This page is the TOML schema reference. It does not cover the runtime's behavior ([Runtime & Sessions](../concepts/runtime-and-sessions)) or the conceptual story behind MCP and skills ([Extensibility](../concepts/extensibility)).
