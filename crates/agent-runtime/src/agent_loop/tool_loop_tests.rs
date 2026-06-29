use super::*;
use agent_core::{SessionId, TaskId, TrajectoryId, WorkspaceId};
use agent_models::ToolCall;
use agent_store::SqliteEventStore;
use agent_tools::{
    PermissionEngine, Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolRegistry, ToolRisk,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// FakeTool
// ---------------------------------------------------------------------------

struct FakeTool {
    tool_name: String,
    result: std::sync::Mutex<Option<agent_tools::Result<ToolOutput>>>,
}

impl FakeTool {
    fn succeeding(name: &str, output: &str) -> Self {
        Self {
            tool_name: name.to_string(),
            result: std::sync::Mutex::new(Some(Ok(ToolOutput {
                text: output.to_string(),
                truncated: false,
                exit_code: None,
                images: vec![],
            }))),
        }
    }

    fn with_images(name: &str, output: &str, images: Vec<agent_tools::ImageAttachment>) -> Self {
        Self {
            tool_name: name.to_string(),
            result: std::sync::Mutex::new(Some(Ok(ToolOutput {
                text: output.to_string(),
                truncated: false,
                exit_code: None,
                images,
            }))),
        }
    }

    fn failing(name: &str, error_msg: &str) -> Self {
        Self {
            tool_name: name.to_string(),
            result: std::sync::Mutex::new(Some(Err(agent_tools::ToolError::ExecutionFailed(
                error_msg.to_string(),
            )))),
        }
    }

    fn completed(name: &str, output: &str, exit_code: Option<i32>) -> Self {
        Self {
            tool_name: name.to_string(),
            result: std::sync::Mutex::new(Some(Ok(ToolOutput {
                text: output.to_string(),
                truncated: output.chars().count() > 500,
                exit_code,
                images: vec![],
            }))),
        }
    }
}

#[async_trait]
impl Tool for FakeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: self.tool_name.clone(),
            description: "fake tool for testing".to_string(),
            required_capability: String::new(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::read(&self.tool_name)
    }

    async fn invoke(&self, _invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        self.result
            .lock()
            .unwrap()
            .take()
            .expect("FakeTool invoked more than once without resetting result")
    }
}

/// A variant that reports a write risk so sandbox can deny it.
struct WriteRiskFakeTool {
    tool_name: String,
}

#[async_trait]
impl Tool for WriteRiskFakeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: self.tool_name.clone(),
            description: "write-risk tool".to_string(),
            required_capability: String::new(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::write(&self.tool_name)
    }

    async fn invoke(&self, _invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        Ok(ToolOutput {
            text: "should not reach here".to_string(),
            truncated: false,
            exit_code: None,
            images: vec![],
        })
    }
}

struct RecordingInvocationTool {
    tool_name: String,
    timeouts: Arc<std::sync::Mutex<Vec<u64>>>,
}

impl RecordingInvocationTool {
    fn new(tool_name: &str, timeouts: Arc<std::sync::Mutex<Vec<u64>>>) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            timeouts,
        }
    }
}

#[async_trait]
impl Tool for RecordingInvocationTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: self.tool_name.clone(),
            description: "records invocation metadata for testing".to_string(),
            required_capability: String::new(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::read(&self.tool_name)
    }

    async fn invoke(&self, invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        self.timeouts.lock().unwrap().push(invocation.timeout_ms);
        Ok(ToolOutput {
            text: "recorded".to_string(),
            truncated: false,
            exit_code: None,
            images: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// FakeTrajectoryStore
// ---------------------------------------------------------------------------

struct FakeTrajectoryStore {
    recorded_steps: Mutex<Vec<agent_core::TrajectoryStep>>,
}

impl FakeTrajectoryStore {
    fn new() -> Self {
        Self {
            recorded_steps: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl agent_store::TrajectoryStore for FakeTrajectoryStore {
    async fn start_trajectory(
        &self,
        _trajectory_id: &TrajectoryId,
        _task_id: &str,
        _session_id: &str,
    ) -> agent_store::Result<()> {
        Ok(())
    }

    async fn record_step(
        &self,
        _trajectory_id: &TrajectoryId,
        step: &agent_core::TrajectoryStep,
    ) -> agent_store::Result<()> {
        self.recorded_steps.lock().await.push(step.clone());
        Ok(())
    }

    async fn complete_trajectory(
        &self,
        _trajectory_id: &TrajectoryId,
        _outcome: agent_core::trajectory::TrajectoryOutcome,
    ) -> agent_store::Result<()> {
        Ok(())
    }

    async fn load_steps(
        &self,
        _trajectory_id: &TrajectoryId,
    ) -> agent_store::Result<Vec<agent_core::TrajectoryStep>> {
        Ok(self.recorded_steps.lock().await.clone())
    }

    async fn get_meta(
        &self,
        _trajectory_id: &TrajectoryId,
    ) -> agent_store::Result<Option<agent_core::trajectory::TrajectoryMeta>> {
        Ok(None)
    }

    async fn list_by_session(
        &self,
        _session_id: &str,
    ) -> agent_store::Result<Vec<agent_core::trajectory::TrajectoryMeta>> {
        Ok(vec![])
    }

    async fn export_json(
        &self,
        _trajectory_id: &TrajectoryId,
    ) -> agent_store::Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_tool_call(id: &str, name: &str, args: &str) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments: serde_json::json!(args),
    }
}

struct TestHarness {
    tool_registry: Arc<Mutex<ToolRegistry>>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    store: Arc<SqliteEventStore>,
    event_tx: tokio::sync::broadcast::Sender<agent_core::DomainEvent>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    pending_permissions: crate::permission::PendingPermissionsMap,
    pending_task_confirmations: crate::task_confirmation::PendingTaskConfirmationsMap,
    task_graphs: Arc<Mutex<HashMap<String, crate::task_graph::TaskGraph>>>,
    root_task_id: TaskId,
    config: agent_config::Config,
    turn_cancellation: CancellationToken,
    trajectory_step_counter: AtomicU32,
}

impl TestHarness {
    async fn new() -> Self {
        Self::with_permission_engine(PermissionEngine::new(
            agent_tools::ApprovalPolicy::Never,
            agent_tools::SandboxPolicy::DangerFullAccess,
        ))
        .await
    }

    async fn with_permission_engine(engine: PermissionEngine) -> Self {
        let store = Arc::new(SqliteEventStore::in_memory().await.unwrap());
        let (event_tx, _) = tokio::sync::broadcast::channel(100);
        let session_id = SessionId::new();
        let root_task_id = TaskId::new();

        // Seed a TaskGraph for this session so sub-task creation works.
        let mut task_graphs_map = HashMap::new();
        task_graphs_map.insert(
            session_id.to_string(),
            crate::task_graph::TaskGraph::default(),
        );

        Self {
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            permission_engine: Arc::new(Mutex::new(engine)),
            store,
            event_tx,
            workspace_id: WorkspaceId::new(),
            session_id,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            pending_task_confirmations: Arc::new(Mutex::new(HashMap::new())),
            task_graphs: Arc::new(Mutex::new(task_graphs_map)),
            root_task_id,
            config: agent_config::Config::defaults(),
            turn_cancellation: CancellationToken::new(),
            trajectory_step_counter: AtomicU32::new(0),
        }
    }

    fn register_tool(&self, tool: Box<dyn Tool>) {
        // We can't await inside a non-async fn, so use try_lock which is fine
        // since nothing else holds the lock in test setup.
        self.tool_registry.try_lock().unwrap().register(tool);
    }

    async fn execute(
        &self,
        tool_calls: &[ToolCall],
        trajectory_store: &Option<Arc<dyn agent_store::TrajectoryStore>>,
        trajectory_id: &Option<TrajectoryId>,
    ) -> agent_core::Result<ToolLoopResult> {
        execute_tool_calls(
            tool_calls,
            &self.tool_registry,
            &self.permission_engine,
            &self.store,
            &self.event_tx,
            &self.workspace_id,
            &self.session_id,
            &self.pending_permissions,
            &self.pending_task_confirmations,
            &self.task_graphs,
            &self.root_task_id,
            &self.config,
            &None, // workspace_scoped_builtin_tools
            None,  // root_path
            &self.turn_cancellation,
            trajectory_store,
            trajectory_id,
            &self.trajectory_step_counter,
        )
        .await
    }

    async fn execute_simple(&self, tool_calls: &[ToolCall]) -> agent_core::Result<ToolLoopResult> {
        self.execute(tool_calls, &None, &None).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn task_confirmation_tool_emits_request_and_returns_user_response() {
    let harness = TestHarness::new().await;
    let mut rx = harness.event_tx.subscribe();
    let calls = vec![ToolCall {
        id: "call-clarify".into(),
        name: "task_confirmation.request".into(),
        arguments: serde_json::json!({
            "prompt": "Which scope should I use?",
            "options": [
                {
                    "id": "tests",
                    "label": "Tests only",
                    "description": "Add failing tests first"
                },
                {
                    "id": "full",
                    "label": "Full implementation"
                }
            ],
            "allow_multiple": true,
            "allow_custom": true
        }),
    }];

    let pending = harness.pending_task_confirmations.clone();
    let exec = tokio::spawn(async move { harness.execute_simple(&calls).await });

    let requested = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let event = rx.recv().await.unwrap();
            if matches!(
                event.payload,
                agent_core::EventPayload::TaskConfirmationRequested { .. }
            ) {
                break event;
            }
        }
    })
    .await
    .expect("task confirmation request should be emitted");

    let request_id = match requested.payload {
        agent_core::EventPayload::TaskConfirmationRequested {
            request_id,
            prompt,
            options,
            allow_multiple,
            allow_custom,
        } => {
            assert_eq!(prompt, "Which scope should I use?");
            assert_eq!(options.len(), 2);
            assert!(allow_multiple);
            assert!(allow_custom);
            request_id
        }
        other => panic!("expected TaskConfirmationRequested, got {other:?}"),
    };

    crate::task_confirmation::resolve_task_confirmation(
        &pending,
        agent_core::TaskConfirmationDecision {
            request_id,
            selected_option_ids: vec!["tests".into()],
            custom_response: Some("Also update TUI".into()),
        },
    )
    .await
    .unwrap();

    let result = exec.await.unwrap().unwrap();
    assert_eq!(result.tool_results.len(), 1);
    assert_eq!(result.tool_results[0].0, "call-clarify");
    assert!(result.tool_results[0]
        .1
        .contains("selected_option_ids=[\"tests\"]"));
    assert!(result.tool_results[0]
        .1
        .contains("custom_response=Also update TUI"));
}

#[tokio::test]
async fn tool_not_found() {
    let harness = TestHarness::new().await;
    let calls = vec![make_tool_call("call-1", "nonexistent_tool", "")];

    let result = harness.execute_simple(&calls).await.unwrap();

    assert_eq!(result.tool_results.len(), 1);
    let (ref call_id, ref output) = result.tool_results[0];
    assert_eq!(call_id, "call-1");
    assert!(
        output.contains("NotFound") || output.contains("not found"),
        "expected NotFound error, got: {output}"
    );
}

#[tokio::test]
async fn successful_tool_execution() {
    let harness = TestHarness::new().await;
    harness.register_tool(Box::new(FakeTool::succeeding("greet", "hello world")));

    let calls = vec![make_tool_call("call-1", "greet", "")];
    let result = harness.execute_simple(&calls).await.unwrap();

    assert_eq!(result.tool_results.len(), 1);
    let (ref call_id, ref output) = result.tool_results[0];
    assert_eq!(call_id, "call-1");
    assert!(
        output.contains("hello world"),
        "expected output to contain 'hello world', got: {output}"
    );
}

#[tokio::test]
async fn shell_exec_uses_longer_agent_loop_timeout() {
    let harness = TestHarness::new().await;
    let timeouts = Arc::new(std::sync::Mutex::new(Vec::new()));
    harness.register_tool(Box::new(RecordingInvocationTool::new(
        "shell.exec",
        timeouts.clone(),
    )));

    let calls = vec![make_tool_call("call-shell", "shell.exec", "cargo test")];
    harness.execute_simple(&calls).await.unwrap();

    assert_eq!(*timeouts.lock().unwrap(), vec![300_000]);
}

#[tokio::test]
async fn non_shell_tools_keep_default_agent_loop_timeout() {
    let harness = TestHarness::new().await;
    let timeouts = Arc::new(std::sync::Mutex::new(Vec::new()));
    harness.register_tool(Box::new(RecordingInvocationTool::new(
        "greet",
        timeouts.clone(),
    )));

    let calls = vec![make_tool_call("call-greet", "greet", "")];
    harness.execute_simple(&calls).await.unwrap();

    assert_eq!(*timeouts.lock().unwrap(), vec![30_000]);
}

#[tokio::test]
async fn tool_execution_failure() {
    let harness = TestHarness::new().await;
    harness.register_tool(Box::new(FakeTool::failing(
        "broken",
        "something went wrong",
    )));

    let calls = vec![make_tool_call("call-1", "broken", "")];
    let result = harness.execute_simple(&calls).await.unwrap();

    assert_eq!(result.tool_results.len(), 1);
    let (_, ref output) = result.tool_results[0];
    assert!(
        output.contains("Error") && output.contains("something went wrong"),
        "expected error message, got: {output}"
    );
}

#[tokio::test]
async fn failed_completed_tool_preview_preserves_tail() {
    let harness = TestHarness::new().await;
    let mut rx = harness.event_tx.subscribe();
    let output = format!("HEAD_MARKER\n{}\nTAIL_MARKER", "x".repeat(900));
    harness.register_tool(Box::new(FakeTool::completed(
        "shell.exec",
        &output,
        Some(101),
    )));

    let calls = vec![make_tool_call("call-1", "shell.exec", "cargo test")];
    harness.execute_simple(&calls).await.unwrap();

    let completed = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let event = rx.recv().await.unwrap();
            if let agent_core::EventPayload::ToolInvocationCompleted {
                output_preview,
                exit_code,
                ..
            } = event.payload
            {
                break (output_preview, exit_code);
            }
        }
    })
    .await
    .expect("completion event should be emitted");

    assert_eq!(completed.1, Some(101));
    assert!(
        completed.0.contains("TAIL_MARKER"),
        "preview should preserve failure tail, got: {}",
        completed.0
    );
}

#[tokio::test]
async fn permission_denied() {
    // ReadOnly sandbox + Never approval policy → write risk is denied.
    let engine = PermissionEngine::new(
        agent_tools::ApprovalPolicy::Never,
        agent_tools::SandboxPolicy::ReadOnly,
    );
    let harness = TestHarness::with_permission_engine(engine).await;
    harness.register_tool(Box::new(WriteRiskFakeTool {
        tool_name: "write_file".to_string(),
    }));

    let calls = vec![make_tool_call("call-1", "write_file", "")];
    let result = harness.execute_simple(&calls).await.unwrap();

    assert_eq!(result.tool_results.len(), 1);
    let (_, ref output) = result.tool_results[0];
    assert!(
        output.contains("Permission denied"),
        "expected permission denied, got: {output}"
    );
}

#[tokio::test]
async fn multiple_tool_calls() {
    let harness = TestHarness::new().await;
    harness.register_tool(Box::new(FakeTool::succeeding("tool_a", "output_a")));
    harness.register_tool(Box::new(FakeTool::succeeding("tool_b", "output_b")));

    let calls = vec![
        make_tool_call("call-1", "tool_a", ""),
        make_tool_call("call-2", "tool_b", ""),
    ];
    let result = harness.execute_simple(&calls).await.unwrap();

    assert_eq!(result.tool_results.len(), 2);
    assert_eq!(result.tool_results[0].0, "call-1");
    assert!(result.tool_results[0].1.contains("output_a"));
    assert_eq!(result.tool_results[1].0, "call-2");
    assert!(result.tool_results[1].1.contains("output_b"));
}

#[tokio::test]
async fn task_graph_sub_task_creation() {
    let harness = TestHarness::new().await;
    harness.register_tool(Box::new(FakeTool::succeeding("sub_tool", "done")));

    let calls = vec![make_tool_call("call-1", "sub_tool", "")];
    harness.execute_simple(&calls).await.unwrap();

    let graphs = harness.task_graphs.lock().await;
    let graph = graphs
        .get(&harness.session_id.to_string())
        .expect("session graph should exist");

    // The graph should now contain at least one task (the sub-task created for "sub_tool").
    let snapshot = graph.snapshot();
    assert!(
        !snapshot.is_empty(),
        "expected at least one sub-task in the graph"
    );
    // The sub-task should be named after the tool.
    let has_sub_tool_task = snapshot.iter().any(|task| task.title == "sub_tool");
    assert!(
        has_sub_tool_task,
        "expected a sub-task titled 'sub_tool' in the graph"
    );
}

#[tokio::test]
async fn trajectory_step_recorded() {
    let harness = TestHarness::new().await;
    harness.register_tool(Box::new(FakeTool::succeeding(
        "traced_tool",
        "trace_output",
    )));

    let fake_store = Arc::new(FakeTrajectoryStore::new());
    let trajectory_store: Option<Arc<dyn agent_store::TrajectoryStore>> = Some(fake_store.clone());
    let trajectory_id = Some(TrajectoryId::new());

    let calls = vec![make_tool_call("call-1", "traced_tool", "")];
    harness
        .execute(&calls, &trajectory_store, &trajectory_id)
        .await
        .unwrap();

    let steps = fake_store.recorded_steps.lock().await;
    assert_eq!(steps.len(), 1, "expected exactly one trajectory step");
    assert_eq!(steps[0].action, "traced_tool");
    assert!(
        steps[0].observation.contains("trace_output"),
        "observation should contain tool output"
    );
    assert_eq!(steps[0].step_index, 0);
}

#[tokio::test]
async fn cancellation_stops_execution() {
    let harness = TestHarness::new().await;
    harness.register_tool(Box::new(FakeTool::succeeding("cancelled_tool", "nope")));

    // Cancel before executing.
    harness.turn_cancellation.cancel();

    let calls = vec![make_tool_call("call-1", "cancelled_tool", "")];
    let result = harness.execute_simple(&calls).await.unwrap();

    assert_eq!(result.tool_results.len(), 1);
    let (_, ref output) = result.tool_results[0];
    assert!(
        output.contains("cancelled") || output.contains("Error"),
        "expected cancellation or error, got: {output}"
    );
}

#[tokio::test]
async fn tool_result_includes_image_data_uris() {
    let harness = TestHarness::new().await;
    harness.register_tool(Box::new(FakeTool::with_images(
        "screenshot_tool",
        "Screenshot captured",
        vec![agent_tools::ImageAttachment {
            media_type: "image/png".to_string(),
            data: "iVBORw0KGgoAAAANSUhEUg==".to_string(),
            label: Some("screenshot".to_string()),
        }],
    )));

    let calls = vec![make_tool_call("call-1", "screenshot_tool", "")];
    let result = harness.execute_simple(&calls).await.unwrap();

    assert_eq!(result.tool_results.len(), 1);
    let (ref call_id, ref output) = result.tool_results[0];
    assert_eq!(call_id, "call-1");
    assert!(
        output.contains("Screenshot captured"),
        "should contain text output"
    );
    assert!(
        output.contains("![screenshot](data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==)"),
        "should embed image as markdown data-URI, got: {output}"
    );
}

#[tokio::test]
async fn tool_result_multiple_images() {
    let harness = TestHarness::new().await;
    harness.register_tool(Box::new(FakeTool::with_images(
        "multi_img_tool",
        "done",
        vec![
            agent_tools::ImageAttachment {
                media_type: "image/png".to_string(),
                data: "AAAA".to_string(),
                label: Some("first".to_string()),
            },
            agent_tools::ImageAttachment {
                media_type: "image/jpeg".to_string(),
                data: "BBBB".to_string(),
                label: None,
            },
        ],
    )));

    let calls = vec![make_tool_call("call-1", "multi_img_tool", "")];
    let result = harness.execute_simple(&calls).await.unwrap();

    let (_, ref output) = result.tool_results[0];
    assert!(
        output.contains("![first](data:image/png;base64,AAAA)"),
        "should embed first image with label"
    );
    assert!(
        output.contains("![image](data:image/jpeg;base64,BBBB)"),
        "should embed second image with default label"
    );
}
