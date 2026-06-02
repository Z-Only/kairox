use super::*;
use std::fs;

#[test]
fn devtools_defaults_to_release_off_and_debug_on() {
    assert!(!default_devtools_enabled_for(false));
    assert!(default_devtools_enabled_for(true));
}

#[test]
fn gui_settings_uses_build_default_when_file_is_missing() {
    let temp = tempfile::tempdir().unwrap();

    let view = read_gui_settings(temp.path(), false, None).unwrap();

    assert!(!view.devtools_enabled);
    assert!(!view.default_devtools_enabled);
    assert!(!view.requires_restart);
}

#[test]
fn gui_settings_persists_devtools_override() {
    let temp = tempfile::tempdir().unwrap();

    let view = write_gui_devtools_enabled(temp.path(), true, false, Some(false)).unwrap();
    let persisted = read_gui_settings(temp.path(), false, Some(false)).unwrap();
    let raw = fs::read_to_string(temp.path().join("gui-settings.toml")).unwrap();

    assert!(view.devtools_enabled);
    assert!(view.requires_restart);
    assert!(persisted.devtools_enabled);
    assert!(raw.contains("devtools_enabled = true"));
}

#[test]
fn gui_settings_restart_flag_tracks_running_window_state() {
    let temp = tempfile::tempdir().unwrap();

    let unchanged = write_gui_devtools_enabled(temp.path(), false, false, Some(false)).unwrap();
    let changed = write_gui_devtools_enabled(temp.path(), true, false, Some(false)).unwrap();
    let changed_back = write_gui_devtools_enabled(temp.path(), false, false, Some(false)).unwrap();

    assert!(!unchanged.requires_restart);
    assert!(changed.requires_restart);
    assert!(!changed_back.requires_restart);
}
