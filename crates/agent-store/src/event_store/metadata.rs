use sqlx::Row;

use super::{
    ProjectSessionMetaRow, SessionRow, SessionRowForQuery, SqliteEventStore, WorkspaceRow,
};

impl SqliteEventStore {
    pub async fn upsert_workspace(&self, workspace_id: &str, path: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO kairox_workspaces (workspace_id, path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(workspace_id) DO UPDATE SET path = ?2, updated_at = ?4",
        )
        .bind(workspace_id)
        .bind(path)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_session(&self, meta: &SessionRow) -> crate::Result<()> {
        sqlx::query(
            "INSERT INTO kairox_sessions (session_id, workspace_id, title, model_profile, model_id, provider, permission_mode, approval_policy, sandbox_policy, deleted_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(session_id) DO UPDATE SET title = ?3, model_profile = ?4, model_id = ?5, provider = ?6, permission_mode = ?7, approval_policy = ?8, sandbox_policy = ?9, updated_at = ?12",
        )
        .bind(&meta.session_id)
        .bind(&meta.workspace_id)
        .bind(&meta.title)
        .bind(&meta.model_profile)
        .bind(&meta.model_id)
        .bind(&meta.provider)
        .bind(&meta.permission_mode)
        .bind(&meta.approval_policy)
        .bind(&meta.sandbox_policy)
        .bind(&meta.deleted_at)
        .bind(&meta.created_at)
        .bind(&meta.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceRow>> {
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT workspace_id, path, created_at, updated_at FROM kairox_workspaces ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_active_sessions(&self, workspace_id: &str) -> crate::Result<Vec<SessionRow>> {
        let rows = sqlx::query_as::<_, SessionRowForQuery>(
            "SELECT session_id, workspace_id, title, model_profile, model_id, provider, permission_mode, approval_policy, sandbox_policy, deleted_at, created_at, updated_at
             FROM kairox_sessions WHERE workspace_id = ?1 AND deleted_at IS NULL ORDER BY updated_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(SessionRow::from).collect())
    }

    pub async fn rename_session(&self, session_id: &str, title: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE kairox_sessions SET title = ?1, updated_at = ?2 WHERE session_id = ?3")
            .bind(title)
            .bind(&now)
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn soft_delete_session(&self, session_id: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_sessions SET deleted_at = ?1, updated_at = ?1 WHERE session_id = ?2",
        )
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO kairox_session_visibility (session_id, visibility, updated_at)
             VALUES (?1, 'archived', ?2)
             ON CONFLICT(session_id) DO UPDATE SET visibility = 'archived', updated_at = ?2",
        )
        .bind(session_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn permanently_delete_session(&self, session_id: &str) -> crate::Result<()> {
        sqlx::query("DELETE FROM events WHERE session_id = ?1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM kairox_session_visibility WHERE session_id = ?1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM kairox_project_sessions WHERE session_id = ?1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM kairox_sessions WHERE session_id = ?1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn restore_archived_session(&self, session_id: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_sessions SET deleted_at = NULL, updated_at = ?1 WHERE session_id = ?2",
        )
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO kairox_session_visibility (session_id, visibility, updated_at)
             VALUES (?1, 'visible', ?2)
             ON CONFLICT(session_id) DO UPDATE SET visibility = 'visible', updated_at = ?2",
        )
        .bind(session_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> crate::Result<usize> {
        let threshold = chrono::Utc::now()
            - chrono::Duration::from_std(older_than)
                .unwrap_or_else(|_| chrono::Duration::seconds(0));
        let threshold_str = threshold.to_rfc3339();

        let mut transaction = self.pool.begin().await?;
        let expired: Vec<String> = sqlx::query_scalar(
            "SELECT session_id FROM kairox_sessions WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
        )
        .bind(&threshold_str)
        .fetch_all(&mut *transaction)
        .await?;

        let count = expired.len();
        if count == 0 {
            transaction.commit().await?;
            return Ok(0);
        }

        for session_id in &expired {
            sqlx::query("DELETE FROM events WHERE session_id = ?1")
                .bind(session_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM kairox_project_sessions WHERE session_id = ?1")
                .bind(session_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM kairox_session_visibility WHERE session_id = ?1")
                .bind(session_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM kairox_sessions WHERE session_id = ?1")
                .bind(session_id)
                .execute(&mut *transaction)
                .await?;
        }

        transaction.commit().await?;
        Ok(count)
    }

    pub(super) async fn list_visible_project_session_rows(
        &self,
        project_id: &str,
    ) -> crate::Result<Vec<ProjectSessionMetaRow>> {
        let rows = sqlx::query_as::<_, ProjectSessionMetaRow>(
            "SELECT sessions.session_id, sessions.workspace_id, sessions.title,
                    sessions.model_profile, sessions.model_id, sessions.provider,
                    sessions.deleted_at, sessions.created_at, sessions.updated_at,
                    bindings.project_id, bindings.worktree_path, bindings.branch, visibility.visibility,
                    sessions.permission_mode,
                    sessions.approval_policy, sessions.sandbox_policy
             FROM kairox_sessions AS sessions
             INNER JOIN kairox_project_sessions AS bindings
                ON bindings.session_id = sessions.session_id
             INNER JOIN kairox_session_visibility AS visibility
                ON visibility.session_id = sessions.session_id
             WHERE bindings.project_id = ?1
               AND sessions.deleted_at IS NULL
               AND visibility.visibility IN ('visible', 'draft_hidden')
             ORDER BY sessions.updated_at DESC, sessions.created_at ASC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub(super) async fn list_archived_project_session_rows(
        &self,
        workspace_id: &str,
    ) -> crate::Result<Vec<ProjectSessionMetaRow>> {
        let rows = sqlx::query_as::<_, ProjectSessionMetaRow>(
            "SELECT sessions.session_id, sessions.workspace_id, sessions.title,
                    sessions.model_profile, sessions.model_id, sessions.provider,
                    sessions.deleted_at, sessions.created_at, sessions.updated_at,
                    bindings.project_id, bindings.worktree_path, bindings.branch, visibility.visibility,
                    sessions.permission_mode,
                    sessions.approval_policy, sessions.sandbox_policy
             FROM kairox_sessions AS sessions
             INNER JOIN kairox_project_sessions AS bindings
                ON bindings.session_id = sessions.session_id
             INNER JOIN kairox_projects AS projects
                ON projects.project_id = bindings.project_id
             INNER JOIN kairox_session_visibility AS visibility
                ON visibility.session_id = sessions.session_id
             WHERE projects.workspace_id = ?1
               AND visibility.visibility = 'archived'
             ORDER BY sessions.updated_at DESC, sessions.created_at ASC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn save_draft(&self, session_id: &str, draft_text: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO session_drafts (session_id, draft_text, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET draft_text = excluded.draft_text, updated_at = excluded.updated_at",
        )
        .bind(session_id)
        .bind(draft_text)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_permission_mode(&self, session_id: &str, mode: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_sessions SET permission_mode = ?1, updated_at = ?2 WHERE session_id = ?3",
        )
        .bind(mode)
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_approval_policy(
        &self,
        session_id: &str,
        approval_policy: &str,
    ) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_sessions SET approval_policy = ?1, updated_at = ?2 WHERE session_id = ?3",
        )
        .bind(approval_policy)
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_sandbox_policy(
        &self,
        session_id: &str,
        sandbox_policy_json: &str,
    ) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_sessions SET sandbox_policy = ?1, updated_at = ?2 WHERE session_id = ?3",
        )
        .bind(sandbox_policy_json)
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_draft(&self, session_id: &str) -> crate::Result<String> {
        let row = sqlx::query("SELECT draft_text FROM session_drafts WHERE session_id = ?1")
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row
            .map(|r: sqlx::sqlite::SqliteRow| r.get::<String, _>("draft_text"))
            .unwrap_or_default())
    }
}
