//! Per-session helpers for converting `ModelLimits` into a `ContextBudget`
//! and for EMA-correcting our cl100k_base estimate against the real
//! `ModelUsage` returned by providers.

use agent_memory::ContextBudget;
use agent_models::ModelLimits;

/// Convert a model's limits into a context budget.
///
/// We reserve `output_limit + 10%` (clamped to a 2k floor) so the input
/// fits when the model writes its longest legal completion.
pub fn build_budget(limits: &ModelLimits) -> ContextBudget {
    let safety = (limits.output_limit / 10).max(2_000);
    ContextBudget {
        context_window: limits.context_window,
        output_reservation: limits.output_limit + safety,
        source_caps: vec![],
    }
}

/// EMA-corrects our cl100k_base token estimate against real
/// `input_tokens` reported by the provider. Clamped to [0.7, 1.5] so a
/// single broken usage report can't blow up the budget.
#[derive(Debug, Clone)]
pub struct UsageCorrector {
    pub ratio: f32,
    pub samples: u32,
}

impl Default for UsageCorrector {
    fn default() -> Self {
        Self {
            ratio: 1.0,
            samples: 0,
        }
    }
}

impl UsageCorrector {
    pub fn apply(&self, estimated: u64) -> u64 {
        ((estimated as f32) * self.ratio).round() as u64
    }

    pub fn update(&mut self, real_input_tokens: u64, last_estimate: u64) {
        if last_estimate == 0 {
            return;
        }
        let new_ratio = (real_input_tokens as f32) / (last_estimate as f32);
        let clamped = new_ratio.clamp(0.7, 1.5);
        // simple EMA with alpha=0.4
        self.ratio = if self.samples == 0 {
            clamped
        } else {
            self.ratio * 0.6 + clamped * 0.4
        };
        self.samples += 1;
    }
}

#[cfg(test)]
#[path = "context_budget_tests.rs"]
mod tests;
