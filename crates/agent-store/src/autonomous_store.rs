use async_trait::async_trait;
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AutonomousTaskRow {
    pub autonomous_task_id: String,
    pub workspace_id: String,
    pub goal_json: String,
    pub config_json: String,
    pub state: String,
    pub current_session_id: Option<String>,
    pub session_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AutonomousCheckpointRow {
    pub checkpoint_id: String,
    pub autonomous_task_id: String,
    pub session_id: String,
    pub session_index: i64,
    pub checkpoint_json: String,
    pub end_reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionChainRow {
    pub autonomous_task_id: String,
    pub session_id: String,
    pub session_index: i64,
    pub created_at: String,
}

#[async_trait]
pub trait AutonomousTaskStore: Send + Sync {
    async fn create_autonomous_task(&self, row: &AutonomousTaskRow) -> crate::Result<()>;
    async fn get_autonomous_task(&self, id: &str) -> crate::Result<Option<AutonomousTaskRow>>;
    async fn update_autonomous_task_state(
        &self,
        id: &str,
        state: &str,
        current_session_id: Option<&str>,
    ) -> crate::Result<()>;
    async fn increment_session_count(&self, id: &str) -> crate::Result<u32>;
    async fn insert_checkpoint(&self, row: &AutonomousCheckpointRow) -> crate::Result<()>;
    async fn get_latest_checkpoint(
        &self,
        task_id: &str,
    ) -> crate::Result<Option<AutonomousCheckpointRow>>;
    async fn list_checkpoints(&self, task_id: &str) -> crate::Result<Vec<AutonomousCheckpointRow>>;
    async fn insert_session_chain_entry(
        &self,
        task_id: &str,
        session_id: &str,
        index: u32,
    ) -> crate::Result<()>;
    async fn list_session_chain(&self, task_id: &str) -> crate::Result<Vec<SessionChainRow>>;
    async fn list_active_autonomous_tasks(
        &self,
        workspace_id: &str,
    ) -> crate::Result<Vec<AutonomousTaskRow>>;
    async fn get_autonomous_task_for_session(
        &self,
        session_id: &str,
    ) -> crate::Result<Option<AutonomousTaskRow>>;
}

#[derive(Clone)]
pub struct SqliteAutonomousTaskStore {
    pool: SqlitePool,
}

impl SqliteAutonomousTaskStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> crate::Result<()> {
        if let Err(e) = sqlx::query(include_str!("../migrations/0010_autonomous_tasks.sql"))
            .execute(&self.pool)
            .await
        {
            let msg = e.to_string();
            if !msg.contains("already exists") && !msg.contains("duplicate") {
                return Err(crate::StoreError::Sqlx(e));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl AutonomousTaskStore for SqliteAutonomousTaskStore {
    async fn create_autonomous_task(&self, row: &AutonomousTaskRow) -> crate::Result<()> {
        sqlx::query(
            "INSERT INTO kairox_autonomous_tasks
             (autonomous_task_id, workspace_id, goal_json, config_json, state,
              current_session_id, session_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&row.autonomous_task_id)
        .bind(&row.workspace_id)
        .bind(&row.goal_json)
        .bind(&row.config_json)
        .bind(&row.state)
        .bind(&row.current_session_id)
        .bind(row.session_count)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_autonomous_task(&self, id: &str) -> crate::Result<Option<AutonomousTaskRow>> {
        let row = sqlx::query_as::<_, AutonomousTaskRow>(
            "SELECT autonomous_task_id, workspace_id, goal_json, config_json, state,
                    current_session_id, session_count, created_at, updated_at
             FROM kairox_autonomous_tasks
             WHERE autonomous_task_id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn update_autonomous_task_state(
        &self,
        id: &str,
        state: &str,
        current_session_id: Option<&str>,
    ) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_autonomous_tasks
             SET state = ?2, current_session_id = ?3, updated_at = ?4
             WHERE autonomous_task_id = ?1",
        )
        .bind(id)
        .bind(state)
        .bind(current_session_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn increment_session_count(&self, id: &str) -> crate::Result<u32> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE kairox_autonomous_tasks
             SET session_count = session_count + 1, updated_at = ?2
             WHERE autonomous_task_id = ?1",
        )
        .bind(id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let row: (i64,) = sqlx::query_as(
            "SELECT session_count FROM kairox_autonomous_tasks WHERE autonomous_task_id = ?1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0 as u32)
    }

    async fn insert_checkpoint(&self, row: &AutonomousCheckpointRow) -> crate::Result<()> {
        sqlx::query(
            "INSERT INTO kairox_autonomous_checkpoints
             (checkpoint_id, autonomous_task_id, session_id, session_index,
              checkpoint_json, end_reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&row.checkpoint_id)
        .bind(&row.autonomous_task_id)
        .bind(&row.session_id)
        .bind(row.session_index)
        .bind(&row.checkpoint_json)
        .bind(&row.end_reason)
        .bind(&row.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_latest_checkpoint(
        &self,
        task_id: &str,
    ) -> crate::Result<Option<AutonomousCheckpointRow>> {
        let row = sqlx::query_as::<_, AutonomousCheckpointRow>(
            "SELECT checkpoint_id, autonomous_task_id, session_id, session_index,
                    checkpoint_json, end_reason, created_at
             FROM kairox_autonomous_checkpoints
             WHERE autonomous_task_id = ?1
             ORDER BY session_index DESC
             LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn list_checkpoints(&self, task_id: &str) -> crate::Result<Vec<AutonomousCheckpointRow>> {
        let rows = sqlx::query_as::<_, AutonomousCheckpointRow>(
            "SELECT checkpoint_id, autonomous_task_id, session_id, session_index,
                    checkpoint_json, end_reason, created_at
             FROM kairox_autonomous_checkpoints
             WHERE autonomous_task_id = ?1
             ORDER BY session_index ASC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn insert_session_chain_entry(
        &self,
        task_id: &str,
        session_id: &str,
        index: u32,
    ) -> crate::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO kairox_autonomous_session_chain
             (autonomous_task_id, session_id, session_index, created_at)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(task_id)
        .bind(session_id)
        .bind(index as i64)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_session_chain(&self, task_id: &str) -> crate::Result<Vec<SessionChainRow>> {
        let rows = sqlx::query_as::<_, SessionChainRow>(
            "SELECT autonomous_task_id, session_id, session_index, created_at
             FROM kairox_autonomous_session_chain
             WHERE autonomous_task_id = ?1
             ORDER BY session_index ASC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn list_active_autonomous_tasks(
        &self,
        workspace_id: &str,
    ) -> crate::Result<Vec<AutonomousTaskRow>> {
        let rows = sqlx::query_as::<_, AutonomousTaskRow>(
            "SELECT autonomous_task_id, workspace_id, goal_json, config_json, state,
                    current_session_id, session_count, created_at, updated_at
             FROM kairox_autonomous_tasks
             WHERE workspace_id = ?1 AND state IN ('active', 'paused')
             ORDER BY created_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn get_autonomous_task_for_session(
        &self,
        session_id: &str,
    ) -> crate::Result<Option<AutonomousTaskRow>> {
        let row = sqlx::query_as::<_, AutonomousTaskRow>(
            "SELECT t.autonomous_task_id, t.workspace_id, t.goal_json, t.config_json, t.state,
                    t.current_session_id, t.session_count, t.created_at, t.updated_at
             FROM kairox_autonomous_tasks t
             JOIN kairox_autonomous_session_chain c ON t.autonomous_task_id = c.autonomous_task_id
             WHERE c.session_id = ?1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }
}

#[cfg(test)]
#[path = "autonomous_store_tests.rs"]
mod tests;
