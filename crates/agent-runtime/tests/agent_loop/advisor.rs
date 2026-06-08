//! Advisor integration tests: verify that the advisor self-reflection
//! layer fires inline within the agent loop when configured.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_core::{AdvisorMode, AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::EchoTool;

/// A model that emits a dangerous `shell.exec rm -rf` tool call on the first
/// request so the advisor has something to review, then a simple text
/// completion on the second request (after tool results are fed back).
///
/// When used as the advisor model it returns a structured JSON verdict.
#[derive(Debug, Clone)]
struct AdvisorTestModel {
    call_count: Arc<AtomicUsize>,
}

impl AdvisorTestModel {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for AdvisorTestModel {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);

        // Detect advisor review calls: system prompt contains "safety advisor"
        let is_advisor_call = request
            .system_prompt
            .as_ref()
            .map(|s| s.contains("safety advisor"))
            .unwrap_or(false);

        let events: Vec<agent_models::Result<ModelEvent>> = if is_advisor_call {
            // Advisor: return approve_with_warnings
            let response = r#"{"verdict":"approve_with_warnings","concerns":[{"tool_name":"echo","severity":"low","message":"echo is harmless"}],"summary":"Proceed with minor note."}"#;
            vec![
                Ok(ModelEvent::TokenDelta(response.into())),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        } else if count == 0 {
            // Primary agent: first call → tool call
            vec![
                Ok(ModelEvent::TokenDelta("Let me check".into())),
                Ok(ModelEvent::ToolCallRequested {
                    tool_call_id: "call_adv_1".into(),
                    tool_id: "echo".into(),
                    arguments: serde_json::json!({"text": "advisor test"}),
                }),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        } else {
            // Primary agent: second call → text completion
            vec![
                Ok(ModelEvent::TokenDelta("All done".into())),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

/// Build a config with advisor set to the given mode.
fn advisor_config(mode: AdvisorMode) -> Arc<agent_config::Config> {
    Arc::new(agent_config::Config {
        profiles: vec![],
        mcp_servers: vec![],
        source: agent_config::ConfigSource::Defaults,
        context: agent_config::ContextPolicy::default(),
        disabled_mcp_servers: vec![],
        instructions: None,
        features: agent_config::FeatureFlags { hooks: true },
        hooks: vec![],
        lsp_servers: vec![],
        dap_servers: vec![],
        advisor: agent_config::AdvisorConfig {
            mode,
            profile: None, // reuse same model
            max_concerns: 5,
        },
    })
}

#[tokio::test]
async fn advisor_full_mode_emits_review_events_on_tool_call() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = AdvisorTestModel::new();
    let runtime = LocalRuntime::new(store, model)
        .with_approval_and_sandbox(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
        .with_config(advisor_config(AdvisorMode::Full));

    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(EchoTool));

    let workspace = runtime
        .open_workspace("/tmp/test-advisor-full".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "run advisor test".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        event_types.contains(&"AdvisorReviewStarted"),
        "missing AdvisorReviewStarted in trace: {event_types:?}"
    );
    assert!(
        event_types.contains(&"AdvisorReviewCompleted"),
        "missing AdvisorReviewCompleted in trace: {event_types:?}"
    );
    // Tool should still execute since verdict is approve_with_warnings
    assert!(
        event_types.contains(&"ToolInvocationStarted"),
        "tool should still execute after approve_with_warnings: {event_types:?}"
    );
    assert!(
        event_types.contains(&"ToolInvocationCompleted"),
        "tool should complete after approve_with_warnings: {event_types:?}"
    );
}

#[tokio::test]
async fn advisor_off_mode_skips_review_events() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = AdvisorTestModel::new();
    let runtime = LocalRuntime::new(store, model)
        .with_approval_and_sandbox(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
        .with_config(advisor_config(AdvisorMode::Off));

    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(EchoTool));

    let workspace = runtime
        .open_workspace("/tmp/test-advisor-off".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "run without advisor".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        !event_types.contains(&"AdvisorReviewStarted"),
        "advisor Off mode should not emit review events: {event_types:?}"
    );
    assert!(
        !event_types.contains(&"AdvisorReviewCompleted"),
        "advisor Off mode should not emit review events: {event_types:?}"
    );
    // Tool should still execute normally
    assert!(
        event_types.contains(&"ToolInvocationCompleted"),
        "tool should complete without advisor: {event_types:?}"
    );
}

/// A model that returns a reject verdict from the advisor, causing tool
/// execution to be blocked.
#[derive(Debug, Clone)]
struct AdvisorRejectModel {
    call_count: Arc<AtomicUsize>,
}

impl AdvisorRejectModel {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for AdvisorRejectModel {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);

        let is_advisor_call = request
            .system_prompt
            .as_ref()
            .map(|s| s.contains("safety advisor"))
            .unwrap_or(false);

        let events: Vec<agent_models::Result<ModelEvent>> = if is_advisor_call {
            let response = r#"{"verdict":"reject","concerns":[{"tool_name":"echo","severity":"high","message":"blocked for testing"}],"summary":"Rejected for safety."}"#;
            vec![
                Ok(ModelEvent::TokenDelta(response.into())),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        } else if count == 0 {
            vec![
                Ok(ModelEvent::TokenDelta("I will use a tool".into())),
                Ok(ModelEvent::ToolCallRequested {
                    tool_call_id: "call_rej_1".into(),
                    tool_id: "echo".into(),
                    arguments: serde_json::json!({"text": "should be blocked"}),
                }),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        } else {
            vec![
                Ok(ModelEvent::TokenDelta("fallback".into())),
                Ok(ModelEvent::Completed { usage: None }),
            ]
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

#[tokio::test]
async fn advisor_reject_blocks_tool_execution() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = AdvisorRejectModel::new();
    let runtime = LocalRuntime::new(store, model)
        .with_approval_and_sandbox(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
        .with_config(advisor_config(AdvisorMode::Full));

    runtime
        .tool_registry()
        .lock()
        .await
        .register(Box::new(EchoTool));

    let workspace = runtime
        .open_workspace("/tmp/test-advisor-reject".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "test".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "try blocked tool".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        event_types.contains(&"AdvisorReviewStarted"),
        "should start review: {event_types:?}"
    );
    assert!(
        event_types.contains(&"AdvisorReviewCompleted"),
        "should complete review: {event_types:?}"
    );
    // Tool should NOT execute when advisor rejects
    assert!(
        !event_types.contains(&"ToolInvocationStarted"),
        "tool should NOT start when advisor rejects: {event_types:?}"
    );
    // Should have an assistant message explaining the rejection
    assert!(
        event_types.contains(&"AssistantMessageCompleted"),
        "should emit rejection message: {event_types:?}"
    );
}
