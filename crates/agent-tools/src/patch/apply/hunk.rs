use crate::patch::parse::{Hunk, PatchLine};
use crate::ToolError;

const MAX_HUNK_LINE_DRIFT: usize = 200;

/// Validate that Context and Remove lines in `hunk` match the actual file
/// content. The declared hunk line is tried first, then a bounded exact-context
/// search tolerates nearby line drift.
#[cfg(test)]
pub fn apply_hunk_validate(lines: &[String], hunk: &Hunk) -> Result<(), ToolError> {
    locate_hunk(lines, hunk).map(|_| ())
}

pub(super) fn locate_hunk(lines: &[String], hunk: &Hunk) -> Result<usize, ToolError> {
    let expected_offset = hunk.old_start.saturating_sub(1); // convert 1-based to 0-based
    if hunk_matches_at(lines, hunk, expected_offset) {
        return Ok(expected_offset);
    }

    let consumed_count = hunk_consumed_line_count(hunk);
    if consumed_count == 0 {
        return Err(context_mismatch_at(lines, hunk, expected_offset));
    }

    let max_start = lines.len().saturating_sub(consumed_count);
    let search_start = expected_offset.saturating_sub(MAX_HUNK_LINE_DRIFT);
    let search_end = expected_offset
        .saturating_add(MAX_HUNK_LINE_DRIFT)
        .min(max_start);
    let mut candidates = Vec::new();

    for offset in search_start..=search_end {
        if offset != expected_offset && hunk_matches_at(lines, hunk, offset) {
            candidates.push(offset + 1);
        }
    }

    match candidates.len() {
        0 => Err(context_mismatch_at(lines, hunk, expected_offset)),
        1 => Ok(candidates[0] - 1),
        _ => Err(ToolError::AmbiguousPatchContext {
            line: hunk.old_start,
            candidates,
        }),
    }
}

pub(super) fn hunk_consumed_line_count(hunk: &Hunk) -> usize {
    hunk.lines
        .iter()
        .filter(|line| matches!(line, PatchLine::Context(_) | PatchLine::Remove(_)))
        .count()
}

fn hunk_matches_at(lines: &[String], hunk: &Hunk, offset: usize) -> bool {
    if offset > lines.len() {
        return false;
    }

    let mut file_idx = offset;
    for patch_line in &hunk.lines {
        match patch_line {
            PatchLine::Context(expected) | PatchLine::Remove(expected) => {
                if file_idx >= lines.len() {
                    return false;
                }
                let actual = &lines[file_idx];
                if actual != expected {
                    return false;
                }
                file_idx += 1;
            }
            PatchLine::Add(_) => {
                // Add lines don't consume file lines
            }
        }
    }
    true
}

fn context_mismatch_at(lines: &[String], hunk: &Hunk, offset: usize) -> ToolError {
    let mut file_idx = offset;
    for patch_line in &hunk.lines {
        match patch_line {
            PatchLine::Context(expected) | PatchLine::Remove(expected) => {
                if file_idx >= lines.len() {
                    return ToolError::ContextMismatch {
                        line: file_idx + 1,
                        expected: expected.clone(),
                        actual: String::from("<EOF>"),
                    };
                }
                let actual = &lines[file_idx];
                if actual != expected {
                    return ToolError::ContextMismatch {
                        line: file_idx + 1,
                        expected: expected.clone(),
                        actual: actual.clone(),
                    };
                }
                file_idx += 1;
            }
            PatchLine::Add(_) => {}
        }
    }
    ToolError::ContextMismatch {
        line: offset + 1,
        expected: String::from("<hunk context>"),
        actual: String::from("<unknown>"),
    }
}

/// Apply a validated hunk by rebuilding the hunk region.
/// Replaces the slice of lines covered by the hunk with Context + Add lines.
#[cfg(test)]
pub fn apply_hunk(lines: &mut Vec<String>, hunk: &Hunk) {
    let offset = hunk.old_start.saturating_sub(1); // 0-based start
    apply_hunk_at(lines, hunk, offset);
}

pub(super) fn apply_hunk_at(lines: &mut Vec<String>, hunk: &Hunk, offset: usize) {
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
