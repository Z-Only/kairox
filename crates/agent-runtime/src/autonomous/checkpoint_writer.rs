use agent_core::autonomous::{
    AutonomousTaskGoal, Checkpoint, SessionEndReason, VerificationResult,
};
use agent_core::{DomainEvent, EventPayload, SessionId};
use chrono::Utc;

pub struct CheckpointWriter;

impl CheckpointWriter {
    pub fn build_checkpoint(
        session_events: &[DomainEvent],
        goal: &AutonomousTaskGoal,
        session_id: &SessionId,
        session_index: u32,
        git_sha: Option<String>,
        verification_outputs: Vec<VerificationResult>,
    ) -> Checkpoint {
        let (completed, failed) = Self::extract_task_outcomes(session_events);
        let remaining = Self::compute_remaining(&goal.acceptance_criteria, &completed);
        let end_reason = Self::detect_end_reason(session_events);

        let notes = Self::build_notes(session_events, &end_reason);

        Checkpoint {
            checkpoint_id: format!("ckpt_{}", uuid::Uuid::new_v4().simple()),
            session_id: session_id.clone(),
            session_index,
            git_sha,
            completed_items: completed,
            remaining_items: remaining,
            verification_results: if verification_outputs.is_empty() {
                failed
                    .iter()
                    .map(|f| VerificationResult {
                        criterion: f.clone(),
                        passed: false,
                        output_preview: "task failed".into(),
                    })
                    .collect()
            } else {
                verification_outputs
            },
            notes,
            created_at: Utc::now(),
        }
    }

    pub fn detect_end_reason(events: &[DomainEvent]) -> SessionEndReason {
        for event in events.iter().rev() {
            match &event.payload {
                EventPayload::SessionCancelled { .. } => return SessionEndReason::UserPaused,
                EventPayload::AgentTaskFailed { error, .. } => {
                    if error.contains("max iterations") {
                        return SessionEndReason::MaxIterationsReached;
                    }
                    return SessionEndReason::TaskFailed;
                }
                EventPayload::AgentTaskCompleted { .. } => {
                    return SessionEndReason::TaskCompleted;
                }
                EventPayload::ContextCompactionCompleted { .. }
                    if Self::context_near_limit(events) =>
                {
                    return SessionEndReason::ContextLimitReached;
                }
                _ => {}
            }
        }
        SessionEndReason::TaskCompleted
    }

    fn extract_task_outcomes(events: &[DomainEvent]) -> (Vec<String>, Vec<String>) {
        let mut completed = Vec::new();
        let mut failed = Vec::new();
        let mut task_titles: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for event in events {
            match &event.payload {
                EventPayload::AgentTaskCreated { task_id, title, .. } => {
                    task_titles.insert(task_id.to_string(), title.clone());
                }
                EventPayload::AgentTaskCompleted { task_id } => {
                    let title = task_titles
                        .get(task_id.as_str())
                        .cloned()
                        .unwrap_or_else(|| task_id.to_string());
                    completed.push(title);
                }
                EventPayload::AgentTaskFailed { task_id, error } => {
                    let title = task_titles
                        .get(task_id.as_str())
                        .cloned()
                        .unwrap_or_else(|| task_id.to_string());
                    failed.push(format!("{title}: {error}"));
                }
                _ => {}
            }
        }
        (completed, failed)
    }

    fn compute_remaining(criteria: &[String], completed: &[String]) -> Vec<String> {
        criteria
            .iter()
            .filter(|c| !completed.iter().any(|done| done.contains(c.as_str())))
            .cloned()
            .collect()
    }

    fn context_near_limit(events: &[DomainEvent]) -> bool {
        for event in events.iter().rev() {
            if let EventPayload::ContextAssembled { usage } = &event.payload {
                return usage.ratio() >= 0.85;
            }
        }
        false
    }

    fn build_notes(events: &[DomainEvent], end_reason: &SessionEndReason) -> String {
        let mut files_touched = Vec::new();
        for event in events {
            if let EventPayload::FilePatchApplied { patch_id } = &event.payload {
                files_touched.push(patch_id.clone());
            }
        }

        let mut notes = format!("Session ended: {end_reason}.");
        if !files_touched.is_empty() {
            notes.push_str(&format!(" Files patched: {}.", files_touched.len()));
        }
        notes
    }
}

#[cfg(test)]
#[path = "checkpoint_writer_tests.rs"]
mod tests;
