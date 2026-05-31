use crate::patch::apply::hunk::{apply_hunk_at, hunk_consumed_line_count, locate_hunk};
use crate::patch::apply::path::resolve_workspace_path;
use crate::patch::parse::{FilePatch, Hunk, PatchLine};
use crate::ToolError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

type FileLock = AsyncMutex<()>;
type LockMap = Mutex<HashMap<PathBuf, Weak<FileLock>>>;

pub(super) struct ResolvedPatch {
    pub file_patch: FilePatch,
    pub path: PathBuf,
}

pub(super) struct PatchPlan {
    files: Vec<PlannedFile>,
}

struct PlannedFile {
    path: PathBuf,
    affected_path: String,
    original_content: Option<String>,
    final_content: Option<String>,
}

struct WorkingFile {
    path: PathBuf,
    affected_path: String,
    original_content: Option<String>,
    current_content: Option<String>,
}

static PATCH_FILE_LOCKS: OnceLock<LockMap> = OnceLock::new();

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

pub(super) async fn acquire_file_locks(resolved: &[ResolvedPatch]) -> Vec<OwnedMutexGuard<()>> {
    let mut paths: Vec<PathBuf> = resolved.iter().map(|rp| rp.path.clone()).collect();
    paths.sort();
    paths.dedup();

    let locks: Vec<Arc<FileLock>> = paths.iter().map(|path| file_lock(path)).collect();
    let mut guards = Vec::with_capacity(locks.len());
    for lock in locks {
        guards.push(lock.lock_owned().await);
    }
    guards
}

fn file_lock(path: &Path) -> Arc<FileLock> {
    let lock_map = PATCH_FILE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = lock_map.lock().expect("patch file lock map poisoned");
    locks.retain(|_, lock| lock.strong_count() > 0);

    if let Some(lock) = locks.get(path).and_then(Weak::upgrade) {
        return lock;
    }

    let lock = Arc::new(AsyncMutex::new(()));
    locks.insert(path.to_path_buf(), Arc::downgrade(&lock));
    lock
}

/// Build all final file contents in memory before touching disk.
pub(super) async fn plan_patches(resolved: &[ResolvedPatch]) -> Result<PatchPlan, ToolError> {
    let mut working_files: HashMap<PathBuf, WorkingFile> = HashMap::new();
    let mut order: Vec<PathBuf> = Vec::new();

    for rp in resolved {
        if !working_files.contains_key(&rp.path) {
            let original_content = read_optional_string(&rp.path).await?;
            order.push(rp.path.clone());
            working_files.insert(
                rp.path.clone(),
                WorkingFile {
                    path: rp.path.clone(),
                    affected_path: affected_path(&rp.file_patch),
                    current_content: original_content.clone(),
                    original_content,
                },
            );
        }

        let working_file = working_files
            .get_mut(&rp.path)
            .expect("working file must be inserted before applying patch");
        working_file.affected_path = affected_path(&rp.file_patch);
        apply_file_patch(working_file, &rp.file_patch)?;
    }

    let mut files = Vec::new();
    for path in order {
        let working_file = working_files
            .remove(&path)
            .expect("ordered working file must exist");
        files.push(PlannedFile {
            path: working_file.path,
            affected_path: working_file.affected_path,
            original_content: working_file.original_content,
            final_content: working_file.current_content,
        });
    }

    Ok(PatchPlan { files })
}

async fn read_optional_string(path: &Path) -> Result<Option<String>, ToolError> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(ToolError::Io(err)),
    }
}

fn apply_file_patch(
    working_file: &mut WorkingFile,
    file_patch: &FilePatch,
) -> Result<(), ToolError> {
    if file_patch.is_new_file {
        if working_file.current_content.is_some() {
            return Err(ToolError::ExecutionFailed(format!(
                "new file already exists: {}",
                working_file.path.display()
            )));
        }
        working_file.current_content = Some(new_file_content(file_patch));
        return Ok(());
    }

    if file_patch.is_delete {
        if working_file.current_content.is_none() {
            return Err(ToolError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("file to delete not found: {}", working_file.path.display()),
            )));
        }
        working_file.current_content = None;
        return Ok(());
    }

    let content = working_file.current_content.as_ref().ok_or_else(|| {
        ToolError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("file to patch not found: {}", working_file.path.display()),
        ))
    })?;
    working_file.current_content = Some(apply_modify_patch(
        content,
        &file_patch.hunks,
        &working_file.path,
    )?);
    Ok(())
}

fn new_file_content(file_patch: &FilePatch) -> String {
    file_patch
        .hunks
        .iter()
        .flat_map(|h| h.lines.iter())
        .filter_map(|pl| match pl {
            PatchLine::Add(s) => Some(s.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn apply_modify_patch(content: &str, hunks: &[Hunk], path: &Path) -> Result<String, ToolError> {
    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let mut located_hunks = Vec::with_capacity(hunks.len());

    for hunk in hunks {
        let offset = locate_hunk(&lines, hunk)?;
        located_hunks.push((offset, hunk));
    }

    validate_hunks_do_not_overlap(&located_hunks, path)?;
    located_hunks.sort_by_key(|(offset, _)| std::cmp::Reverse(*offset));

    for (offset, hunk) in located_hunks {
        apply_hunk_at(&mut lines, hunk, offset);
    }

    let new_content = lines.join("\n");
    if content.ends_with('\n') {
        Ok(format!("{new_content}\n"))
    } else {
        Ok(new_content)
    }
}

fn validate_hunks_do_not_overlap(
    located_hunks: &[(usize, &Hunk)],
    path: &Path,
) -> Result<(), ToolError> {
    let mut ranges: Vec<(usize, usize)> = located_hunks
        .iter()
        .filter_map(|(offset, hunk)| {
            let consumed = hunk_consumed_line_count(hunk);
            (consumed > 0).then_some((*offset, offset + consumed))
        })
        .collect();
    ranges.sort_unstable();

    for pair in ranges.windows(2) {
        let (prev_start, prev_end) = pair[0];
        let (next_start, next_end) = pair[1];
        if next_start < prev_end {
            return Err(ToolError::ExecutionFailed(format!(
                "overlapping hunks for {} at lines {}-{} and {}-{}",
                path.display(),
                prev_start + 1,
                prev_end,
                next_start + 1,
                next_end
            )));
        }
    }

    Ok(())
}

fn affected_path(file_patch: &FilePatch) -> String {
    let path = if file_patch.is_delete {
        &file_patch.old_path
    } else {
        &file_patch.new_path
    };
    path.to_str().unwrap_or("(invalid path)").to_string()
}

/// Phase 2: apply each resolved patch to disk and return the relative paths
/// of affected files in the order they were applied.
pub(super) async fn apply_patches(plan: PatchPlan) -> Result<Vec<String>, ToolError> {
    let mut affected_files: Vec<String> = Vec::new();
    let mut applied_indexes: Vec<usize> = Vec::new();

    for (index, file) in plan.files.iter().enumerate() {
        if let Err(err) = write_planned_file(file).await {
            let mut rollback_indexes = applied_indexes.clone();
            rollback_indexes.push(index);
            rollback_files(&plan.files, &rollback_indexes).await;
            return Err(err);
        }

        applied_indexes.push(index);
        affected_files.push(file.affected_path.clone());
    }

    Ok(affected_files)
}

async fn write_planned_file(file: &PlannedFile) -> Result<(), ToolError> {
    match &file.final_content {
        Some(content) => {
            if let Some(parent) = file.path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&file.path, content).await?;
        }
        None => {
            tokio::fs::remove_file(&file.path).await?;
        }
    }
    Ok(())
}

async fn rollback_files(files: &[PlannedFile], indexes: &[usize]) {
    for index in indexes.iter().rev() {
        if let Some(file) = files.get(*index) {
            let _ = restore_original_file(file).await;
        }
    }
}

async fn restore_original_file(file: &PlannedFile) -> Result<(), ToolError> {
    match &file.original_content {
        Some(content) => {
            if let Some(parent) = file.path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&file.path, content).await?;
        }
        None => match tokio::fs::remove_file(&file.path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(ToolError::Io(err)),
        },
    }
    Ok(())
}
