use agent_core::ContextSource;

/// Token budget for the context assembly pass. Combines the active model's
/// context window with the reservation for the upcoming completion and any
/// optional per-source soft caps.
#[derive(Debug, Clone)]
pub struct ContextBudget {
    /// Total context window of the active model (e.g. 200_000 for Sonnet 4).
    pub context_window: u64,
    /// Tokens reserved for the upcoming completion. Effective input budget
    /// is `context_window - output_reservation`.
    pub output_reservation: u64,
    /// Optional per-source soft caps (applied before the global drop pass).
    pub source_caps: Vec<(ContextSource, u64)>,
}

impl ContextBudget {
    pub fn input_budget(&self) -> u64 {
        self.context_window.saturating_sub(self.output_reservation)
    }
}
