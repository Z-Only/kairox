use crate::ToolError;
use std::path::{Path, PathBuf};

/// Normalize a path by resolving `.` and `..` components without requiring
/// the path to exist on disk. Returns the lexically normalized path.
pub(super) fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if let Some(std::path::Component::Normal(_)) = components.last() {
                    components.pop();
                } else {
                    components.push(comp);
                }
            }
            _ => components.push(comp),
        }
    }
    components.iter().collect()
}

/// Resolve a relative path within `workspace_root`.
/// For existing files, canonicalize and check it stays within root.
/// For new files (where the file doesn't exist yet), canonicalize the
/// parent directory and check that stays within root.
pub(super) fn resolve_workspace_path(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, ToolError> {
    // Canonicalize root once
    let root = workspace_root.canonicalize().map_err(|e| {
        ToolError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("workspace root not found: {}", e),
        ))
    })?;

    // Build candidate path using the canonicalized root to avoid
    // symlink mismatch issues (e.g., /var vs /private/var on macOS)
    let candidate = root.join(relative_path);

    // Lexical escape check: normalize and verify it stays within root
    let normalized = normalize_path(&candidate);
    if !normalized.starts_with(&root) {
        return Err(ToolError::WorkspaceEscape(relative_path.into()));
    }

    if candidate.exists() {
        // Existing file: full canonicalize check
        let canon = candidate.canonicalize()?;
        if !canon.starts_with(&root) {
            return Err(ToolError::WorkspaceEscape(relative_path.into()));
        }
        Ok(canon)
    } else {
        // New file: canonicalize parent directory
        if let Some(parent) = candidate.parent() {
            if parent.as_os_str().is_empty() || parent == root {
                // File is at the workspace root
                return Ok(root.join(
                    candidate
                        .file_name()
                        .ok_or_else(|| ToolError::WorkspaceEscape(relative_path.into()))?,
                ));
            }

            if parent.exists() {
                let canon_parent = parent.canonicalize()?;
                if !canon_parent.starts_with(&root) {
                    return Err(ToolError::WorkspaceEscape(relative_path.into()));
                }
                Ok(canon_parent.join(
                    candidate
                        .file_name()
                        .ok_or_else(|| ToolError::WorkspaceEscape(relative_path.into()))?,
                ))
            } else {
                Err(ToolError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("parent directory not found: {}", parent.display()),
                )))
            }
        } else {
            Err(ToolError::WorkspaceEscape(relative_path.into()))
        }
    }
}
