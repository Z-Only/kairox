use agent_core::{AutonomousFacade, AutonomousTaskId, SessionId, WorkspaceId};
use agent_models::FakeModelClient;
use agent_store::{
    AutonomousCheckpointRow, AutonomousTaskRow, AutonomousTaskStore, SqliteAutonomousTaskStore,
    SqliteEventStore,
};
use std::sync::Arc;

use crate::facade_runtime::LocalRuntime;

use super::{checkpoint_row_to_view, row_to_view};

// ── Helpers ────────────────────────────────────────────────────────

async fn create_test_store() -> SqliteAutonomousTaskStore {
    let event_store = SqliteEventStore::in_memory().await.unwrap();
    let store = SqliteAutonomousTaskStore::new(event_store.pool().clone());
    store.migrate().await.unwrap();
    store
}

async fn build_runtime_without_autonomous() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let event_store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(event_store, model)
}

async fn build_runtime_with_store(
    store: SqliteAutonomousTaskStore,
) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let event_store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(event_store, model).with_autonomous_store(Arc::new(store))
}

fn make_task_row(id: &str, workspace_id: &str) -> AutonomousTaskRow {
    let now = chrono::Utc::now().to_rfc3339();
    AutonomousTaskRow {
        autonomous_task_id: id.into(),
        workspace_id: workspace_id.into(),
        goal_json: r#"{"description":"build feature","acceptance_criteria":["tests pass"],"verification_commands":["cargo test"]}"#.into(),
        config_json: r#"{"max_sessions":10,"auto_continue":true,"verification_required":true,"git_checkpoint":true}"#.into(),
        state: "active".into(),
        current_session_id: None,
        session_count: 0,
        created_at: now.clone(),
        updated_at: now,
    }
}

fn make_checkpoint_row(
    checkpoint_id: &str,
    task_id: &str,
    session_id: &str,
    index: i64,
) -> AutonomousCheckpointRow {
    let now = chrono::Utc::now().to_rfc3339();
    let checkpoint_json = serde_json::json!({
        "checkpoint_id": checkpoint_id,
        "session_id": session_id,
        "session_index": index as u32,
        "git_sha": null,
        "completed_items": ["step 1"],
        "remaining_items": ["step 2"],
        "verification_results": [],
        "notes": "",
        "created_at": now
    })
    .to_string();
    AutonomousCheckpointRow {
        checkpoint_id: checkpoint_id.into(),
        autonomous_task_id: task_id.into(),
        session_id: session_id.into(),
        session_index: index,
        checkpoint_json,
        end_reason: "context_limit_reached".into(),
        created_at: now,
    }
}

// ── row_to_view ────────────────────────────────────────────────────

#[test]
fn test_row_to_view_valid_conversion() {
    let goal_json = serde_json::json!({
        "description": "Implement feature X",
        "acceptance_criteria": ["passes tests", "no regressions"],
        "verification_commands": ["cargo test"]
    })
    .to_string();

    let config_json = serde_json::json!({
        "max_sessions": 5,
        "auto_continue": true,
        "verification_required": false,
        "git_checkpoint": true
    })
    .to_string();

    let row = AutonomousTaskRow {
        autonomous_task_id: "atk_abc123".to_string(),
        workspace_id: "wrk_ws1".to_string(),
        goal_json,
        config_json,
        state: "active".to_string(),
        current_session_id: Some("ses_current".to_string()),
        session_count: 3,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-02T00:00:00Z".to_string(),
    };

    let view = row_to_view(&row).expect("valid row should produce a view");

    assert_eq!(
        view.autonomous_task_id,
        AutonomousTaskId::from_string("atk_abc123".into())
    );
    assert_eq!(view.workspace_id, WorkspaceId::from("wrk_ws1".to_string()));
    assert_eq!(view.goal, "Implement feature X");
    assert_eq!(view.state, "active");
    assert_eq!(view.current_session_id, Some("ses_current".to_string()));
    assert_eq!(view.session_count, 3);
    assert_eq!(view.max_sessions, 5);
    assert_eq!(view.created_at, "2026-01-01T00:00:00Z");
    assert_eq!(view.updated_at, "2026-01-02T00:00:00Z");
}

#[test]
fn test_row_to_view_invalid_goal_json_returns_none() {
    let config_json = serde_json::json!({
        "max_sessions": 5,
        "auto_continue": true,
        "verification_required": false,
        "git_checkpoint": true
    })
    .to_string();

    let row = AutonomousTaskRow {
        autonomous_task_id: "atk_abc".to_string(),
        workspace_id: "wrk_ws1".to_string(),
        goal_json: "not valid json{{{".to_string(),
        config_json,
        state: "active".to_string(),
        current_session_id: None,
        session_count: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    };

    assert!(row_to_view(&row).is_none());
}

#[test]
fn test_row_to_view_invalid_config_json_returns_none() {
    let goal_json = serde_json::json!({
        "description": "task goal",
        "acceptance_criteria": [],
        "verification_commands": []
    })
    .to_string();

    let row = AutonomousTaskRow {
        autonomous_task_id: "atk_abc".to_string(),
        workspace_id: "wrk_ws1".to_string(),
        goal_json,
        config_json: "broken config".to_string(),
        state: "active".to_string(),
        current_session_id: None,
        session_count: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    };

    assert!(row_to_view(&row).is_none());
}

// ── checkpoint_row_to_view ─────────────────────────────────────────

#[test]
fn test_checkpoint_row_to_view_valid_conversion() {
    let now = "2026-01-01T12:00:00Z";
    let checkpoint_json = serde_json::json!({
        "checkpoint_id": "ckpt_001",
        "session_id": "ses_s1",
        "session_index": 1,
        "git_sha": "abc1234",
        "completed_items": ["step 1", "step 2"],
        "remaining_items": ["step 3"],
        "verification_results": [],
        "notes": "good progress",
        "created_at": "2026-01-01T00:00:00Z"
    })
    .to_string();

    let row = AutonomousCheckpointRow {
        checkpoint_id: "ckpt_001".to_string(),
        autonomous_task_id: "atk_task1".to_string(),
        session_id: "ses_s1".to_string(),
        session_index: 2,
        checkpoint_json,
        end_reason: "context_limit_reached".to_string(),
        created_at: now.to_string(),
    };

    let view = checkpoint_row_to_view(row).expect("valid row should produce a view");

    assert_eq!(view.checkpoint_id, "ckpt_001");
    assert_eq!(view.session_id, SessionId::from_string("ses_s1".into()));
    assert_eq!(view.session_index, 2);
    assert_eq!(view.completed_items, vec!["step 1", "step 2"]);
    assert_eq!(view.remaining_items, vec!["step 3"]);
    assert_eq!(view.git_sha, Some("abc1234".to_string()));
    assert_eq!(view.end_reason, "context_limit_reached");
    assert_eq!(view.created_at, now);
}

#[test]
fn test_checkpoint_row_to_view_invalid_json_returns_none() {
    let row = AutonomousCheckpointRow {
        checkpoint_id: "ckpt_002".to_string(),
        autonomous_task_id: "atk_task1".to_string(),
        session_id: "ses_s2".to_string(),
        session_index: 1,
        checkpoint_json: "}{invalid".to_string(),
        end_reason: "max_iterations_reached".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
    };

    assert!(checkpoint_row_to_view(row).is_none());
}

// ── list_autonomous_tasks ──────────────────────────────────────────

#[tokio::test]
async fn test_list_autonomous_tasks_no_store() {
    let runtime = build_runtime_without_autonomous().await;
    let result = runtime
        .list_autonomous_tasks(WorkspaceId::from("wrk_1".to_string()))
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_list_autonomous_tasks_with_store() {
    let store = create_test_store().await;
    store
        .create_autonomous_task(&make_task_row("atk_1", "wrk_1"))
        .await
        .unwrap();
    store
        .create_autonomous_task(&make_task_row("atk_2", "wrk_1"))
        .await
        .unwrap();
    store
        .create_autonomous_task(&make_task_row("atk_3", "wrk_other"))
        .await
        .unwrap();

    let runtime = build_runtime_with_store(store).await;
    let views = runtime
        .list_autonomous_tasks(WorkspaceId::from("wrk_1".to_string()))
        .await
        .unwrap();

    assert_eq!(views.len(), 2);
    let ids: Vec<&str> = views
        .iter()
        .map(|v| v.autonomous_task_id.as_str())
        .collect();
    assert!(ids.contains(&"atk_1"));
    assert!(ids.contains(&"atk_2"));
}

// ── get_autonomous_task ────────────────────────────────────────────

#[tokio::test]
async fn test_get_autonomous_task_no_store() {
    let runtime = build_runtime_without_autonomous().await;
    let result = runtime
        .get_autonomous_task(AutonomousTaskId::from_string("atk_x".into()))
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_autonomous_task_not_found() {
    let store = create_test_store().await;
    let runtime = build_runtime_with_store(store).await;
    let result = runtime
        .get_autonomous_task(AutonomousTaskId::from_string("atk_missing".into()))
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_autonomous_task_found() {
    let store = create_test_store().await;
    store
        .create_autonomous_task(&make_task_row("atk_found", "wrk_1"))
        .await
        .unwrap();

    let runtime = build_runtime_with_store(store).await;
    let view = runtime
        .get_autonomous_task(AutonomousTaskId::from_string("atk_found".into()))
        .await
        .unwrap()
        .expect("task should be found");

    assert_eq!(view.autonomous_task_id.as_str(), "atk_found");
    assert_eq!(view.goal, "build feature");
    assert_eq!(view.state, "active");
    assert_eq!(view.max_sessions, 10);
    assert_eq!(view.session_count, 0);
}

// ── get_autonomous_checkpoints ─────────────────────────────────────

#[tokio::test]
async fn test_get_checkpoints_no_store() {
    let runtime = build_runtime_without_autonomous().await;
    let result = runtime
        .get_autonomous_checkpoints(AutonomousTaskId::from_string("atk_x".into()))
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_get_checkpoints_with_data() {
    let store = create_test_store().await;
    store
        .create_autonomous_task(&make_task_row("atk_cp", "wrk_1"))
        .await
        .unwrap();
    store
        .insert_checkpoint(&make_checkpoint_row("cp_0", "atk_cp", "ses_0", 0))
        .await
        .unwrap();
    store
        .insert_checkpoint(&make_checkpoint_row("cp_1", "atk_cp", "ses_1", 1))
        .await
        .unwrap();

    let runtime = build_runtime_with_store(store).await;
    let checkpoints = runtime
        .get_autonomous_checkpoints(AutonomousTaskId::from_string("atk_cp".into()))
        .await
        .unwrap();

    assert_eq!(checkpoints.len(), 2);
    assert_eq!(checkpoints[0].checkpoint_id, "cp_0");
    assert_eq!(checkpoints[0].session_index, 0);
    assert_eq!(checkpoints[1].checkpoint_id, "cp_1");
    assert_eq!(checkpoints[1].session_index, 1);
    assert_eq!(checkpoints[0].completed_items, vec!["step 1"]);
    assert_eq!(checkpoints[0].remaining_items, vec!["step 2"]);
}

// ── pause_autonomous_task ──────────────────────────────────────────

#[tokio::test]
async fn test_pause_no_store_returns_error() {
    let runtime = build_runtime_without_autonomous().await;
    let result = runtime
        .pause_autonomous_task(AutonomousTaskId::from_string("atk_any".into()))
        .await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("autonomous store not available"),
        "unexpected: {err_msg}"
    );
}

#[tokio::test]
async fn test_pause_autonomous_task_updates_state() {
    let store = create_test_store().await;
    store
        .create_autonomous_task(&make_task_row("atk_pause", "wrk_1"))
        .await
        .unwrap();

    let runtime = build_runtime_with_store(store).await;
    runtime
        .pause_autonomous_task(AutonomousTaskId::from_string("atk_pause".into()))
        .await
        .unwrap();

    let view = runtime
        .get_autonomous_task(AutonomousTaskId::from_string("atk_pause".into()))
        .await
        .unwrap()
        .expect("task should exist");
    assert_eq!(view.state, "paused");
}

// ── resume_autonomous_task ─────────────────────────────────────────

#[tokio::test]
async fn test_resume_no_store_returns_error() {
    let runtime = build_runtime_without_autonomous().await;
    let result = runtime
        .resume_autonomous_task(AutonomousTaskId::from_string("atk_any".into()))
        .await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("autonomous store not available"),
        "unexpected: {err_msg}"
    );
}

#[tokio::test]
async fn test_resume_autonomous_task_updates_state() {
    let store = create_test_store().await;
    store
        .create_autonomous_task(&make_task_row("atk_resume", "wrk_1"))
        .await
        .unwrap();

    let runtime = build_runtime_with_store(store).await;
    runtime
        .pause_autonomous_task(AutonomousTaskId::from_string("atk_resume".into()))
        .await
        .unwrap();
    runtime
        .resume_autonomous_task(AutonomousTaskId::from_string("atk_resume".into()))
        .await
        .unwrap();

    let view = runtime
        .get_autonomous_task(AutonomousTaskId::from_string("atk_resume".into()))
        .await
        .unwrap()
        .expect("task should exist");
    assert_eq!(view.state, "active");
}

// ── cancel_autonomous_task ─────────────────────────────────────────

#[tokio::test]
async fn test_cancel_no_controller_returns_error() {
    let runtime = build_runtime_without_autonomous().await;
    let result = runtime
        .cancel_autonomous_task(
            WorkspaceId::from("wrk_1".to_string()),
            SessionId::from_string("ses_1".into()),
            AutonomousTaskId::from_string("atk_any".into()),
        )
        .await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("autonomous controller not available"),
        "unexpected: {err_msg}"
    );
}
