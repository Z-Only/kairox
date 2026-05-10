use agent_core::ProjectId;
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectRow {
    pub project_id: String,
    pub workspace_id: String,
    pub display_name: String,
    pub root_path: String,
    pub created_at: String,
    pub updated_at: String,
    pub removed_at: Option<String>,
    pub sort_order: i64,
    pub expanded: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectSessionBindingRow {
    pub session_id: String,
    pub project_id: String,
    pub worktree_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionVisibilityRow {
    pub session_id: String,
    pub visibility: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct ProjectMetaRepository {
    pool: SqlitePool,
}

impl ProjectMetaRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_project(
        &self,
        workspace_id: &str,
        display_name: &str,
        root_path: &str,
        sort_order: i64,
    ) -> crate::Result<ProjectRow> {
        let project_id = ProjectId::new().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO kairox_projects (
                project_id, workspace_id, display_name, root_path, created_at, updated_at,
                removed_at, sort_order, expanded
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
        )
        .bind(&project_id)
        .bind(workspace_id)
        .bind(display_name)
        .bind(root_path)
        .bind(&now)
        .bind(&now)
        .bind(sort_order)
        .bind(true)
        .execute(&self.pool)
        .await?;

        self.get_project(&project_id).await
    }

    pub async fn get_project(&self, project_id: &str) -> crate::Result<ProjectRow> {
        let row = sqlx::query_as::<_, ProjectRow>(
            "SELECT project_id, workspace_id, display_name, root_path, created_at, updated_at,
                    removed_at, sort_order, expanded
             FROM kairox_projects
             WHERE project_id = ?1",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_active_projects(&self, workspace_id: &str) -> crate::Result<Vec<ProjectRow>> {
        let rows = sqlx::query_as::<_, ProjectRow>(
            "SELECT project_id, workspace_id, display_name, root_path, created_at, updated_at,
                    removed_at, sort_order, expanded
             FROM kairox_projects
             WHERE workspace_id = ?1 AND removed_at IS NULL
             ORDER BY sort_order ASC, created_at ASC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn list_removed_projects(
        &self,
        workspace_id: &str,
    ) -> crate::Result<Vec<ProjectRow>> {
        let rows = sqlx::query_as::<_, ProjectRow>(
            "SELECT project_id, workspace_id, display_name, root_path, created_at, updated_at,
                    removed_at, sort_order, expanded
             FROM kairox_projects
             WHERE workspace_id = ?1 AND removed_at IS NOT NULL
             ORDER BY updated_at DESC, project_id ASC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn rename_project(&self, project_id: &str, display_name: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_projects
             SET display_name = ?1, updated_at = ?2
             WHERE project_id = ?3",
        )
        .bind(display_name)
        .bind(&now)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_project(&self, project_id: &str) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut transaction = self.pool.begin().await?;

        sqlx::query(
            "UPDATE kairox_projects
             SET removed_at = ?1, updated_at = ?1
             WHERE project_id = ?2",
        )
        .bind(&now)
        .bind(project_id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "UPDATE kairox_session_visibility
             SET visibility = 'archived', updated_at = ?1
             WHERE visibility = 'visible'
               AND session_id IN (
                 SELECT session_id FROM kairox_project_sessions WHERE project_id = ?2
               )",
        )
        .bind(&now)
        .bind(project_id)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(())
    }

    pub async fn restore_project(&self, project_id: &str) -> crate::Result<ProjectRow> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_projects
             SET removed_at = NULL, updated_at = ?1
             WHERE project_id = ?2",
        )
        .bind(&now)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        self.get_project(project_id).await
    }

    pub async fn update_project_order(&self, project_ids: &[String]) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut transaction = self.pool.begin().await?;

        for (sort_order, project_id) in project_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE kairox_projects
                 SET sort_order = ?1, updated_at = ?2
                 WHERE project_id = ?3",
            )
            .bind(sort_order as i64)
            .bind(&now)
            .bind(project_id)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub async fn update_project_expanded(
        &self,
        project_id: &str,
        expanded: bool,
    ) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_projects
             SET expanded = ?1, updated_at = ?2
             WHERE project_id = ?3",
        )
        .bind(expanded)
        .bind(&now)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn bind_session(
        &self,
        session_id: &str,
        project_id: &str,
        worktree_path: &str,
    ) -> crate::Result<ProjectSessionBindingRow> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut transaction = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO kairox_project_sessions (
                session_id, project_id, worktree_path, created_at, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(session_id) DO UPDATE SET
                project_id = ?2,
                worktree_path = ?3,
                updated_at = ?5",
        )
        .bind(session_id)
        .bind(project_id)
        .bind(worktree_path)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO kairox_session_visibility (session_id, visibility, updated_at)
             VALUES (?1, 'visible', ?2)
             ON CONFLICT(session_id) DO NOTHING",
        )
        .bind(session_id)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        self.get_session_binding(session_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound.into())
    }

    pub async fn get_session_binding(
        &self,
        session_id: &str,
    ) -> crate::Result<Option<ProjectSessionBindingRow>> {
        let row = sqlx::query_as::<_, ProjectSessionBindingRow>(
            "SELECT session_id, project_id, worktree_path, created_at, updated_at
             FROM kairox_project_sessions
             WHERE session_id = ?1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn set_session_visibility(
        &self,
        session_id: &str,
        visibility: &str,
    ) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO kairox_session_visibility (session_id, visibility, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET visibility = ?2, updated_at = ?3",
        )
        .bind(session_id)
        .bind(visibility)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_session_visibility(&self, session_id: &str) -> crate::Result<Option<String>> {
        let visibility = sqlx::query_scalar::<_, String>(
            "SELECT visibility
             FROM kairox_session_visibility
             WHERE session_id = ?1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(visibility)
    }

    pub async fn list_archived_sessions(
        &self,
        workspace_id: &str,
    ) -> crate::Result<Vec<ProjectSessionBindingRow>> {
        let rows = sqlx::query_as::<_, ProjectSessionBindingRow>(
            "SELECT bindings.session_id, bindings.project_id, bindings.worktree_path,
                    bindings.created_at, bindings.updated_at
             FROM kairox_project_sessions AS bindings
             INNER JOIN kairox_projects AS projects
                ON projects.project_id = bindings.project_id
             INNER JOIN kairox_session_visibility AS visibility
                ON visibility.session_id = bindings.session_id
             WHERE projects.workspace_id = ?1
               AND visibility.visibility = 'archived'
             ORDER BY bindings.updated_at DESC, bindings.session_id ASC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectMetaRepository;
    use crate::SqliteEventStore;

    #[tokio::test]
    async fn creates_lists_renames_and_removes_project() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let repository = ProjectMetaRepository::new(store.pool().clone());

        let project = repository
            .create_project("workspace-1", "Original", "/tmp/workspace", 10)
            .await
            .unwrap();

        repository
            .rename_project(&project.project_id, "Renamed")
            .await
            .unwrap();
        repository
            .remove_project(&project.project_id)
            .await
            .unwrap();

        let active_projects = repository
            .list_active_projects("workspace-1")
            .await
            .unwrap();
        let removed_projects = repository
            .list_removed_projects("workspace-1")
            .await
            .unwrap();

        assert!(active_projects.is_empty());
        assert_eq!(removed_projects.len(), 1);
        assert_eq!(removed_projects[0].display_name, "Renamed");
        assert!(removed_projects[0].removed_at.is_some());
    }
}
