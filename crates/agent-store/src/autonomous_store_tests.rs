use super::*;
use crate::SqliteEventStore;

async fn setup() -> SqliteAutonomousTaskStore {
    let event_store = SqliteEventStore::in_memory().await.unwrap();
    let store = SqliteAutonomousTaskStore::new(event_store.pool().clone());
    store.migrate().await.unwrap();
    store
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
    AutonomousCheckpointRow {
        checkpoint_id: checkpoint_id.into(),
        autonomous_task_id: task_id.into(),
        session_id: session_id.into(),
        session_index: index,
        checkpoint_json: r#"{"completed_items":["step 1"],"remaining_items":["step 2"]}"#.into(),
        end_reason: "context_limit_reached".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[tokio::test]
async fn create_and_get_autonomous_task() {
    let store = setup().await;
    let row = make_task_row("atk_test1", "wrk_w1");
    store.create_autonomous_task(&row).await.unwrap();

    let fetched = store.get_autonomous_task("atk_test1").await.unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.autonomous_task_id, "atk_test1");
    assert_eq!(fetched.workspace_id, "wrk_w1");
    assert_eq!(fetched.state, "active");
    assert_eq!(fetched.session_count, 0);
}

#[tokio::test]
async fn get_nonexistent_returns_none() {
    let store = setup().await;
    let fetched = store.get_autonomous_task("atk_nope").await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn update_state() {
    let store = setup().await;
    store
        .create_autonomous_task(&make_task_row("atk_s1", "wrk_w1"))
        .await
        .unwrap();

    store
        .update_autonomous_task_state("atk_s1", "paused", Some("ses_abc"))
        .await
        .unwrap();

    let fetched = store.get_autonomous_task("atk_s1").await.unwrap().unwrap();
    assert_eq!(fetched.state, "paused");
    assert_eq!(fetched.current_session_id.as_deref(), Some("ses_abc"));
}

#[tokio::test]
async fn increment_session_count() {
    let store = setup().await;
    store
        .create_autonomous_task(&make_task_row("atk_cnt", "wrk_w1"))
        .await
        .unwrap();

    let count = store.increment_session_count("atk_cnt").await.unwrap();
    assert_eq!(count, 1);

    let count = store.increment_session_count("atk_cnt").await.unwrap();
    assert_eq!(count, 2);

    let fetched = store.get_autonomous_task("atk_cnt").await.unwrap().unwrap();
    assert_eq!(fetched.session_count, 2);
}

#[tokio::test]
async fn checkpoint_crud() {
    let store = setup().await;
    store
        .create_autonomous_task(&make_task_row("atk_ckpt", "wrk_w1"))
        .await
        .unwrap();

    let c0 = make_checkpoint_row("ckpt_0", "atk_ckpt", "ses_0", 0);
    store.insert_checkpoint(&c0).await.unwrap();

    let c1 = make_checkpoint_row("ckpt_1", "atk_ckpt", "ses_1", 1);
    store.insert_checkpoint(&c1).await.unwrap();

    let latest = store
        .get_latest_checkpoint("atk_ckpt")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.checkpoint_id, "ckpt_1");
    assert_eq!(latest.session_index, 1);

    let all = store.list_checkpoints("atk_ckpt").await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].session_index, 0);
    assert_eq!(all[1].session_index, 1);
}

#[tokio::test]
async fn session_chain_operations() {
    let store = setup().await;
    store
        .create_autonomous_task(&make_task_row("atk_chain", "wrk_w1"))
        .await
        .unwrap();

    store
        .insert_session_chain_entry("atk_chain", "ses_a", 0)
        .await
        .unwrap();
    store
        .insert_session_chain_entry("atk_chain", "ses_b", 1)
        .await
        .unwrap();

    let chain = store.list_session_chain("atk_chain").await.unwrap();
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].session_id, "ses_a");
    assert_eq!(chain[0].session_index, 0);
    assert_eq!(chain[1].session_id, "ses_b");
    assert_eq!(chain[1].session_index, 1);
}

#[tokio::test]
async fn list_active_tasks() {
    let store = setup().await;
    store
        .create_autonomous_task(&make_task_row("atk_a1", "wrk_w1"))
        .await
        .unwrap();

    let mut completed = make_task_row("atk_a2", "wrk_w1");
    completed.state = "completed".into();
    store.create_autonomous_task(&completed).await.unwrap();

    store
        .create_autonomous_task(&make_task_row("atk_a3", "wrk_w2"))
        .await
        .unwrap();

    let active = store.list_active_autonomous_tasks("wrk_w1").await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].autonomous_task_id, "atk_a1");
}

#[tokio::test]
async fn get_task_for_session() {
    let store = setup().await;
    store
        .create_autonomous_task(&make_task_row("atk_lookup", "wrk_w1"))
        .await
        .unwrap();
    store
        .insert_session_chain_entry("atk_lookup", "ses_x", 0)
        .await
        .unwrap();

    let task = store
        .get_autonomous_task_for_session("ses_x")
        .await
        .unwrap();
    assert!(task.is_some());
    assert_eq!(task.unwrap().autonomous_task_id, "atk_lookup");

    let none = store
        .get_autonomous_task_for_session("ses_nope")
        .await
        .unwrap();
    assert!(none.is_none());
}
