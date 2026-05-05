# MCP Tool Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate MCP (Model Context Protocol) into Kairox with stdio + SSE transport, permission trust model, and full GUI/TUI support.

**Architecture:** New `agent-mcp` crate implements the MCP protocol client (JSON-RPC over stdio subprocess or SSE HTTP), lifecycle management, and discovery. `agent-tools` bridges MCP tools via `McpToolAdapter` and extends `PermissionEngine` with server trust. `agent-config` parses `[mcp_servers]` TOML sections. `agent-runtime` orchestrates server lifecycle via `McpServerManager`. GUI adds status indicator, server manager panel, and MCP-aware permission prompt.

**Tech Stack:** Rust (tokio async, serde JSON-RPC, reqwest SSE, MCP SDK fixture for tests), Vue 3 + Pinia (GUI frontend), Playwright (E2E)

---

## File Structure

### New Files

| File                                                       | Responsibility                                                                                                                               |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/agent-mcp/Cargo.toml`                              | Crate manifest                                                                                                                               |
| `crates/agent-mcp/src/lib.rs`                              | Public exports + McpError                                                                                                                    |
| `crates/agent-mcp/src/types.rs`                            | JSON-RPC types, MCP protocol types (McpServerDef, McpToolDef, McpResourceDef, McpPromptDef, McpToolResult, McpContentBlock, McpServerStatus) |
| `crates/agent-mcp/src/transport/mod.rs`                    | Transport trait definition                                                                                                                   |
| `crates/agent-mcp/src/transport/stdio.rs`                  | StdioTransport — child process stdin/stdout                                                                                                  |
| `crates/agent-mcp/src/transport/sse.rs`                    | SseTransport — HTTP SSE remote connection                                                                                                    |
| `crates/agent-mcp/src/client.rs`                           | McpClient — handshake, discover, invoke                                                                                                      |
| `crates/agent-mcp/src/lifecycle.rs`                        | ServerLifecycle — on-demand start, idle timeout, crash restart                                                                               |
| `crates/agent-mcp/src/discovery.rs`                        | Tool/resource/prompt discovery and caching                                                                                                   |
| `crates/agent-mcp/tests/stdio_integration.rs`              | Stdio integration tests with echo-mcp-server fixture                                                                                         |
| `crates/agent-mcp/tests/sse_integration.rs`                | SSE integration tests with wiremock                                                                                                          |
| `crates/agent-mcp/tests/lifecycle_integration.rs`          | Full lifecycle integration tests                                                                                                             |
| `crates/agent-mcp/tests/fixtures/echo-mcp-server.mjs`      | Minimal MCP server for integration tests                                                                                                     |
| `crates/agent-mcp/tests/fixtures/package.json`             | Fixture package.json with MCP SDK dependency                                                                                                 |
| `apps/agent-gui/src/stores/mcp.ts`                         | Pinia store for MCP server state                                                                                                             |
| `apps/agent-gui/src/stores/mcp.test.ts`                    | Vitest tests for mcp store                                                                                                                   |
| `apps/agent-gui/src/components/McpStatusIndicator.vue`     | Status bar MCP indicator                                                                                                                     |
| `apps/agent-gui/src/components/McpStatusIndicator.test.ts` | Vitest tests                                                                                                                                 |
| `apps/agent-gui/src/components/McpServerManager.vue`       | Server management drawer panel                                                                                                               |
| `apps/agent-gui/src/components/McpServerManager.test.ts`   | Vitest tests                                                                                                                                 |
| `apps/agent-gui/e2e/mcp.spec.ts`                           | Playwright E2E tests for MCP UI                                                                                                              |
| `crates/agent-runtime/tests/mcp_integration.rs`            | Runtime MCP integration tests                                                                                                                |

### Modified Files

| File                                                     | Change                                                                                       |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `Cargo.toml`                                             | Add `agent-mcp` to workspace members; add `eventsource-stream` to workspace deps             |
| `crates/agent-core/src/events.rs`                        | Add 8 MCP EventPayload variants + match arms                                                 |
| `crates/agent-tools/src/lib.rs`                          | Remove `mcp` module, add `agent-mcp` dependency re-exports                                   |
| `crates/agent-tools/Cargo.toml`                          | Add `agent-mcp` dependency                                                                   |
| `crates/agent-tools/src/mcp.rs`                          | DELETE — replaced by agent-mcp                                                               |
| `crates/agent-tools/src/provider/mod.rs`                 | Keep `mcp_provider` module export                                                            |
| `crates/agent-tools/src/provider/mcp_provider.rs`        | Rewrite with McpToolAdapter                                                                  |
| `crates/agent-tools/src/permission.rs`                   | Add `trusted_mcp_servers`, `check_mcp_permission`, `PromptWithTrust`, `McpInvoke` ToolEffect |
| `crates/agent-config/Cargo.toml`                         | Add `agent-mcp` dependency                                                                   |
| `crates/agent-config/src/lib.rs`                         | Add `mcp_servers` field to Config, add McpServerConfig parsing                               |
| `crates/agent-config/src/loader.rs`                      | Parse `[mcp_servers]` TOML section, validate, resolve env vars                               |
| `crates/agent-runtime/Cargo.toml`                        | Add `agent-mcp` dependency                                                                   |
| `crates/agent-runtime/src/lib.rs`                        | Export `mcp_manager` module                                                                  |
| `crates/agent-runtime/src/mcp_manager.rs`                | NEW — McpServerManager implementation                                                        |
| `crates/agent-runtime/src/facade_runtime.rs`             | Add `mcp_manager` field, integrate MCP lifecycle                                             |
| `apps/agent-gui/src-tauri/Cargo.toml`                    | Add `agent-mcp` dependency with specta feature                                               |
| `apps/agent-gui/src-tauri/src/commands.rs`               | Add 9 MCP Tauri commands + response types                                                    |
| `apps/agent-gui/src-tauri/src/specta.rs`                 | Register MCP commands and types                                                              |
| `apps/agent-gui/src-tauri/src/lib.rs`                    | Register MCP commands in generate_handler                                                    |
| `apps/agent-gui/src-tauri/src/app_state.rs`              | Add mcp_manager access                                                                       |
| `apps/agent-gui/src/components/PermissionPrompt.vue`     | Add MCP trust UI                                                                             |
| `apps/agent-gui/src/components/PermissionPrompt.test.ts` | Extend with MCP tests                                                                        |
| `apps/agent-gui/src/components/StatusBar.vue`            | Add McpStatusIndicator                                                                       |
| `apps/agent-gui/src/composables/useTauriEvents.ts`       | Add MCP event handlers                                                                       |
| `apps/agent-gui/e2e/tauri-mock.js`                       | Add MCP command mocks                                                                        |
| `crates/agent-tui/src/components/permission_modal.rs`    | Add MCP permission request variant                                                           |
| `justfile`                                               | Add `test-mcp` command                                                                       |

---

## Task 1: agent-mcp Crate Skeleton + Types

**Files:**

- Create: `crates/agent-mcp/Cargo.toml`
- Create: `crates/agent-mcp/src/lib.rs`
- Create: `crates/agent-mcp/src/types.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Add agent-mcp to workspace**

Edit `Cargo.toml` — add `"crates/agent-mcp"` to `workspace.members`.

- [ ] **Step 2: Create `crates/agent-mcp/Cargo.toml`**

```toml
[package]
name = "agent-mcp"
version.workspace = true
edition.workspace = true
license.workspace = true

[features]
specta = ["dep:specta"]

[dependencies]
agent-core = { path = "../agent-core" }
async-trait.workspace = true
futures.workspace = true
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "sync", "time", "process", "fs"] }
tracing.workspace = true
reqwest = { workspace = true, optional = true }
eventsource-stream = { workspace = true, optional = true }
specta = { workspace = true, optional = true, features = ["chrono"] }

sse = ["reqwest", "eventsource-stream"]
specta = ["dep:specta"]

[dev-dependencies]
tempfile.workspace = true
wiremock.workspace = true
```

- [ ] **Step 3: Create `crates/agent-mcp/src/types.rs`**

Define all MCP protocol types: `McpServerDef`, `McpTransportDef`, `McpToolDef`, `McpResourceDef`, `McpPromptDef`, `McpPromptArgument`, `McpToolResult`, `McpContentBlock`, `McpResourceContent`, `McpServerStatus`. Also define JSON-RPC types: `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcNotification`, `JsonRpcError`, `ServerInfo`, `ServerCapabilities`.

All types with `#[cfg_attr(feature = "specta", derive(specta::Type))]` where applicable.

- [ ] **Step 4: Create `crates/agent-mcp/src/lib.rs`**

```rust
pub mod types;
pub mod transport;
pub mod client;
pub mod lifecycle;
pub mod discovery;

pub use types::*;
pub use client::McpClient;
pub use lifecycle::ServerLifecycle;
pub use discovery::DiscoveryCache;

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("server not running: {0}")]
    NotRunning(String),
    #[error("handshake failed: {0}")]
    Handshake(String),
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("resource not found: {0}")]
    ResourceNotFound(String),
    #[error("prompt not found: {0}")]
    PromptNotFound(String),
    #[error("invocation failed: {0}")]
    InvocationFailed(String),
    #[error("server crashed: {0}")]
    ServerCrash(String),
    #[error("max restart attempts exceeded for {0}")]
    MaxRestartsExceeded(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, McpError>;
```

- [ ] **Step 5: Write failing tests for types.rs**

Add tests in `types.rs` for TOML serde roundtrip of `McpServerDef` (stdio and sse), `McpToolResult` parsing, `McpContentBlock` deserialization, `McpServerStatus` serialization.

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test -p agent-mcp -- types
```

Expected: All type tests PASS (serde roundtrip tests work as definition tests).

- [ ] **Step 7: Commit**

```bash
git add crates/agent-mcp/ Cargo.toml
git commit -m "feat(mcp): add agent-mcp crate skeleton with protocol types"
```

---

## Task 2: Transport Trait + StdioTransport

**Files:**

- Create: `crates/agent-mcp/src/transport/mod.rs`
- Create: `crates/agent-mcp/src/transport/stdio.rs`

- [ ] **Step 1: Write failing tests for StdioTransport**

In `crates/agent-mcp/src/transport/stdio.rs`, add a test module that launches `cat` as a subprocess (echoes stdin to stdout), sends a JSON-RPC request, and verifies the response comes back.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::JsonRpcRequest;

    #[tokio::test]
    async fn stdio_transport_sends_and_receives_via_cat() {
        let transport = StdioTransport::spawn(
            "cat",
            &[],
            std::collections::HashMap::new(),
            None,
        ).await.unwrap();

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(1),
            method: "test".into(),
            params: Some(serde_json::json!({"key": "value"})),
        };
        let response = transport.send_request(request).await.unwrap();
        assert_eq!(response.id, Some(1));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p agent-mcp -- stdio_transport_sends_and_receives_via_cat
```

Expected: FAIL (StdioTransport not implemented)

- [ ] **Step 3: Create `crates/agent-mcp/src/transport/mod.rs`**

```rust
use crate::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::Result;

#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse>;
    async fn send_notification(&self, notification: JsonRpcNotification) -> Result<()>;
    async fn close(&self) -> Result<()>;
}

pub mod stdio;
pub mod sse;
```

- [ ] **Step 4: Implement StdioTransport**

`StdioTransport` wraps a `tokio::process::Child`:

- `spawn()` creates the child process with optional env/cwd
- Uses `tokio::io::BufWriter<ChildStdin>` for writing
- Uses `tokio::io::BufReader<ChildStdout>` for reading
- Line-delimited JSON: write `\n`-terminated JSON to stdin, read `\n`-terminated JSON from stdout
- `send_request()`: serialize request → write to stdin → read response line → deserialize
- `send_notification()`: serialize notification → write to stdin (no response)
- `close()`: kill child process
- Uses `tokio::sync::Mutex` to serialize stdin writes (multiple concurrent callers)

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test -p agent-mcp -- stdio_transport_sends_and_receives_via_cat
```

Expected: PASS

- [ ] **Step 6: Add more StdioTransport tests**

- `stdio_transport_handles_env_variables` — spawn `env` command, verify env var set
- `stdio_transport_handles_cwd` — spawn `pwd`, verify cwd
- `stdio_transport_close_kills_process` — close transport, verify process is gone
- `stdio_transport_detects_dead_process` — kill process externally, verify send_request errors

- [ ] **Step 7: Commit**

```bash
git add crates/agent-mcp/src/transport/
git commit -m "feat(mcp): add Transport trait and StdioTransport implementation"
```

---

## Task 3: SseTransport

**Files:**

- Create: `crates/agent-mcp/src/transport/sse.rs`

- [ ] **Step 1: Write failing tests for SseTransport**

Use `wiremock` to create a mock HTTP server that responds to POST requests and streams SSE events:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn sse_transport_sends_request_and_receives_response() {
        let mock_server = MockServer::start().await;
        // Mount mock that returns JSON-RPC response
        let transport = SseTransport::new(
            &format!("{}/mcp", mock_server.uri()),
            std::collections::HashMap::new(),
            None,
        ).await.unwrap();
        // Test request/response cycle
    }

    #[tokio::test]
    async fn sse_transport_handles_connection_error() { ... }

    #[tokio::test]
    async fn sse_transport_api_key_from_env() { ... }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p agent-mcp -- sse_transport
```

Expected: FAIL

- [ ] **Step 3: Implement SseTransport**

`SseTransport`:

- `new()` creates HTTP client, establishes SSE connection for receiving responses
- `send_request()`: POST JSON-RPC request to configured URL, read response from SSE stream
- `send_notification()`: POST notification (no response expected)
- Headers include configured values + api_key_env resolved header
- Uses `reqwest` for HTTP, `eventsource-stream` for SSE parsing
- Correlates request/response via JSON-RPC `id` field

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p agent-mcp -- sse_transport
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-mcp/src/transport/sse.rs
git commit -m "feat(mcp): add SseTransport implementation"
```

---

## Task 4: McpClient (Handshake + Discover + Invoke)

**Files:**

- Create: `crates/agent-mcp/src/client.rs`
- Create: `crates/agent-mcp/src/discovery.rs`

- [ ] **Step 1: Write failing tests for McpClient with MockTransport**

Create a `MockTransport` in tests that records requests and returns preset responses:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Transport;
    use crate::types::*;

    struct MockTransport {
        responses: std::collections::VecDeque<JsonRpcResponse>,
        sent_requests: std::sync::Mutex<Vec<JsonRpcRequest>>,
    }

    #[tokio::test]
    async fn handshake_sends_initialize_and_initialized() { ... }

    #[tokio::test]
    async fn discover_tools_calls_tools_list() { ... }

    #[tokio::test]
    async fn discover_tools_caches_result() { ... }

    #[tokio::test]
    async fn call_tool_sends_tools_call_with_arguments() { ... }

    #[tokio::test]
    async fn read_resource_sends_resources_read() { ... }

    #[tokio::test]
    async fn get_prompt_sends_prompts_get() { ... }

    #[tokio::test]
    async fn shutdown_sends_shutdown_notification() { ... }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p agent-mcp -- client
```

Expected: FAIL

- [ ] **Step 3: Implement McpClient**

`McpClient`:

- Constructor takes `server_id` and `Arc<dyn Transport>`
- `handshake()`: send `initialize` request with client capabilities, wait for `ServerInfo` response, send `initialized` notification. Store `ServerInfo` in `OnceCell`.
- `discover_tools()`: send `tools/list` request, parse response into `Vec<McpToolDef>`, cache in `OnceCell`.
- `discover_resources()`: send `resources/list` request, parse response into `Vec<McpResourceDef>`, cache.
- `discover_prompts()`: send `prompts/list` request, parse response into `Vec<McpPromptDef>`, cache.
- `call_tool()`: send `tools/call` request with name and arguments, parse `McpToolResult`.
- `read_resource()`: send `resources/read` request with uri, parse content blocks.
- `get_prompt()`: send `prompts/get` request with name and arguments, parse content blocks.
- `shutdown()`: send shutdown notification, close transport.

All methods check `OnceCell` cache first to avoid redundant requests.

- [ ] **Step 4: Implement DiscoveryCache**

`DiscoveryCache`:

- Wraps `McpClient` discovery with explicit cache invalidation
- `invalidate_tools()`, `invalidate_resources()`, `invalidate_prompts()` clear `OnceCell` values
- Used by `ServerLifecycle` when reconnecting

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p agent-mcp -- client
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/agent-mcp/src/client.rs crates/agent-mcp/src/discovery.rs
git commit -m "feat(mcp): add McpClient with handshake, discover, and invoke"
```

---

## Task 5: ServerLifecycle

**Files:**

- Create: `crates/agent-mcp/src/lifecycle.rs`

- [ ] **Step 1: Write failing tests for ServerLifecycle**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ensure_running_starts_server_on_first_call() { ... }

    #[tokio::test]
    async fn ensure_running_reuses_existing_client() { ... }

    #[tokio::test]
    async fn mark_active_updates_last_activity() { ... }

    #[tokio::test]
    async fn check_idle_timeout_shuts_down_after_timeout() { ... }

    #[tokio::test]
    async fn check_idle_timeout_does_nothing_if_active() { ... }

    #[tokio::test]
    async fn keep_alive_skips_idle_timeout() { ... }

    #[tokio::test]
    async fn auto_restart_restarts_after_crash() { ... }

    #[tokio::test]
    async fn auto_restart_gives_up_after_max_attempts() { ... }

    #[tokio::test]
    async fn shutdown_sets_status_to_stopped() { ... }

    #[tokio::test]
    async fn status_transitions_stopped_to_running() { ... }

    #[tokio::test]
    async fn status_transitions_to_failed_on_error() { ... }
}
```

Tests use a real `StdioTransport` with `cat` as a lightweight subprocess (no full MCP handshake required for lifecycle state machine tests).

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p agent-mcp -- lifecycle
```

Expected: FAIL

- [ ] **Step 3: Implement ServerLifecycle**

`ServerLifecycle`:

- Holds `McpServerDef`, optional `Arc<McpClient>`, `McpServerStatus`, `last_activity: Option<Instant>`, `restart_count: u32`
- `ensure_running()`: if status is Running, return existing client. If Stopped, create transport → McpClient → handshake → set status Running. Track restart_count.
- `mark_active()`: update `last_activity` to `Instant::now()`
- `check_idle_timeout()`: if `keep_alive`, skip. If `last_activity + idle_timeout < now`, call `shutdown()`.
- `shutdown()`: call `client.shutdown()`, set status Stopped, clear client.
- On handshake failure: if `auto_restart` and `restart_count < max_restart_attempts`, increment and retry. Otherwise set status to Failed.
- Background task: tokio interval loop calling `check_idle_timeout()` on each server.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p agent-mcp -- lifecycle
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-mcp/src/lifecycle.rs
git commit -m "feat(mcp): add ServerLifecycle with on-demand start, idle timeout, crash restart"
```

---

## Task 6: Integration Test Fixture (echo-mcp-server)

**Files:**

- Create: `crates/agent-mcp/tests/fixtures/echo-mcp-server.mjs`
- Create: `crates/agent-mcp/tests/fixtures/package.json`

- [ ] **Step 1: Create fixture package.json**

```json
{
  "name": "echo-mcp-server-fixture",
  "version": "1.0.0",
  "private": true,
  "type": "module",
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.12.0"
  }
}
```

- [ ] **Step 2: Install fixture dependencies**

```bash
cd crates/agent-mcp/tests/fixtures && npm install
```

- [ ] **Step 3: Create echo-mcp-server.mjs**

Minimal MCP stdio server implementing:

- `initialize` → returns `{ name: "echo-test-server", version: "1.0.0" }` with capabilities: tools, resources, prompts
- `tools/list` → returns `echo` tool (echoes input), `env` tool (returns env var)
- `tools/call` → echo tool returns input as text, env tool returns `process.env[TARGET_VAR]`
- `resources/list` → returns `test://echo` resource
- `resources/read` → returns text content for `test://echo`
- `prompts/list` → returns `test-prompt` with `topic` argument
- `prompts/get` → returns user message with topic

- [ ] **Step 4: Verify fixture runs standalone**

```bash
cd crates/agent-mcp/tests/fixtures && echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | node echo-mcp-server.mjs
```

Expected: JSON-RPC initialize response printed to stdout.

- [ ] **Step 5: Add fixture to .gitignore (node_modules)**

Add `crates/agent-mcp/tests/fixtures/node_modules/` to `.gitignore`.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-mcp/tests/fixtures/ .gitignore
git commit -m "test(mcp): add echo-mcp-server fixture for integration tests"
```

---

## Task 7: Stdio Integration Tests

**Files:**

- Create: `crates/agent-mcp/tests/stdio_integration.rs`

- [ ] **Step 1: Write stdio integration tests**

```rust
// Uses echo-mcp-server fixture

#[tokio::test]
async fn stdio_handshake_with_real_server() {
    let transport = StdioTransport::spawn(
        "node", &["crates/agent-mcp/tests/fixtures/echo-mcp-server.mjs"], ..., None
    ).await.unwrap();
    let client = McpClient::new("test", Arc::new(transport));
    let info = client.handshake().await.unwrap();
    assert_eq!(info.name, "echo-test-server");
}

#[tokio::test]
async fn stdio_discover_and_call_echo_tool() { ... }

#[tokio::test]
async fn stdio_read_resource() { ... }

#[tokio::test]
async fn stdio_get_prompt() { ... }

#[tokio::test]
async fn stdio_handles_server_crash() { ... }

#[tokio::test]
async fn stdio_env_variables_passed_to_child() { ... }
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test -p agent-mcp --test stdio_integration
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/tests/stdio_integration.rs
git commit -m "test(mcp): add stdio integration tests with echo-mcp-server fixture"
```

---

## Task 8: SSE Integration Tests

**Files:**

- Create: `crates/agent-mcp/tests/sse_integration.rs`

- [ ] **Step 1: Write SSE integration tests**

Use `wiremock` to simulate an SSE MCP server endpoint.

```rust
#[tokio::test]
async fn sse_connects_to_server() { ... }

#[tokio::test]
async fn sse_sends_request_and_receives_response() { ... }

#[tokio::test]
async fn sse_handles_connection_error() { ... }

#[tokio::test]
async fn sse_api_key_from_env() { ... }
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test -p agent-mcp --test sse_integration
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/tests/sse_integration.rs
git commit -m "test(mcp): add SSE integration tests with wiremock"
```

---

## Task 9: Lifecycle Integration Tests

**Files:**

- Create: `crates/agent-mcp/tests/lifecycle_integration.rs`

- [ ] **Step 1: Write lifecycle integration tests**

```rust
#[tokio::test]
async fn full_lifecycle_start_discover_call_timeout() { ... }

#[tokio::test]
async fn keep_alive_server_never_times_out() { ... }

#[tokio::test]
async fn crash_recovery_with_auto_restart() { ... }

#[tokio::test]
async fn max_restart_attempts_exhausted() { ... }
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test -p agent-mcp --test lifecycle_integration
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/agent-mcp/tests/lifecycle_integration.rs
git commit -m "test(mcp): add lifecycle integration tests"
```

---

## Task 10: agent-core MCP Events

**Files:**

- Modify: `crates/agent-core/src/events.rs`

- [ ] **Step 1: Write failing tests for MCP event payloads**

Add tests in `events.rs` for each new variant:

```rust
#[test]
fn mcp_server_starting_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(), SessionId::new(), AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpServerStarting { server_id: "test".into() },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpServerStarting");
    assert_eq!(json["payload"]["server_id"], "test");
}
// ... same for McpServerReady, McpServerStopped, McpServerFailed,
//     McpToolCallStarted, McpToolCallCompleted, McpTrustGranted, McpTrustRevoked
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p agent-core -- mcp_
```

Expected: FAIL (variants don't exist)

- [ ] **Step 3: Add 8 EventPayload variants**

Add to `EventPayload` enum:

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

Add corresponding match arms in `EventPayload::event_type()`.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p agent-core -- mcp_
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-core/src/events.rs
git commit -m "feat(core): add MCP event payload variants"
```

---

## Task 11: agent-tools Permission + MCP Bridge

**Files:**

- Modify: `crates/agent-tools/Cargo.toml`
- Modify: `crates/agent-tools/src/lib.rs`
- Delete: `crates/agent-tools/src/mcp.rs`
- Modify: `crates/agent-tools/src/provider/mcp_provider.rs`
- Modify: `crates/agent-tools/src/permission.rs`

- [ ] **Step 1: Write failing tests for PermissionEngine MCP extensions**

```rust
#[test]
fn mcp_untrusted_server_always_prompts_with_trust() {
    let engine = PermissionEngine::new(PermissionMode::Autonomous);
    let outcome = engine.check_mcp_permission("unknown-server", "mcp.unknown.tool");
    assert_eq!(outcome, PermissionOutcome::PromptWithTrust);
}

#[test]
fn mcp_trusted_server_follows_permission_mode() { ... }

#[test]
fn mcp_trusted_server_readonly_denies() { ... }

#[test]
fn trust_and_revoke_roundtrip() { ... }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p agent-tools -- mcp_
```

Expected: FAIL

- [ ] **Step 3: Add `agent-mcp` dependency to agent-tools**

Edit `crates/agent-tools/Cargo.toml`:

```toml
agent-mcp = { path = "../agent-mcp" }
```

- [ ] **Step 4: Add `PromptWithTrust` to `PermissionOutcome`**

```rust
pub enum PermissionOutcome {
    Allowed,
    RequiresApproval,
    Pending,
    Denied(String),
    PromptWithTrust,  // NEW
}
```

Update all existing `match` arms on `PermissionOutcome` to handle `PromptWithTrust` — treat it like `RequiresApproval` in `require_permission()` and `invoke_with_permission()`.

- [ ] **Step 5: Add `McpInvoke` to `ToolEffect`**

```rust
pub enum ToolEffect {
    Read,
    Write,
    Shell { destructive: bool },
    Network,
    Destructive,
    McpInvoke,  // NEW
}
```

- [ ] **Step 6: Add `trusted_mcp_servers` and MCP methods to `PermissionEngine`**

```rust
pub struct PermissionEngine {
    mode: PermissionMode,
    trusted_mcp_servers: HashSet<String>,
}

impl PermissionEngine {
    // Add to new():
    // Self { mode, trusted_mcp_servers: HashSet::new() }

    pub fn check_mcp_permission(&self, server_id: &str, _tool_id: &str) -> PermissionOutcome {
        if self.trusted_mcp_servers.contains(server_id) {
            match self.mode {
                PermissionMode::ReadOnly => PermissionOutcome::Denied("read-only mode blocks MCP tools".into()),
                PermissionMode::Autonomous => PermissionOutcome::Allowed,
                _ => PermissionOutcome::RequiresApproval,
            }
        } else {
            PermissionOutcome::PromptWithTrust
        }
    }

    pub fn trust_server(&mut self, server_id: String) {
        self.trusted_mcp_servers.insert(server_id);
    }

    pub fn revoke_trust(&mut self, server_id: &str) {
        self.trusted_mcp_servers.remove(server_id);
    }

    pub fn trusted_servers(&self) -> &HashSet<String> {
        &self.trusted_mcp_servers
    }
}
```

- [ ] **Step 7: Rewrite mcp_provider.rs**

Delete old `McpProvider` placeholder. Replace with `McpToolAdapter`:

```rust
use crate::permission::{ToolEffect, ToolRisk};
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use agent_mcp::McpClient;
use async_trait::async_trait;
use std::sync::Arc;

pub struct McpToolAdapter {
    server_id: String,
    tool_def: agent_mcp::McpToolDef,
    client: Arc<McpClient>,
}

impl McpToolAdapter {
    pub fn new(server_id: String, tool_def: agent_mcp::McpToolDef, client: Arc<McpClient>) -> Self {
        Self { server_id, tool_def, client }
    }
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

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk {
            tool_id: format!("mcp.{}.{}", self.server_id, self.tool_def.name),
            effect: ToolEffect::McpInvoke,
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let result = self.client
            .call_tool(&self.tool_def.name, invocation.arguments)
            .await
            .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;

        let text: String = result.content.iter().map(|block| match block {
            agent_mcp::McpContentBlock::Text { text } => text.clone(),
            agent_mcp::McpContentBlock::Image { data, .. } => format!("[image: {} bytes]", data.len()),
            agent_mcp::McpContentBlock::Resource { resource } => format!("[resource: {}]", resource.uri),
        }).collect::<Vec<_>>().join("\n");

        Ok(ToolOutput {
            text,
            truncated: result.is_error,
        })
    }
}
```

Add test for `McpToolAdapter::definition()` tool_id format.

- [ ] **Step 8: Delete mcp.rs, update lib.rs**

Remove `pub mod mcp;` from `lib.rs` and its `pub use` statements. Update re-exports from `agent_mcp` instead:

```rust
pub use agent_mcp::McpServerDef;
pub use agent_mcp::McpTransportDef;
```

Remove `pub mod mcp;` line.

- [ ] **Step 9: Update `require_permission` and `invoke_with_permission` for PromptWithTrust**

In `registry.rs`, add `PermissionOutcome::PromptWithTrust` match arm in both functions — treat as `RequiresApproval`.

- [ ] **Step 10: Run all agent-tools tests**

```bash
cargo test -p agent-tools --all-targets
```

Expected: PASS

- [ ] **Step 11: Commit**

```bash
git add crates/agent-tools/
git commit -m "feat(tools): extend PermissionEngine with MCP trust, add McpToolAdapter"
```

---

## Task 12: agent-config MCP Server Parsing

**Files:**

- Modify: `crates/agent-config/Cargo.toml`
- Modify: `crates/agent-config/src/lib.rs`
- Modify: `crates/agent-config/src/loader.rs`

- [ ] **Step 1: Write failing tests for MCP config parsing**

```rust
#[test]
fn parse_stdio_server_config() { ... }

#[test]
fn parse_sse_server_config() { ... }

#[test]
fn reject_stdio_without_command() { ... }

#[test]
fn reject_sse_without_url() { ... }

#[test]
fn env_empty_value_resolves_from_env() { ... }

#[test]
fn headers_variable_expansion() { ... }

#[test]
fn default_values() { ... }

#[test]
fn full_profile_with_mcp_servers() { ... }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p agent-config -- mcp
```

Expected: FAIL

- [ ] **Step 3: Add agent-mcp dependency to agent-config**

Edit `crates/agent-config/Cargo.toml`:

```toml
agent-mcp = { path = "../agent-mcp" }
```

- [ ] **Step 4: Add McpServerConfig to Config**

In `lib.rs`, add:

```rust
pub struct Config {
    pub profiles: Vec<(String, ProfileDef)>,
    pub mcp_servers: Vec<(String, agent_mcp::McpTransportDef)>,  // NEW
    pub source: ConfigSource,
}
```

Define `McpServerConfig` (the TOML-facing config struct with optional fields) as a separate struct in `lib.rs`, with conversion to `agent_mcp::McpServerDef`.

- [ ] **Step 5: Parse `[mcp_servers]` in loader.rs**

Extend TOML parsing to handle `[mcp_servers.SERVER_ID]` sections. Each section parses into `McpServerConfig`, validates required fields, resolves env vars (empty env values and `${VAR}` in headers), converts to `McpServerDef`.

- [ ] **Step 6: Add mcp_servers to defaults and Config::load()**

`Config::defaults()` returns empty `mcp_servers`. `Config::load()` includes MCP servers from parsed config.

- [ ] **Step 7: Add `mcp_server_defs()` method to Config**

```rust
impl Config {
    pub fn mcp_server_defs(&self) -> Vec<agent_mcp::McpServerDef> {
        self.mcp_servers.iter().map(|(id, def)| def.clone().into_server_def(id)).collect()
    }
}
```

- [ ] **Step 8: Run tests to verify they pass**

```bash
cargo test -p agent-config --all-targets
```

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add crates/agent-config/
git commit -m "feat(config): parse MCP server definitions from kairox.toml"
```

---

## Task 13: agent-runtime McpServerManager

**Files:**

- Modify: `crates/agent-runtime/Cargo.toml`
- Create: `crates/agent-runtime/src/mcp_manager.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs`

- [ ] **Step 1: Write failing tests for McpServerManager**

In a new test file `crates/agent-runtime/tests/mcp_integration.rs`:

```rust
#[tokio::test]
async fn mcp_manager_registers_tools_in_registry() { ... }

#[tokio::test]
async fn mcp_tool_invocation_through_runtime() { ... }

#[tokio::test]
async fn untrusted_mcp_tool_prompts_for_permission() { ... }

#[tokio::test]
async fn trusted_mcp_tool_auto_allowed_in_agent_mode() { ... }

#[tokio::test]
async fn mcp_server_events_emitted() { ... }

#[tokio::test]
async fn mcp_server_crash_and_recovery_in_runtime() { ... }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p agent-runtime --test mcp_integration
```

Expected: FAIL

- [ ] **Step 3: Add agent-mcp dependency to agent-runtime**

Edit `crates/agent-runtime/Cargo.toml`:

```toml
agent-mcp = { path = "../agent-mcp" }
```

Add to dev-dependencies:

```toml
agent-mcp = { path = "../agent-mcp" }
```

- [ ] **Step 4: Implement McpServerManager**

Create `crates/agent-runtime/src/mcp_manager.rs`:

```rust
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

`refresh_tools()` discovers tools from a server, creates `McpToolAdapter` instances, and registers them in the `ToolRegistry`.

- [ ] **Step 5: Integrate McpServerManager into LocalRuntime**

In `facade_runtime.rs`:

Add field:

```rust
mcp_manager: Option<Arc<Mutex<McpServerManager>>>,
```

Add builder method:

```rust
pub async fn with_mcp_servers(mut self, configs: Vec<McpServerDef>) -> Self {
    let manager = McpServerManager::from_config(
        configs,
        self.tool_registry.clone(),
        // permission_engine access via new shared reference
    );
    let mut manager = manager;
    manager.start_persistent_servers().await;
    self.mcp_manager = Some(Arc::new(Mutex::new(manager)));
    self
}
```

In `start_session()`: ensure MCP tools are registered in the session's tool registry.

In the agent loop: MCP tools are called through the existing `ToolRegistry` flow. The `PermissionEngine::check_mcp_permission()` is called for tools with `ToolEffect::McpInvoke` instead of the standard `decide()`.

In `cancel_session()` and `Drop`: call `mcp_manager.shutdown_all()`.

- [ ] **Step 6: Update lib.rs**

```rust
pub mod mcp_manager;  // NEW
pub use mcp_manager::McpServerManager;  // NEW
```

- [ ] **Step 7: Run tests to verify they pass**

```bash
cargo test -p agent-runtime --test mcp_integration
```

Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/agent-runtime/
git commit -m "feat(runtime): add McpServerManager and integrate MCP lifecycle into LocalRuntime"
```

---

## Task 14: GUI Tauri Commands + Type Generation

**Files:**

- Modify: `apps/agent-gui/src-tauri/Cargo.toml`
- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/app_state.rs`

- [ ] **Step 1: Add agent-mcp dependency**

Edit `apps/agent-gui/src-tauri/Cargo.toml`:

```toml
agent-mcp = { path = "../../../crates/agent-mcp", features = ["specta"] }
```

- [ ] **Step 2: Add MCP response types and commands to commands.rs**

Add response types: `McpServerStatusResponse`, `McpToolDefResponse`, `McpResourceDefResponse`, `McpPromptDefResponse`, `McpContentBlockResponse`.

Add 9 Tauri commands: `list_mcp_servers`, `start_mcp_server`, `stop_mcp_server`, `refresh_mcp_tools`, `trust_mcp_server`, `revoke_mcp_trust`, `list_mcp_resources`, `list_mcp_prompts`, `read_mcp_resource`.

Each command accesses `GuiState` → `runtime` → `mcp_manager`.

- [ ] **Step 3: Add mcp_manager access to GuiState**

In `app_state.rs`, no structural change needed — access `mcp_manager` through `runtime.mcp_manager` field. But add a convenience method:

```rust
impl GuiState {
    pub async fn mcp_manager(&self) -> tokio::sync::MutexGuard<'_, McpServerManager> {
        // Access through runtime's mcp_manager
    }
}
```

- [ ] **Step 4: Register commands in specta.rs and lib.rs**

In `specta.rs` `collect_commands![]`: add all 9 new commands.

In `specta.rs` `.typ()`: add `McpServerStatusResponse`, `McpToolDefResponse`, `McpResourceDefResponse`, `McpPromptDefResponse`, `McpContentBlockResponse`, `McpServerStatus`.

In `lib.rs` `generate_handler![]`: add all 9 new commands.

- [ ] **Step 5: Run type generation**

```bash
just gen-types
```

Verify no errors and generated TypeScript files include MCP types.

- [ ] **Step 6: Run clippy + existing tests**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src-tauri/ apps/agent-gui/src/generated/
git commit -m "feat(gui): add MCP Tauri commands and regenerate TypeScript bindings"
```

---

## Task 15: GUI Frontend — McpStore + Components

**Files:**

- Create: `apps/agent-gui/src/stores/mcp.ts`
- Create: `apps/agent-gui/src/stores/mcp.test.ts`
- Create: `apps/agent-gui/src/components/McpStatusIndicator.vue`
- Create: `apps/agent-gui/src/components/McpStatusIndicator.test.ts`
- Create: `apps/agent-gui/src/components/McpServerManager.vue`
- Create: `apps/agent-gui/src/components/McpServerManager.test.ts`
- Modify: `apps/agent-gui/src/components/StatusBar.vue`
- Modify: `apps/agent-gui/src/components/PermissionPrompt.vue`
- Modify: `apps/agent-gui/src/components/PermissionPrompt.test.ts`
- Modify: `apps/agent-gui/src/composables/useTauriEvents.ts`

- [ ] **Step 1: Create McpStore with failing tests**

Write `mcp.test.ts` first with tests for: fetchServers, startServer, stopServer, trustServer, revokeTrust, event updates, computed properties, refreshTools.

- [ ] **Step 2: Run test to verify it fails**

```bash
pnpm --filter agent-gui run test -- mcp
```

Expected: FAIL

- [ ] **Step 3: Implement McpStore**

Pinia store with reactive server list, trusted server set, computed properties (runningServers, failedServers), and async methods calling Tauri invoke.

- [ ] **Step 4: Run test to verify it passes**

```bash
pnpm --filter agent-gui run test -- mcp
```

Expected: PASS

- [ ] **Step 5: Create McpStatusIndicator.vue with tests**

Simple indicator component showing dot + count. Tests: renders correct color per status, click emits event.

- [ ] **Step 6: Create McpServerManager.vue with tests**

Drawer panel with server list, trust/revoke, start/stop, error display. Tests: renders servers, trust action, button states.

- [ ] **Step 7: Extend PermissionPrompt.vue for MCP tools**

Add conditional section for MCP tools: server name, trust checkbox, Allow Once / Allow Always buttons. Extend existing tests.

- [ ] **Step 8: Update StatusBar.vue**

Add `McpStatusIndicator` component and `McpServerManager` drawer.

- [ ] **Step 9: Update useTauriEvents.ts**

Add MCP event handlers routing to mcp store's `handleEvent()` method.

- [ ] **Step 10: Run all GUI tests**

```bash
pnpm --filter agent-gui run test
```

Expected: PASS

- [ ] **Step 11: Commit**

```bash
git add apps/agent-gui/src/
git commit -m "feat(gui): add MCP store, status indicator, server manager, and permission extension"
```

---

## Task 16: TUI MCP Adaptation

**Files:**

- Modify: `crates/agent-tui/src/components/permission_modal.rs`

- [ ] **Step 1: Add PermissionRequest enum**

```rust
pub enum PermissionRequest {
    BuiltIn { tool_id: String, preview: String },
    Mcp { server_id: String, tool_name: String, full_tool_id: String, preview: String, server_trusted: bool },
}
```

- [ ] **Step 2: Update rendering for MCP tools**

MCP tools show `[MCP] server/tool` label. Add `(T) Trust server` shortcut key option.

- [ ] **Step 3: Add MCP status display to TUI status bar**

Simple `[MCP:N↑]` display where N is number of running servers.

- [ ] **Step 4: Run TUI tests**

```bash
just test-tui
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-tui/
git commit -m "feat(tui): add MCP permission modal and status indicator"
```

---

## Task 17: E2E Tests + Tauri Mock Update

**Files:**

- Modify: `apps/agent-gui/e2e/tauri-mock.js`
- Create: `apps/agent-gui/e2e/mcp.spec.ts`

- [ ] **Step 1: Add MCP command mocks**

In `tauri-mock.js`, add handlers for `list_mcp_servers`, `start_mcp_server`, `stop_mcp_server`, `trust_mcp_server`, `revoke_mcp_trust`, `refresh_mcp_tools`, `list_mcp_resources`, `list_mcp_prompts`, `read_mcp_resource`.

- [ ] **Step 2: Write E2E tests**

`mcp.spec.ts` with tests for:

- MCP status indicator visible in status bar
- Click opens server manager
- Server list rendering with mock data
- Start/stop server actions
- Trust server action
- MCP permission dialog appearance
- Trust checkbox + allow always interaction
- Allow once / deny interaction
- MCP event-driven UI updates (simulate events → indicator changes)

- [ ] **Step 3: Run E2E tests**

```bash
just test-e2e
```

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/e2e/
git commit -m "test(gui): add MCP E2E tests and update Tauri IPC mock"
```

---

## Task 18: Update kairox.toml.example + Docs

**Files:**

- Modify: `kairox.toml.example`
- Modify: `ROADMAP.md`

- [ ] **Step 1: Add MCP server section to kairox.toml.example**

Add commented-out `[mcp_servers.XXX]` examples showing stdio and sse configurations with all available fields documented.

- [ ] **Step 2: Update ROADMAP.md**

Move "Wire MCP tool execution" from Mid-term to Near-term (✅). Update description to reflect completed implementation.

- [ ] **Step 3: Commit**

```bash
git add kairox.toml.example ROADMAP.md
git commit -m "docs: add MCP server configuration examples and update roadmap"
```

---

## Task 19: justfile + Final Verification

**Files:**

- Modify: `justfile`

- [ ] **Step 1: Add `test-mcp` command to justfile**

```just
# Run MCP-related unit and integration tests
test-mcp:
    cargo test -p agent-mcp --all-targets
    cargo test -p agent-tools -- mcp
    cargo test -p agent-config -- mcp
    cargo test -p agent-runtime --test mcp_integration
    @echo "✅ MCP tests passed"
```

- [ ] **Step 2: Run full verification**

```bash
just check
just test-mcp
just test-e2e
just check-types
```

Expected: All PASS with zero warnings.

- [ ] **Step 3: Commit**

```bash
git add justfile
git commit -m "chore: add test-mcp command to justfile"
```
