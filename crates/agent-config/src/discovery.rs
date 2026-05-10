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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_config(path: &std::path::Path) {
        std::fs::create_dir_all(path.parent().expect("config has parent"))
            .expect("create config parent");
        std::fs::write(
            path,
            "[profiles.fast]\nprovider = \"fake\"\nmodel_id = \"fake\"\n",
        )
        .expect("write config");
    }

    #[test]
    fn project_local_config_wins_over_user_config() {
        let project_dir = TempDir::new().expect("project temp dir");
        let home_dir = TempDir::new().expect("home temp dir");
        let project_config = project_dir.path().join(".kairox/config.toml");
        let user_config = home_dir.path().join(".kairox/config.toml");
        write_config(&project_config);
        write_config(&user_config);

        let (path, source) = find_config_from(Some(project_dir.path()), Some(home_dir.path()))
            .expect("project config is discovered");

        assert_eq!(path, project_config);
        assert_eq!(source, ConfigSource::ProjectFile);
    }

    #[test]
    fn user_config_is_used_when_project_config_is_absent() {
        let project_dir = TempDir::new().expect("project temp dir");
        let home_dir = TempDir::new().expect("home temp dir");
        let user_config = home_dir.path().join(".kairox/config.toml");
        write_config(&user_config);

        let (path, source) = find_config_from(Some(project_dir.path()), Some(home_dir.path()))
            .expect("user config is discovered");

        assert_eq!(path, user_config);
        assert_eq!(source, ConfigSource::UserFile);
    }

    #[test]
    fn user_config_is_used_when_current_dir_is_unavailable() {
        let home_dir = TempDir::new().expect("home temp dir");
        let user_config = home_dir.path().join(".kairox/config.toml");
        write_config(&user_config);

        let (path, source) = find_config_from(None, Some(home_dir.path()))
            .expect("user config is discovered without cwd");

        assert_eq!(path, user_config);
        assert_eq!(source, ConfigSource::UserFile);
    }

    #[test]
    fn legacy_project_root_config_is_ignored() {
        let project_dir = TempDir::new().expect("project temp dir");
        let legacy_config = project_dir.path().join("kairox.toml");
        std::fs::write(
            &legacy_config,
            "[profiles.fast]\nprovider = \"fake\"\nmodel_id = \"fake\"\n",
        )
        .expect("write legacy config");

        let result = find_config_from(Some(project_dir.path()), None);

        assert!(result.is_none());
    }

    #[test]
    fn no_config_returns_none() {
        let project_dir = TempDir::new().expect("project temp dir");

        let result = find_config_from(Some(project_dir.path()), None);

        assert!(result.is_none());
    }
}
