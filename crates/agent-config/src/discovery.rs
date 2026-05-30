//! Configuration file discovery.

use crate::ConfigSource;
use std::path::{Path, PathBuf};

const CONFIG_DIR: &str = ".kairox";
const CONFIG_FILENAME: &str = "config.toml";

/// Find a configuration file by searching in order:
/// 1. Current working directory (`./.kairox/config.toml`)
/// 2. User home directory (`~/.kairox/config.toml`)
///
/// Returns the path and which source it came from, or `None` if no config found.
pub fn find_config() -> Option<(PathBuf, ConfigSource)> {
    let cwd = std::env::current_dir().ok();
    find_config_from(cwd.as_deref(), dirs::home_dir().as_deref())
}

fn find_config_from(cwd: Option<&Path>, home: Option<&Path>) -> Option<(PathBuf, ConfigSource)> {
    if let Some(project_dir) = cwd {
        let project_path = project_dir.join(CONFIG_DIR).join(CONFIG_FILENAME);
        if project_path.is_file() {
            return Some((project_path, ConfigSource::ProjectFile));
        }
    }

    if let Some(home_dir) = home {
        let user_path = home_dir.join(CONFIG_DIR).join(CONFIG_FILENAME);
        if user_path.is_file() {
            return Some((user_path, ConfigSource::UserFile));
        }
    }

    None
}

/// Walk up from `start_dir` to at most 5 parent directories looking for
/// `.kairox/config.toml`. Returns the path and `ConfigSource::ProjectFile`
/// when found, or `None`.
pub fn find_config_upward(start_dir: &Path) -> Option<(PathBuf, ConfigSource)> {
    let mut current = Some(start_dir);
    for _ in 0..=5 {
        let dir = current?;
        let candidate = dir.join(CONFIG_DIR).join(CONFIG_FILENAME);
        if candidate.is_file() {
            return Some((candidate, ConfigSource::ProjectFile));
        }
        current = dir.parent();
    }
    None
}

/// Find a local override config at `<project_root>/.kairox/config.local.toml`.
/// This file is gitignored and provides per-developer overrides that take
/// the highest priority.
pub fn find_local_config(project_root: Option<&Path>) -> Option<PathBuf> {
    let root = project_root?;
    let local = root.join(CONFIG_DIR).join("config.local.toml");
    if local.exists() {
        Some(local)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
