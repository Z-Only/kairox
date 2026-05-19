mod fallback;
mod format;
mod path;
mod rg;

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use crate::shell;
use crate::{Result, ToolError};

// ── Re-exports ────────────────────────────────────────────────────────────

pub use path::glob_matches;

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

    /// Search using ripgrep binary.
    async fn search_with_rg(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        max_results: usize,
    ) -> Result<SearchResults> {
        let rg_path = path::find_rg_binary()
            .ok_or_else(|| ToolError::ExecutionFailed("rg binary not found".into()))?;
        let search_dir = path::resolve_search_path(&self.workspace_root, path)?;
        rg::run_rg(&rg_path, &search_dir, pattern, file_glob, max_results).await
    }

    /// Fallback search using pure Rust regex + directory walk.
    async fn search_with_fallback(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        max_results: usize,
    ) -> Result<SearchResults> {
        let search_dir = path::resolve_search_path(&self.workspace_root, path)?;
        fallback::run_fallback(
            &search_dir,
            &self.workspace_root,
            pattern,
            file_glob,
            max_results,
        )
        .await
    }
}

// ── Tool impl ─────────────────────────────────────────────────────────────

#[async_trait]
impl Tool for RipgrepSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: shell::SEARCH_TOOL_ID.to_string(),
            description: "Search for patterns in workspace files using ripgrep or fallback engine"
                .to_string(),
            required_capability: "search.ripgrep".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The search pattern (regex supported)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Relative path to search within (optional, defaults to workspace root)"
                    },
                    "file_glob": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g., *.rs, *.{ts,vue})"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 50)"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::read(shell::SEARCH_TOOL_ID)
    }

    async fn invoke(&self, invocation: ToolInvocation) -> Result<ToolOutput> {
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
        let search_results = if path::find_rg_binary().is_some() {
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

        let text = format::format_search_results(&search_results);

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
    use serde_json::json;

    fn make_invocation(
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        max_results: usize,
    ) -> ToolInvocation {
        let mut args = json!({
            "pattern": pattern,
            "max_results": max_results,
        });
        if let Some(p) = path {
            args["path"] = json!(p);
        }
        if let Some(g) = file_glob {
            args["file_glob"] = json!(g);
        }
        ToolInvocation {
            tool_id: shell::SEARCH_TOOL_ID.to_string(),
            arguments: args,
            workspace_id: "test".to_string(),
            preview: format!("search {}", pattern),
            timeout_ms: 10000,
            output_limit_bytes: 102_400,
        }
    }

    // ── Full Tool::invoke tests ──────────────────────────────────────────

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
        assert_eq!(def.tool_id, shell::SEARCH_TOOL_ID);
        assert_eq!(def.required_capability, "search.ripgrep");
    }

    #[test]
    fn risk_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
        let inv = make_invocation("test", None, None, 10);
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::read(shell::SEARCH_TOOL_ID));
    }
}
