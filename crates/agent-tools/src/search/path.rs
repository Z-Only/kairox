use std::path::{Path, PathBuf};

use crate::{Result, ToolError};

/// Locate the `rg` binary: env var > `which rg` > None.
pub fn find_rg_binary() -> Option<PathBuf> {
    // 1. Check KAIROX_RG_PATH env var
    if let Ok(path) = std::env::var("KAIROX_RG_PATH") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Run `which rg`
    if let Ok(output) = std::process::Command::new("which").arg("rg").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                let p = PathBuf::from(&path);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    None
}

/// Resolve a user-provided path relative to workspace root.
/// Rejects paths that escape the workspace via `..`, absolute paths,
/// or symlinks pointing outside.
pub fn resolve_search_path(workspace_root: &Path, rel_path: Option<&str>) -> Result<PathBuf> {
    let candidate = match rel_path {
        Some(p) => workspace_root.join(p),
        None => workspace_root.to_path_buf(),
    };

    let resolved = candidate
        .canonicalize()
        .map_err(|e| ToolError::ExecutionFailed(format!("cannot resolve search path: {}", e)))?;

    let root_resolved = workspace_root
        .canonicalize()
        .map_err(|e| ToolError::ExecutionFailed(format!("cannot resolve workspace root: {}", e)))?;

    if !resolved.starts_with(&root_resolved) {
        return Err(ToolError::ExecutionFailed(format!(
            "search path escapes workspace: {}",
            rel_path.unwrap_or(".")
        )));
    }

    Ok(resolved)
}

/// Simple glob matcher: supports `*.ext` and `*.{ext1,ext2}` patterns.
pub fn glob_matches(filename: &str, pattern: &str) -> bool {
    // Handle brace group: *.{rs,toml}
    if let Some(inner) = pattern
        .strip_prefix("*.{")
        .and_then(|s| s.strip_suffix('}'))
    {
        let extensions: Vec<&str> = inner.split(',').collect();
        return extensions
            .iter()
            .any(|ext| filename.ends_with(&format!(".{}", ext.trim())));
    }

    // Handle simple: *.ext
    if let Some(ext) = pattern.strip_prefix("*.") {
        return filename.ends_with(&format!(".{}", ext));
    }

    // Fallback: exact match
    filename == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_search_path ─────────────────────────────────────────────────

    #[test]
    fn resolve_valid_relative_path() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("subdir")).unwrap();
        let resolved = resolve_search_path(root.path(), Some("subdir")).unwrap();
        let root_canonical = root.path().canonicalize().unwrap();
        assert!(resolved.ends_with("subdir"));
        assert!(resolved.starts_with(&root_canonical));
    }

    #[test]
    fn resolve_none_path_returns_workspace_root() {
        let root = tempfile::tempdir().unwrap();
        let resolved = resolve_search_path(root.path(), None).unwrap();
        assert_eq!(resolved, root.path().canonicalize().unwrap());
    }

    #[test]
    fn resolve_rejects_dot_dot_traversal() {
        let root = tempfile::tempdir().unwrap();
        let result = resolve_search_path(root.path(), Some("../outside"));
        assert!(
            result.is_err(),
            "dot-dot traversal should be rejected, got {:?}",
            result
        );
    }

    #[test]
    fn resolve_rejects_dot_dot_inside_path() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("a")).unwrap();
        let result = resolve_search_path(root.path(), Some("a/../../outside"));
        assert!(
            result.is_err(),
            "dot-dot inside path should be rejected, got {:?}",
            result
        );
    }

    #[test]
    fn resolve_rejects_absolute_path() {
        let root = tempfile::tempdir().unwrap();
        let result = resolve_search_path(root.path(), Some("/etc/passwd"));
        assert!(
            result.is_err(),
            "absolute path should be rejected, got {:?}",
            result
        );
    }

    #[test]
    fn resolve_rejects_symlink_outside_workspace() {
        let root = tempfile::tempdir().unwrap();
        // Create a symlink inside workspace pointing to /tmp
        let symlink_path = root.path().join("escape");
        std::os::unix::fs::symlink("/tmp", &symlink_path).unwrap();
        let result = resolve_search_path(root.path(), Some("escape"));
        assert!(
            result.is_err(),
            "symlink outside workspace should be rejected, got {:?}",
            result
        );
    }

    #[test]
    fn resolve_allows_symlink_inside_workspace() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("real")).unwrap();
        std::os::unix::fs::symlink("real", root.path().join("link")).unwrap();
        let resolved = resolve_search_path(root.path(), Some("link")).unwrap();
        let root_canonical = root.path().canonicalize().unwrap();
        assert!(resolved.ends_with("real"));
        assert!(resolved.starts_with(&root_canonical));
    }

    #[test]
    fn resolve_nonexistent_path_errors() {
        let root = tempfile::tempdir().unwrap();
        let result = resolve_search_path(root.path(), Some("does_not_exist"));
        assert!(
            result.is_err(),
            "nonexistent path should fail canonicalize, got {:?}",
            result
        );
    }

    #[test]
    fn resolve_rejects_symlink_chain_outside() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("inner")).unwrap();
        // symlink A → inner (valid, inside)
        std::os::unix::fs::symlink("inner", root.path().join("link_a")).unwrap();
        // symlink B → /tmp (outside) — chained from inside
        std::os::unix::fs::symlink("/tmp", root.path().join("inner").join("escape")).unwrap();
        // Resolving "link_a/escape" should fail because it points to /tmp
        let result = resolve_search_path(root.path(), Some("link_a/escape"));
        assert!(
            result.is_err(),
            "symlink chain to outside should be rejected, got {:?}",
            result
        );
    }

    #[test]
    fn resolve_rejects_dangling_symlink() {
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("nonexistent_target", root.path().join("dangling")).unwrap();
        let result = resolve_search_path(root.path(), Some("dangling"));
        assert!(
            result.is_err(),
            "dangling symlink should fail canonicalize, got {:?}",
            result
        );
    }

    // ── glob_matches ──────────────────────────────────────────────────────

    #[test]
    fn glob_simple_extension() {
        assert!(glob_matches("main.rs", "*.rs"));
        assert!(glob_matches("lib.toml", "*.toml"));
        assert!(!glob_matches("main.ts", "*.rs"));
    }

    #[test]
    fn glob_brace_group() {
        assert!(glob_matches("main.rs", "*.{rs,toml}"));
        assert!(glob_matches("cargo.toml", "*.{rs,toml}"));
        assert!(!glob_matches("main.ts", "*.{rs,toml}"));
    }

    #[test]
    fn glob_brace_group_with_spaces() {
        assert!(glob_matches("main.rs", "*.{rs, toml}"));
    }

    #[test]
    fn glob_exact_match_fallback() {
        assert!(glob_matches("Cargo.toml", "Cargo.toml"));
        assert!(!glob_matches("cargo.toml", "Cargo.toml"));
    }

    #[test]
    fn glob_matches_no_asterisk_plain_ext() {
        // Without *, treated as exact filename match
        assert!(!glob_matches("main.rs", ".rs"));
        assert!(glob_matches("Cargo.toml", "Cargo.toml"));
        assert!(!glob_matches("Cargo.toml", "Cargo.lock"));
    }

    #[test]
    fn glob_matches_single_character() {
        // *.rs matches files ending with .rs
        assert!(glob_matches("foo.rs", "*.rs"));
        assert!(!glob_matches("foo.rst", "*.rs"));
        assert!(!glob_matches("foo.rs.backup", "*.rs"));
    }
}
