use crate::patch::apply::hunk::{apply_hunk, apply_hunk_validate};
use crate::patch::apply::path::resolve_workspace_path;
use crate::patch::parse::{FilePatch, PatchLine};
use crate::ToolError;
use std::path::{Path, PathBuf};

pub(super) struct ResolvedPatch {
    pub file_patch: FilePatch,
    pub path: PathBuf,
}

/// Resolve every file patch path against `workspace_root`.
pub(super) fn resolve_patches(
    workspace_root: &Path,
    file_patches: &[FilePatch],
) -> Result<Vec<ResolvedPatch>, ToolError> {
    let mut resolved: Vec<ResolvedPatch> = Vec::new();

    for fp in file_patches {
        let relative_path = if fp.is_new_file {
            fp.new_path.to_str().ok_or_else(|| {
                ToolError::PatchParseFailed(format!(
                    "invalid new file path: {}",
                    fp.new_path.display()
                ))
            })?
        } else {
            fp.old_path.to_str().ok_or_else(|| {
                ToolError::PatchParseFailed(format!(
                    "invalid old file path: {}",
                    fp.old_path.display()
                ))
            })?
        };

        let path = resolve_workspace_path(workspace_root, relative_path)?;
        resolved.push(ResolvedPatch {
            file_patch: fp.clone(),
            path,
        });
    }

    Ok(resolved)
}

/// Phase 1: validate every hunk against the current file contents.
/// New-file and delete-file patches are checked for existence only.
pub(super) async fn validate_patches(resolved: &[ResolvedPatch]) -> Result<(), ToolError> {
    for rp in resolved {
        if rp.file_patch.is_new_file {
            // No existing content to validate
            continue;
        }

        if rp.file_patch.is_delete {
            if !rp.path.exists() {
                return Err(ToolError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("file to delete not found: {}", rp.path.display()),
                )));
            }
            continue;
        }

        let content = tokio::fs::read_to_string(&rp.path).await?;
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

        for hunk in &rp.file_patch.hunks {
            apply_hunk_validate(&lines, hunk)?;
        }
    }

    Ok(())
}

/// Phase 2: apply each resolved patch to disk and return the relative paths
/// of affected files in the order they were applied.
pub(super) async fn apply_patches(resolved: &[ResolvedPatch]) -> Result<Vec<String>, ToolError> {
    let mut affected_files: Vec<String> = Vec::new();

    for rp in resolved {
        if rp.file_patch.is_new_file {
            // Extract Add lines as content
            let content = rp
                .file_patch
                .hunks
                .iter()
                .flat_map(|h| h.lines.iter())
                .filter_map(|pl| match pl {
                    PatchLine::Add(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            // Create parent directories if needed
            if let Some(parent) = rp.path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&rp.path, content).await?;
            affected_files.push(
                rp.file_patch
                    .new_path
                    .to_str()
                    .unwrap_or("(invalid path)")
                    .to_string(),
            );
        } else if rp.file_patch.is_delete {
            tokio::fs::remove_file(&rp.path).await?;
            affected_files.push(
                rp.file_patch
                    .old_path
                    .to_str()
                    .unwrap_or("(invalid path)")
                    .to_string(),
            );
        } else {
            // Modify existing file
            let content = tokio::fs::read_to_string(&rp.path).await?;
            let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

            // Apply hunks in reverse order so earlier offsets stay valid
            for hunk in rp.file_patch.hunks.iter().rev() {
                apply_hunk(&mut lines, hunk);
            }

            let new_content = lines.join("\n");
            // Preserve trailing newline if original had one
            let final_content = if content.ends_with('\n') {
                format!("{}\n", new_content)
            } else {
                new_content
            };
            tokio::fs::write(&rp.path, final_content).await?;
            affected_files.push(
                rp.file_patch
                    .new_path
                    .to_str()
                    .unwrap_or("(invalid path)")
                    .to_string(),
            );
        }
    }

    Ok(affected_files)
}
