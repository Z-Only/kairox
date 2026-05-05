# MCP Tool Execution — Design Spec

**Date**: 2026-05-05
**Status**: Draft
**Approach**: Big-bang — implement stdio + SSE transport, permissions, UI, resources, and prompts in one coordinated release.

## Overview

Integrate the Model Context Protocol (MCP) into Kairox so users can configure external MCP servers in `kairox.toml`, discover their tools/resources/prompts, and invoke them with the same permission and event-sourcing pipeline used by built-in tools. This covers a new `agent-mcp` crate, extensions to `agent-tools`, `agent-config`, `agent-runtime`, `agent-core`, `agent-tui`, and `agent-gui`.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  UI Layer                                                    │
│  ┌──────────────────────┐  ┌──────────────────────────────┐ │
│  │  agent-tui           │  │  agent-gui                   │ │
│  │  permission_modal.rs │  │  McpStatusIndicator.vue      │ │
│  │  (MCP tool confirm)  │  │  PermissionPrompt (extended) │ │
│  └──────────┬───────────┘  │  McpServerManager.vue        │ │
│             │              └──────────────┬───────────────┘ │
└─────────────┼─────────────────────────────┼─────────────────┘
              │                             │
┌─────────────┼─────────────────────────────┼─────────────────┐
│  Runtime     │                             │                 │
│  ┌──────────▼─────────────────────────────▼───────────────┐ │
│  │  agent-runtime (facade_runtime.rs)                     │ │
│  │  - McpServerManager (lifecycle)                        │ │
│  │  - MCP tool call routing                               │ │
│  │  - Permission decision delegation                      │ │
│  └──────────┬──────────────────────┬─────────────────────┘ │
└─────────────┼──────────────────────┼───────────────────────┘
              │                      │
┌─────────────┼──────────────────────┼───────────────────────┐
│  Tool Layer  │                      │                       │
│  ┌──────────▼──────────┐  ┌───────▼─────────────────────┐ │
│  │  agent-tools        │  │  agent-mcp (NEW crate)       │ │
│  │  - PermissionEngine │  │  - McpClient                 │ │
│  │  - ToolRegistry     │  │  - Transport (stdio / sse)   │ │
│  │  - trusted_servers  │  │  - ServerResources            │ │
│  │  - Tool trait       │  │  - ServerPrompts              │ │
│  └─────────────────────┘  └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
              │
┌─────────────┼─────────────────────────────────────────────┐
│  Config      │                                             │
│  ┌──────────▼──────────────────────────────────────────┐  │
│  │  agent-config                                       │  │
│  │  - McpServerConfig (stdio / sse parsing)            │  │
│  │  - kairox.toml [mcp_servers] section                │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## New Crate: agent-mcp

### Module Structure

```
crates/agent-mcp/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public exports + McpError
    ├── types.rs            # MCP protocol types (JSON-RPC, tool/resource/prompt defs)
    ├── transport/
    │   ├── mod.rs          # Transport trait
    │   ├── stdio.rs        # StdioTransport — child process stdin/stdout
    │   └── sse.rs          # SseTransport — HTTP SSE remote connection
    ├── client.rs           # McpClient — handshake + tool/resource/prompt calls
    ├── lifecycle.rs        # ServerLifecycle — on-demand start, idle timeout, crash restart
    └── discovery.rs        # Tool/resource/prompt discovery and caching
```

### Dependencies

- `agent-core` (ID types)
- `tokio` (async runtime, process, sync)
- `serde` / `serde_json` (JSON-RPC codec)
- `reqwest` (SSE transport HTTP client)
- `futures` (stream utilities)
- `tracing` (logging)
- `thiserror` (error type)
- Optional feature `specta` for GUI type generation

### Core Types (types.rs)

```rust
/// MCP server definition (parsed from agent-config)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerDef {
    pub id: String,
    pub transport: McpTransportDef,
    pub keep_alive: bool,           // default false
    pub idle_timeout_secs: u64,     // default 300
    pub auto_restart: bool,         // default true
    pub max_restart_attempts: u32,  // default 3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransportDef {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        cwd: Option<String>,
    },
    Sse {
        url: String,
        headers: HashMap<String, String>,
        api_key_env: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpToolDef {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpResourceDef {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpPromptDef {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<McpPromptArgument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpPromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpToolResult {
    pub content: Vec<McpContentBlock>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContentBlock {
    Text { text: String },
    Image { data: String, mime_type: String },
    Resource { resource: McpResourceContent },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpResourceContent {
    pub uri: String,
    pub name: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,
    pub blob: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatus {
    Stopped,
    Starting,
    Running,
    Failed { error: String },
}
```

### Transport Trait (transport/mod.rs)

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse>;
    async fn send_notification(&self, notification: JsonRpcNotification) -> Result<()>;
    async fn close(&self) -> Result<()>;
}
```

**StdioTransport**:

- Launch child process via `tokio::process::Command`
- Write JSON-RPC requests to stdin (line-delimited)
- Read JSON-RPC responses from stdout (line-delimited, `tokio::io::BufReader`)
- Forward stderr to `tracing`
- Support `env` and `cwd` config

**SseTransport**:

- Send requests via HTTP POST
- Receive responses via SSE stream
- Support `headers` and `api_key_env` auth
- Heartbeat detection to keep connection alive

### McpClient (client.rs)

```rust
pub struct McpClient {
    server_id: String,
    transport: Arc<dyn Transport>,
    server_info: OnceCell<ServerInfo>,
    tools: OnceCell<Vec<McpToolDef>>,
    resources: OnceCell<Vec<McpResourceDef>>,
    prompts: OnceCell<Vec<McpPromptDef>>,
}

impl McpClient {
    pub async fn handshake(&self) -> Result<ServerInfo>;
    pub async fn discover_tools(&self) -> Result<&[McpToolDef]>;
    pub async fn discover_resources(&self) -> Result<&[McpResourceDef]>;
    pub async fn discover_prompts(&self) -> Result<&[McpPromptDef]>;
    pub async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<McpToolResult>;
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<McpContentBlock>>;
    pub async fn get_prompt(&self, name: &str, arguments: HashMap<String, String>) -> Result<Vec<McpContentBlock>>;
    pub async fn shutdown(&self) -> Result<()>;
}
```

Handshake follows MCP spec:

1. `initialize` request → server returns `ServerInfo` (name, version, capabilities)
2. Client sends `initialized` notification
3. After handshake: `tools/list`, `resources/list`, `prompts/list` available

### ServerLifecycle (lifecycle.rs)

```rust
pub struct ServerLifecycle {
    def: McpServerDef,
    client: Option<Arc<McpClient>>,
    status: McpServerStatus,
    last_activity: Option<Instant>,
    restart_count: u32,
}

impl ServerLifecycle {
    pub async fn ensure_running(&mut self) -> Result<Arc<McpClient>>;
    pub fn mark_active(&mut self);
    pub async fn check_idle_timeout(&mut self) -> Result<()>;
    pub async fn shutdown(&mut self) -> Result<()>;
    pub fn status(&self) -> &McpServerStatus;
}
```

Key behaviors:

- **On-demand start**: first `ensure_running()` creates transport → `McpClient::handshake()`
- **Idle timeout**: background tokio task periodically calls `check_idle_timeout()`, shuts down on expiry
- **Crash restart**: `call_tool`/`read_resource` failure detects process status; if terminated and `auto_restart=true`, re-runs `ensure_running()`
- **keep_alive**: skip idle timeout checks; pre-start at application launch

## Config: agent-config

### TOML Format

```toml
[mcp_servers.filesystem]
type = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/projects"]
env = { NODE_OPTIONS = "--max-old-space-size=4096" }
keep_alive = true

[mcp_servers.github]
type = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_PERSONAL_ACCESS_TOKEN = "" }  # empty value = read from env var of same name

[mcp_servers.remote-search]
type = "sse"
url = "https://mcp.example.com/search"
headers = { Authorization = "Bearer ${API_TOKEN}" }  # ${VAR} expanded from env
api_key_env = "MCP_SEARCH_API_KEY"
```

### Config struct

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    pub r#type: McpTransportType,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cwd: Option<String>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub keep_alive: bool,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,       // default 300
    #[serde(default = "default_auto_restart")]
    pub auto_restart: bool,           // default true
    #[serde(default = "default_max_restart_attempts")]
    pub max_restart_attempts: u32,    // default 3
}
```

Validation:

- `type = "stdio"` → `command` required
- `type = "sse"` → `url` required
- `env` empty values resolve from env var of same name
- `headers` `${VAR}` patterns expand from environment

Add `mcp_servers: Vec<McpServerConfig>` to `ProfileDef`.

## agent-tools Changes

### Remove old mcp.rs, bridge to agent-mcp

Delete `crates/agent-tools/src/mcp.rs`. Rewrite `crates/agent-tools/src/provider/mcp_provider.rs`:

```rust
pub struct McpToolAdapter {
    server_id: String,
    tool_def: McpToolDef,
    client: Arc<McpClient>,
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: format!("mcp.{}.{}", self.server_id, self.tool_def.name),
            description: self.tool_def.description.clone().unwrap_or_default(),
            required_capability: "mcp.invoke".into(),
        }
    }
    async fn invoke(&self, invocation: ToolInvocation) -> Result<ToolOutput> {
        let result = self.client
            .call_tool(&self.tool_def.name, invocation.arguments)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(tool_result_to_output(result))
    }
}
```

### PermissionEngine MCP Extension

Add `trusted_mcp_servers: HashSet<String>` field to `PermissionEngine`.

```rust
pub fn check_mcp_permission(&self, server_id: &str, tool_id: &str) -> PermissionOutcome {
    if self.trusted_mcp_servers.contains(server_id) {
        match self.mode {
            PermissionMode::ReadOnly => PermissionOutcome::Denied,
            PermissionMode::Autonomous => PermissionOutcome::Allowed,
            _ => PermissionOutcome::Prompt,
        }
    } else {
        PermissionOutcome::PromptWithTrust
    }
}

pub fn trust_server(&mut self, server_id: String);
pub fn revoke_trust(&mut self, server_id: &str);
pub fn trusted_servers(&self) -> &HashSet<String>;
```

### PermissionOutcome Extension

```rust
pub enum PermissionOutcome {
    Allowed,
    Denied,
    Prompt,
    PromptWithTrust,  // NEW: prompt user + offer "trust this server" option
}
```

### ToolEffect Extension

Add `McpInvoke` variant to `ToolEffect` enum.

## agent-core: New EventPayload Variants

```rust
McpServerStarting { server_id: String },
McpServerReady { server_id: String, tool_count: usize },
McpServerStopped { server_id: String },
McpServerFailed { server_id: String, error: String },
McpToolCallStarted { server_id: String, tool_name: String },
McpToolCallCompleted { server_id: String, tool_name: String, duration_ms: u64 },
McpTrustGranted { server_id: String },
McpTrustRevoked { server_id: String },
```

All new variants follow existing `EventPayload` patterns with `#[cfg_attr(feature = "specta", derive(specta::Type))]`.

## agent-runtime: McpServerManager

```rust
// crates/agent-runtime/src/mcp_manager.rs

pub struct McpServerManager {
    servers: HashMap<String, ServerLifecycle>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
}

impl McpServerManager {
    pub fn from_config(configs: Vec<McpServerDef>, tool_registry: Arc<Mutex<ToolRegistry>>, permission_engine: Arc<Mutex<PermissionEngine>>) -> Self;
    pub async fn start_persistent_servers(&mut self) -> Vec<Result<()>>;
    pub async fn ensure_server(&mut self, server_id: &str) -> Result<Arc<McpClient>>;
    pub async fn refresh_tools(&mut self, server_id: &str) -> Result<Vec<McpToolDef>>;
    pub async fn check_idle_timeouts(&mut self) -> Result<()>;
    pub async fn trust_server(&self, server_id: &str) -> Result<()>;
    pub async fn revoke_trust(&self, server_id: &str) -> Result<()>;
    pub async fn shutdown_server(&mut self, server_id: &str) -> Result<()>;
    pub fn server_statuses(&self) -> HashMap<String, McpServerStatus>;
    pub async fn list_resources(&self, server_id: &str) -> Result<Vec<McpResourceDef>>;
    pub async fn list_prompts(&self, server_id: &str) -> Result<Vec<McpPromptDef>>;
    pub async fn read_resource(&self, server_id: &str, uri: &str) -> Result<Vec<McpContentBlock>>;
    pub async fn shutdown_all(&mut self) -> Result<()>;
}
```

LocalRuntime integration:

- `new()` / `with_config()`: create `McpServerManager` from config, start keep_alive servers
- `start_session()`: register MCP tools into session's `ToolRegistry`
- Agent loop tool calls: MCP tools route through `McpToolAdapter` → `ToolRegistry` normally, permission check goes through `check_mcp_permission`
- `cancel_session()` / drop: trigger `shutdown_all()`
- Add `mcp_manager: Option<Arc<Mutex<McpServerManager>>>` field to `LocalRuntime`

## GUI: Tauri IPC Commands

New commands in `commands.rs`:

| Command              | Purpose                        |
| -------------------- | ------------------------------ |
| `list_mcp_servers`   | Get all MCP server statuses    |
| `start_mcp_server`   | Start a specific server        |
| `stop_mcp_server`    | Stop a specific server         |
| `refresh_mcp_tools`  | Refresh tool list for a server |
| `trust_mcp_server`   | Trust a server                 |
| `revoke_mcp_trust`   | Revoke server trust            |
| `list_mcp_resources` | Get resources for a server     |
| `list_mcp_prompts`   | Get prompts for a server       |
| `read_mcp_resource`  | Read a specific resource       |

Response types: `McpServerStatusResponse`, `McpToolDefResponse`, `McpResourceDefResponse`, `McpPromptDefResponse`, `McpContentBlockResponse`.

Register in `specta.rs` and `lib.rs` (`generate_handler![]` + `collect_commands![]`).

## GUI: Frontend Components

### New: `apps/agent-gui/src/stores/mcp.ts`

Pinia store managing MCP server state. Methods: `fetchServers`, `startServer`, `stopServer`, `trustServer`, `revokeTrust`, `refreshTools`. Listens to MCP events via `useTauriEvents`.

### New: `apps/agent-gui/src/components/McpStatusIndicator.vue`

Status bar indicator:

- 🟢 `N MCP` — N servers running
- 🟡 `N MCP` — servers starting
- 🔴 `N MCP` — servers failed
- ⚪ `MCP` — no servers

Click opens `McpServerManager`.

### New: `apps/agent-gui/src/components/McpServerManager.vue`

Side drawer panel:

- Server list with status, tool/resource/prompt counts
- Trust badges for trusted servers, trust/revoke buttons
- Start/Stop/Restart buttons
- Error display for failed servers
- Expand individual servers to see tool details

### Modified: `apps/agent-gui/src/components/PermissionPrompt.vue`

MCP permission dialog additions:

- Show server name and full tool ID (`mcp.<server>.<tool>`)
- "Trust this server" checkbox
- Three buttons: `Deny` / `Allow Once` / `Allow Always` (with trust checked)

### Modified: `apps/agent-gui/src/components/StatusBar.vue`

Add `McpStatusIndicator` entry point.

### Modified: `apps/agent-gui/src/composables/useTauriEvents.ts`

Add MCP event handlers (`McpServerStarting`, `McpServerReady`, `McpServerStopped`, `McpServerFailed`, `McpTrustGranted`, `McpTrustRevoked`) routing to mcp store.

## TUI Adaptation

### Modified: `crates/agent-tui/src/components/permission_modal.rs`

New `PermissionRequest` enum distinguishing built-in vs MCP tools:

```rust
pub enum PermissionRequest {
    BuiltIn { tool_id: String, preview: String },
    Mcp { server_id: String, tool_name: String, full_tool_id: String, preview: String, server_trusted: bool },
}
```

MCP tools show `[MCP] server/tool` label. Add `(T) Trust server` shortcut key.

### MCP status indicator in TUI status bar

Simple `[MCP:2↑ 1↓]` display (2 running, 1 stopped). No full management panel — that belongs in GUI.

## Event Forwarder

Existing `event_forwarder.rs` generically forwards `DomainEvent`. New `EventPayload` variants are automatically forwarded. Run `just gen-types` to update TypeScript bindings.

## Testing Strategy

### agent-mcp Unit Tests (~25)

| Module        | Tests                                                                                                                                                                                                                                   |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| types         | TOML serde roundtrip (stdio/sse config), tool result parsing, content block deserialization, server status serialization                                                                                                                |
| JSON-RPC      | Request serialization with id, response deserialization (result/error), notification no-id                                                                                                                                              |
| client (mock) | handshake sends initialize+initialized, discover_tools calls tools/list, discovery caching, call_tool sends tools/call, read_resource, get_prompt, shutdown, retry on transport error                                                   |
| lifecycle     | ensure_running starts on first call, reuses existing client, mark_active updates timestamp, idle timeout shuts down, keep_alive skips timeout, auto_restart on crash, max attempts exhausted, status transitions, shutdown sets stopped |
| discovery     | tools cache + invalidation, resources empty if not supported, prompts empty if not supported                                                                                                                                            |

MockTransport: records sent requests, returns preset responses from VecDeque.

### agent-mcp Integration Tests (~10)

| Test File             | Tests                                                                                                                                        |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| stdio_integration     | Real echo-mcp-server.mjs fixture: handshake, discover+call echo tool, read resource, get prompt, server crash handling, env variable passing |
| sse_integration       | wiremock mock HTTP server: connect, request/response, connection error, transient retry, api_key from env                                    |
| lifecycle_integration | Full lifecycle: start→discover→call→timeout→restart, keep_alive never times out, crash recovery with auto_restart, max restart attempts      |

Fixture: `crates/agent-mcp/tests/fixtures/echo-mcp-server.mjs` — minimal MCP server with echo tool, test resource, and test prompt using `@modelcontextprotocol/sdk`.

### agent-tools Unit Tests (~6)

- `check_mcp_permission`: untrusted always prompts, trusted follows mode, denied in ReadOnly, trust+revoke roundtrip
- `McpToolAdapter`: definition conversion, mock invoke

### agent-config Unit Tests (~7)

- Parse stdio config, parse sse config, reject stdio without command, reject sse without url, env empty value resolves, header variable expansion, default values, full profile with mcp_servers

### agent-core Unit Tests (~5)

- MCP event payload serde roundtrip for all 8 new variants

### agent-runtime Integration Tests (~6)

- McpServerManager registers tools in registry, MCP tool invocation through runtime, untrusted tool prompts permission, trusted tool auto-allowed in Agent mode, MCP server events emitted, crash and recovery in runtime

### GUI Frontend Tests (~15 unit + ~10 E2E)

**Vitest unit tests**:

- `mcp.test.ts`: fetch servers, start/stop, trust/revoke, event updates, computed properties, refresh tools
- `McpStatusIndicator.test.ts`: rendering for each status state, click event
- `McpServerManager.test.ts`: server list, trust badge, button states, trust action, error display
- `PermissionPrompt.test.ts` (extend): MCP server name display, trust checkbox, allow-once/allow-always/deny interactions, hidden for built-in tools

**Playwright E2E tests** (`mcp.spec.ts`):

- MCP status indicator in status bar
- Click opens server manager
- Server list rendering
- Start/stop server
- Trust server
- MCP permission dialog appearance
- Trust checkbox + allow always
- Allow once / deny
- MCP event-driven UI updates (starting→ready→failed)

**Tauri IPC mock extension**: add handlers for `list_mcp_servers`, `start_mcp_server`, `stop_mcp_server`, `trust_mcp_server`, `revoke_mcp_trust`, `refresh_mcp_tools`, `list_mcp_resources`, `list_mcp_prompts`, `read_mcp_resource`.

### justfile Addition

```just
# Run MCP-related unit and integration tests
test-mcp:
    cargo test -p agent-mcp --all-targets
    cargo test -p agent-tools -- mcp
    cargo test -p agent-config -- mcp
    cargo test -p agent-runtime --test mcp_integration
    @echo "✅ MCP tests passed"
```

### Estimated Test Count: ~84

## Crate Dependency Changes

Root `Cargo.toml` workspace:

- Add `agent-mcp` to workspace members
- Add shared deps: `tokio`, `serde`, `serde_json`, `reqwest`, `futures`, `thiserror` (most already in workspace deps)
- Add new deps: `eventsource-stream` (SSE parsing)

`agent-tools`:

- Remove `mcp.rs`
- Add dependency on `agent-mcp`

`agent-config`:

- Add dependency on `agent-mcp` (for `McpServerDef` / `McpTransportDef`)

`agent-runtime`:

- Add dependency on `agent-mcp`

`agent-gui-tauri`:

- Add dependency on `agent-mcp`
- Enable `agent-mcp/specta` feature

## Migration Notes

- Existing `McpServerConfig` and `McpTool` types in `agent-tools/src/mcp.rs` are replaced by `agent-mcp` types
- `map_mcp_tool()` helper removed; tools are discovered dynamically from servers
- `McpProvider` in `agent-tools/src/provider/mcp_provider.rs` rewrites to use `McpToolAdapter`
- Config file format: `[mcp_servers.XXX]` section is new; no breaking changes to existing `[profiles]`
