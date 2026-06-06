use agent_core::autonomous::{AutonomousTaskGoal, Checkpoint};

pub struct OrientationPromptBuilder;

impl OrientationPromptBuilder {
    pub fn build(
        goal: &AutonomousTaskGoal,
        checkpoint: &Checkpoint,
        session_index: u32,
        max_sessions: u32,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str("# Autonomous Task — Continuation Session\n\n");
        prompt.push_str(&format!(
            "This is session {} of up to {} for the following goal.\n\n",
            session_index + 1,
            max_sessions
        ));

        prompt.push_str("## Original Goal\n\n");
        prompt.push_str(&goal.description);
        prompt.push_str("\n\n");

        if !goal.acceptance_criteria.is_empty() {
            prompt.push_str("## Acceptance Criteria\n\n");
            for criterion in &goal.acceptance_criteria {
                prompt.push_str(&format!("- {criterion}\n"));
            }
            prompt.push('\n');
        }

        if !checkpoint.completed_items.is_empty() {
            prompt.push_str("## Completed in Previous Sessions\n\n");
            for item in &checkpoint.completed_items {
                prompt.push_str(&format!("- ✅ {item}\n"));
            }
            prompt.push('\n');
        }

        if !checkpoint.remaining_items.is_empty() {
            prompt.push_str("## Remaining Work\n\n");
            for item in &checkpoint.remaining_items {
                prompt.push_str(&format!("- ⬜ {item}\n"));
            }
            prompt.push('\n');
        }

        if let Some(sha) = &checkpoint.git_sha {
            prompt.push_str(&format!("## Git Checkpoint\n\nLast commit: `{sha}`\n\n"));
        }

        if !checkpoint.verification_results.is_empty() {
            prompt.push_str("## Last Verification Results\n\n");
            for vr in &checkpoint.verification_results {
                let status = if vr.passed { "✅" } else { "❌" };
                prompt.push_str(&format!(
                    "- {status} {}: {}\n",
                    vr.criterion, vr.output_preview
                ));
            }
            prompt.push('\n');
        }

        if !checkpoint.notes.is_empty() {
            prompt.push_str(&format!(
                "## Notes from Previous Session\n\n{}\n\n",
                checkpoint.notes
            ));
        }

        if !goal.verification_commands.is_empty() {
            prompt.push_str("## Instructions\n\n");
            prompt.push_str(
                "1. **Verify previous work first**: Run the verification commands below and confirm existing work still passes before making changes.\n",
            );
            prompt.push_str(
                "2. **Continue from where the previous session left off**: Focus on the remaining items listed above.\n",
            );
            prompt.push_str(
                "3. **Commit your work**: Make incremental commits as you complete items.\n\n",
            );
            prompt.push_str("### Verification Commands\n\n");
            for cmd in &goal.verification_commands {
                prompt.push_str(&format!("```\n{cmd}\n```\n"));
            }
        }

        prompt
    }

    pub fn build_initial_prompt(goal: &AutonomousTaskGoal) -> String {
        let mut prompt = String::new();

        prompt.push_str("# Autonomous Task\n\n");
        prompt.push_str("## Goal\n\n");
        prompt.push_str(&goal.description);
        prompt.push_str("\n\n");

        if !goal.acceptance_criteria.is_empty() {
            prompt.push_str("## Acceptance Criteria\n\n");
            for criterion in &goal.acceptance_criteria {
                prompt.push_str(&format!("- {criterion}\n"));
            }
            prompt.push('\n');
        }

        if !goal.verification_commands.is_empty() {
            prompt.push_str("## Verification Commands\n\n");
            for cmd in &goal.verification_commands {
                prompt.push_str(&format!("```\n{cmd}\n```\n"));
            }
            prompt.push('\n');
        }

        prompt.push_str("## Instructions\n\n");
        prompt.push_str("Work through the acceptance criteria one by one. ");
        prompt.push_str(
            "After completing each item, run the verification commands to confirm it passes. ",
        );
        prompt.push_str("Commit your work incrementally.\n");

        prompt
    }
}

#[cfg(test)]
#[path = "orientation_tests.rs"]
mod tests;
