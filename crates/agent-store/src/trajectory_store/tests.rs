use agent_core::trajectory::{TrajectoryId, TrajectoryOutcome, TrajectoryStep};
use chrono::Utc;

use super::{SqliteTrajectoryStore, TrajectoryStore};
use crate::SqliteEventStore;

async fn setup() -> SqliteTrajectoryStore {
    let event_store = SqliteEventStore::in_memory().await.unwrap();
    let store = SqliteTrajectoryStore::new(event_store.pool().clone());
    store.migrate().await.unwrap();
    store
}

fn make_step(index: u32, action: &str) -> TrajectoryStep {
    TrajectoryStep {
        step_index: index,
        action: action.into(),
        action_input: serde_json::json!({"command": "test"}),
        observation: format!("result of {action}"),
        screenshot_id: None,
        timestamp: Utc::now(),
        duration_ms: 100 + u64::from(index) * 50,
    }
}

#[tokio::test]
async fn start_record_complete_roundtrip() {
    let store = setup().await;
    let tid = TrajectoryId::new();

    store
        .start_trajectory(&tid, "tsk_abc", "ses_xyz")
        .await
        .unwrap();

    // Record a few steps
    store
        .record_step(&tid, &make_step(0, "shell.exec"))
        .await
        .unwrap();
    store
        .record_step(&tid, &make_step(1, "fs.read"))
        .await
        .unwrap();
    store
        .record_step(&tid, &make_step(2, "fs.write"))
        .await
        .unwrap();

    // Complete the trajectory
    store
        .complete_trajectory(&tid, TrajectoryOutcome::Success)
        .await
        .unwrap();

    // Load steps
    let steps = store.load_steps(&tid).await.unwrap();
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].step_index, 0);
    assert_eq!(steps[0].action, "shell.exec");
    assert_eq!(steps[1].step_index, 1);
    assert_eq!(steps[2].step_index, 2);

    // Verify metadata
    let meta = store.get_meta(&tid).await.unwrap().unwrap();
    assert_eq!(meta.trajectory_id, tid);
    assert_eq!(meta.task_id, "tsk_abc");
    assert_eq!(meta.session_id, "ses_xyz");
    assert_eq!(meta.step_count, 3);
    assert_eq!(meta.outcome, TrajectoryOutcome::Success);
    assert!(meta.completed_at.is_some());
}

#[tokio::test]
async fn export_json_format() {
    let store = setup().await;
    let tid = TrajectoryId::new();

    store
        .start_trajectory(&tid, "tsk_1", "ses_1")
        .await
        .unwrap();
    store
        .record_step(&tid, &make_step(0, "shell.exec"))
        .await
        .unwrap();
    store
        .complete_trajectory(&tid, TrajectoryOutcome::Success)
        .await
        .unwrap();

    let json = store.export_json(&tid).await.unwrap();
    assert!(json.get("meta").is_some());
    assert!(json.get("steps").is_some());

    let steps = json["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["action"], "shell.exec");
}

#[tokio::test]
async fn list_by_session() {
    let store = setup().await;
    let tid1 = TrajectoryId::new();
    let tid2 = TrajectoryId::new();

    store
        .start_trajectory(&tid1, "tsk_a", "ses_shared")
        .await
        .unwrap();
    store
        .start_trajectory(&tid2, "tsk_b", "ses_shared")
        .await
        .unwrap();

    // A trajectory in a different session
    let tid3 = TrajectoryId::new();
    store
        .start_trajectory(&tid3, "tsk_c", "ses_other")
        .await
        .unwrap();

    let metas = store.list_by_session("ses_shared").await.unwrap();
    assert_eq!(metas.len(), 2);
    assert!(metas.iter().all(|m| m.session_id == "ses_shared"));
}

#[tokio::test]
async fn empty_trajectory_loads_no_steps() {
    let store = setup().await;
    let tid = TrajectoryId::new();

    store
        .start_trajectory(&tid, "tsk_empty", "ses_empty")
        .await
        .unwrap();

    let steps = store.load_steps(&tid).await.unwrap();
    assert!(steps.is_empty());

    let meta = store.get_meta(&tid).await.unwrap().unwrap();
    assert_eq!(meta.step_count, 0);
    assert_eq!(meta.outcome, TrajectoryOutcome::InProgress);
    assert!(meta.completed_at.is_none());
}

#[tokio::test]
async fn get_meta_returns_none_for_unknown_id() {
    let store = setup().await;
    let unknown = TrajectoryId("nonexistent".into());
    let result = store.get_meta(&unknown).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn step_with_screenshot_id_persists() {
    let store = setup().await;
    let tid = TrajectoryId::new();

    store
        .start_trajectory(&tid, "tsk_ss", "ses_ss")
        .await
        .unwrap();

    let step = TrajectoryStep {
        step_index: 0,
        action: "browser.screenshot".into(),
        action_input: serde_json::json!({"url": "http://localhost:3000"}),
        observation: "screenshot captured".into(),
        screenshot_id: Some("img_abc123".into()),
        timestamp: Utc::now(),
        duration_ms: 500,
    };
    store.record_step(&tid, &step).await.unwrap();

    let loaded = store.load_steps(&tid).await.unwrap();
    assert_eq!(loaded[0].screenshot_id, Some("img_abc123".into()));
}
