---
title: Configuration
description: Discovery order, profile schema, MCP server schema, context budgeting, and worked examples for `kairox.toml`.
outline: [2, 3]
---

# Configuration

Kairox reads configuration from layered TOML files: user-level config, workspace config, and optional local overrides. The format is shared between the TUI and the GUI; the same fields are read by both. This page is the reference for every supported field.

The source of truth for examples is [`kairox.toml.example`](https://github.com/Z-Only/kairox/blob/main/kairox.toml.example) at the repo root. This page documents what each field means, when it applies, and what happens if you leave it out.

## Discovery order

When the runtime boots it starts with built-in defaults, then overlays higher-priority files in this order:

1. **Built-in defaults.** Kairox ships the `fake` provider for offline testing, a disabled `local-code` profile pointing at Ollama, and — when `OPENAI_API_KEY` is in the environment — a `fast` profile pointing at OpenAI.
2. **User config.** `~/.kairox/config.toml`. This is the per-user layer. It is the right home for personal API keys and personal profile preferences.
3. **Project config.** `./.kairox/config.toml`, walking up from the current working directory through up to 5 parent directories. This is the workspace-level file; commit it to share team conventions.
4. **Local project override.** `./.kairox/config.local.toml`, discovered from the project root and intended to stay gitignored for machine-local overrides.

Higher layers override or extend lower layers. Profiles keep their original order when an alias is replaced; new aliases append. MCP servers, knowledge bases, hooks, LSP servers, and DAP servers replace by id. Disabled MCP server ids are additive. Instructions concatenate with a blank line. `[context]`, `[features]`, and `[advisor]` use the highest layer that sets them, with advisor inheriting from the lower layer when the overlay is still the default.

::: tip Project root vs. workspace root
"Project config" walks up from the process's current working directory looking for `.kairox/config.toml`. In the TUI that is wherever you launched `kairox`. In the GUI that is the workspace root chosen at session creation. The five-parent walk means you can `cd` into a subdirectory and still pick up the workspace config. `config.local.toml` is for local overrides on top of that project config.
:::

## Profiles

A profile is a named configuration for one model. The session picks a profile by name; the profile decides which provider client to use, what model ID to pass, and which API key environment variable to consult.

### Profile schema

| Field                        | Type   | Required | Default          | Notes                                                                                                         |
| ---------------------------- | ------ | -------- | ---------------- | ------------------------------------------------------------------------------------------------------------- |
| `provider`                   | string | yes      | —                | Any provider name. Known: `anthropic`, `ollama`, `fake`. Everything else uses the OpenAI-compatible client.   |
| `model_id`                   | string | yes      | —                | The model identifier sent to the API (e.g. `gpt-4.1`, `claude-sonnet-4-20250514`).                            |
| `enabled`                    | bool   | no       | `true`           | Disabled profiles are parsed but not registered in the router or shown as selectable profiles.                |
| `base_url`                   | string | no       | provider default | API base URL. Omit for `anthropic` to use Anthropic's official endpoint.                                      |
| `connect_timeout_secs`       | int    | no       | client default   | HTTP connect timeout for clients that support it.                                                             |
| `request_timeout_secs`       | int    | no       | client default   | Total HTTP request timeout. Streaming clients usually leave this unset.                                       |
| `api_key`                    | string | no       | —                | Literal API key. Takes priority over `api_key_env`. Avoid in committed files.                                 |
| `api_key_env`                | string | no       | —                | Environment variable name holding the API key. Resolved at runtime.                                           |
| `context_window`             | int    | no       | model metadata   | Max input + history tokens. Falls back through 3 layers: profile → `ModelRegistry` → provider default.        |
| `output_limit`               | int    | no       | model metadata   | Max output tokens. Same 3-layer fallback as `context_window`.                                                 |
| `max_tokens`                 | int    | no       | `output_limit`   | Per-response cap. Anthropic uses this to set their `max_tokens` parameter explicitly.                         |
| `temperature`                | float  | no       | provider default | Sampling temperature, 0.0–2.0.                                                                                |
| `top_p`                      | float  | no       | provider default | Nucleus sampling, 0.0–1.0.                                                                                    |
| `top_k`                      | int    | no       | provider default | Top-k sampling. Anthropic only.                                                                               |
| `headers`                    | table  | no       | —                | Extra HTTP headers sent with every request. Useful for enterprise gateways.                                   |
| `client_identity`            | string | no       | —                | `claude_code` adds Claude Code client headers for Anthropic-compatible gateways that gate behavior by client. |
| `supports_tools`             | bool   | no       | auto-detected    | Override auto-detected tool-calling capability.                                                               |
| `supports_vision`            | bool   | no       | auto-detected    | Override auto-detected vision capability.                                                                     |
| `supports_reasoning`         | bool   | no       | auto-detected    | Override auto-detected reasoning capability.                                                                  |
| `server_tool_code_execution` | bool   | no       | `false`          | Enables Anthropic server-side code execution (`code_execution_20250825`) and the required beta header.        |
| `server_tool_web_search`     | bool   | no       | `false`          | Enables Anthropic server-side web search (`web_search_20250305`).                                             |
| `extra_params`               | table  | no       | —                | Provider-specific parameters passed through verbatim (e.g. Anthropic `thinking`).                             |
| `response`                   | string | no       | —                | Static response. Only used by the `fake` provider.                                                            |

### Provider auto-detection

The runtime maps `provider` to a client type:

| Provider / base URL match                                                     | Client                                                   |
| ----------------------------------------------------------------------------- | -------------------------------------------------------- |
| `provider = "anthropic"`                                                      | Anthropic SDK with `messages` endpoint                   |
| `provider = "ollama"`                                                         | Ollama HTTP client (`http://localhost:11434` by default) |
| `provider = "fake"`                                                           | Fixture client that returns the configured `response`    |
| `provider = "openai_compatible"`                                              | OpenAI Chat Completions client (explicit name)           |
| custom provider containing `anthropic`, or `base_url` containing `/anthropic` | Anthropic-compatible client                              |
| anything else                                                                 | OpenAI-compatible client (Groq, xAI, DeepSeek, etc.)     |

You do not need to pretend a new provider is `openai_compatible` — `provider = "deepseek"` works directly. The runtime treats unknown non-Anthropic providers as OpenAI-compatible.

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

Anthropic server-side tools and Claude Code client identity:

```toml
[profiles.claude-tools]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
server_tool_code_execution = true
server_tool_web_search = true
client_identity = "claude_code"
```

`server_tool_code_execution` adds Anthropic's `code-execution-2025-08-25` beta header and sends the `code_execution_20250825` tool type. `server_tool_web_search` sends the `web_search_20250305` tool type. These are provider-hosted tools, not local Kairox tools, so local Approval × Sandbox policy does not apply to their internal execution.

Anthropic-compatible gateway with explicit timeouts:

```toml
[profiles.enterprise-claude]
provider = "enterprise-anthropic"
model_id = "claude-sonnet-4-20250514"
base_url = "https://gateway.example.com/anthropic"
api_key_env = "ENTERPRISE_ANTHROPIC_KEY"
connect_timeout_secs = 10
request_timeout_secs = 120
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
| `type`                 | string | —       | `"stdio"`, `"sse"`, or `"streamable_http"`. Required.                     |
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

### Streamable HTTP-specific fields

`streamable_http` uses the MCP Streamable HTTP transport and accepts the same HTTP fields as `sse`: `url`, `headers`, and `api_key_env`.

Example — remote MCP endpoint using Streamable HTTP:

```toml
[mcp_servers.remote-http]
type = "streamable_http"
url = "https://mcp.example.com/mcp"
api_key_env = "MCP_API_TOKEN"
```

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

## Knowledge bases

`[knowledge_bases.<id>]` declares a retriever that can add external documents to turn context. Sources can be global or scoped to selected model profile aliases.

```toml
[knowledge_bases.company-docs]
kind = "sqlite_fts"
path = ".kairox/kb/company.sqlite"
table = "kb_docs"
id_column = "doc_id"
title_column = "title"
content_column = "body"
workspace_id_column = "workspace_id"
profile_aliases = ["fast", "claude"]
max_results = 4
min_score = 0.25
```

| Field                 | Type     | Default      | Notes                                                                                        |
| --------------------- | -------- | ------------ | -------------------------------------------------------------------------------------------- |
| `kind`                | string   | `sqlite_fts` | Supported values: `sqlite_fts`/`sqlite`, `tantivy`, `bedrock`, `pinecone`, and `weaviate`.   |
| `enabled`             | bool     | `true`       | Disable without deleting the source.                                                         |
| `profile_aliases`     | string[] | `[]`         | Empty means available to every profile; otherwise only these profile aliases see the source. |
| `path`                | string   | —            | Local database or index path, used by local connectors such as SQLite FTS.                   |
| `endpoint`            | string   | —            | Remote service endpoint for cloud/vector connectors.                                         |
| `api_key_env`         | string   | —            | Credential env var for connector kinds that have runtime adapters.                           |
| `region`              | string   | —            | Cloud region, used by Bedrock Knowledge Bases.                                               |
| `knowledge_base_id`   | string   | —            | Bedrock Knowledge Base identifier.                                                           |
| `index_name`          | string   | —            | Vector index name for connectors such as Pinecone.                                           |
| `namespace`           | string   | —            | Optional vector namespace.                                                                   |
| `collection`          | string   | —            | Collection name for connectors such as Weaviate.                                             |
| `table`               | string   | connector    | SQLite FTS table name.                                                                       |
| `id_column`           | string   | connector    | Document id column.                                                                          |
| `title_column`        | string   | connector    | Document title column.                                                                       |
| `content_column`      | string   | connector    | Document body/content column.                                                                |
| `workspace_id_column` | string   | connector    | Optional workspace filter column.                                                            |
| `max_results`         | int      | connector    | Per-source result cap.                                                                       |
| `min_score`           | float    | connector    | Drop hits below this connector-specific score threshold.                                     |

SQLite FTS connectors are wired into runtime context assembly today. The other `kind` values are represented in the config model so service-specific retrievers can plug into the same `WorkspaceRetriever` boundary.

## LSP and DAP servers

`[lsp_servers.<id>]` and `[dap_servers.<id>]` configure native code intelligence and debugging servers. These are not MCP servers; they are managed by `agent-lsp` and surfaced as dynamic tools through `agent-tools`.

```toml
[lsp_servers.rust-analyzer]
command = "rust-analyzer"
args = ["--stdio"]
languages = ["rust"]
file_patterns = ["*.rs"]
initialization_options = { check = { command = "clippy" } }
auto_start = false

[lsp_servers.rust-analyzer.env]
RA_LOG = "info"

[dap_servers.lldb]
command = "codelldb"
args = ["--port", "0"]
languages = ["rust"]

[dap_servers.lldb.env]
RUST_LOG = "debug"
```

| Field                    | LSP | DAP | Default     | Notes                                                                 |
| ------------------------ | --- | --- | ----------- | --------------------------------------------------------------------- |
| `command`                | yes | yes | —           | Server executable. Required.                                          |
| `args`                   | yes | yes | `[]`        | Command-line arguments.                                               |
| `env`                    | yes | yes | `{}`        | Environment variables passed to the server process.                   |
| `cwd`                    | yes | yes | runtime cwd | Working directory for the server process.                             |
| `languages`              | yes | yes | `[]`        | Language ids associated with the server.                              |
| `file_patterns`          | yes | no  | `[]`        | File globs used by LSP server selection.                              |
| `initialization_options` | yes | no  | unset       | JSON value sent in the LSP initialize request.                        |
| `auto_start`             | yes | no  | `true`      | Whether the LSP lifecycle should start automatically when applicable. |

## Instructions, hooks, and feature flags

`instructions` is an optional top-level string that is appended after the system prompt. Across config layers, instructions concatenate with a blank line.

```toml
instructions = """
Follow the repository's Rust and Vue style.
Prefer focused patches and tests that cover the changed behavior.
"""
```

`disabled_mcp_servers` is an additive top-level list. It lets a project layer disable user-level MCP servers by id:

```toml
disabled_mcp_servers = ["personal-browser", "experimental-shell"]
```

`[features]` currently exposes the hooks toggle:

```toml
[features]
hooks = true
```

Command hooks live under `[hooks.<Event>.<id>]`. Supported events are `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, and `Stop`.

```toml
[hooks.Stop.verify]
matcher = "*"
command = "cargo test --workspace --all-targets"
status_message = "Running workspace tests"
timeout_secs = 120
enabled = true

[hooks.PreToolUse.block_rm]
matcher = "shell"
command = "python3 .kairox/hooks/pre_tool.py"
enabled = false
```

| Field            | Type   | Default | Notes                                                  |
| ---------------- | ------ | ------- | ------------------------------------------------------ |
| `matcher`        | string | unset   | Event-specific selector, such as a tool family or `*`. |
| `command`        | string | —       | Shell command to execute. Required.                    |
| `status_message` | string | unset   | Optional message shown while the hook runs.            |
| `timeout_secs`   | int    | unset   | Optional timeout.                                      |
| `enabled`        | bool   | `true`  | Disable a hook without deleting it.                    |

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

## `[advisor]` — tool-call self-reflection

Optional. Controls whether the runtime asks a secondary advisor pass to review planned tool calls before execution.

```toml
[advisor]
mode = "lightweight"
# profile = "fast"
# max_concerns = 5
```

| Field          | Type   | Default | Notes                                                                                                                    |
| -------------- | ------ | ------- | ------------------------------------------------------------------------------------------------------------------------ |
| `mode`         | string | `off`   | `"off"` disables advisor review. `"lightweight"` reviews high-risk tool batches. `"full"` reviews every tool-call batch. |
| `profile`      | string | unset   | Model profile alias for advisor reviews. When unset, the session's active profile is reused.                             |
| `max_concerns` | int    | `5`     | Maximum number of concerns the advisor should report for one review.                                                     |

Advisor review is fail-open: if the advisor model call fails or returns malformed JSON, the main agent continues and the runtime logs a warning. If the advisor returns `reject`, the runtime records `AdvisorReviewCompleted`, emits an assistant message explaining the block, and skips that tool batch.

Use a fast, inexpensive profile for `profile` when you enable `full`; the advisor runs on the critical path before tools execute.

## Privacy defaults

There is no `[privacy]` section in `kairox.toml` today; privacy defaults are enforced in code rather than via config. The rules:

- Sessions configured with the `fake` provider and no real shell tools may enable verbose tracing for development.
- Sessions configured with a real model client or a real shell tool default to **minimal trace** in production builds. This is asserted by the runtime at boot.

If you are running a production deployment and want to relax this, you must do so through the runtime configuration in `agent-runtime` — not through the TOML file. The intent is that "I forgot to flip a config flag" cannot leak prompts or tool output into shared logs.

## Environment variable resolution

Three runtime paths currently consult environment variables:

1. **Profile `api_key_env`.** Read once at provider client construction.
2. **MCP stdio `env` table with empty values.** `KEY = ""` means "read the env var named `KEY` and use its value." Non-empty values are passed through literally.
3. **MCP SSE / Streamable HTTP `headers` with `${VAR}`.** `${VAR}` substrings inside header values are expanded at request time, so rotating the env var rotates the header without restarting the server.

Knowledge base `api_key_env` is parsed as connector metadata, but this build only wires the SQLite FTS knowledge base adapter, which does not require a credential. Service-specific KB adapters should consume that env var when they are connected.

If a required env var is missing, the runtime emits a startup diagnostic and the affected profile or server is marked unavailable. The other configured sources keep working.

## What this page does not cover

This page is the TOML schema reference. It does not cover the runtime's behavior ([Runtime & Sessions](../concepts/runtime-and-sessions)) or the conceptual story behind MCP and skills ([Extensibility](../concepts/extensibility)).
