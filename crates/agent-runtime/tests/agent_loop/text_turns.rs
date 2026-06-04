//! Plain text turns (no tool calls) and the loop-iteration guard constant.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelRequest, ModelUsage};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct EarlyUsageThenTextModel;

#[async_trait]
impl ModelClient for EarlyUsageThenTextModel {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Ok(Box::pin(stream::iter(vec![
            Ok(ModelEvent::Completed {
                usage: Some(ModelUsage {
                    input_tokens: 5,
                    output_tokens: 0,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                }),
            }),
            Ok(ModelEvent::TokenDelta("reply".into())),
            Ok(ModelEvent::Completed {
                usage: Some(ModelUsage {
                    input_tokens: 5,
                    output_tokens: 1,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                }),
            }),
        ])))
    }
}

#[derive(Clone)]
struct RecordingTextModel {
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

#[async_trait]
impl ModelClient for RecordingTextModel {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        self.requests.lock().await.push(request);
        Ok(Box::pin(stream::iter(vec![
            Ok(ModelEvent::TokenDelta("reply".into())),
            Ok(ModelEvent::Completed { usage: None }),
        ])))
    }
}

/// Verify MAX_AGENT_LOOP_ITERATIONS is a reasonable value — the constant
/// guards against infinite loops, so it must be positive and bounded.
#[test]
#[allow(clippy::assertions_on_constants)]
fn max_agent_loop_iterations_is_reasonable() {
    use agent_runtime::agent_loop::MAX_AGENT_LOOP_ITERATIONS;
    assert!(MAX_AGENT_LOOP_ITERATIONS > 0);
    assert!(MAX_AGENT_LOOP_ITERATIONS <= 100);
}

#[tokio::test]
async fn agent_loop_stops_when_no_tool_calls() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Just a text response".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-no-tools".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].content, "hello");
    assert_eq!(projection.messages[1].content, "Just a text response");
}

#[tokio::test]
async fn reasoning_capable_profile_does_not_default_effort() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut config = agent_config::Config::defaults();
    config.profiles.push((
        "ali-mo-claude".into(),
        agent_config::ProfileDef {
            provider: "ali-mo".into(),
            model_id: "claude-opus-4-6".into(),
            base_url: Some("https://example.invalid".into()),
            api_key: Some("test-key".into()),
            api_key_env: None,
            context_window: Some(200_000),
            output_limit: Some(32_000),
            response: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            headers: None,
            client_identity: None,
            supports_tools: None,
            supports_vision: None,
            supports_reasoning: None,
            extra_params: None,
            server_tool_code_execution: None,
            server_tool_web_search: None,
            enabled: true,
        },
    ));
    let runtime = LocalRuntime::new(
        store,
        RecordingTextModel {
            requests: requests.clone(),
        },
    )
    .with_config(Arc::new(config));

    let workspace = runtime
        .open_workspace("/tmp/test-no-default-reasoning".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "ali-mo-claude".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id,
            content: "hello".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let requests = requests.lock().await;
    let request = requests.first().expect("model should be called");
    assert_eq!(request.model_profile, "ali-mo-claude");
    assert_eq!(request.reasoning_effort, None);
}

#[tokio::test]
async fn project_instructions_are_sent_in_model_request() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let runtime = LocalRuntime::new(
        store,
        RecordingTextModel {
            requests: requests.clone(),
        },
    );
    let project_root = tempfile::tempdir().expect("project root");
    tokio::fs::write(
        project_root.path().join("AGENTS.md"),
        "PROJECT_BEHAVIOR_CHECK=WT_INSTRUCTION_0604",
    )
    .await
    .expect("write project instructions");

    let workspace = runtime
        .open_workspace(project_root.path().display().to_string())
        .await
        .unwrap();
    let project = runtime
        .add_existing_project(
            workspace.workspace_id.clone(),
            project_root.path().display().to_string(),
        )
        .await
        .unwrap();
    let session_id = runtime
        .create_project_draft_session(project.project_id)
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id,
            content: "perform project behavior check".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let requests = requests.lock().await;
    let request = requests.first().expect("model should be called");
    let system_prompt = request
        .system_prompt
        .as_deref()
        .expect("system prompt should be present");
    assert!(
        system_prompt.contains("PROJECT_BEHAVIOR_CHECK=WT_INSTRUCTION_0604"),
        "project instructions should be included in the model request system prompt: {system_prompt}"
    );
}

#[tokio::test]
async fn agent_loop_ignores_usage_only_completion_before_text() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = LocalRuntime::new(store, EarlyUsageThenTextModel);

    let workspace = runtime
        .open_workspace("/tmp/test-early-usage".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let projection = runtime
        .get_session_projection(session_id.clone())
        .await
        .unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].content, "hello");
    assert_eq!(projection.messages[1].content, "reply");

    let trace = runtime.get_trace(session_id).await.unwrap();
    let assistant_contents: Vec<_> = trace
        .iter()
        .filter_map(|e| match &e.event.payload {
            agent_core::EventPayload::AssistantMessageCompleted { content, .. } => {
                Some(content.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(assistant_contents, vec!["reply"]);
}

/// Verify the exact event sequence for a simple (non-tool-call) completion.
/// Key events must appear in the expected relative order.
#[tokio::test]
async fn agent_loop_emits_completion_event_sequence() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Short reply".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-event-seq".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<String> = trace.iter().map(|e| e.event.event_type.clone()).collect();

    // Verify key events exist
    assert!(
        event_types.contains(&"UserMessageAdded".to_string()),
        "Missing UserMessageAdded: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"ModelTokenDelta".to_string()),
        "Missing ModelTokenDelta: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted".to_string()),
        "Missing AssistantMessageCompleted: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"AgentTaskCompleted".to_string()),
        "Missing AgentTaskCompleted: {:?}",
        event_types
    );

    // Verify expected relative order
    let user_pos = event_types
        .iter()
        .position(|t| t == "UserMessageAdded")
        .unwrap();
    let assistant_pos = event_types
        .iter()
        .position(|t| t == "AssistantMessageCompleted")
        .unwrap();
    let completed_pos = event_types
        .iter()
        .position(|t| t == "AgentTaskCompleted")
        .unwrap();

    assert!(
        user_pos < assistant_pos,
        "UserMessageAdded should come before AssistantMessageCompleted"
    );
    assert!(
        assistant_pos < completed_pos,
        "AssistantMessageCompleted should come before AgentTaskCompleted"
    );
}
