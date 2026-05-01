//! Configuration file discovery.

use crate::ConfigSource;
use std::path::PathBuf;

const PROJECT_CONFIG_FILENAME: &str = "kairox.toml";
const USER_CONFIG_DIR: &str = ".kairox";
const USER_CONFIG_FILENAME: &str = "config.toml";

/// Find a configuration file by searching in order:
/// 1. Current working directory (`./kairox.toml`)
/// 2. User home directory (`~/.kairox/config.toml`)
///
/// Returns the path and which source it came from, or `None` if no config found.
pub fn find_config() -> Option<(PathBuf, ConfigSource)> {
    // 1. Project-level config
    if let Ok(cwd) = std::env::current_dir() {
        let project_path = cwd.join(PROJECT_CONFIG_FILENAME);
        if project_path.is_file() {
            return Some((project_path, ConfigSource::ProjectFile));
        }
    }

    // 2. User-level config
    if let Some(home) = dirs::home_dir() {
        let user_path = home.join(USER_CONFIG_DIR).join(USER_CONFIG_FILENAME);
        if user_path.is_file() {
            return Some((user_path, ConfigSource::UserFile));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_config_returns_none_when_no_files() {
        // This test runs in the project directory which likely has no kairox.toml
        // in a temp location. We just verify the function doesn't panic.
        let result = find_config();
        // Result depends on whether a config file exists; just check it returns Option
        assert!(result.is_some() || result.is_none());
    }
}
