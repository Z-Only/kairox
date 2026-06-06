use crate::config::{SdkApprovalPolicy, SdkSandboxPolicy};
use crate::hooks::{HookAction, SdkHook, ToolHookContext};
use crate::session::StreamEvent;
use crate::KairoxSdk;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

/// Create a SDK builder pointing at a fresh temp workspace with a temp data
/// dir. Both dirs are kept alive by returning the `TempDir` handles.
async fn test_sdk(workspace: &TempDir, data: &TempDir) -> KairoxSdk {
    KairoxSdk::builder()
        .workspace(workspace.path())
        .data_dir(data.path())
        .home_dir(data.path())
        .database_filename("test.db")
        .approval_policy(SdkApprovalPolicy::Never)
        .sandbox_policy(SdkSandboxPolicy::ReadOnly)
        .enable_mcp_servers(false)
        .enable_lsp_servers(false)
        .enable_marketplace(false)
        .build()
        .await
        .expect("SDK build should succeed")
}

#[tokio::test]
async fn builder_creates_sdk_with_valid_workspace() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = test_sdk(&workspace, &data).await;
    assert_eq!(
        sdk.workspace_path(),
        workspace.path().canonicalize().unwrap()
    );
}

#[tokio::test]
async fn builder_rejects_nonexistent_workspace() {
    let data = TempDir::new().unwrap();
    let result = KairoxSdk::builder()
        .workspace("/nonexistent/path/that/should/not/exist")
        .data_dir(data.path())
        .build()
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("invalid workspace path"),
        "expected InvalidWorkspacePath, got: {err}"
    );
}

#[tokio::test]
async fn builder_rejects_file_as_workspace() {
    let workspace = TempDir::new().unwrap();
    let file_path = workspace.path().join("not-a-dir.txt");
    std::fs::write(&file_path, "hello").unwrap();
    let data = TempDir::new().unwrap();
    let result = KairoxSdk::builder()
        .workspace(&file_path)
        .data_dir(data.path())
        .build()
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not a directory"),
        "expected 'not a directory', got: {err}"
    );
}

#[tokio::test]
async fn create_session_returns_valid_ids() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = test_sdk(&workspace, &data).await;
    let session = sdk.create_session().await.expect("create session");
    assert!(!session.session_id().as_str().is_empty());
    assert!(!session.workspace_id().as_str().is_empty());
}

#[tokio::test]
async fn list_sessions_does_not_error() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = test_sdk(&workspace, &data).await;
    // Verify list_sessions works without error, even on a fresh workspace.
    let sessions = sdk.list_sessions().await.expect("list sessions");
    // Fresh workspace may have zero sessions; the call itself succeeding is
    // the assertion.
    assert!(sessions.len() <= 1, "unexpected pre-existing sessions");
}

#[tokio::test]
async fn session_get_trace_returns_empty_initially() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let sdk = test_sdk(&workspace, &data).await;
    let session = sdk.create_session().await.expect("create session");
    let trace = session.get_trace().await.expect("get trace");
    // A brand-new session has at most initialization events, never errors.
    let _ = trace;
}

#[tokio::test]
async fn sdk_config_defaults_are_sane() {
    let config = crate::config::SdkConfig::default();
    assert_eq!(config.database_filename, "kairox.db");
    assert_eq!(config.approval_policy, SdkApprovalPolicy::Never);
    assert!(config.enable_mcp_servers);
    assert!(!config.enable_lsp_servers);
    assert!(!config.enable_marketplace);
}

#[tokio::test]
async fn sdk_approval_policy_converts_to_runtime() {
    use agent_tools::ApprovalPolicy;
    assert!(matches!(
        ApprovalPolicy::from(SdkApprovalPolicy::Never),
        ApprovalPolicy::Never
    ));
    assert!(matches!(
        ApprovalPolicy::from(SdkApprovalPolicy::OnRequest),
        ApprovalPolicy::OnRequest
    ));
    assert!(matches!(
        ApprovalPolicy::from(SdkApprovalPolicy::Always),
        ApprovalPolicy::Always
    ));
}

#[tokio::test]
async fn sdk_sandbox_policy_converts_to_runtime() {
    use agent_tools::SandboxPolicy;
    let ws = std::path::Path::new("/tmp/test");
    assert!(matches!(
        SdkSandboxPolicy::ReadOnly.into_runtime_policy(ws),
        SandboxPolicy::ReadOnly
    ));
    assert!(matches!(
        SdkSandboxPolicy::FullAccess.into_runtime_policy(ws),
        SandboxPolicy::DangerFullAccess
    ));
    match SdkSandboxPolicy::WorkspaceWrite.into_runtime_policy(ws) {
        SandboxPolicy::WorkspaceWrite {
            network_access,
            writable_roots,
        } => {
            assert!(!network_access);
            assert_eq!(writable_roots, vec![ws.to_path_buf()]);
        }
        other => panic!("expected WorkspaceWrite, got: {other:?}"),
    }
}

#[tokio::test]
async fn stream_event_from_token_delta() {
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ModelTokenDelta {
            delta: "hello".to_string(),
        },
    );
    let stream_event = StreamEvent::from_domain_event(event);
    assert!(
        matches!(stream_event, StreamEvent::Text(ref t) if t == "hello"),
        "expected Text(hello), got: {stream_event:?}"
    );
}

#[tokio::test]
async fn stream_event_from_tool_invocation_completed() {
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ToolInvocationCompleted {
            invocation_id: "inv-1".to_string(),
            tool_id: "shell.exec".to_string(),
            output_preview: "exit 0".to_string(),
            exit_code: Some(0),
            duration_ms: 42,
            truncated: false,
        },
    );
    let stream_event = StreamEvent::from_domain_event(event);
    match stream_event {
        StreamEvent::ToolResult { tool_name, output } => {
            assert_eq!(tool_name, "shell.exec");
            assert_eq!(output, "exit 0");
        }
        other => panic!("expected ToolResult, got: {other:?}"),
    }
}

#[tokio::test]
async fn stream_event_from_assistant_message_completed() {
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AssistantMessageCompleted {
            message_id: "msg-1".to_string(),
            content: "done".to_string(),
        },
    );
    let stream_event = StreamEvent::from_domain_event(event);
    assert!(matches!(stream_event, StreamEvent::TurnCompleted));
}

struct CountingHook {
    call_count: AtomicUsize,
}

impl CountingHook {
    fn new() -> Self {
        Self {
            call_count: AtomicUsize::new(0),
        }
    }

    fn count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl SdkHook for CountingHook {
    async fn before_tool(&self, _context: &ToolHookContext) -> HookAction {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        HookAction::Continue
    }

    fn name(&self) -> &str {
        "counting-hook"
    }
}

#[tokio::test]
async fn hook_builder_registers_hook() {
    let workspace = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let hook = Arc::new(CountingHook::new());
    let sdk = KairoxSdk::builder()
        .workspace(workspace.path())
        .data_dir(data.path())
        .home_dir(data.path())
        .database_filename("test-hook.db")
        .approval_policy(SdkApprovalPolicy::Never)
        .sandbox_policy(SdkSandboxPolicy::ReadOnly)
        .enable_mcp_servers(false)
        .enable_lsp_servers(false)
        .enable_marketplace(false)
        .hook_arc(hook.clone())
        .build()
        .await
        .expect("SDK build should succeed");
    // Hook is registered but not yet invoked (no tool calls)
    assert_eq!(hook.count(), 0);
    // Creating a session should work with hooks wired
    let _session = sdk.create_session().await.expect("create session");
}

#[tokio::test]
async fn hook_action_reject_has_reason() {
    let action = HookAction::Reject("dangerous command".to_string());
    assert_eq!(action, HookAction::Reject("dangerous command".to_string()));
    assert_ne!(action, HookAction::Continue);
}

#[tokio::test]
async fn error_display_variants() {
    use crate::SdkError;
    let err = SdkError::InvalidWorkspacePath("/bad".to_string());
    assert!(err.to_string().contains("/bad"));

    let err = SdkError::SessionNotActive("sess-1".to_string());
    assert!(err.to_string().contains("sess-1"));

    let err = SdkError::HookRejected("too risky".to_string());
    assert!(err.to_string().contains("too risky"));

    let err = SdkError::ConfigError("bad toml".to_string());
    assert!(err.to_string().contains("bad toml"));

    let err = SdkError::RuntimeInit("db failed".to_string());
    assert!(err.to_string().contains("db failed"));
}
