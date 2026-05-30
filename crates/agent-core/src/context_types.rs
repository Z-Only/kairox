use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    System,
    ProjectInstruction,
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub total_tokens: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub budget_tokens: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub context_window: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub output_reservation: u64,
    #[cfg_attr(feature = "specta", specta(skip))]
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
#[path = "context_types_tests.rs"]
mod tests;
