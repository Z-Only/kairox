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

    #[test]
    fn find_config_upward_discovers_in_parent() {
        let project_dir = TempDir::new().expect("project temp dir");
        let nested_dir = project_dir.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested_dir).expect("create nested dirs");
        let config_path = project_dir.path().join(".kairox/config.toml");
        write_config(&config_path);

        let (path, source) =
            find_config_upward(&nested_dir).expect("config found via upward search");
        assert_eq!(path, config_path);
        assert_eq!(source, ConfigSource::ProjectFile);
    }

    #[test]
    fn find_config_upward_stops_after_5_levels() {
        let project_dir = TempDir::new().expect("project temp dir");
        // Create config 6 levels above — should NOT be found
        let deep_dir = (0..=6).fold(project_dir.path().to_path_buf(), |p, i| {
            let d = p.join(format!("d{i}"));
            std::fs::create_dir_all(&d).expect("create dir");
            d
        });
        let config_path = project_dir.path().join(".kairox/config.toml");
        write_config(&config_path);

        let result = find_config_upward(&deep_dir);
        assert!(result.is_none(), "should not find config beyond 5 levels");
    }

    #[test]
    fn find_config_upward_returns_none_when_no_config() {
        let project_dir = TempDir::new().expect("project temp dir");
        let nested = project_dir.path().join("a").join("b");
        std::fs::create_dir_all(&nested).expect("create nested dirs");

        let result = find_config_upward(&nested);
        assert!(result.is_none());
    }
}
