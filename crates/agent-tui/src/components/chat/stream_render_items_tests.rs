use super::*;
use agent_core::events::CompactionSkipReason;

#[test]
fn format_duration_renders_ms_under_one_second() {
    assert_eq!(format_duration(120), "120ms");
    assert_eq!(format_duration(999), "999ms");
}

#[test]
fn format_duration_renders_seconds_at_or_above_one_second() {
    assert_eq!(format_duration(1_000), "1.0s");
    assert_eq!(format_duration(2_500), "2.5s");
}

#[test]
fn truncate_chars_appends_ellipsis_when_long() {
    assert_eq!(truncate_chars("hello", 10), "hello");
    assert_eq!(truncate_chars("hello world", 5), "hello…");
}

#[test]
fn compaction_savings_pct_rounds_and_handles_edges() {
    assert_eq!(compaction_savings_pct(25_000, 12_000), 52);
    assert_eq!(compaction_savings_pct(10_000, 10_000), 0);
    assert_eq!(compaction_savings_pct(10_000, 12_000), 0);
    assert_eq!(compaction_savings_pct(0, 0), 0);
    assert_eq!(compaction_savings_pct(0, 5_000), 0);
    assert_eq!(compaction_savings_pct(10_000, 0), 100);
}

#[test]
fn skipped_compaction_not_enough_history_renders_reason_without_ratio() {
    let mut lines = Vec::new();

    append_compaction_skipped(&mut lines, CompactionSkipReason::NotEnoughHistory, 0.0);

    let rendered = lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Compaction skipped"));
    assert!(rendered.contains("not enough history"));
    assert!(!rendered.contains("ratio"));
    assert!(!rendered.contains("0.00"));
}
