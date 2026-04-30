use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use crate::ToolError;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;

// ── Data structures ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub line_number: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchEngine {
    Ripgrep,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    pub results: Vec<SearchResult>,
    pub total_matches: usize,
    pub truncated: bool,
    pub engine: SearchEngine,
}

// ── RipgrepSearchTool ─────────────────────────────────────────────────────

pub struct RipgrepSearchTool {
    workspace_root: PathBuf,
}

impl RipgrepSearchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Locate the `rg` binary: env var > `which rg` > None.
    fn find_rg_binary() -> Option<PathBuf> {
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

    /// Search using ripgrep binary.
    async fn search_with_rg(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        max_results: usize,
    ) -> crate::Result<SearchResults> {
        let rg_path = Self::find_rg_binary()
            .ok_or_else(|| ToolError::ExecutionFailed("rg binary not found".into()))?;

        let search_dir = match path {
            Some(p) => self.workspace_root.join(p),
            None => self.workspace_root.clone(),
        };

        let mut cmd = tokio::process::Command::new(&rg_path);
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

        cmd.arg(pattern).arg(&search_dir);
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

        let results = Self::parse_rg_json_output(&output.stdout, max_results)?;
        let total_matches = results.len();
        let truncated = total_matches >= max_results;

        Ok(SearchResults {
            results,
            total_matches,
            truncated,
            engine: SearchEngine::Ripgrep,
        })
    }

    /// Parse ripgrep's JSON output, one JSON object per line.
    fn parse_rg_json_output(raw: &[u8], max_results: usize) -> crate::Result<Vec<SearchResult>> {
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

    /// Fallback search using pure Rust regex + directory walk.
    async fn search_with_fallback(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        max_results: usize,
    ) -> crate::Result<SearchResults> {
        let re = Regex::new(pattern)
            .map_err(|e| ToolError::ExecutionFailed(format!("invalid regex pattern: {}", e)))?;

        let search_dir = match path {
            Some(p) => self.workspace_root.join(p),
            None => self.workspace_root.clone(),
        };

        let mut results = Vec::new();
        let mut files_visited = 0usize;
        const MAX_FILES: usize = 500;
        const MAX_DEPTH: usize = 10;

        Self::walk_and_grep(
            &search_dir,
            &self.workspace_root,
            &re,
            file_glob,
            max_results,
            &mut results,
            &mut files_visited,
            MAX_FILES,
            0,
            MAX_DEPTH,
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

    /// Recursive async directory walk + grep.
    #[allow(clippy::too_many_arguments)]
    async fn walk_and_grep(
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
    ) {
        if current_depth > max_depth || *files_visited >= max_files || results.len() >= max_results
        {
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
                Box::pin(Self::walk_and_grep(
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

                // Grep line by line
                for (i, line) in content.lines().enumerate() {
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
                        });
                    }
                }
            }
        }
    }

    /// Format search results for output.
    pub fn format_search_results(results: &SearchResults) -> String {
        let engine_label = match results.engine {
            SearchEngine::Ripgrep => "rg",
            SearchEngine::Fallback => "fallback",
        };

        let files: std::collections::HashSet<&str> = results
            .results
            .iter()
            .map(|r| r.file_path.as_str())
            .collect();
        let file_count = files.len();

        let header = if results.truncated {
            format!(
                "[{}] Found {} matches in {} files (max, truncated):\n",
                engine_label, results.total_matches, file_count
            )
        } else {
            format!(
                "[{}] Found {} matches in {} files:\n",
                engine_label, results.total_matches, file_count
            )
        };

        let lines: Vec<String> = results
            .results
            .iter()
            .map(|r| format!("{}:{}:{}", r.file_path, r.line_number, r.line_content))
            .collect();

        format!("{}{}", header, lines.join("\n"))
    }
}

// ── glob_matches ──────────────────────────────────────────────────────────

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

// ── Tool impl ─────────────────────────────────────────────────────────────

#[async_trait]
impl Tool for RipgrepSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: crate::shell::SEARCH_TOOL_ID.to_string(),
            description: "Search for patterns in workspace files using ripgrep or fallback engine"
                .to_string(),
            required_capability: "search.ripgrep".to_string(),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::read(crate::shell::SEARCH_TOOL_ID)
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let pattern = invocation
            .arguments
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if pattern.is_empty() {
            return Err(ToolError::ExecutionFailed(
                "empty search pattern".to_string(),
            ));
        }

        let path = invocation.arguments.get("path").and_then(|v| v.as_str());
        let file_glob = invocation
            .arguments
            .get("file_glob")
            .and_then(|v| v.as_str());
        let max_results = invocation
            .arguments
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;

        // Try ripgrep first, fall back on any error
        let search_results = if Self::find_rg_binary().is_some() {
            match self
                .search_with_rg(pattern, path, file_glob, max_results)
                .await
            {
                Ok(results) => results,
                Err(_) => {
                    self.search_with_fallback(pattern, path, file_glob, max_results)
                        .await?
                }
            }
        } else {
            self.search_with_fallback(pattern, path, file_glob, max_results)
                .await?
        };

        let text = Self::format_search_results(&search_results);

        Ok(ToolOutput {
            text,
            truncated: search_results.truncated,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;

    fn make_invocation(
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        max_results: usize,
    ) -> ToolInvocation {
        let mut args = serde_json::json!({
            "pattern": pattern,
            "max_results": max_results,
        });
        if let Some(p) = path {
            args["path"] = serde_json::json!(p);
        }
        if let Some(g) = file_glob {
            args["file_glob"] = serde_json::json!(g);
        }
        ToolInvocation {
            tool_id: crate::shell::SEARCH_TOOL_ID.to_string(),
            arguments: args,
            workspace_id: "test".to_string(),
            preview: format!("search {}", pattern),
            timeout_ms: 10000,
            output_limit_bytes: 102_400,
        }
    }

    // ── parse_rg_json_output ───────────────────────────────────────────────

    #[test]
    fn parse_rg_json_match_line() {
        let json = r#"{"type":"match","data":{"path":{"text":"src/main.rs"},"line_number":10,"lines":{"text":"fn main() {\n"},"submatches":[{"start":3,"end":8,"match":{"text":"main"}}]}}"#;
        let raw = json.as_bytes();
        let results = RipgrepSearchTool::parse_rg_json_output(raw, 100).unwrap();
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
        let results = RipgrepSearchTool::parse_rg_json_output(input.as_bytes(), 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "foo.rs");
    }

    #[test]
    fn parse_rg_json_respects_max_results() {
        let line1 = r#"{"type":"match","data":{"path":{"text":"a.rs"},"line_number":1,"lines":{"text":"a\n"},"submatches":[{"start":0,"end":1,"match":{"text":"a"}}]}}"#;
        let line2 = r#"{"type":"match","data":{"path":{"text":"b.rs"},"line_number":2,"lines":{"text":"b\n"},"submatches":[{"start":0,"end":1,"match":{"text":"b"}}]}}"#;
        let input = format!("{}\n{}", line1, line2);
        let results = RipgrepSearchTool::parse_rg_json_output(input.as_bytes(), 1).unwrap();
        assert_eq!(results.len(), 1);
    }

    // ── glob_matches ──────────────────────────────────────────────────────

    #[test]
    fn glob_simple_extension() {
        assert!(glob_matches("main.rs", "*.rs"));
        assert!(glob_matches("lib.toml", "*.toml"));
        assert!(!glob_matches("main.ts", "*.rs"));
    }

    #[test]
    fn glob_brace_group() {
        assert!(glob_matches("main.rs", "*.{rs,toml}"));
        assert!(glob_matches("cargo.toml", "*.{rs,toml}"));
        assert!(!glob_matches("main.ts", "*.{rs,toml}"));
    }

    #[test]
    fn glob_brace_group_with_spaces() {
        assert!(glob_matches("main.rs", "*.{rs, toml}"));
    }

    #[test]
    fn glob_exact_match_fallback() {
        assert!(glob_matches("Cargo.toml", "Cargo.toml"));
        assert!(!glob_matches("cargo.toml", "Cargo.toml"));
    }

    // ── Fallback search integration tests ─────────────────────────────────

    #[tokio::test]
    async fn fallback_finds_pattern_in_files() {
        let dir = tempfile::tempdir().unwrap();
        // Write two files
        tokio::fs::write(dir.path().join("a.txt"), "hello world\nfoo bar\n")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("b.txt"), "no match here\nhello again\n")
            .await
            .unwrap();

        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let results = tool
            .search_with_fallback("hello", None, None, 100)
            .await
            .unwrap();

        assert_eq!(results.engine, SearchEngine::Fallback);
        assert_eq!(results.results.len(), 2);
        assert_eq!(results.total_matches, 2);
        assert!(!results.truncated);

        let paths: Vec<&str> = results
            .results
            .iter()
            .map(|r| r.file_path.as_str())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("a.txt")));
        assert!(paths.iter().any(|p| p.ends_with("b.txt")));
    }

    #[tokio::test]
    async fn fallback_respects_glob_filter() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("main.rs"), "fn find_me() {}\n")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("index.ts"), "function findMe() {}\n")
            .await
            .unwrap();

        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let results = tool
            .search_with_fallback("find", None, Some("*.rs"), 100)
            .await
            .unwrap();

        assert_eq!(results.results.len(), 1);
        assert!(results.results[0].file_path.ends_with("main.rs"));
    }

    #[tokio::test]
    async fn fallback_respects_max_results() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("a.txt"),
            "match line 1\nmatch line 2\nmatch line 3\n",
        )
        .await
        .unwrap();

        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let results = tool
            .search_with_fallback("match", None, None, 1)
            .await
            .unwrap();

        assert_eq!(results.results.len(), 1);
        assert!(results.truncated);
    }

    #[tokio::test]
    async fn fallback_skips_hidden_and_ignored_dirs() {
        let dir = tempfile::tempdir().unwrap();
        // main file
        tokio::fs::write(dir.path().join("main.rs"), "findme\n")
            .await
            .unwrap();
        // .git dir
        tokio::fs::create_dir_all(dir.path().join(".git"))
            .await
            .unwrap();
        tokio::fs::write(dir.path().join(".git").join("config"), "findme\n")
            .await
            .unwrap();
        // target dir
        tokio::fs::create_dir_all(dir.path().join("target"))
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("target").join("output.txt"), "findme\n")
            .await
            .unwrap();

        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let results = tool
            .search_with_fallback("findme", None, None, 100)
            .await
            .unwrap();

        assert_eq!(results.results.len(), 1);
        assert!(results.results[0].file_path.ends_with("main.rs"));
    }

    #[tokio::test]
    async fn fallback_skips_binary_files() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("text.txt"), "findme in text\n")
            .await
            .unwrap();
        // Write binary data (invalid UTF-8)
        let binary_data: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x01, 0x02, 0x03];
        let mut f = std::fs::File::create(dir.path().join("binary.bin")).unwrap();
        f.write_all(&binary_data).unwrap();
        drop(f);

        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let results = tool
            .search_with_fallback("findme", None, None, 100)
            .await
            .unwrap();

        assert_eq!(results.results.len(), 1);
        assert!(results.results[0].file_path.ends_with("text.txt"));
    }

    // ── format_search_results ─────────────────────────────────────────────

    #[test]
    fn format_search_results_basic() {
        let results = SearchResults {
            results: vec![SearchResult {
                file_path: "src/main.rs".into(),
                line_number: 10,
                line_content: "fn main() {}".into(),
                match_start: 3,
                match_end: 8,
            }],
            total_matches: 1,
            truncated: false,
            engine: SearchEngine::Fallback,
        };
        let text = RipgrepSearchTool::format_search_results(&results);
        assert!(text.contains("[fallback] Found 1 matches in 1 files"));
        assert!(text.contains("src/main.rs:10:fn main() {}"));
    }

    #[test]
    fn format_search_results_truncated() {
        let results = SearchResults {
            results: vec![SearchResult {
                file_path: "a.rs".into(),
                line_number: 1,
                line_content: "match".into(),
                match_start: 0,
                match_end: 5,
            }],
            total_matches: 1,
            truncated: true,
            engine: SearchEngine::Ripgrep,
        };
        let text = RipgrepSearchTool::format_search_results(&results);
        assert!(text.contains("(max, truncated)"));
        assert!(text.contains("[rg]"));
    }

    // ── Full Tool::invoke test ────────────────────────────────────────────

    #[tokio::test]
    async fn search_tool_invocation_works_with_fallback() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("hello.txt"), "world says hello\n")
            .await
            .unwrap();

        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let invocation = make_invocation("hello", None, None, 50);
        let output = tool.invoke(invocation).await.unwrap();

        assert!(!output.text.is_empty());
        assert!(output.text.contains("hello.txt"));
    }

    #[tokio::test]
    async fn search_tool_empty_pattern_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let invocation = make_invocation("", None, None, 50);
        let result = tool.invoke(invocation).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionFailed(msg) => assert_eq!(msg, "empty search pattern"),
            other => panic!("expected ExecutionFailed, got {:?}", other),
        }
    }

    // ── Tool trait tests ──────────────────────────────────────────────────

    #[test]
    fn definition_returns_correct_id() {
        let dir = tempfile::tempdir().unwrap();
        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let def = tool.definition();
        assert_eq!(def.tool_id, crate::shell::SEARCH_TOOL_ID);
        assert_eq!(def.required_capability, "search.ripgrep");
    }

    #[test]
    fn risk_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let inv = make_invocation("test", None, None, 10);
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::read(crate::shell::SEARCH_TOOL_ID));
    }
}
