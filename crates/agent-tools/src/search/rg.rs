use std::path::Path;
use std::process::Stdio;

use crate::{Result, ToolError};

use super::{SearchEngine, SearchResult, SearchResults};

/// Parse ripgrep's JSON output, one JSON object per line.
pub fn parse_rg_json_output(raw: &[u8], max_results: usize) -> Result<Vec<SearchResult>> {
    let text = String::from_utf8_lossy(raw);
    let mut results = Vec::new();

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

        if value.get("type").and_then(|v| v.as_str()) != Some("match") {
            continue;
        }

        let data = match value.get("data") {
            Some(d) => d,
            None => continue,
        };

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

        results.push(SearchResult {
            file_path,
            line_number,
            line_content,
            match_start,
            match_end,
        });
    }

    Ok(results)
}

/// Run ripgrep search and return results.
pub async fn run_rg(
    rg_path: &Path,
    search_dir: &Path,
    pattern: &str,
    file_glob: Option<&str>,
    max_results: usize,
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

    let results = parse_rg_json_output(&output.stdout, max_results)?;
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
mod tests {
    use super::*;

    // ── parse_rg_json_output ───────────────────────────────────────────────

    #[test]
    fn parse_rg_json_match_line() {
        let json = r#"{"type":"match","data":{"path":{"text":"src/main.rs"},"line_number":10,"lines":{"text":"fn main() {\n"},"submatches":[{"start":3,"end":8,"match":{"text":"main"}}]}}"#;
        let raw = json.as_bytes();
        let results = parse_rg_json_output(raw, 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "src/main.rs");
        assert_eq!(results[0].line_number, 10);
        assert_eq!(results[0].line_content, "fn main() {\n");
        assert_eq!(results[0].match_start, 3);
        assert_eq!(results[0].match_end, 8);
    }

    #[test]
    fn parse_rg_json_skips_non_match_lines() {
        let input = r#"{"type":"summary","data":{"elapsed":0.01}}
{"type":"match","data":{"path":{"text":"foo.rs"},"line_number":1,"lines":{"text":"hello\n"},"submatches":[{"start":0,"end":5,"match":{"text":"hello"}}]}}"#;
        let results = parse_rg_json_output(input.as_bytes(), 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "foo.rs");
    }

    #[test]
    fn parse_rg_json_respects_max_results() {
        let line1 = r#"{"type":"match","data":{"path":{"text":"a.rs"},"line_number":1,"lines":{"text":"a\n"},"submatches":[{"start":0,"end":1,"match":{"text":"a"}}]}}"#;
        let line2 = r#"{"type":"match","data":{"path":{"text":"b.rs"},"line_number":2,"lines":{"text":"b\n"},"submatches":[{"start":0,"end":1,"match":{"text":"b"}}]}}"#;
        let input = format!("{}\n{}", line1, line2);
        let results = parse_rg_json_output(input.as_bytes(), 1).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn parse_rg_json_empty_input() {
        let results = parse_rg_json_output(b"", 100).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn parse_rg_json_malformed_lines_skipped() {
        let input = b"not json at all\n{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":1,\"lines\":{\"text\":\"line\\n\"},\"submatches\":[{\"start\":0,\"end\":4,\"match\":{\"text\":\"line\"}}]}}\n";
        let results = parse_rg_json_output(input, 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "a.rs");
    }

    #[test]
    fn parse_rg_json_missing_submatches() {
        let input = b"{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":1,\"lines\":{\"text\":\"line\\n\"}}}\n";
        let results = parse_rg_json_output(input, 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "a.rs");
        assert_eq!(results[0].match_start, 0);
        assert_eq!(results[0].match_end, 0);
    }

    #[test]
    fn parse_rg_json_with_empty_lines() {
        let input = b"\n\n{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":1,\"lines\":{\"text\":\"x\\n\"},\"submatches\":[{\"start\":0,\"end\":1,\"match\":{\"text\":\"x\"}}]}}\n\n\n";
        let results = parse_rg_json_output(input, 100).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn parse_rg_json_multiple_submatches_uses_first() {
        let input = b"{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":1,\"lines\":{\"text\":\"foo bar baz\\n\"},\"submatches\":[{\"start\":0,\"end\":3,\"match\":{\"text\":\"foo\"}},{\"start\":4,\"end\":7,\"match\":{\"text\":\"bar\"}}]}}\n";
        let results = parse_rg_json_output(input, 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].match_start, 0);
        assert_eq!(results[0].match_end, 3);
    }
}
