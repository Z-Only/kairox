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
#[path = "path_tests.rs"]
mod tests;
