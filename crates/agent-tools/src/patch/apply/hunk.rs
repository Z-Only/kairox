use crate::patch::parse::{Hunk, PatchLine};
use crate::ToolError;

/// Validate that Context and Remove lines in `hunk` match the actual file
/// content starting at `hunk.old_start - 1` (0-based) in `lines`.
pub fn apply_hunk_validate(lines: &[String], hunk: &Hunk) -> Result<(), ToolError> {
    let offset = hunk.old_start.saturating_sub(1); // convert 1-based → 0-based
    let mut file_idx = offset;

    for patch_line in &hunk.lines {
        match patch_line {
            PatchLine::Context(expected) | PatchLine::Remove(expected) => {
                if file_idx >= lines.len() {
                    return Err(ToolError::ContextMismatch {
                        line: file_idx + 1,
                        expected: expected.clone(),
                        actual: String::from("<EOF>"),
                    });
                }
                let actual = &lines[file_idx];
                if actual != expected {
                    return Err(ToolError::ContextMismatch {
                        line: file_idx + 1,
                        expected: expected.clone(),
                        actual: actual.clone(),
                    });
                }
                file_idx += 1;
            }
            PatchLine::Add(_) => {
                // Add lines don't consume file lines
            }
        }
    }
    Ok(())
}

/// Apply a validated hunk by rebuilding the hunk region.
/// Replaces the slice of lines covered by the hunk with Context + Add lines.
pub fn apply_hunk(lines: &mut Vec<String>, hunk: &Hunk) {
    let offset = hunk.old_start.saturating_sub(1); // 0-based start

    let mut kept_count = 0usize;
    let mut remove_count = 0usize;
    let mut new_lines: Vec<String> = Vec::new();

    for patch_line in &hunk.lines {
        match patch_line {
            PatchLine::Context(s) => {
                new_lines.push(s.clone());
                kept_count += 1;
            }
            PatchLine::Remove(_) => {
                remove_count += 1;
            }
            PatchLine::Add(s) => {
                new_lines.push(s.clone());
            }
        }
    }

    // Replace the region [offset .. offset + kept_count + remove_count) with new_lines
    let end = offset + kept_count + remove_count;
    lines.splice(offset..end, new_lines);
}
