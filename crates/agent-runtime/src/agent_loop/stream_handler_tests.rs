use super::*;
use agent_config::Config;
use agent_core::{DomainEvent, EventPayload, SendMessageRequest, SessionId, TaskId, WorkspaceId};
use agent_models::{ModelClient, ModelError, ModelEvent, ModelRequest, ToolCall};
use agent_store::{EventStore, SqliteEventStore};
use agent_tools::{ApprovalPolicy, PermissionEngine, SandboxPolicy, ToolRegistry};
use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Custom model clients for failure / edge-case scenarios
// ---------------------------------------------------------------------------

/// Factory function type that produces a stream of model events.
/// Using a factory avoids the `Clone` requirement on `ModelError`.
type EventFactory = Box<dyn Fn() -> Vec<agent_models::Result<ModelEvent>> + Send + Sync>;

/// A model client that replays events produced by a factory on each `stream()` call.
struct ScriptedModelClient {
    factory: EventFactory,
}

impl std::fmt::Debug for ScriptedModelClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptedModelClient").finish()
    }
}

impl ScriptedModelClient {
    fn from_ok_events(events: Vec<ModelEvent>) -> Self {
        Self {
            factory: Box::new(move || events.clone().into_iter().map(Ok).collect()),
        }
    }

    fn with_stream_error(error_message: &str) -> Self {
        let message = error_message.to_string();
        Self {
            factory: Box::new(move || vec![Err(ModelError::Request(message.clone()))]),
        }
    }
}

#[async_trait]
impl ModelClient for ScriptedModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let events = (self.factory)();
        Ok(Box::pin(stream::iter(events)))
    }
}

/// A model client whose `stream()` method itself returns `Err` (connection-level failure).
#[derive(Debug)]
struct FailingStreamClient {
    message: String,
}

#[async_trait]
impl ModelClient for FailingStreamClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Err(ModelError::Connection(self.message.clone()))
    }
}

#[derive(Debug)]
struct HangingEventStreamClient;

#[async_trait]
impl ModelClient for HangingEventStreamClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Ok(Box::pin(futures::stream::pending()))
    }
}

#[derive(Debug)]
struct CompletedThenHangingEventStreamClient;

#[async_trait]
impl ModelClient for CompletedThenHangingEventStreamClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let stream = futures::stream::unfold(0, |state| async move {
            match state {
                0 => Some((Ok(ModelEvent::TokenDelta("done".into())), 1)),
                1 => Some((Ok(ModelEvent::Completed { usage: None }), 2)),
                _ => futures::future::pending().await,
            }
        });
        Ok(Box::pin(stream))
    }
}

#[derive(Debug)]
struct HangingStreamStartClient;

#[async_trait]
impl ModelClient for HangingStreamStartClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        futures::future::pending().await
    }
}

#[derive(Debug)]
struct StartTimeoutOnceClient {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ModelClient for StartTimeoutOnceClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            futures::future::pending().await
        } else {
            Ok(Box::pin(stream::iter(vec![
                Ok(ModelEvent::TokenDelta("recovered".into())),
                Ok(ModelEvent::Completed { usage: None }),
            ])))
        }
    }
}

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

struct StreamTestHarness<M: ModelClient + 'static> {
    store: Arc<SqliteEventStore>,
    model: Arc<M>,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    pending: crate::permission::PendingPermissionsMap,
    pending_task_confirmations: crate::task_confirmation::PendingTaskConfirmationsMap,
    task_graphs: Arc<Mutex<HashMap<String, crate::task_graph::TaskGraph>>>,
    config: Arc<Config>,
    session_states: Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    active_skills: Arc<Mutex<HashMap<String, Vec<String>>>>,
    memory_store: Option<Arc<dyn agent_memory::MemoryStore>>,
    workspace_rag_index: Option<Arc<agent_memory::WorkspaceRagIndex>>,
    knowledge_base_retrievers: HashMap<String, Arc<dyn agent_memory::WorkspaceRetriever>>,
    skill_registry: Option<Arc<dyn agent_skills::SkillRegistry>>,
    workspace_scoped: Option<Arc<agent_tools::WorkspaceScopedBuiltinTools>>,
    trajectory_store: Option<Arc<dyn agent_store::TrajectoryStore>>,
}

impl<M: ModelClient + 'static> StreamTestHarness<M> {
    async fn new(model: M) -> Self {
        let store = Arc::new(SqliteEventStore::in_memory().await.unwrap());
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
        let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
        let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )));
        let pending: crate::permission::PendingPermissionsMap =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_task_confirmations: crate::task_confirmation::PendingTaskConfirmationsMap =
            Arc::new(Mutex::new(HashMap::new()));
        let task_graphs = Arc::new(Mutex::new(HashMap::new()));
        let config = Arc::new(Config::defaults());
        let session_states = Arc::new(Mutex::new(HashMap::new()));
        let active_skills = Arc::new(Mutex::new(HashMap::new()));
        Self {
            store,
            model: Arc::new(model),
            event_tx,
            tool_registry,
            permission_engine,
            pending,
            pending_task_confirmations,
            task_graphs,
            config,
            session_states,
            active_skills,
            memory_store: None,
            workspace_rag_index: None,
            knowledge_base_retrievers: HashMap::new(),
            skill_registry: None,
            workspace_scoped: None,
            trajectory_store: None,
        }
    }

    fn set_model_stream_idle_timeout_secs(&mut self, timeout_secs: u64) {
        Arc::get_mut(&mut self.config)
            .expect("test harness config should not be shared")
            .context
            .model_stream_idle_timeout_secs = Some(timeout_secs);
    }

    fn deps(&self) -> AgentLoopDeps<'_, SqliteEventStore, M> {
        AgentLoopDeps {
            store: &self.store,
            model: &self.model,
            event_tx: &self.event_tx,
            tool_registry: &self.tool_registry,
            permission_engine: &self.permission_engine,
            pending_permissions: &self.pending,
            pending_task_confirmations: &self.pending_task_confirmations,
            memory_store: &self.memory_store,
            workspace_rag_index: &self.workspace_rag_index,
            knowledge_base_retrievers: &self.knowledge_base_retrievers,
            task_graphs: &self.task_graphs,
            config: &self.config,
            session_states: &self.session_states,
            skill_registry: &self.skill_registry,
            active_skills: &self.active_skills,
            workspace_scoped_builtin_tools: &self.workspace_scoped,
            trajectory_store: &self.trajectory_store,
            turn_cancellation: CancellationToken::new(),
            root_path: None,
        }
    }
}

fn make_request() -> SendMessageRequest {
    SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: "test message".into(),
        display_content: None,
        attachments: vec![],
    }
}

fn minimal_model_request() -> ModelRequest {
    ModelRequest::user_text("fake", "test")
}

// ===========================================================================
// process_model_stream tests
// ===========================================================================

#[tokio::test]
async fn stream_success_accumulates_assistant_text() {
    let model = ScriptedModelClient::from_ok_events(vec![
        ModelEvent::TokenDelta("Hello ".into()),
        ModelEvent::TokenDelta("world!".into()),
        ModelEvent::Completed { usage: None },
    ]);
    let harness = StreamTestHarness::new(model).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let output = process_model_stream(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        None,
    )
    .await
    .expect("stream should succeed");

    assert_eq!(output.assistant_text, "Hello world!");
    assert!(output.tool_calls.is_empty());
}

#[tokio::test]
async fn stream_collects_tool_calls() {
    let model = ScriptedModelClient::from_ok_events(vec![
        ModelEvent::ToolCallRequested {
            tool_call_id: "call_1".into(),
            tool_id: "fs.read".into(),
            arguments: serde_json::json!({"path": "README.md"}),
        },
        ModelEvent::Completed { usage: None },
    ]);
    let harness = StreamTestHarness::new(model).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let output = process_model_stream(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        None,
    )
    .await
    .expect("stream with tool calls should succeed");

    assert!(output.assistant_text.is_empty());
    assert_eq!(output.tool_calls.len(), 1);
    assert_eq!(output.tool_calls[0].name, "fs.read");
    assert_eq!(output.tool_calls[0].id, "call_1");
}

#[tokio::test]
async fn stream_model_failed_event_returns_error() {
    let model = ScriptedModelClient::from_ok_events(vec![ModelEvent::Failed {
        message: "rate limit exceeded".into(),
    }]);
    let harness = StreamTestHarness::new(model).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let result = process_model_stream(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        None,
    )
    .await;

    match result {
        Ok(_) => panic!("should return error on model failure"),
        Err(err) => {
            let error_message = err.to_string();
            assert!(
                error_message.contains("rate limit exceeded"),
                "error should propagate model failure message, got: {error_message}"
            );
        }
    }
}

#[tokio::test]
async fn stream_error_in_event_returns_error() {
    let model = ScriptedModelClient::with_stream_error("connection reset");
    let harness = StreamTestHarness::new(model).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let result = process_model_stream(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        None,
    )
    .await;

    assert!(result.is_err(), "stream-level error should propagate");
}

#[tokio::test]
async fn stream_empty_response_returns_empty_model_error() {
    let model = ScriptedModelClient::from_ok_events(vec![ModelEvent::Completed { usage: None }]);
    let harness = StreamTestHarness::new(model).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let result = process_model_stream(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        None,
    )
    .await;

    match result {
        Ok(_) => panic!("empty response should be an error"),
        Err(err) => {
            let error_message = err.to_string();
            assert!(
                error_message.contains("empty response"),
                "should return EMPTY_MODEL_RESPONSE_ERROR, got: {error_message}"
            );
        }
    }
}

#[tokio::test]
async fn completed_model_event_ends_stream_without_waiting_for_eof() {
    let harness = StreamTestHarness::new(CompletedThenHangingEventStreamClient).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let output = tokio::time::timeout(
        Duration::from_millis(250),
        process_model_stream_with_idle_timeout(
            &deps,
            &request,
            &cancel_token,
            &root_task_id,
            &minimal_model_request(),
            None,
            Duration::from_millis(100),
        ),
    )
    .await
    .expect("completed stream should finish before the test timeout")
    .expect("completed stream should succeed");

    assert_eq!(output.assistant_text, "done");
    assert!(output.tool_calls.is_empty());
}

#[tokio::test]
async fn stream_empty_response_fallback_marks_output() {
    let model = ScriptedModelClient::from_ok_events(vec![ModelEvent::Completed { usage: None }]);
    let harness = StreamTestHarness::new(model).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let output = process_model_stream(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        Some("I completed the requested tool call, but the model returned no final text."),
    )
    .await
    .expect("fallback should produce an assistant message");

    assert!(output.empty_response_fallback_used);
    assert!(output.tool_calls.is_empty());
    assert!(output.assistant_text.contains("no final text"));
}

#[tokio::test]
async fn stream_cancellation_exits_early() {
    // Use a lazy stream that yields a pending future, ensuring the select!
    // branch can observe the cancellation before the stream produces items.

    let cancel_token = CancellationToken::new();
    let cancel_clone = cancel_token.clone();

    // Build a stream that emits one token, then cancels the token, then
    // tries to emit another token + Completed. The second token may or may
    // not be consumed depending on task scheduling, but the key invariant
    // is that the function returns Ok (not an empty-response error) because
    // the cancel check suppresses the empty-response guard.
    let events: Vec<agent_models::Result<ModelEvent>> = vec![
        Ok(ModelEvent::TokenDelta("first".into())),
        Ok(ModelEvent::TokenDelta("second".into())),
        Ok(ModelEvent::Completed { usage: None }),
    ];
    let stream = futures::stream::unfold(
        (events.into_iter(), cancel_clone),
        |(mut iter, cancel)| async move {
            let item = iter.next()?;
            // Cancel after the first item is yielded.
            cancel.cancel();
            // Yield to let the select! observe cancellation.
            tokio::task::yield_now().await;
            Some((item, (iter, cancel)))
        },
    );

    // Build a custom model client that returns our crafted stream.
    struct CancellationTestClient {
        stream: tokio::sync::Mutex<
            Option<futures::stream::BoxStream<'static, agent_models::Result<ModelEvent>>>,
        >,
    }

    impl std::fmt::Debug for CancellationTestClient {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("CancellationTestClient").finish()
        }
    }

    #[async_trait]
    impl ModelClient for CancellationTestClient {
        async fn stream(
            &self,
            _request: ModelRequest,
        ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
            let taken = self
                .stream
                .lock()
                .await
                .take()
                .expect("stream called more than once");
            Ok(taken)
        }
    }

    let model = CancellationTestClient {
        stream: tokio::sync::Mutex::new(Some(Box::pin(stream))),
    };
    let harness = StreamTestHarness::new(model).await;
    let deps = harness.deps();
    let request = make_request();
    let root_task_id = TaskId::new();

    let result = process_model_stream(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        None,
    )
    .await;

    // Cancellation is inherently racy: the token was cancelled before the
    // stream was polled, but the runtime may still yield one or more items
    // before noticing. Both Ok-with-partial-text and Err (if cancellation
    // wins) are valid outcomes.
    match result {
        Ok(output) => {
            // Some tokens may have been accumulated before the cancellation
            // was observed.  The key invariant is that we did NOT get the
            // EMPTY_MODEL_RESPONSE_ERROR — the cancellation path bypasses
            // that check.
            assert!(
                !output
                    .assistant_text
                    .contains("model returned an empty response"),
                "cancellation should bypass the empty-response error"
            );
        }
        Err(_) => {
            // Acceptable — cancellation beat the first yield.
        }
    }
}

#[tokio::test]
async fn stream_connection_failure_returns_error() {
    let model = FailingStreamClient {
        message: "TCP connection refused".into(),
    };
    let harness = StreamTestHarness::new(model).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let result = process_model_stream(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        None,
    )
    .await;

    match result {
        Ok(_) => panic!("connection failure should be an error"),
        Err(err) => {
            let error_message = err.to_string();
            assert!(
                error_message.contains("TCP connection refused"),
                "should propagate connection error, got: {error_message}"
            );
        }
    }
}

#[tokio::test]
async fn stream_idle_timeout_fails_root_task() {
    let harness = StreamTestHarness::new(HangingEventStreamClient).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let result = tokio::time::timeout(
        Duration::from_millis(200),
        process_model_stream_with_idle_timeout(
            &deps,
            &request,
            &cancel_token,
            &root_task_id,
            &minimal_model_request(),
            None,
            Duration::from_millis(50),
        ),
    )
    .await
    .expect("hanging stream should fail before the test timeout");

    let err = match result {
        Ok(_) => panic!("hanging stream should time out"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("timed out"),
        "timeout error should explain the stalled model stream, got: {err}"
    );

    let events = harness
        .store
        .load_session(&request.session_id)
        .await
        .unwrap();
    assert!(
        events.iter().any(|event| {
            matches!(
                &event.payload,
                EventPayload::AgentTaskFailed { task_id, error }
                    if task_id == &root_task_id && error.contains("timed out")
            )
        }),
        "root task should be marked failed on model stream timeout: {events:?}"
    );
}

#[tokio::test]
async fn process_model_stream_uses_configured_idle_timeout() {
    let mut harness = StreamTestHarness::new(HangingEventStreamClient).await;
    harness.set_model_stream_idle_timeout_secs(1);
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let result = tokio::time::timeout(
        Duration::from_millis(1_500),
        process_model_stream(
            &deps,
            &request,
            &cancel_token,
            &root_task_id,
            &minimal_model_request(),
            None,
        ),
    )
    .await
    .expect("configured stream idle timeout should fail before the test timeout");

    let err = match result {
        Ok(_) => panic!("hanging stream should time out"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("timed out after 1s"),
        "timeout error should use configured duration, got: {err}"
    );
    assert!(
        err.to_string().contains("phase=stream_event"),
        "timeout error should include stall phase, got: {err}"
    );
    assert!(
        err.to_string().contains("last_event=stream_opened"),
        "timeout error should include last model event kind, got: {err}"
    );
    assert!(
        err.to_string().contains("tool_results=0"),
        "timeout error should include request tool result count, got: {err}"
    );
}

#[tokio::test]
async fn stream_event_timeout_status_includes_request_stats() {
    let harness = StreamTestHarness::new(HangingEventStreamClient).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();
    let model_request = ModelRequest::user_text("fake", "test")
        .add_assistant_with_tools(
            "",
            vec![ToolCall {
                id: "call_1".into(),
                name: "shell.exec".into(),
                arguments: serde_json::json!({"command": "echo hi"}),
            }],
        )
        .add_tool_result("call_1", "hi");

    let result = process_model_stream_with_idle_timeout(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &model_request,
        None,
        Duration::from_millis(25),
    )
    .await;

    assert!(result.is_err(), "hanging stream should time out");
    let events = harness
        .store
        .load_session(&request.session_id)
        .await
        .unwrap();
    let message = events
        .iter()
        .find_map(|event| match &event.payload {
            EventPayload::ModelStreamStatus { phase, message, .. } if phase == "stream_event" => {
                Some(message.as_str())
            }
            _ => None,
        })
        .expect("stream timeout should emit ModelStreamStatus");

    assert!(
        message.contains("tool_results=1"),
        "timeout status should include tool result count, got: {message}"
    );
    assert!(
        message.contains("assistant_tool_messages=1"),
        "timeout status should include assistant tool-call message count, got: {message}"
    );
}

#[tokio::test]
async fn stream_start_timeout_fails_root_task() {
    let harness = StreamTestHarness::new(HangingStreamStartClient).await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let result = tokio::time::timeout(
        Duration::from_millis(200),
        process_model_stream_with_idle_timeout(
            &deps,
            &request,
            &cancel_token,
            &root_task_id,
            &minimal_model_request(),
            None,
            Duration::from_millis(50),
        ),
    )
    .await
    .expect("hanging stream start should fail before the test timeout");

    let err = match result {
        Ok(_) => panic!("hanging stream start should time out"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("timed out"),
        "timeout error should explain the stalled model stream, got: {err}"
    );

    let events = harness
        .store
        .load_session(&request.session_id)
        .await
        .unwrap();
    assert!(
        events.iter().any(|event| {
            matches!(
                &event.payload,
                EventPayload::AgentTaskFailed { task_id, error }
                    if task_id == &root_task_id && error.contains("timed out")
            )
        }),
        "root task should be marked failed on model stream start timeout: {events:?}"
    );
}

#[tokio::test]
async fn stream_start_timeout_retries_before_failing_turn() {
    let calls = Arc::new(AtomicUsize::new(0));
    let harness = StreamTestHarness::new(StartTimeoutOnceClient {
        calls: calls.clone(),
    })
    .await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    let output = tokio::time::timeout(
        Duration::from_millis(200),
        process_model_stream_with_idle_timeout(
            &deps,
            &request,
            &cancel_token,
            &root_task_id,
            &minimal_model_request(),
            None,
            Duration::from_millis(25),
        ),
    )
    .await
    .expect("stream start retry should recover before the test timeout")
    .expect("second stream attempt should succeed");

    assert_eq!(output.assistant_text, "recovered");
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let events = harness
        .store
        .load_session(&request.session_id)
        .await
        .unwrap();
    assert!(
        !events.iter().any(|event| {
            matches!(
                &event.payload,
                EventPayload::AgentTaskFailed { task_id, .. } if task_id == &root_task_id
            )
        }),
        "recoverable stream start timeout should not fail the root task: {events:?}"
    );
}

#[tokio::test]
async fn stream_start_retry_emits_event() {
    let calls = Arc::new(AtomicUsize::new(0));
    let harness = StreamTestHarness::new(StartTimeoutOnceClient {
        calls: calls.clone(),
    })
    .await;
    let deps = harness.deps();
    let request = make_request();
    let cancel_token = CancellationToken::new();
    let root_task_id = TaskId::new();

    process_model_stream_with_idle_timeout(
        &deps,
        &request,
        &cancel_token,
        &root_task_id,
        &minimal_model_request(),
        None,
        Duration::from_millis(25),
    )
    .await
    .expect("second stream attempt should succeed");

    let events = harness
        .store
        .load_session(&request.session_id)
        .await
        .unwrap();
    assert!(
        events.iter().any(|event| {
            matches!(
                &event.payload,
                EventPayload::ModelStreamStatus {
                    phase,
                    retrying: true,
                    retry_attempt: 1,
                    max_retries: 1,
                    message,
                } if phase == "stream_start" && message.contains("retrying")
            )
        }),
        "stream start retry should be visible in session events: {events:?}"
    );
}

#[test]
fn model_stream_timeout_log_classification_distinguishes_retry_from_final() {
    let retrying = ModelStreamProgress::retrying(
        "stream_start",
        0,
        0,
        "none",
        1,
        MODEL_STREAM_START_IDLE_RETRIES,
    );
    assert!(retrying.is_retrying());
    assert_eq!(retrying.retry_attempt(), 1);
    assert_eq!(
        model_stream_timeout_log_message(retrying),
        "model stream start idle timeout; retrying"
    );

    let final_timeout = ModelStreamProgress::new("stream_start", 0, 0, "none");
    assert!(!final_timeout.is_retrying());
    assert_eq!(final_timeout.retry_attempt(), 0);
    assert_eq!(
        model_stream_timeout_log_message(final_timeout),
        "model stream idle timeout"
    );
}
