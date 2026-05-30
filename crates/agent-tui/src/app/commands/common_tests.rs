use super::*;

#[cfg(target_os = "macos")]
#[test]
fn system_file_manager_command_uses_open_on_macos_with_path_arg() {
    let path = std::path::PathBuf::from("/tmp/kairox-target");
    let cmd = system_file_manager_command(&path);
    assert_eq!(cmd.get_program(), "open");
    let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
    assert_eq!(args, vec![path.as_os_str()]);
}

#[cfg(target_os = "windows")]
#[test]
fn system_file_manager_command_uses_explorer_on_windows_with_path_arg() {
    let path = std::path::PathBuf::from("C:/kairox-target");
    let cmd = system_file_manager_command(&path);
    assert_eq!(cmd.get_program(), "explorer");
    let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
    assert_eq!(args, vec![path.as_os_str()]);
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
#[test]
fn system_file_manager_command_uses_xdg_open_on_other_platforms_with_path_arg() {
    let path = std::path::PathBuf::from("/tmp/kairox-target");
    let cmd = system_file_manager_command(&path);
    assert_eq!(cmd.get_program(), "xdg-open");
    let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
    assert_eq!(args, vec![path.as_os_str()]);
}

#[test]
fn project_config_path_appends_kairox_config_toml_to_current_dir() {
    // current_dir() is the cargo test working directory, which always
    // exists; we don't mutate it. The assertion only checks the suffix
    // so this stays hermetic across hosts.
    let path = project_config_path().expect("current_dir should resolve");
    assert!(
        path.ends_with(".kairox/config.toml"),
        "expected suffix .kairox/config.toml, got {}",
        path.display()
    );
    assert!(
        path.is_absolute(),
        "current_dir should produce an absolute path, got {}",
        path.display()
    );
}

#[test]
fn user_config_path_returns_pathbuf_ending_with_kairox_config_toml() {
    // Avoid mutating HOME (Rust 2024 marks set_var unsafe and tests run
    // multi-threaded by default). Instead rely on the function's
    // structural invariant: regardless of whether HOME is set, the
    // result always ends with the .kairox/config.toml suffix.
    let path = user_config_path();
    assert!(
        path.ends_with(".kairox/config.toml"),
        "expected suffix .kairox/config.toml, got {}",
        path.display()
    );
}
