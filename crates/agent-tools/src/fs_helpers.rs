use std::path::{Path, PathBuf};

/// Validate a read path: the path must already exist and resolve inside the workspace.
pub fn resolve_workspace_read_path(
    workspace_root: &Path,
    relative_path: &str,
) -> crate::Result<PathBuf> {
    let candidate = workspace_root.join(relative_path);
    let root = workspace_root.canonicalize()?;
    let path = candidate.canonicalize()?;
    if path.starts_with(&root) {
        Ok(path)
    } else {
        Err(crate::ToolError::WorkspaceEscape(relative_path.into()))
    }
}

/// Validate a write path: the file may not exist yet, but the resolved path must
/// stay inside the workspace. Rejects paths containing `..`. If the file exists,
/// canonicalize and check. Otherwise, validate via the nearest existing parent.
pub fn resolve_workspace_write_path(
    workspace_root: &Path,
    relative_path: &str,
) -> crate::Result<PathBuf> {
    // Reject any relative path with ".." components to prevent traversal
    if relative_path.split(['/', '\\']).any(|c| c == "..") {
        return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
    }

    let root = workspace_root.canonicalize()?;
    let candidate = root.join(relative_path);

    if candidate.exists() {
        // File/dir exists — canonicalize and check containment
        let resolved = candidate.canonicalize()?;
        if resolved.starts_with(&root) {
            Ok(resolved)
        } else {
            Err(crate::ToolError::WorkspaceEscape(relative_path.into()))
        }
    } else {
        // File doesn't exist yet — validate via nearest existing parent
        let mut parent = candidate.parent();
        while let Some(p) = parent {
            if p.exists() {
                let resolved = p.canonicalize()?;
                if resolved.starts_with(&root) {
                    return Ok(candidate);
                } else {
                    return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
                }
            }
            parent = p.parent();
        }
        // No existing parent found within workspace — that's OK, create_dir_all
        // will be called later. The path is safe because we already rejected "..".
        Ok(candidate)
    }
}

#[cfg(test)]
#[path = "fs_helpers_tests.rs"]
mod tests;
