use std::path::Path;
use std::process::Stdio;

use crate::{Result, ToolError};

use super::{SearchContextLine, SearchEngine, SearchResult, SearchResults};

/// Parse ripgrep's JSON output, one JSON object per line.
pub fn parse_rg_json_output(raw: &[u8], max_results: usize) -> Result<Vec<SearchResult>> {
    parse_rg_json_output_with_context(raw, max_results, 0)
}

pub fn parse_rg_json_output_with_context(
    raw: &[u8],
    max_results: usize,
    context_lines: usize,
) -> Result<Vec<SearchResult>> {
    let text = String::from_utf8_lossy(raw);
    let mut results: Vec<SearchResult> = Vec::new();
    let mut pending_context: Vec<RgContextLine> = Vec::new();

    for line in text.lines() {
        if results.len() >= max_results {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }

        let value: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // skip unparseable lines
        };

        let event_type = value.get("type").and_then(|v| v.as_str());

        let data = match value.get("data") {
            Some(d) => d,
            None => continue,
        };

        if event_type == Some("context") {
            if context_lines == 0 {
                continue;
            }
            let Some(context_line) = context_line_from_data(data) else {
                continue;
            };
            if let Some(last_result) = results.last_mut() {
                if last_result.context_after.len() < context_lines
                    && last_result.file_path == context_line.file_path
                    && context_line.line_number > last_result.line_number
                {
                    last_result.context_after.push(SearchContextLine {
                        line_number: context_line.line_number,
                        line_content: context_line.line_content.clone(),
                    });
                }
            }
            pending_context.push(context_line);
            if pending_context.len() > context_lines {
                pending_context.remove(0);
            }
            continue;
        }

        if event_type != Some("match") {
            continue;
        }

        let file_path = data
            .pointer("/path/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let line_number = data
            .get("line_number")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let line_content = data
            .pointer("/lines/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Extract first submatch offset
        let (match_start, match_end) = data
            .get("submatches")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .map(|sm| {
                let s = sm.get("start").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let e = sm.get("end").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                (s, e)
            })
            .unwrap_or((0, 0));

        let context_before = if context_lines == 0 {
            Vec::new()
        } else {
            pending_context
                .iter()
                .filter(|line| line.file_path == file_path && line.line_number < line_number)
                .rev()
                .take(context_lines)
                .map(|line| SearchContextLine {
                    line_number: line.line_number,
                    line_content: line.line_content.clone(),
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        };
        pending_context.clear();

        results.push(SearchResult {
            file_path,
            line_number,
            line_content,
            match_start,
            match_end,
            context_before,
            context_after: Vec::new(),
        });
    }

    Ok(results)
}

#[derive(Clone)]
struct RgContextLine {
    file_path: String,
    line_number: usize,
    line_content: String,
}

fn context_line_from_data(data: &serde_json::Value) -> Option<RgContextLine> {
    let file_path = data.pointer("/path/text")?.as_str()?.to_string();
    let line_number = data.get("line_number")?.as_u64()? as usize;
    let line_content = data.pointer("/lines/text")?.as_str()?.to_string();
    Some(RgContextLine {
        file_path,
        line_number,
        line_content,
    })
}

/// Run ripgrep search and return results.
pub async fn run_rg(
    rg_path: &Path,
    search_dir: &Path,
    pattern: &str,
    file_glob: Option<&str>,
    max_results: usize,
    context_lines: usize,
) -> Result<SearchResults> {
    let mut cmd = tokio::process::Command::new(rg_path);
    cmd.arg("--json")
        .arg("--max-count")
        .arg(max_results.to_string())
        .arg("--max-filesize")
        .arg("10M")
        .arg("--sort-path")
        .arg("--color")
        .arg("never");
    if context_lines > 0 {
        cmd.arg("--context").arg(context_lines.to_string());
    }

    if let Some(glob) = file_glob {
        cmd.arg("--glob").arg(glob);
    }

    cmd.arg(pattern).arg(search_dir);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd
        .output()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("failed to execute rg: {}", e)))?;

    // rg exit code 1 = no matches (not an error)
    let exit_code = output.status.code().unwrap_or(-1);
    if exit_code > 1 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ToolError::ExecutionFailed(format!(
            "rg exited with code {}: {}",
            exit_code, stderr
        )));
    }

    let results = if context_lines == 0 {
        parse_rg_json_output(&output.stdout, max_results)?
    } else {
        parse_rg_json_output_with_context(&output.stdout, max_results, context_lines)?
    };
    let total_matches = results.len();
    let truncated = total_matches >= max_results;

    Ok(SearchResults {
        results,
        total_matches,
        truncated,
        engine: SearchEngine::Ripgrep,
    })
}

#[cfg(test)]
#[path = "rg_tests.rs"]
mod tests;
