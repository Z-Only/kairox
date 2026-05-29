use super::*;

// -- Ctrl-C progressive exit --------------------------------------------

#[test]
fn ctrl_c_progressive_exit_interrupt_then_confirm_then_force() {
    let mut state = AppState::new("fake");

    assert_eq!(state.record_ctrl_c(), CtrlCAction::Interrupt);
    assert_eq!(state.record_ctrl_c(), CtrlCAction::ConfirmQuit);
    assert_eq!(state.record_ctrl_c(), CtrlCAction::ForceQuit);
}

#[test]
fn ctrl_c_resets_after_timeout() {
    let mut state = AppState::new("fake");

    assert_eq!(state.record_ctrl_c(), CtrlCAction::Interrupt);

    state.last_ctrl_c = Some(Instant::now() - Duration::from_secs(6));

    assert_eq!(state.record_ctrl_c(), CtrlCAction::Interrupt);
    assert_eq!(state.ctrl_c_count, 1);
}

#[test]
fn status_log_keeps_latest_entries_only() {
    let mut state = AppState::new("fake");

    for index in 0..105 {
        state.push_status_message(format!("status {index}"));
    }

    assert_eq!(state.status_log.len(), AppState::STATUS_LOG_LIMIT);
    assert_eq!(
        state
            .latest_status_message()
            .map(|entry| entry.message.as_str()),
        Some("status 104")
    );
    assert_eq!(
        state.status_log.first().map(|entry| entry.message.as_str()),
        Some("status 5")
    );
}
