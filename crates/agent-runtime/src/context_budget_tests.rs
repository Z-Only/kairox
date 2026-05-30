use super::*;
use agent_models::LimitSource;

fn limits(ctx: u64, out: u64) -> ModelLimits {
    ModelLimits {
        context_window: ctx,
        output_limit: out,
        source: LimitSource::BuiltinRegistry,
    }
}

#[test]
fn build_budget_reserves_output_plus_safety_margin() {
    let b = build_budget(&limits(200_000, 8_192));
    assert_eq!(b.context_window, 200_000);
    // 8192 + max(819, 2000) = 8192 + 2000 = 10192
    assert_eq!(b.output_reservation, 10_192);
    assert_eq!(b.input_budget(), 200_000 - 10_192);
}

#[test]
fn build_budget_safety_floor_kicks_in_for_small_models() {
    let b = build_budget(&limits(8_000, 1_024));
    // 1024 + max(102, 2000) = 1024 + 2000 = 3024
    assert_eq!(b.output_reservation, 3_024);
}

#[test]
fn corrector_default_is_identity() {
    let c = UsageCorrector::default();
    assert_eq!(c.apply(1_000), 1_000);
}

#[test]
fn corrector_first_sample_takes_clamped_ratio() {
    let mut c = UsageCorrector::default();
    c.update(1_200, 1_000); // ratio 1.2
    assert!((c.ratio - 1.2).abs() < 1e-3);
    assert_eq!(c.apply(1_000), 1_200);
}

#[test]
fn corrector_clamps_pathological_ratios() {
    let mut c = UsageCorrector::default();
    c.update(10_000, 1_000); // ratio 10 → clamped 1.5
    assert!((c.ratio - 1.5).abs() < 1e-3);
    c.update(100, 1_000); // ratio 0.1 → clamped 0.7
                          // EMA: 1.5*0.6 + 0.7*0.4 = 1.18
    assert!((c.ratio - 1.18).abs() < 1e-2);
}

#[test]
fn corrector_ignores_zero_last_estimate() {
    let mut c = UsageCorrector::default();
    c.update(500, 0);
    assert_eq!(c.ratio, 1.0);
    assert_eq!(c.samples, 0);
}
