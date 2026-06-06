use super::*;
use agent_core::{SendMessageRequest, SessionId, WorkspaceId};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use std::sync::Arc;

// ── Helpers ───────────────────────────────────────────────────────────────

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

fn make_request(content: &str) -> SendMessageRequest {
    SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: content.to_string(),
        display_content: None,
        attachments: vec![],
    }
}

fn build_executor(
    runtime: &LocalRuntime<SqliteEventStore, FakeModelClient>,
) -> LocalRuntimeTurnExecutor<SqliteEventStore, FakeModelClient> {
    LocalRuntimeTurnExecutor::from_runtime(runtime)
}

// ── from_runtime ──────────────────────────────────────────────────────────

#[tokio::test]
async fn from_runtime_produces_usable_executor() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    // Verify the executor holds a working store by calling snapshot.
    let _config = executor.config.snapshot();
}

// ── execution_mode ────────────────────────────────────────────────────────

#[tokio::test]
async fn execution_mode_auto_prefix() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    let request = make_request("/auto do something");
    assert_eq!(executor.execution_mode(&request), ExecutionMode::Autonomous);
}

#[tokio::test]
async fn execution_mode_plan_prefix_without_dag_executor() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    // dag_executor is None by default → falls back to SingleStep
    assert!(executor.dag_executor.is_none());
    let request = make_request("/plan build feature");
    assert_eq!(executor.execution_mode(&request), ExecutionMode::SingleStep);
}

#[tokio::test]
async fn execution_mode_plain_message() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    let request = make_request("hello world");
    assert_eq!(executor.execution_mode(&request), ExecutionMode::SingleStep);
}

#[tokio::test]
async fn execution_mode_auto_without_trailing_space() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    // "/auto" without trailing space is NOT the prefix "/auto "
    let request = make_request("/automate");
    assert_eq!(executor.execution_mode(&request), ExecutionMode::SingleStep);
}

#[tokio::test]
async fn execution_mode_plan_without_trailing_space() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    let request = make_request("/planning");
    assert_eq!(executor.execution_mode(&request), ExecutionMode::SingleStep);
}

// ── resolve_compactor_profile ─────────────────────────────────────────────

#[tokio::test]
async fn resolve_compactor_profile_uses_config_when_set() {
    let runtime = build_runtime().await;
    // Set a custom config with compactor_profile
    let mut config = agent_config::Config::defaults();
    config.context.compactor_profile = Some("my-compactor".to_string());
    runtime.update_config(Arc::new(config));

    let executor = build_executor(&runtime);
    let session_id = SessionId::new();
    let profile = executor.resolve_compactor_profile(&session_id).await;
    assert_eq!(profile, "my-compactor");
}

#[tokio::test]
async fn resolve_compactor_profile_falls_back_to_fake_for_empty_session() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    // No session events → latest_model_profile_for returns default
    let session_id = SessionId::new();
    let profile = executor.resolve_compactor_profile(&session_id).await;
    // With no events, `latest_model_profile_for` returns "fake"
    assert_eq!(profile, "fake");
}

// ── maybe_schedule_auto_compaction ────────────────────────────────────────

#[tokio::test]
async fn maybe_schedule_auto_compaction_no_session_state_does_not_panic() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    // No session state inserted → early return, no panic
    let request = make_request("hello");
    executor.maybe_schedule_auto_compaction(&request).await;
}

#[tokio::test]
async fn maybe_schedule_auto_compaction_threshold_disabled() {
    let runtime = build_runtime().await;
    // Set threshold to 1.0 (disabled)
    let mut config = agent_config::Config::defaults();
    config.context.auto_compact_threshold = 1.0;
    runtime.update_config(Arc::new(config));

    let executor = build_executor(&runtime);
    let request = make_request("hello");

    // Insert a session state so we don't early-return
    {
        let mut states = executor.session_states.lock().await;
        states.insert(
            request.session_id.to_string(),
            crate::session::SessionState {
                last_estimated_tokens: 5000,
                model_limits: Some(agent_models::ModelLimits {
                    context_window: 8192,
                    output_limit: 4096,
                    source: agent_models::LimitSource::Fallback,
                }),
                compacting: false,
                ..Default::default()
            },
        );
    }

    // Should not panic; threshold >= 1.0 means skip
    executor.maybe_schedule_auto_compaction(&request).await;
}

#[tokio::test]
async fn maybe_schedule_auto_compaction_already_compacting() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    let request = make_request("hello");

    {
        let mut states = executor.session_states.lock().await;
        states.insert(
            request.session_id.to_string(),
            crate::session::SessionState {
                last_estimated_tokens: 5000,
                model_limits: Some(agent_models::ModelLimits {
                    context_window: 8192,
                    output_limit: 4096,
                    source: agent_models::LimitSource::Fallback,
                }),
                compacting: true,
                ..Default::default()
            },
        );
    }

    // Should not panic; already_compacting → skip
    executor.maybe_schedule_auto_compaction(&request).await;
}

// ── retry_task / cancel_task without dag_executor ─────────────────────────

#[tokio::test]
async fn retry_task_without_dag_executor_returns_invalid_state() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    assert!(executor.dag_executor.is_none());

    let result = executor
        .retry_task(
            WorkspaceId::new(),
            SessionId::new(),
            agent_core::TaskId::new(),
        )
        .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("DAG executor not available"),
        "unexpected error: {err_msg}"
    );
}

#[tokio::test]
async fn cancel_task_without_dag_executor_returns_invalid_state() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);

    let result = executor
        .cancel_task(
            WorkspaceId::new(),
            SessionId::new(),
            agent_core::TaskId::new(),
        )
        .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("DAG executor not available"),
        "unexpected error: {err_msg}"
    );
}

// ── execute_turn dispatch (SingleStep) ───────────────────────────────────

#[tokio::test]
async fn execute_turn_single_step_runs_without_session_binding() {
    let runtime = build_runtime().await;
    let executor = build_executor(&runtime);
    let request = make_request("hello");
    let cancel = tokio_util::sync::CancellationToken::new();

    // execute_single_step will fail because there's no session binding or
    // real model, but it should not panic — we verify it returns an error
    // gracefully rather than unwinding.
    let result = executor.execute_turn(request, cancel).await;
    // FakeModelClient with empty responses causes an error in the agent loop,
    // which is expected. The key assertion is: no panic.
    assert!(result.is_err());
}
