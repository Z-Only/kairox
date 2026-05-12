# Refactor & Test Hardening — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add integration tests to 4 under-tested crates, decompose `facade_runtime.rs` (2758→~600 lines) and `agent_loop.rs` (1496→4 submodules), then split the monolithic `AppFacade` trait into focused sub-traits.

**Architecture:** Rust workspace with 9 crates following a dependency-layered architecture (core → store/config → models/tools/memory → runtime → tui/gui). Refactoring follows impl-block extraction within `LocalRuntime<S, M>` (Rust allows cross-file `impl` blocks) with no public API changes. Testing uses in-memory SQLite + temp dirs + deterministic fakes.

**Tech Stack:** Rust (tokio, sqlx, serde, async_trait), TypeScript (Vue 3, Tauri 2), SQLite

---

## Phase 1: Test Foundation

### Task 1.1: agent-store integration tests

**Files:**

- Create: `crates/agent-store/tests/integration.rs`
- Check: `crates/agent-store/src/event_store.rs:1-50` (EventStore trait)

- [ ] **Step 1: Create integration test file with imports and round-trip test**

```rust
// crates/agent-store/tests/integration.rs
use agent_core::{AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId};
use agent_store::{EventStore, SqliteEventStore, WorkspaceRow};
use sqlx::SqlitePool;

async fn new_store() -> SqliteEventStore {
    SqliteEventStore::new_in_memory().await.unwrap()
}

#[tokio::test]
async fn append_and_load_session_events() {
    let store = new_store().await;
    let ws = WorkspaceId::new();
    let sid = SessionId::new();
    store.upsert_workspace(&ws.to_string(), "/tmp/test").await.unwrap();

    let e1 = DomainEvent::new(
        ws.clone(), sid.clone(), AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::UserMessageAdded { message_id: "m1".into(), content: "hello".into() },
    );
    store.append(&e1).await.unwrap();

    let e2 = DomainEvent::new(
        ws.clone(), sid.clone(), AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::AssistantMessageCompleted { message_id: "m2".into(), content: "hi!".into(), usage: None },
    );
    store.append(&e2).await.unwrap();

    let loaded = store.load_session(&sid).await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert!(matches!(&loaded[0].payload, EventPayload::UserMessageAdded { content, .. } if content == "hello"));
    assert!(matches!(&loaded[1].payload, EventPayload::AssistantMessageCompleted { content, .. } if content == "hi!"));
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p agent-store --test integration append_and_load_session_events`
Expected: PASS

- [ ] **Step 3: Add workspace/session metadata CRUD test**

```rust
#[tokio::test]
async fn workspace_and_session_crud() {
    let store = new_store().await;
    let ws_id = "ws-crud";
    let path = "/home/user/project";

    // Create workspace
    store.upsert_workspace(ws_id, path).await.unwrap();
    let workspaces = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, ws_id);

    // Create session
    let sid = SessionId::new();
    store.upsert_session(&agent_store::SessionRow {
        session_id: sid.to_string(),
        workspace_id: ws_id.to_string(),
        title: "test session".into(),
        model_profile_alias: Some("fake".into()),
        created_at: chrono::Utc::now(),
        is_archived: false,
    }).await.unwrap();

    let sessions = store.list_active_sessions(ws_id).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "test session");

    // Rename
    store.rename_session(&sid.to_string(), "renamed").await.unwrap();
    let sessions = store.list_active_sessions(ws_id).await.unwrap();
    assert_eq!(sessions[0].title, "renamed");

    // Soft-delete
    store.soft_delete_session(&sid.to_string()).await.unwrap();
    let sessions = store.list_active_sessions(ws_id).await.unwrap();
    assert!(sessions.is_empty());
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-store --test integration workspace_and_session_crud`
Expected: PASS

- [ ] **Step 5: Add concurrent writes test**

```rust
#[tokio::test]
async fn concurrent_event_writes() {
    let store = Arc::new(new_store().await);
    let ws = WorkspaceId::new();
    let sid = SessionId::new();
    store.upsert_workspace(&ws.to_string(), "/tmp/test").await.unwrap();

    let mut handles = vec![];
    for i in 0..10 {
        let store = store.clone();
        let ws = ws.clone();
        let sid = sid.clone();
        handles.push(tokio::spawn(async move {
            let e = DomainEvent::new(
                ws, sid, AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::UserMessageAdded {
                    message_id: format!("m{}", i),
                    content: format!("msg {}", i),
                },
            );
            store.append(&e).await.unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let loaded = store.load_session(&sid).await.unwrap();
    assert_eq!(loaded.len(), 10);
}
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p agent-store --test integration concurrent_event_writes`
Expected: PASS

- [ ] **Step 7: Add ProjectMetaRepository tests**

```rust
use agent_store::ProjectMetaRepository;
use agent_core::ProjectId;

#[tokio::test]
async fn project_meta_crud() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let repo = ProjectMetaRepository::new(pool.clone());

    let pid = ProjectId::new();
    let ws = WorkspaceId::new();
    let meta = agent_core::ProjectMeta {
        id: pid.clone(),
        name: "test project".into(),
        workspace_id: ws.clone(),
        path: "/tmp/proj".into(),
        display_order: 0,
        expanded: true,
        created_at: chrono::Utc::now(),
    };
    repo.upsert(&meta).await.unwrap();

    let list = repo.list(&ws).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "test project");

    repo.remove(&pid).await.unwrap();
    let list = repo.list(&ws).await.unwrap();
    assert!(list.is_empty());
}
```

- [ ] **Step 8: Run test and commit**

Run: `cargo test -p agent-store --test integration`
Expected: ALL PASS

```
git add crates/agent-store/tests/integration.rs
git commit -m "test(store): add integration tests for event persistence, metadata CRUD, and concurrency"
```

### Task 1.2: agent-config integration tests

**Files:**

- Create: `crates/agent-config/tests/integration.rs`
- Check: `crates/agent-config/src/loader.rs:1-50` (Config, load_from_str, build_router)

- [ ] **Step 1: Create integration test with TOML parsing round-trip**

```rust
// crates/agent-config/tests/integration.rs
use agent_config::{Config, build_router, resolve_api_keys, validate};
use agent_models::FakeModelClient;

#[test]
fn parse_minimal_config() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake-model"
"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.profiles.len(), 1);
    assert_eq!(config.profiles["fake"].provider, "fake");
}
```

- [ ] **Step 2: Run test to verify**

Run: `cargo test -p agent-config --test integration parse_minimal_config`
Expected: PASS

- [ ] **Step 3: Add router build + API key resolution test**

```rust
#[tokio::test]
async fn build_router_from_profiles() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake-model"
[profiles.other]
provider = "fake"
model_id = "other-model"
"#;
    let config: Config = toml::from_str(toml).unwrap();
    let resolved = resolve_api_keys(config).unwrap();
    validate(&resolved).unwrap();
    let router = build_router(&resolved);
    // Router should have two profiles, defaulting to first
    let client = router.client_for("fake").unwrap();
    // Should get a FakeModelClient back
    assert!(client.stream(Default::default()).await.is_ok());
}
```

- [ ] **Step 4: Run test**

Run: `cargo test -p agent-config --test integration build_router_from_profiles`
Expected: PASS

- [ ] **Step 5: Add .kairox/ discovery test (uses tempdir)**

```rust
#[tokio::test]
async fn discovers_project_local_config() {
    let dir = tempfile::tempdir().unwrap();
    let kairox_dir = dir.path().join(".kairox");
    std::fs::create_dir(&kairox_dir).unwrap();
    std::fs::write(kairox_dir.join("config.toml"), r#"
[profiles.project-local]
provider = "fake"
model_id = "local-model"
"#).unwrap();

    let found = agent_config::discover::find_project_config(dir.path()).unwrap();
    assert!(found.is_some());
    let config_path = found.unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("project-local"));
}
```

- [ ] **Step 6: Run test and commit**

Run: `cargo test -p agent-config --test integration`
Expected: ALL PASS

```
git add crates/agent-config/tests/integration.rs
git commit -m "test(config): add integration tests for TOML parsing, router building, and .kairox/ discovery"
```

### Task 1.3: agent-models integration tests

**Files:**

- Create: `crates/agent-models/tests/integration.rs`
- Check: `crates/agent-models/src/router.rs` (ModelRouter), `crates/agent-models/src/fake.rs` (FakeModelClient)

- [ ] **Step 1: Create integration test with ModelRouter selection**

```rust
// crates/agent-models/tests/integration.rs
use agent_models::{FakeModelClient, ModelClient, ModelRequest, ModelRouter};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn router_selects_correct_client() {
    let mut clients: HashMap<String, Arc<dyn ModelClient>> = HashMap::new();
    clients.insert("gpt4".into(), Arc::new(FakeModelClient::new("GPT-4 response")));
    clients.insert("claude".into(), Arc::new(FakeModelClient::new("Claude response")));

    let router = ModelRouter::new(clients, "gpt4".into());
    let client = router.client_for("gpt4").unwrap();
    let mut stream = client.stream(ModelRequest {
        messages: vec![],
        model_id: "gpt4".into(),
        ..Default::default()
    }).await.unwrap();

    use futures::StreamExt;
    let mut text = String::new();
    while let Some(Ok(event)) = stream.next().await {
        if let agent_models::ModelEvent::TokenDelta(t) = event {
            text.push_str(&t);
        }
    }
    assert!(text.contains("GPT-4"));
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p agent-models --test integration router_selects_correct_client`
Expected: PASS

- [ ] **Step 3: Add model profile switching test and provider smoke tests**

```rust
#[tokio::test]
async fn router_falls_back_to_default_when_unknown() {
    let mut clients: HashMap<String, Arc<dyn ModelClient>> = HashMap::new();
    clients.insert("default".into(), Arc::new(FakeModelClient::new("default")));

    let router = ModelRouter::new(clients, "default".into());
    // Unknown profile should fall back to default
    let client = router.client_for("nonexistent").unwrap();
    assert!(client.stream(Default::default()).await.is_ok());
}

#[tokio::test]
async fn fake_model_produces_tokens() {
    let client = FakeModelClient::new("Hello, world!");
    let mut stream = client.stream(ModelRequest {
        messages: vec![],
        model_id: "fake".into(),
        ..Default::default()
    }).await.unwrap();

    use futures::StreamExt;
    let mut text = String::new();
    while let Some(Ok(event)) = stream.next().await {
        match event {
            agent_models::ModelEvent::TokenDelta(t) => text.push_str(&t),
            agent_models::ModelEvent::Completed { .. } => break,
            _ => {}
        }
    }
    assert!(!text.is_empty());
}
```

- [ ] **Step 4: Run test and commit**

Run: `cargo test -p agent-models --test integration`
Expected: ALL PASS

```
git add crates/agent-models/tests/integration.rs
git commit -m "test(models): add integration tests for ModelRouter selection and provider smoke tests"
```

### Task 1.4: agent-tools integration tests

**Files:**

- Create: `crates/agent-tools/tests/integration.rs`
- Check: `crates/agent-tools/src/registry.rs` (ToolRegistry), `crates/agent-tools/src/permission.rs` (PermissionEngine)

- [ ] **Step 1: Create integration test with full tool pipeline**

```rust
// crates/agent-tools/tests/integration.rs
use agent_tools::{BuiltinProvider, PermissionEngine, PermissionMode, ToolRegistry, Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

struct EchoTool;
#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "echo".into(),
            description: "echoes".into(),
            required_capability: "echo".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }
    async fn execute(&self, _invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        Ok(ToolOutput { content: "echo ok".into(), preview: "echo ok".into() })
    }
}

#[tokio::test]
async fn registry_lists_all_tools_including_custom() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).await;
    let tools = registry.list_all().await;
    assert!(tools.iter().any(|t| t.tool_id == "echo"));
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p agent-tools --test integration registry_lists_all_tools_including_custom`
Expected: PASS

- [ ] **Step 3: Add permission engine tests**

```rust
#[tokio::test]
async fn permission_engine_decide_per_mode() {
    let engine = PermissionEngine::new(PermissionMode::Suggest);
    let def = ToolDefinition {
        tool_id: "fs.read".into(),
        description: "read file".into(),
        required_capability: "read".into(),
        parameters: serde_json::json!({}),
    };

    // ReadOnly mode: read tools allowed
    let engine = PermissionEngine::new(PermissionMode::ReadOnly);
    let result = engine.decide(&def, &ToolInvocation {
        tool_id: "fs.read".into(),
        arguments: serde_json::json!({"path": "/tmp/test"}),
        call_id: "call1".into(),
    }).await;
    assert!(result.is_allowed());

    // ReadOnly mode: shell tool denied
    let shell_def = ToolDefinition {
        tool_id: "shell".into(),
        description: "run shell".into(),
        required_capability: "shell".into(),
        parameters: serde_json::json!({}),
    };
    let result = engine.decide(&shell_def, &ToolInvocation {
        tool_id: "shell".into(),
        arguments: serde_json::json!({"cmd": "rm -rf /"}),
        call_id: "call2".into(),
    }).await;
    assert!(result.is_denied());
}
```

- [ ] **Step 4: Add shell tool smoke test against tempdir**

```rust
#[tokio::test]
async fn shell_tool_executes_trivial_command() {
    let tool = agent_tools::ShellExecTool::new();
    let inv = ToolInvocation {
        tool_id: "shell".into(),
        arguments: serde_json::json!({"command": "echo hello"}),
        call_id: "call3".into(),
    };
    let output = tool.execute(inv).await.unwrap();
    assert!(output.content.contains("hello"));
}
```

- [ ] **Step 5: Add FS tools test with tempdir**

```rust
#[tokio::test]
async fn fs_read_write_list_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let file_path_str = file_path.to_string_lossy().to_string();

    // Write
    let write_tool = agent_tools::FsWriteTool::new(dir.path().to_path_buf());
    let inv = ToolInvocation {
        tool_id: "fs.write".into(),
        arguments: serde_json::json!({"path": "test.txt", "content": "hello world"}),
        call_id: "call4".into(),
    };
    let output = write_tool.execute(inv).await.unwrap();
    assert!(output.content.contains("written") || output.content.contains("Wrote"));

    // Read
    let read_tool = agent_tools::FsReadTool::new(dir.path().to_path_buf());
    let inv = ToolInvocation {
        tool_id: "fs.read".into(),
        arguments: serde_json::json!({"path": "test.txt"}),
        call_id: "call5".into(),
    };
    let output = read_tool.execute(inv).await.unwrap();
    assert!(output.content.contains("hello world"));

    // List
    let list_tool = agent_tools::FsListTool::new(dir.path().to_path_buf());
    let inv = ToolInvocation {
        tool_id: "fs.list".into(),
        arguments: serde_json::json!({"path": "."}),
        call_id: "call6".into(),
    };
    let output = list_tool.execute(inv).await.unwrap();
    assert!(output.content.contains("test.txt"));

    // Reject path traversal
    let inv = ToolInvocation {
        tool_id: "fs.read".into(),
        arguments: serde_json::json!({"path": "../etc/passwd"}),
        call_id: "call7".into(),
    };
    let result = read_tool.execute(inv).await;
    assert!(result.is_err());
}
```

- [ ] **Step 6: Run test and commit**

Run: `cargo test -p agent-tools --test integration`
Expected: ALL PASS

```
git add crates/agent-tools/tests/integration.rs
git commit -m "test(tools): add integration tests for registry, permissions, shell, and filesystem tools"
```

---

## Phase 2: Decompose facade_runtime.rs

### Task 2.1: Extract facade_sessions.rs

**Files:**

- Create: `crates/agent-runtime/src/facade_sessions.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs` (remove session methods, add `mod` + delegation)
- Modify: `crates/agent-runtime/src/lib.rs` (add `pub(crate) mod facade_sessions`)

- [ ] **Step 1: Identify session-related methods in facade_runtime.rs**

Read the current file and locate the following methods within `impl<S, M> AppFacade for LocalRuntime<S, M>`:

- `open_workspace` (~line 779)
- `start_session` (~line 783)
- `cancel_session` (~line 911)
- `get_session_projection` (~line 926)
- `get_trace` (~line 933)
- `list_workspaces` (~line 945)
- `list_sessions` (~line 949)
- `rename_session` (~line 956)
- `soft_delete_session` (~line 964)
- `cleanup_expired_sessions` (~line 968)
- `mark_session_visible` (~line 312, in `impl<S, M> LocalRuntime<S, M>`)

Also identify the `#[cfg(test)]` tests related to these methods.

- [ ] **Step 2: Create facade_sessions.rs with extracted methods**

```rust
// crates/agent-runtime/src/facade_sessions.rs
use crate::facade_runtime::LocalRuntime;
use crate::session::SessionState;
use agent_core::{
    AppFacade, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification,
    SendMessageRequest, SessionId, SessionMeta, StartSessionRequest,
    TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_memory::MemoryStore;
use agent_store::EventStore;
use std::sync::Arc;
use tokio::sync::Mutex;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn mark_session_visible(
        &self,
        session_id: &SessionId,
        project_id: Option<agent_core::ProjectId>,
        visibility: agent_core::ProjectSessionVisibility,
    ) -> agent_core::Result<()> {
        // [COPY the entire method body from facade_runtime.rs ~line 312]
    }
}

#[async_trait::async_trait]
impl<S, M> AppFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn open_workspace(&self, path: String) -> agent_core::Result<WorkspaceInfo> {
        // [COPY the entire method body from facade_runtime.rs]
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        // [COPY the entire method body]
    }

    async fn cancel_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        // [COPY the entire method body]
    }

    async fn get_session_projection(&self, session_id: &SessionId) -> agent_core::Result<agent_core::projection::SessionProjection> {
        // [COPY the entire method body]
    }

    async fn get_trace(&self, session_id: SessionId) -> agent_core::Result<Vec<TraceEntry>> {
        // [COPY the entire method body]
    }

    async fn list_workspaces(&self) -> agent_core::Result<Vec<WorkspaceInfo>> {
        // [COPY the entire method body]
    }

    async fn list_sessions(&self, workspace_id: &WorkspaceId) -> agent_core::Result<Vec<SessionMeta>> {
        // [COPY the entire method body]
    }

    async fn rename_session(&self, session_id: &SessionId, title: String) -> agent_core::Result<()> {
        // [COPY the entire method body]
    }

    async fn soft_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        // [COPY the entire method body]
    }

    async fn cleanup_expired_sessions(&self, older_than: std::time::Duration) -> agent_core::Result<()> {
        // [COPY the entire method body]
    }
}
```

> **Note for agentic worker**: Steps marked `[COPY the entire method body]` require reading the current method from `facade_runtime.rs` and moving it verbatim. The method signatures are shown above to identify which methods to move. Search for the method name in `facade_runtime.rs` to find the exact body. Do NOT modify the method bodies during extraction.

- [ ] **Step 3: Move session-related tests to facade_sessions.rs**

Add a `#[cfg(test)] mod tests { ... }` block at the bottom of `facade_sessions.rs`. Move the tests from `facade_runtime.rs` that exercise session methods. Each test should be copied verbatim.

- [ ] **Step 4: Add `mod facade_sessions` to lib.rs**

In `crates/agent-runtime/src/lib.rs`, add:

```rust
pub(crate) mod facade_sessions;
```

- [ ] **Step 5: Remove session methods from facade_runtime.rs**

Delete the moved methods from `facade_runtime.rs`. Delete the moved tests. Verify there are no duplicate method definitions.

- [ ] **Step 6: Compile and test**

Run: `cargo build -p agent-runtime 2>&1 | head -20`
Expected: Compiles without error

Run: `cargo test -p agent-runtime`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```
git add crates/agent-runtime/src/facade_sessions.rs crates/agent-runtime/src/facade_runtime.rs crates/agent-runtime/src/lib.rs
git commit -m "refactor(runtime): extract session management into facade_sessions.rs"
```

### Task 2.2: Extract facade_skills.rs

> **Follows same pattern as Task 2.1.** Create `facade_skills.rs`, move methods verbatim, move tests, add `mod` declaration to `lib.rs`, remove from `facade_runtime.rs`, verify `cargo test -p agent-runtime` passes.

**Files:**

- Create: `crates/agent-runtime/src/facade_skills.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs` (remove skills methods)
- Modify: `crates/agent-runtime/src/lib.rs` (add `pub(crate) mod facade_skills`)

- [ ] **Step 1: Create facade_skills.rs**

Create the file with the following `impl AppFacade` block containing these methods (copied verbatim from facade_runtime.rs):

- `list_skills`
- `get_skill`
- `activate_skill`
- `deactivate_skill`
- `list_active_skills`
- `list_skill_settings`
- `get_skill_settings_detail`
- `set_skill_enabled`
- `delete_skill_settings`
- `search_remote_skills`
- `install_remote_skill`
- `install_github_skill`
- `update_skill`
- `list_skill_catalog`
- `list_skill_sources`
- `add_skill_source`
- `remove_skill_source`
- `set_skill_source_enabled`
- `refresh_skill_catalog`

And any private helper methods these call (e.g., `skill_document_to_detail`, `skill_metadata_to_view`, `skill_metadata_to_active_view` from `crate::skills` — note these are already in `crate::skills` module, not in facade_runtime.rs).

- [ ] **Step 2: Move tests and remove from facade_runtime.rs**

- [ ] **Step 3: Build, test, commit**

Run: `cargo test -p agent-runtime`
Expected: ALL PASS

```
git add crates/agent-runtime/src/facade_skills.rs crates/agent-runtime/src/facade_runtime.rs crates/agent-runtime/src/lib.rs
git commit -m "refactor(runtime): extract skills management into facade_skills.rs"
```

### Task 2.3: Extract facade_mcp.rs

> **Follows same pattern as Task 2.1.** Create, move verbatim, move tests, declare mod, remove, verify.

**Files:**

- Create: `crates/agent-runtime/src/facade_mcp.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/src/lib.rs`

- [ ] **Step 1: Create facade_mcp.rs**

Extract methods:

- `list_mcp_server_settings`, `upsert_mcp_server_settings`, `delete_mcp_server_settings`, `set_mcp_server_enabled`, `open_mcp_config_file`
- `list_catalog`, `get_catalog_entry`, `refresh_catalog`, `install_catalog_entry`, `uninstall_catalog_entry`, `list_installed_entries`
- `list_catalog_sources`, `add_catalog_source`, `remove_catalog_source`, `set_catalog_source_enabled`

- [ ] **Step 2: Move tests, remove from facade_runtime.rs**

- [ ] **Step 3: Build, test, commit**

Run: `cargo test -p agent-runtime`
Expected: ALL PASS

```
git add crates/agent-runtime/src/facade_mcp.rs crates/agent-runtime/src/facade_runtime.rs crates/agent-runtime/src/lib.rs
git commit -m "refactor(runtime): extract MCP management into facade_mcp.rs"
```

### Task 2.4: Extract facade_projects.rs

> **Follows same pattern as Task 2.1.** Create, move verbatim, move tests, declare mod, remove, verify.

**Files:**

- Create: `crates/agent-runtime/src/facade_projects.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/src/lib.rs`

- [ ] **Step 1: Create facade_projects.rs**

Extract methods:

- `list_projects`, `create_blank_project`, `add_existing_project`, `rename_project`, `remove_project`
- `restore_project_session`, `update_project_order`, `update_project_expanded`
- `create_project_draft_session`, `create_project_worktree_session`

- [ ] **Step 2: Move tests, remove from facade_runtime.rs**

- [ ] **Step 3: Build, test, commit**

Run: `cargo test -p agent-runtime`
Expected: ALL PASS

```
git add crates/agent-runtime/src/facade_projects.rs crates/agent-runtime/src/facade_runtime.rs crates/agent-runtime/src/lib.rs
git commit -m "refactor(runtime): extract project management into facade_projects.rs"
```

### Task 2.5: Verify facade_runtime.rs size reduction

**Files:**

- Check: `crates/agent-runtime/src/facade_runtime.rs`

- [ ] **Step 1: Verify facade_runtime.rs line count**

Run: `wc -l crates/agent-runtime/src/facade_runtime.rs`
Expected: ~600–800 lines (struct + builder + core ops + delegation + remaining tests)

- [ ] **Step 2: Run full CI gate**

Run: `cargo test --workspace && just check-types`
Expected: ALL PASS, no type changes

- [ ] **Step 3: Commit final verification**

```
git add crates/agent-runtime/src/facade_runtime.rs
git commit -m "chore(runtime): verify facade_runtime.rs size reduction after extractions"
```

---

## Phase 3: Decompose agent_loop.rs

### Task 3.1: Create agent_loop/ directory and extract budget.rs + messages.rs

**Files:**

- Create: `crates/agent-runtime/src/agent_loop/mod.rs`
- Create: `crates/agent-runtime/src/agent_loop/budget.rs`
- Create: `crates/agent-runtime/src/agent_loop/messages.rs`
- Delete: `crates/agent-runtime/src/agent_loop.rs`
- Modify: `crates/agent-runtime/src/lib.rs` (`mod agent_loop;` → `pub(crate) mod agent_loop;`)

- [ ] **Step 1: Create agent_loop/mod.rs as re-export hub**

```rust
// crates/agent-runtime/src/agent_loop/mod.rs
mod budget;
mod messages;
mod runner;
mod tool_loop;

pub(crate) use budget::{build_model_messages_within_budget, should_trigger_auto_compaction};
pub(crate) use messages::build_model_messages;
pub(crate) use runner::run_agent_loop;

pub(crate) const SYSTEM_PROMPT: &str = "\
You are Kairox, a helpful AI assistant with memory capabilities.\n\n\
## Memory Protocol\n\
When you learn something worth remembering about the user or workspace, \
use <memory> tags to save it. Examples:\n\
- <memory scope=\"session\">Temporary note for this session</memory>\n\
- <memory scope=\"user\" key=\"preferred-language\">User prefers Rust</memory>\n\
- <memory scope=\"workspace\" key=\"build-cmd\">Use cargo nextest</memory>\n\n\
Guidelines:\n\
- Use scope=\"session\" for temporary notes (auto-accepted)\n\
- Use scope=\"user\" for user preferences (requires approval)\n\
- Use scope=\"workspace\" for project settings (requires approval)\n\
- Always include a key when using user or workspace scope\n\
- You may include multiple <memory> tags in one response\n\
- The <memory> tags will be stripped from displayed output, so also state \
the information naturally in your response.\n\
";

pub(crate) const MAX_AGENT_LOOP_ITERATIONS: usize = 20;
```

- [ ] **Step 2: Create agent_loop/budget.rs**

Move the following from `agent_loop.rs`:

- `should_trigger_auto_compaction` (lines 1166–1175)
- `build_model_messages_within_budget` (lines 1188–1238)
- The related unit tests from `mod tests` (lines 1346–1492)

```rust
// crates/agent-runtime/src/agent_loop/budget.rs
use agent_models::ModelMessage;

/// Returns true when automatic compaction should fire.
pub(crate) fn should_trigger_auto_compaction(
    usage: &agent_core::ContextUsage,
    threshold: f32,
    already_compacting: bool,
) -> bool {
    // [COPY body from agent_loop.rs:1166-1175]
}

/// Builds model messages trimmed to fit within budget_tokens.
pub(crate) fn build_model_messages_within_budget(
    user_content: &str,
    session_events: &[agent_core::DomainEvent],
    budget_tokens: u64,
) -> Vec<ModelMessage> {
    // [COPY body from agent_loop.rs:1188-1238]
}

#[cfg(test)]
mod tests {
    use super::*;
    // [COPY budget-related tests from agent_loop.rs mod tests]
}
```

- [ ] **Step 3: Create agent_loop/messages.rs**

Move `build_model_messages` (lines 44–271 of agent_loop.rs) and its helper functions into this file with the tests.

- [ ] **Step 4: Update agent_loop.rs import**

Temporarily keep `agent_loop.rs` but change its contents to `pub mod agent_loop { ... }` re-exports so existing imports don't break. Then in the next task we'll replace with the full module.

Actually, simpler approach: create the `agent_loop/` directory, make `agent_loop.rs` empty, then the compiler will use `agent_loop/mod.rs`. But we need to handle the `pub mod agent_loop;` in `lib.rs`.

In `crates/agent-runtime/src/lib.rs`, the current declaration is likely `pub(crate) mod agent_loop;`. When we replace `agent_loop.rs` with `agent_loop/mod.rs`, the same `mod` declaration works.

- [ ] **Step 5: Move agent_loop.rs to agent_loop/old.rs (backup), create agent_loop/mod.rs**

```
mv crates/agent-runtime/src/agent_loop.rs crates/agent-runtime/src/agent_loop_old.rs
```

- [ ] **Step 6: Build, test, fix compilation errors**

Run: `cargo build -p agent-runtime 2>&1 | head -30`
Fix any missing imports or re-export issues.
Run: `cargo test -p agent-runtime`
Ensure ALL tests pass.

- [ ] **Step 7: Remove backup and commit**

```
rm crates/agent-runtime/src/agent_loop_old.rs
```

```
git add crates/agent-runtime/src/agent_loop/ crates/agent-runtime/src/lib.rs
git rm crates/agent-runtime/src/agent_loop.rs
git commit -m "refactor(runtime): extract agent_loop budget and messages into submodules"
```

### Task 3.2: Extract agent_loop/tool_loop.rs

**Files:**

- Create: `crates/agent-runtime/src/agent_loop/tool_loop.rs`
- Modify: `crates/agent-runtime/src/agent_loop/runner.rs` (create if not exists)

- [ ] **Step 1: Identify tool loop code in the original agent_loop.rs**

The `run_agent_loop` function (lines 329–1164) contains an inner tool-calling loop. The tool execution section handles:

1. Sending model request
2. Receiving streaming response
3. Collecting tool calls
4. Executing tools via registry
5. Feeding tool results back to model

Extract the tool execution portion into `agent_loop/tool_loop.rs` as a helper function:

```rust
// crates/agent-runtime/src/agent_loop/tool_loop.rs
use agent_core::{DomainEvent, EventPayload, AgentId, PrivacyClassification};
use agent_models::{ModelClient, ToolCall};
use agent_tools::{ToolRegistry, PermissionEngine, ToolInvocation};
use agent_store::EventStore;
use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) struct ToolLoopResult {
    pub(crate) should_continue: bool,
    pub(crate) tool_results: Vec<(String, String)>, // (tool_call_id, output)
}

pub(crate) async fn execute_tool_calls<S: EventStore + 'static>(
    tool_calls: Vec<ToolCall>,
    tool_registry: &Arc<Mutex<ToolRegistry>>,
    permission_engine: &Arc<Mutex<PermissionEngine>>,
    store: &Arc<S>,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    workspace_id: &agent_core::WorkspaceId,
    session_id: &agent_core::SessionId,
    pending_permissions: &Arc<Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>>,
) -> agent_core::Result<ToolLoopResult> {
    // [EXTRACT the tool execution portion from run_agent_loop]
}
```

> **Note for agentic worker**: This extraction requires careful reading of `run_agent_loop` in the original `agent_loop.rs`. The tool-calling section is in the inner `while` loop that processes model responses, collects `ToolCallRequested` events, executes them, and feeds results back. Extract this section as `execute_tool_calls` while keeping the orchestration in `runner.rs`.

- [ ] **Step 2: Create agent_loop/runner.rs with the main loop**

```rust
// crates/agent-runtime/src/agent_loop/runner.rs
// [Copy the run_agent_loop function, now calling into the extracted modules]
```

- [ ] **Step 3: Build, test, commit**

Run: `cargo test -p agent-runtime`
Expected: ALL PASS

```
git add crates/agent-runtime/src/agent_loop/
git commit -m "refactor(runtime): extract tool execution loop into agent_loop submodule"
```

### Task 3.3: Finalize agent_loop/ decomposition

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop/mod.rs` (update re-exports)
- Modify: `crates/agent-runtime/src/agent_loop/runner.rs` (final cleanup)

- [ ] **Step 1: Ensure mod.rs re-exports are clean**

```rust
// crates/agent-runtime/src/agent_loop/mod.rs
mod budget;
mod messages;
mod runner;
mod tool_loop;

pub(crate) use budget::{build_model_messages_within_budget, should_trigger_auto_compaction};
pub(crate) use messages::build_model_messages;
pub(crate) use runner::run_agent_loop;
```

Verify that `crate::agent_loop::run_agent_loop`, `crate::agent_loop::build_model_messages`, etc. all resolve from callers in `facade_runtime.rs`.

- [ ] **Step 2: Run full CI gate**

Run: `cargo test --workspace && just check-types`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```
git add crates/agent-runtime/src/agent_loop/
git commit -m "refactor(runtime): finalize agent_loop decomposition into 4 focused submodules"
```

---

## Phase 4: Split the AppFacade trait

### Task 4.1: Define sub-traits

**Files:**

- Create: `crates/agent-core/src/facade/session.rs`
- Create: `crates/agent-core/src/facade/skills.rs`
- Create: `crates/agent-core/src/facade/mcp.rs`
- Create: `crates/agent-core/src/facade/project.rs`
- Modify: `crates/agent-core/src/facade.rs` (replace monolithic trait with supertrait composition)

- [ ] **Step 1: Create agent-core/src/facade/session.rs**

```rust
// crates/agent-core/src/facade/session.rs
use crate::projection::SessionProjection;
use crate::{DomainEvent, PermissionDecision, SendMessageRequest, SessionId, SessionMeta,
    StartSessionRequest, TaskId, TraceEntry, WorkspaceId, WorkspaceInfo};
use async_trait::async_trait;
use futures::stream::BoxStream;

#[async_trait]
pub trait SessionFacade: Send + Sync {
    async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo>;
    async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId>;
    async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()>;
    async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()>;
    async fn cancel_session(&self, session_id: &SessionId) -> crate::Result<()>;
    async fn get_session_projection(&self, session_id: &SessionId) -> crate::Result<SessionProjection>;
    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>>;
    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>>;
    async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>>;
    async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()>;
    async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()>;
    async fn cleanup_expired_sessions(&self, older_than: std::time::Duration) -> crate::Result<()>;
    fn subscribe_all(&self) -> BoxStream<'static, DomainEvent>;
    async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<crate::TaskGraphSnapshot>;
    async fn retry_task(&self, session_id: SessionId, task_id: TaskId) -> crate::Result<()>;
    async fn cancel_task(&self, session_id: SessionId, task_id: TaskId) -> crate::Result<()>;
    async fn get_agent_status(&self, session_id: SessionId) -> crate::Result<Vec<crate::AgentStatusInfo>>;
}
```

> **Note for agentic worker**: The exact method signatures must match the current `AppFacade` trait in `crates/agent-core/src/facade.rs`. Read the current trait to get every method signature exactly. Copy them verbatim into the appropriate sub-trait file. The method list above is a guide — the actual methods are in `facade.rs`.

- [ ] **Step 2: Create agent-core/src/facade/skills.rs**

Move all skill-related method signatures from `AppFacade` into `SkillsFacade`:

- `list_skills`, `get_skill`, `activate_skill`, `deactivate_skill`, `list_active_skills`
- `list_skill_settings`, `get_skill_settings_detail`, `set_skill_enabled`, `delete_skill_settings`
- `search_remote_skills`, `install_remote_skill`, `install_github_skill`, `update_skill`
- `list_skill_catalog`, `list_skill_sources`, `add_skill_source`, `remove_skill_source`, `set_skill_source_enabled`, `refresh_skill_catalog`

- [ ] **Step 3: Create agent-core/src/facade/mcp.rs**

Move all MCP-related method signatures into `McpFacade`:

- `list_mcp_server_settings`, `upsert_mcp_server_settings`, `delete_mcp_server_settings`, `set_mcp_server_enabled`, `open_mcp_config_file`
- `list_catalog`, `get_catalog_entry`, `refresh_catalog`, `install_catalog_entry`, `uninstall_catalog_entry`, `list_installed_entries`
- `list_catalog_sources`, `add_catalog_source`, `remove_catalog_source`, `set_catalog_source_enabled`

- [ ] **Step 4: Create agent-core/src/facade/project.rs**

Move all project-related method signatures into `ProjectFacade`:

- `list_projects`, `create_blank_project`, `add_existing_project`, `rename_project`, `remove_project`
- `restore_project_session`, `update_project_order`, `update_project_expanded`
- `create_project_draft_session`, `create_project_worktree_session`
- `get_project_git_status`

- [ ] **Step 5: Build, test, commit**

Run: `cargo build --workspace 2>&1 | head -30`
Expected: Need to update `facade.rs` and all impl blocks after defining sub-traits.

Run: `cargo test --workspace`
Expected: ALL PASS after fixes.

```
git add crates/agent-core/src/facade/
git commit -m "refactor(core): split AppFacade into SessionFacade, SkillsFacade, McpFacade, ProjectFacade"
```

### Task 4.2: Update AppFacade to be a supertrait

**Files:**

- Modify: `crates/agent-core/src/facade.rs`

- [ ] **Step 1: Rewrite facade.rs to compose sub-traits**

```rust
// crates/agent-core/src/facade.rs
mod session;
mod skills;
mod mcp;
mod project;

pub use session::SessionFacade;
pub use skills::SkillsFacade;
pub use mcp::McpFacade;
pub use project::ProjectFacade;

// Keep existing DTOs (CatalogQuery, ServerEntry, InstallRequest, etc.) here

/// AppFacade is the complete application facade, combining all sub-traits.
#[async_trait::async_trait]
pub trait AppFacade: SessionFacade + SkillsFacade + McpFacade + ProjectFacade {}
```

- [ ] **Step 2: Verify all existing code compiles**

Run: `cargo build --workspace`
Expected: All `Arc<dyn AppFacade>` usage still compiles because `AppFacade: SessionFacade + ...` means any `AppFacade` implementor also implements all sub-traits.

- [ ] **Step 3: Update type generation**

Run: `just gen-types`
Run: `just check-types`
Expected: No type changes detected

- [ ] **Step 4: Run full CI gate and commit**

Run: `cargo test --workspace`
Expected: ALL PASS

```
git add crates/agent-core/src/facade.rs crates/agent-core/src/facade/
git commit -m "refactor(core): AppFacade becomes supertrait composing SessionFacade, SkillsFacade, McpFacade, ProjectFacade"
```

### Task 4.3: Update GUI/TUI consumers (optional, gradual)

**Files:**

- May modify: `crates/agent-tui/src/` (use sub-traits where only session access needed)
- May modify: `apps/agent-gui/src-tauri/src/` (use sub-traits where appropriate)

- [ ] **Step 1: Identify consumers that only need one sub-trait**

Search for `Arc<dyn AppFacade>` in the codebase. For each location, check which methods are actually called. If only `SessionFacade` methods are used, narrow the type to `Arc<dyn SessionFacade>`.

Run: `rg "AppFacade" crates/agent-tui/src/ apps/agent-gui/src-tauri/src/`

- [ ] **Step 2: Narrow types where safe**

Update select files to use `Arc<dyn SessionFacade>` instead of `Arc<dyn AppFacade>` where only session methods are called. This is a gradual, optional step — `AppFacade` supertrait still works everywhere.

- [ ] **Step 3: Build, test, commit**

Run: `cargo test --workspace && just check-types`
Expected: ALL PASS

```
git add -u
git commit -m "refactor(gui,tui): narrow facade type bounds to specific sub-traits where possible"
```

---

## Final Verification

- [ ] **Step 1: Run full CI gate**

```
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
just check-types
just test-gui
```

Expected: ALL PASS, zero warnings

- [ ] **Step 2: Verify file size targets**

```
wc -l crates/agent-runtime/src/facade_runtime.rs
# Expected: ~600–800 lines
wc -l crates/agent-runtime/src/agent_loop/*.rs
# Expected: each submodule <500 lines
find crates/agent-runtime/src -name "facade_*.rs" -exec wc -l {} +
# Expected: each facade file <500 lines
```

- [ ] **Step 3: Final commit**

```
git add -A
git commit -m "chore: final verification of refactor and test hardening"
```
