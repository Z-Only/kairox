use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    System,
    ToolDefinitions,
    Request,
    Memory,
    History,
    ToolResult,
    SelectedFile,
    CompactionSummary,
    Skill,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ContextUsage {
    pub total_tokens: u64,
    pub budget_tokens: u64,
    pub context_window: u64,
    pub output_reservation: u64,
    pub by_source: Vec<(ContextSource, u64)>,
    pub estimator: String,
    pub corrected_by_real_usage: bool,
}

impl ContextUsage {
    pub fn ratio(&self) -> f32 {
        if self.budget_tokens == 0 {
            0.0
        } else {
            self.total_tokens as f32 / self.budget_tokens as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_returns_fraction_of_budget_consumed() {
        let usage = ContextUsage {
            total_tokens: 60_000,
            budget_tokens: 200_000,
            context_window: 200_000,
            output_reservation: 0,
            by_source: vec![(ContextSource::System, 60_000)],
            estimator: "cl100k_base".into(),
            corrected_by_real_usage: false,
        };
        assert!((usage.ratio() - 0.30).abs() < 1e-4);
    }

    #[test]
    fn context_source_serializes_snake_case_with_new_variants() {
        assert_eq!(
            serde_json::to_value(ContextSource::ToolDefinitions).unwrap(),
            "tool_definitions"
        );
        assert_eq!(
            serde_json::to_value(ContextSource::CompactionSummary).unwrap(),
            "compaction_summary"
        );
        assert_eq!(serde_json::to_value(ContextSource::Skill).unwrap(), "skill");
    }

    #[test]
    fn context_usage_round_trips_through_json() {
        let usage = ContextUsage {
            total_tokens: 1_234,
            budget_tokens: 200_000,
            context_window: 200_000,
            output_reservation: 9_000,
            by_source: vec![
                (ContextSource::System, 800),
                (ContextSource::ToolDefinitions, 434),
            ],
            estimator: "cl100k_base".into(),
            corrected_by_real_usage: true,
        };
        let json = serde_json::to_value(&usage).unwrap();
        let back: ContextUsage = serde_json::from_value(json).unwrap();
        assert_eq!(back, usage);
    }
}
