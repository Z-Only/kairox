//! State transitions for individual tasks within a [`TaskGraph`].

use agent_core::{TaskFailureReason, TaskId, TaskState};

use super::TaskGraph;

impl TaskGraph {
    pub fn mark_completed(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        task.state = TaskState::Completed;
        task.error = None;
        task.failure_reason = None;
        Ok(())
    }

    /// Mark a task as running. No-op if the task is already running or completed.
    /// Returns an error if the task ID is unknown.
    pub fn mark_running(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state == TaskState::Pending || task.state == TaskState::Ready {
            task.state = TaskState::Running;
        }
        Ok(())
    }

    /// Mark a task as failed with an error message. Returns an error if the task ID is unknown.
    pub fn mark_failed(&mut self, id: &TaskId, error: String) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        task.state = TaskState::Failed;
        task.error = Some(error);
        Ok(())
    }

    /// Mark a task as failed with a structured failure reason.
    pub fn mark_failed_with_reason(
        &mut self,
        id: &TaskId,
        error: String,
        reason: TaskFailureReason,
    ) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        task.state = TaskState::Failed;
        task.error = Some(error);
        task.failure_reason = Some(reason);
        Ok(())
    }

    /// Mark a task as blocked because a dependency failed.
    pub fn mark_blocked(&mut self, id: &TaskId, reason: String) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state == TaskState::Pending || task.state == TaskState::Ready {
            task.state = TaskState::Blocked;
            task.error = Some(reason);
        }
        Ok(())
    }

    /// Mark a task as skipped (user action to override a blocked/failed task).
    pub fn mark_skipped(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if !task.state.is_terminal()
            || task.state == TaskState::Failed
            || task.state == TaskState::Blocked
        {
            task.state = TaskState::Skipped;
            task.error = None;
            task.failure_reason = None;
        }
        Ok(())
    }

    /// Mark a task as cancelled.
    pub fn mark_cancelled(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if !task.state.is_terminal() {
            task.state = TaskState::Cancelled;
            task.error = Some("cancelled".into());
            task.failure_reason = Some(TaskFailureReason::Cancelled);
        }
        Ok(())
    }

    /// Reset a failed or blocked task back to pending (for retry).
    /// Increments retry_count. Returns error if the task is not retriable
    /// or has exhausted its retry budget.
    pub fn reset_to_pending(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state != TaskState::Failed && task.state != TaskState::Blocked {
            return Err(crate::RuntimeError::TaskCannotRetry(format!(
                "task {} is in state {:?}, cannot reset to pending (only Failed/Blocked allowed)",
                id, task.state
            )));
        }
        if task.retry_count >= task.max_retries {
            return Err(crate::RuntimeError::TaskCannotRetry(format!(
                "task {} has exhausted retry budget ({}/{})",
                id, task.retry_count, task.max_retries
            )));
        }

        task.state = TaskState::Pending;
        task.retry_count += 1;
        task.error = None;
        task.failure_reason = None;
        Ok(())
    }

    /// Explicitly mark a task as ready (Pending → Ready).
    pub fn mark_ready(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state == TaskState::Pending {
            task.state = TaskState::Ready;
        }
        Ok(())
    }
}
