use sqlx::SqlitePool;

pub(super) async fn run(pool: &SqlitePool) -> crate::Result<()> {
    sqlx::query(include_str!("../../migrations/0001_events.sql"))
        .execute(pool)
        .await?;
    sqlx::query(include_str!("../../migrations/0002_metadata.sql"))
        .execute(pool)
        .await?;
    sqlx::query(include_str!("../../migrations/0003_projects.sql"))
        .execute(pool)
        .await?;
    // 0004 adds a column that may already exist on re-connect; tolerate
    // the duplicate so `connect()` is idempotent for tests that drop and
    // re-open the same database file.
    if let Err(e) = sqlx::query(include_str!(
        "../../migrations/0004_project_session_branch.sql"
    ))
    .execute(pool)
    .await
    {
        let msg = e.to_string();
        if !msg.contains("duplicate column name") {
            return Err(crate::StoreError::Sqlx(e));
        }
    }
    // 0005 adds the session_drafts table; tolerate duplicate on re-connect.
    if let Err(e) = sqlx::query(include_str!("../../migrations/0005_session_drafts.sql"))
        .execute(pool)
        .await
    {
        let msg = e.to_string();
        if !msg.contains("already exists") && !msg.contains("duplicate") {
            return Err(crate::StoreError::Sqlx(e));
        }
    }
    // 0007 adds approval_policy column; tolerate duplicate on re-connect.
    if let Err(e) = sqlx::query(include_str!(
        "../../migrations/0007_approval_sandbox_policy.sql"
    ))
    .execute(pool)
    .await
    {
        let msg = e.to_string();
        if !msg.contains("duplicate column name") {
            return Err(crate::StoreError::Sqlx(e));
        }
    }
    // 0008 adds sandbox_policy column; tolerate duplicate on re-connect.
    if let Err(e) = sqlx::query(include_str!(
        "../../migrations/0008_sandbox_policy_column.sql"
    ))
    .execute(pool)
    .await
    {
        let msg = e.to_string();
        if !msg.contains("duplicate column name") {
            return Err(crate::StoreError::Sqlx(e));
        }
    }
    Ok(())
}
