use super::*;

#[test]
fn focus_manager_default_is_chat() {
    let fm = FocusManager::new(FocusTarget::Chat);
    assert_eq!(fm.current(), FocusTarget::Chat);
}

#[test]
fn focus_manager_push_pop_restores_previous() {
    let mut fm = FocusManager::new(FocusTarget::Chat);
    fm.push(FocusTarget::PermissionModal);
    assert_eq!(fm.current(), FocusTarget::PermissionModal);
    let popped = fm.pop();
    assert_eq!(popped, Some(FocusTarget::PermissionModal));
    assert_eq!(fm.current(), FocusTarget::Chat);
}

#[test]
fn focus_manager_pop_last_returns_none() {
    let mut fm = FocusManager::new(FocusTarget::Chat);
    assert_eq!(fm.pop(), None);
    assert_eq!(fm.current(), FocusTarget::Chat);
}

#[test]
fn focus_manager_cycle_wraps_around() {
    let mut fm = FocusManager::new(FocusTarget::Chat);
    assert_eq!(fm.current(), FocusTarget::Chat);

    fm.cycle_next();
    assert_eq!(fm.current(), FocusTarget::Sessions);

    fm.cycle_next();
    assert_eq!(fm.current(), FocusTarget::Trace);

    fm.cycle_next();
    assert_eq!(fm.current(), FocusTarget::Chat);
}

#[test]
fn focus_manager_set_replaces_top() {
    let mut fm = FocusManager::new(FocusTarget::Chat);

    fm.set(FocusTarget::Trace);
    assert_eq!(fm.current(), FocusTarget::Trace);

    fm.push(FocusTarget::PermissionModal);
    assert_eq!(fm.current(), FocusTarget::PermissionModal);

    fm.set(FocusTarget::Sessions);
    assert_eq!(fm.current(), FocusTarget::Sessions);

    assert_eq!(fm.pop(), Some(FocusTarget::Sessions));
    assert_eq!(fm.current(), FocusTarget::Trace);
}
