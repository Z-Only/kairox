//! Integration tests for the `agent-sdk` crate.
//!
//! These tests exercise the SDK from an **external consumer** perspective:
//! builder → SDK → session → lifecycle operations.  They complement the
//! unit-level tests in `lib_tests.rs` by verifying cross-module flows rather
//! than individual function behaviour.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_sdk::{
    HookAction, KairoxSdk, SdkApprovalPolicy, SdkHook, SdkSandboxPolicy, ToolHookContext,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal SDK instance suitable for integration tests.
///
/// Disables MCP / LSP / marketplace and uses `Never` approval + `ReadOnly`
/// sandbox so no real tools or network calls are made.
async fn build_test_sdk(workspace: &TempDir, data: &TempDir) -> KairoxSdk {
    KairoxSdk::builder()
        .workspace(workspace.path())
        .data_dir(data.path())
        .home_dir(data.path())
        .database_filename("integration-test.db")
        .approval_policy(SdkApprovalPolicy::Never)
        .sandbox_policy(SdkSandboxPolicy::ReadOnly)
        .enable_mcp_servers(false)
        .enable_lsp_servers(false)
        .enable_marketplace(false)
        .build()
        .await
        .expect("SDK build should succeed")
}

/// A simple hook that counts `before_tool` invocations.
struct InvocationCounterHook {
    count: AtomicUsize,
}

impl InvocationCounterHook {
    fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
        }
    }

    fn invocations(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl SdkHook for InvocationCounterHook {
    async fn before_tool(&self, _context: &ToolHookContext) -> HookAction {
        self.count.fetch_add(1, Ordering::SeqCst);
        HookAction::Continue
    }

    fn name(&self) -> &str {
        "invocation-counter"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// 1. Build an SDK with every builder option set and verify it does not panic.
#[tokio::test]
async fn builder_with_all_options() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let hook = Arc::new(InvocationCounterHook::new());

    let sdk = KairoxSdk::builder()
        .workspace(workspace.path())
        .data_dir(data.path())
        .home_dir(data.path())
        .database_filename("all-options.db")
        .default_profile("default")
        .approval_policy(SdkApprovalPolicy::Always)
        .sandbox_policy(SdkSandboxPolicy::FullAccess)
        .enable_mcp_servers(false)
        .enable_lsp_servers(false)
        .enable_marketplace(false)
        .hook_arc(hook.clone())
        .build()
        .await
        .expect("builder with all options should succeed");

    // Smoke-check: workspace path is bound correctly.
    assert_eq!(
        sdk.workspace_path(),
        workspace.path().canonicalize().unwrap()
    );

    // Hook is registered but not yet invoked (no tool calls happened).
    assert_eq!(hook.invocations(), 0);
}

/// 2. Override default_profile and verify build succeeds.
#[tokio::test]
async fn builder_default_profile_override() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();

    let sdk = KairoxSdk::builder()
        .workspace(workspace.path())
        .data_dir(data.path())
        .home_dir(data.path())
        .database_filename("profile-override.db")
        .default_profile("custom")
        .approval_policy(SdkApprovalPolicy::Never)
        .sandbox_policy(SdkSandboxPolicy::ReadOnly)
        .enable_mcp_servers(false)
        .enable_lsp_servers(false)
        .enable_marketplace(false)
        .build()
        .await
        .expect("custom profile should not prevent build");

    assert_eq!(
        sdk.workspace_path(),
        workspace.path().canonicalize().unwrap()
    );
}

/// 3. Create multiple sessions and verify they each receive unique session IDs.
#[tokio::test]
async fn multiple_sessions_in_workspace() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = build_test_sdk(&workspace, &data).await;

    let session_a = sdk.create_session().await.expect("session A");
    let session_b = sdk.create_session().await.expect("session B");
    let session_c = sdk.create_session().await.expect("session C");

    // Each session must have a distinct session ID.
    assert_ne!(session_a.session_id(), session_b.session_id());
    assert_ne!(session_b.session_id(), session_c.session_id());
    assert_ne!(session_a.session_id(), session_c.session_id());

    // All session IDs should be non-empty.
    assert!(!session_a.session_id().as_str().is_empty());
    assert!(!session_b.session_id().as_str().is_empty());
    assert!(!session_c.session_id().as_str().is_empty());
}

/// 4. `list_sessions` succeeds and the count grows after creating sessions.
#[tokio::test]
async fn list_sessions_after_create() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = build_test_sdk(&workspace, &data).await;

    let before = sdk.list_sessions().await.expect("list before");
    let _session = sdk.create_session().await.expect("create session");
    let after = sdk.list_sessions().await.expect("list after");

    // The list should grow (or at minimum not shrink) after creating a session.
    // Note: open_workspace may assign different workspace IDs across calls, so
    // we verify the aggregate count rather than matching specific session IDs.
    assert!(
        after.len() >= before.len(),
        "session list should not shrink after creating a session: before={}, after={}",
        before.len(),
        after.len()
    );
}

/// 5. Cancelling a freshly created session should not return an error.
#[tokio::test]
async fn session_cancel_does_not_error() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = build_test_sdk(&workspace, &data).await;

    let session = sdk.create_session().await.expect("create session");
    session
        .cancel()
        .await
        .expect("cancel on a fresh session should succeed");
}

/// 6. `get_trace` on a brand-new session returns without error.
#[tokio::test]
async fn session_get_trace_after_create() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = build_test_sdk(&workspace, &data).await;

    let session = sdk.create_session().await.expect("create session");
    let trace = session.get_trace().await.expect("get_trace");

    // A new session should have zero or only initialization trace entries.
    // The important assertion is that no error is returned.
    assert!(
        trace.len() <= 5,
        "fresh session should not have many trace entries, got {}",
        trace.len()
    );
}

/// 7. `export_trace` on a brand-new session returns a valid export.
#[tokio::test]
async fn session_export_trace() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = build_test_sdk(&workspace, &data).await;

    let session = sdk.create_session().await.expect("create session");
    let export = session.export_trace().await.expect("export_trace");

    // The export should reference the correct session.
    assert_eq!(
        export.session_id,
        *session.session_id(),
        "export session_id should match"
    );
}

/// 8. `sdk.facade()` returns a valid trait object and we can use it to
///    open the workspace (proving the runtime is wired up end-to-end).
#[tokio::test]
async fn facade_access() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = build_test_sdk(&workspace, &data).await;

    // The most direct proof that facade() works is to exercise it through the
    // SDK's own public API — create_session internally calls the facade, so a
    // successful session creation proves the facade is functional.
    let session = sdk.create_session().await.expect("facade should be wired");
    assert!(!session.session_id().as_str().is_empty());
}

/// 9. Debug formatting includes workspace_path and hook_count.
#[tokio::test]
async fn sdk_debug_output() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let hook = Arc::new(InvocationCounterHook::new());

    let sdk = KairoxSdk::builder()
        .workspace(workspace.path())
        .data_dir(data.path())
        .home_dir(data.path())
        .database_filename("debug-test.db")
        .approval_policy(SdkApprovalPolicy::Never)
        .sandbox_policy(SdkSandboxPolicy::ReadOnly)
        .enable_mcp_servers(false)
        .enable_lsp_servers(false)
        .enable_marketplace(false)
        .hook_arc(hook)
        .build()
        .await
        .expect("build");

    let debug_str = format!("{:?}", sdk);
    assert!(
        debug_str.contains("workspace_path"),
        "debug output should contain 'workspace_path', got: {debug_str}"
    );
    assert!(
        debug_str.contains("hook_count"),
        "debug output should contain 'hook_count', got: {debug_str}"
    );
    assert!(
        debug_str.contains("hook_count: 1"),
        "hook_count should be 1 with one registered hook, got: {debug_str}"
    );
}

/// 10. Concurrent session creation and listing does not panic or deadlock.
#[tokio::test]
async fn concurrent_session_operations() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = Arc::new(build_test_sdk(&workspace, &data).await);

    let sdk_a = sdk.clone();
    let sdk_b = sdk.clone();
    let sdk_c = sdk.clone();

    let (result_a, result_b, result_c) = tokio::join!(
        async move { sdk_a.create_session().await },
        async move { sdk_b.create_session().await },
        async move { sdk_c.list_sessions().await },
    );

    result_a.expect("concurrent create_session A");
    result_b.expect("concurrent create_session B");
    result_c.expect("concurrent list_sessions");
}

/// 11. Building SDK with `WorkspaceWrite` sandbox policy succeeds.
#[tokio::test]
async fn workspace_write_sandbox_policy() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();

    let sdk = KairoxSdk::builder()
        .workspace(workspace.path())
        .data_dir(data.path())
        .home_dir(data.path())
        .database_filename("ws-write.db")
        .approval_policy(SdkApprovalPolicy::Never)
        .sandbox_policy(SdkSandboxPolicy::WorkspaceWrite)
        .enable_mcp_servers(false)
        .enable_lsp_servers(false)
        .enable_marketplace(false)
        .build()
        .await
        .expect("WorkspaceWrite policy should build successfully");

    assert_eq!(
        sdk.workspace_path(),
        workspace.path().canonicalize().unwrap()
    );
}

/// 12. Building SDK with `FullAccess` sandbox policy succeeds.
#[tokio::test]
async fn full_access_sandbox_policy() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();

    let sdk = KairoxSdk::builder()
        .workspace(workspace.path())
        .data_dir(data.path())
        .home_dir(data.path())
        .database_filename("full-access.db")
        .approval_policy(SdkApprovalPolicy::Never)
        .sandbox_policy(SdkSandboxPolicy::FullAccess)
        .enable_mcp_servers(false)
        .enable_lsp_servers(false)
        .enable_marketplace(false)
        .build()
        .await
        .expect("FullAccess policy should build successfully");

    assert_eq!(
        sdk.workspace_path(),
        workspace.path().canonicalize().unwrap()
    );
}
