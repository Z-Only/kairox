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
pub struct SearchContextLine {
    pub line_number: usize,
    pub line_content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub line_number: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_before: Vec<SearchContextLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_after: Vec<SearchContextLine>,
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
        context_lines: usize,
    ) -> Result<SearchResults> {
        let rg_path = path::find_rg_binary()
            .ok_or_else(|| ToolError::ExecutionFailed("rg binary not found".into()))?;
        let search_dir = path::resolve_search_path(&self.workspace_root, path)?;
        rg::run_rg(
            &rg_path,
            &search_dir,
            pattern,
            file_glob,
            max_results,
            context_lines,
        )
        .await
    }

    /// Fallback search using pure Rust regex + directory walk.
    async fn search_with_fallback(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        max_results: usize,
        context_lines: usize,
    ) -> Result<SearchResults> {
        let search_dir = path::resolve_search_path(&self.workspace_root, path)?;
        fallback::run_fallback(
            &search_dir,
            &self.workspace_root,
            pattern,
            file_glob,
            max_results,
            context_lines,
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
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Number of lines before and after each match to include (default: 0, max: 5)"
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
        let context_lines = invocation
            .arguments
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .min(5) as usize;

        // Try ripgrep first, fall back on any error
        let search_results = if path::find_rg_binary().is_some() {
            match self
                .search_with_rg(pattern, path, file_glob, max_results, context_lines)
                .await
            {
                Ok(results) => results,
                Err(_) => {
                    self.search_with_fallback(pattern, path, file_glob, max_results, context_lines)
                        .await?
                }
            }
        } else {
            self.search_with_fallback(pattern, path, file_glob, max_results, context_lines)
                .await?
        };

        let text = format::format_search_results(&search_results);

        Ok(ToolOutput {
            text,
            truncated: search_results.truncated,
            exit_code: None,
            images: vec![],
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
