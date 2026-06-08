use std::time::Duration;

use super::super::*;

/// Helper to build a [`SessionRow`] with sensible defaults.
fn make_session(session_id: &str, workspace_id: &str, title: &str) -> SessionRow {
    let now = chrono::Utc::now().to_rfc3339();
    SessionRow {
        session_id: session_id.into(),
        workspace_id: workspace_id.into(),
        title: title.into(),
        model_profile: "default".into(),
        model_id: None,
        provider: None,
        approval_policy: None,
        sandbox_policy: None,
        deleted_at: None,
        created_at: now.clone(),
        updated_at: now,
    }
}

// ── workspace CRUD ──────────────────────────────────────────────────

#[tokio::test]
async fn upsert_workspace_and_list() {
    let store = SqliteEventStore::in_memory().await.unwrap();

    store
        .upsert_workspace("ws_1", "/projects/alpha")
        .await
        .unwrap();

    let workspaces = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, "ws_1");
    assert_eq!(workspaces[0].path, "/projects/alpha");
}

#[tokio::test]
async fn upsert_workspace_updates_on_conflict() {
    let store = SqliteEventStore::in_memory().await.unwrap();

    store.upsert_workspace("ws_1", "/old/path").await.unwrap();
    store.upsert_workspace("ws_1", "/new/path").await.unwrap();

    let workspaces = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1, "upsert should not duplicate rows");
    assert_eq!(workspaces[0].path, "/new/path");
}

// ── session CRUD ────────────────────────────────────────────────────

#[tokio::test]
async fn upsert_session_and_list_active() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("ws_1", "/tmp/proj").await.unwrap();

    let session = make_session("ses_1", "ws_1", "My session");
    store.upsert_session(&session).await.unwrap();

    let active = store.list_active_sessions("ws_1").await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].session_id, "ses_1");
    assert_eq!(active[0].title, "My session");
}

// ── rename ──────────────────────────────────────────────────────────

#[tokio::test]
async fn rename_session_updates_title() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("ws_1", "/tmp/proj").await.unwrap();

    let session = make_session("ses_1", "ws_1", "Original title");
    store.upsert_session(&session).await.unwrap();

    store
        .rename_session("ses_1", "Renamed title")
        .await
        .unwrap();

    let active = store.list_active_sessions("ws_1").await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].title, "Renamed title");
}

// ── soft-delete / archive ───────────────────────────────────────────

#[tokio::test]
async fn soft_delete_moves_to_archived() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("ws_1", "/tmp/proj").await.unwrap();

    let session = make_session("ses_1", "ws_1", "To archive");
    store.upsert_session(&session).await.unwrap();

    store.soft_delete_session("ses_1").await.unwrap();

    let active = store.list_active_sessions("ws_1").await.unwrap();
    assert!(
        active.is_empty(),
        "soft-deleted session must not appear in active list"
    );

    let archived = store.list_archived_sessions("ws_1").await.unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].session_id, "ses_1");
    assert!(archived[0].deleted_at.is_some());
}

// ── restore ─────────────────────────────────────────────────────────

#[tokio::test]
async fn restore_archived_session() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("ws_1", "/tmp/proj").await.unwrap();

    let session = make_session("ses_1", "ws_1", "Restorable");
    store.upsert_session(&session).await.unwrap();
    store.soft_delete_session("ses_1").await.unwrap();

    // Precondition: session is archived
    assert!(store.list_active_sessions("ws_1").await.unwrap().is_empty());

    store.restore_archived_session("ses_1").await.unwrap();

    let active = store.list_active_sessions("ws_1").await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].session_id, "ses_1");
    assert!(active[0].deleted_at.is_none());
}

// ── permanent delete ────────────────────────────────────────────────

#[tokio::test]
async fn permanently_delete_removes_session() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("ws_1", "/tmp/proj").await.unwrap();

    let session = make_session("ses_1", "ws_1", "Doomed");
    store.upsert_session(&session).await.unwrap();

    store.permanently_delete_session("ses_1").await.unwrap();

    let active = store.list_active_sessions("ws_1").await.unwrap();
    let archived = store.list_archived_sessions("ws_1").await.unwrap();
    assert!(
        active.is_empty(),
        "hard-deleted session must not appear in active list"
    );
    assert!(
        archived.is_empty(),
        "hard-deleted session must not appear in archived list"
    );
}

// ── model profile update ────────────────────────────────────────────

#[tokio::test]
async fn update_session_model_profile() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("ws_1", "/tmp/proj").await.unwrap();

    let session = make_session("ses_1", "ws_1", "Profile test");
    store.upsert_session(&session).await.unwrap();

    store
        .update_session_model_profile(
            "ses_1",
            "ali-mo-claude",
            Some("claude-opus-4"),
            Some("ali-mo"),
        )
        .await
        .unwrap();

    let active = store.list_active_sessions("ws_1").await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].model_profile, "ali-mo-claude");
    assert_eq!(active[0].model_id, Some("claude-opus-4".into()));
    assert_eq!(active[0].provider, Some("ali-mo".into()));
}

// ── cleanup expired sessions ────────────────────────────────────────

#[tokio::test]
async fn cleanup_expired_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store.upsert_workspace("ws_1", "/tmp/proj").await.unwrap();

    // Insert two sessions, soft-delete both, then back-date one's deleted_at
    // so it appears expired.
    let session_old = make_session("ses_old", "ws_1", "Expired");
    let session_new = make_session("ses_new", "ws_1", "Recent");
    store.upsert_session(&session_old).await.unwrap();
    store.upsert_session(&session_new).await.unwrap();
    store.soft_delete_session("ses_old").await.unwrap();
    store.soft_delete_session("ses_new").await.unwrap();

    // Back-date ses_old's deleted_at to 48 hours ago so it qualifies for cleanup.
    let old_deleted_at = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
    sqlx::query("UPDATE kairox_sessions SET deleted_at = ?1 WHERE session_id = ?2")
        .bind(&old_deleted_at)
        .bind("ses_old")
        .execute(store.sqlite_pool().as_ref().unwrap())
        .await
        .unwrap();

    let cleaned = store
        .cleanup_expired_sessions(Duration::from_secs(24 * 3600))
        .await
        .unwrap();

    assert_eq!(cleaned, 1, "only the expired session should be cleaned up");

    // ses_old should be gone entirely
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM kairox_sessions WHERE session_id = 'ses_old'")
            .fetch_one(store.sqlite_pool().as_ref().unwrap())
            .await
            .unwrap();
    assert_eq!(
        count, 0,
        "expired session must be hard-deleted from the table"
    );

    // ses_new should still exist (soft-deleted but not expired)
    let count_new: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM kairox_sessions WHERE session_id = 'ses_new'")
            .fetch_one(store.sqlite_pool().as_ref().unwrap())
            .await
            .unwrap();
    assert_eq!(count_new, 1, "non-expired session must remain");
}
