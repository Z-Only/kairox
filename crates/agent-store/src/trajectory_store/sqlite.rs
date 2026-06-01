use agent_core::trajectory::{TrajectoryId, TrajectoryMeta, TrajectoryOutcome, TrajectoryStep};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;

use super::TrajectoryStore;

#[derive(Clone)]
pub struct SqliteTrajectoryStore {
    pool: SqlitePool,
}

impl SqliteTrajectoryStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> crate::Result<()> {
        if let Err(e) = sqlx::query(include_str!("../../migrations/0009_trajectories.sql"))
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
impl TrajectoryStore for SqliteTrajectoryStore {
    async fn start_trajectory(
        &self,
        trajectory_id: &TrajectoryId,
        task_id: &str,
        session_id: &str,
    ) -> crate::Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO trajectories (trajectory_id, task_id, session_id, started_at, outcome)
             VALUES (?1, ?2, ?3, ?4, 'in_progress')",
        )
        .bind(&trajectory_id.0)
        .bind(task_id)
        .bind(session_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn record_step(
        &self,
        trajectory_id: &TrajectoryId,
        step: &TrajectoryStep,
    ) -> crate::Result<()> {
        let action_input = serde_json::to_string(&step.action_input)?;
        let timestamp = step.timestamp.to_rfc3339();
        sqlx::query(
            "INSERT INTO trajectory_steps
             (trajectory_id, step_index, action, action_input, observation, screenshot_id, timestamp, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&trajectory_id.0)
        .bind(step.step_index)
        .bind(&step.action)
        .bind(&action_input)
        .bind(&step.observation)
        .bind(&step.screenshot_id)
        .bind(&timestamp)
        .bind(step.duration_ms as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn complete_trajectory(
        &self,
        trajectory_id: &TrajectoryId,
        outcome: TrajectoryOutcome,
    ) -> crate::Result<()> {
        let now = Utc::now().to_rfc3339();
        let outcome_str = serde_json::to_value(&outcome)?
            .as_str()
            .unwrap_or("failed")
            .to_string();
        sqlx::query(
            "UPDATE trajectories SET completed_at = ?1, outcome = ?2 WHERE trajectory_id = ?3",
        )
        .bind(&now)
        .bind(&outcome_str)
        .bind(&trajectory_id.0)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn load_steps(&self, trajectory_id: &TrajectoryId) -> crate::Result<Vec<TrajectoryStep>> {
        let rows = sqlx::query_as::<_, StepRow>(
            "SELECT step_index, action, action_input, observation, screenshot_id, timestamp, duration_ms
             FROM trajectory_steps
             WHERE trajectory_id = ?1
             ORDER BY step_index ASC",
        )
        .bind(&trajectory_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    async fn get_meta(
        &self,
        trajectory_id: &TrajectoryId,
    ) -> crate::Result<Option<TrajectoryMeta>> {
        let row = sqlx::query_as::<_, TrajectoryRow>(
            "SELECT trajectory_id, task_id, session_id, started_at, completed_at, outcome
             FROM trajectories
             WHERE trajectory_id = ?1",
        )
        .bind(&trajectory_id.0)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(r.into_meta(&self.pool).await?)),
            None => Ok(None),
        }
    }

    async fn list_by_session(&self, session_id: &str) -> crate::Result<Vec<TrajectoryMeta>> {
        let rows = sqlx::query_as::<_, TrajectoryRow>(
            "SELECT trajectory_id, task_id, session_id, started_at, completed_at, outcome
             FROM trajectories
             WHERE session_id = ?1
             ORDER BY started_at ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        let mut metas = Vec::with_capacity(rows.len());
        for row in rows {
            metas.push(row.into_meta(&self.pool).await?);
        }
        Ok(metas)
    }

    async fn export_json(&self, trajectory_id: &TrajectoryId) -> crate::Result<serde_json::Value> {
        let meta = self.get_meta(trajectory_id).await?;
        let steps = self.load_steps(trajectory_id).await?;
        Ok(serde_json::json!({
            "meta": meta,
            "steps": steps,
        }))
    }
}

#[derive(Debug, sqlx::FromRow)]
struct TrajectoryRow {
    trajectory_id: String,
    task_id: String,
    session_id: String,
    started_at: String,
    completed_at: Option<String>,
    outcome: String,
}

impl TrajectoryRow {
    async fn into_meta(self, pool: &SqlitePool) -> crate::Result<TrajectoryMeta> {
        let step_count: i32 =
            sqlx::query_scalar("SELECT COUNT(*) FROM trajectory_steps WHERE trajectory_id = ?1")
                .bind(&self.trajectory_id)
                .fetch_one(pool)
                .await?;

        let started_at = chrono::DateTime::parse_from_rfc3339(&self.started_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| Utc::now());

        let completed_at = self.completed_at.as_deref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        });

        let outcome = match self.outcome.as_str() {
            "success" => TrajectoryOutcome::Success,
            "failed" => TrajectoryOutcome::Failed,
            "cancelled" => TrajectoryOutcome::Cancelled,
            _ => TrajectoryOutcome::InProgress,
        };

        Ok(TrajectoryMeta {
            trajectory_id: TrajectoryId(self.trajectory_id),
            task_id: self.task_id,
            session_id: self.session_id,
            started_at,
            completed_at,
            step_count: step_count as u32,
            outcome,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct StepRow {
    step_index: i32,
    action: String,
    action_input: String,
    observation: String,
    screenshot_id: Option<String>,
    timestamp: String,
    duration_ms: i64,
}

impl TryFrom<StepRow> for TrajectoryStep {
    type Error = crate::StoreError;

    fn try_from(row: StepRow) -> std::result::Result<Self, Self::Error> {
        let action_input: serde_json::Value = serde_json::from_str(&row.action_input)?;
        let timestamp = chrono::DateTime::parse_from_rfc3339(&row.timestamp)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(TrajectoryStep {
            step_index: row.step_index as u32,
            action: row.action,
            action_input,
            observation: row.observation,
            screenshot_id: row.screenshot_id,
            timestamp,
            duration_ms: row.duration_ms as u64,
        })
    }
}
