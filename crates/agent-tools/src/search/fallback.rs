use std::path::Path;

use regex::Regex;

use crate::Result;

use super::{glob_matches, SearchContextLine, SearchEngine, SearchResult, SearchResults};

/// Recursive async directory walk + grep.
#[allow(clippy::too_many_arguments)]
pub async fn walk_and_grep(
    dir: &Path,
    workspace_root: &Path,
    re: &Regex,
    file_glob: Option<&str>,
    max_results: usize,
    results: &mut Vec<SearchResult>,
    files_visited: &mut usize,
    max_files: usize,
    current_depth: usize,
    max_depth: usize,
    context_lines: usize,
) {
    if current_depth > max_depth || *files_visited >= max_files || results.len() >= max_results {
        return;
    }

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return,
    };

    while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
        if results.len() >= max_results {
            return;
        }

        let path = entry.path();

        if path.is_dir() {
            // Skip hidden dirs, node_modules, target, .git
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.')
                    || name == "node_modules"
                    || name == "target"
                    || name == ".git"
                {
                    continue;
                }
            }
            Box::pin(walk_and_grep(
                &path,
                workspace_root,
                re,
                file_glob,
                max_results,
                results,
                files_visited,
                max_files,
                current_depth + 1,
                max_depth,
                context_lines,
            ))
            .await;
        } else if path.is_file() {
            if *files_visited >= max_files {
                return;
            }

            // Check glob filter on filename
            if let Some(glob) = file_glob {
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !glob_matches(filename, glob) {
                    continue;
                }
            }

            *files_visited += 1;

            // Try to read file as UTF-8 text; skip binary files
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue, // binary or unreadable — skip
            };

            let lines = content.lines().collect::<Vec<_>>();
            for (i, line) in lines.iter().enumerate() {
                if results.len() >= max_results {
                    return;
                }
                if let Some(m) = re.find(line) {
                    let file_path = path
                        .strip_prefix(workspace_root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    results.push(SearchResult {
                        file_path,
                        line_number: i + 1,
                        line_content: line.to_string(),
                        match_start: m.start(),
                        match_end: m.end(),
                        context_before: context_before(&lines, i, context_lines),
                        context_after: context_after(&lines, i, context_lines),
                    });
                }
            }
        }
    }
}

fn context_before(
    lines: &[&str],
    match_index: usize,
    context_lines: usize,
) -> Vec<SearchContextLine> {
    if context_lines == 0 {
        return Vec::new();
    }
    let start = match_index.saturating_sub(context_lines);
    lines[start..match_index]
        .iter()
        .enumerate()
        .map(|(offset, line)| SearchContextLine {
            line_number: start + offset + 1,
            line_content: (*line).to_string(),
        })
        .collect()
}

fn context_after(
    lines: &[&str],
    match_index: usize,
    context_lines: usize,
) -> Vec<SearchContextLine> {
    if context_lines == 0 {
        return Vec::new();
    }
    let start = match_index + 1;
    let end = lines.len().min(start + context_lines);
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, line)| SearchContextLine {
            line_number: start + offset + 1,
            line_content: (*line).to_string(),
        })
        .collect()
}

/// Run fallback search using pure Rust regex + directory walk.
pub async fn run_fallback(
    search_dir: &Path,
    workspace_root: &Path,
    pattern: &str,
    file_glob: Option<&str>,
    max_results: usize,
    context_lines: usize,
) -> Result<SearchResults> {
    let re = Regex::new(pattern)
        .map_err(|e| crate::ToolError::ExecutionFailed(format!("invalid regex pattern: {}", e)))?;

    let mut results = Vec::new();
    let mut files_visited = 0usize;
    const MAX_FILES: usize = 500;
    const MAX_DEPTH: usize = 10;

    walk_and_grep(
        search_dir,
        workspace_root,
        &re,
        file_glob,
        max_results,
        &mut results,
        &mut files_visited,
        MAX_FILES,
        0,
        MAX_DEPTH,
        context_lines,
    )
    .await;

    let truncated = results.len() >= max_results;

    Ok(SearchResults {
        total_matches: results.len(),
        results,
        truncated,
        engine: SearchEngine::Fallback,
    })
}

#[cfg(test)]
#[path = "fallback_tests.rs"]
mod tests;
