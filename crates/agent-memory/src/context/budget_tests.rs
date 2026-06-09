use super::*;
use agent_core::ContextSource;

#[test]
fn input_budget_subtracts_output_reservation() {
    let budget = ContextBudget {
        context_window: 200_000,
        output_reservation: 16_384,
        source_caps: Vec::new(),
    };
    assert_eq!(budget.input_budget(), 200_000 - 16_384);
}

#[test]
fn input_budget_saturates_at_zero() {
    let budget = ContextBudget {
        context_window: 1_000,
        output_reservation: 5_000,
        source_caps: Vec::new(),
    };
    assert_eq!(budget.input_budget(), 0);
}

#[test]
fn input_budget_with_zero_reservation() {
    let budget = ContextBudget {
        context_window: 128_000,
        output_reservation: 0,
        source_caps: Vec::new(),
    };
    assert_eq!(budget.input_budget(), 128_000);
}

#[test]
fn source_caps_stored_correctly() {
    let budget = ContextBudget {
        context_window: 100_000,
        output_reservation: 10_000,
        source_caps: vec![
            (ContextSource::History, 5_000),
            (ContextSource::ToolResult, 3_000),
        ],
    };
    assert_eq!(budget.source_caps.len(), 2);
    assert_eq!(budget.source_caps[0], (ContextSource::History, 5_000));
    assert_eq!(budget.source_caps[1], (ContextSource::ToolResult, 3_000));
}

#[test]
fn clone_produces_independent_copy() {
    let budget = ContextBudget {
        context_window: 50_000,
        output_reservation: 8_000,
        source_caps: vec![(ContextSource::Memory, 2_000)],
    };
    let cloned = budget.clone();
    assert_eq!(cloned.context_window, 50_000);
    assert_eq!(cloned.output_reservation, 8_000);
    assert_eq!(cloned.input_budget(), budget.input_budget());
}
