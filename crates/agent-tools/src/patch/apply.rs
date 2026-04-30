use crate::patch::parse::{parse_unified_diff, FilePatch, Hunk, PatchLine};
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use crate::shell::PATCH_TOOL_ID;
use crate::ToolError;
use async_trait::async_trait;
use std::path::PathBuf;

// ── Hunk application helpers ──────────────────────────────────────────────

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

// ── Path normalization helper ─────────────────────────────────────────────

/// Normalize a path by resolving `.` and `..` components without requiring
/// the path to exist on disk. Returns the lexically normalized path.
fn normalize_path(path: &std::path::Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if let Some(std::path::Component::Normal(_)) = components.last() {
                    components.pop();
                } else {
                    components.push(comp);
                }
            }
            _ => components.push(comp),
        }
    }
    components.iter().collect()
}

// ── PatchApplyTool ────────────────────────────────────────────────────────

pub struct PatchApplyTool {
    workspace_root: PathBuf,
}

impl PatchApplyTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Resolve a relative path within the workspace.
    /// For existing files, canonicalize and check it stays within root.
    /// For new files (where the file doesn't exist yet), canonicalize the
    /// parent directory and check that stays within root.
    fn resolve_workspace_path(&self, relative_path: &str) -> Result<PathBuf, ToolError> {
        // Canonicalize root once
        let root = self.workspace_root.canonicalize().map_err(|e| {
            ToolError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("workspace root not found: {}", e),
            ))
        })?;

        // Build candidate path using the canonicalized root to avoid
        // symlink mismatch issues (e.g., /var vs /private/var on macOS)
        let candidate = root.join(relative_path);

        // Lexical escape check: normalize and verify it stays within root
        let normalized = normalize_path(&candidate);
        if !normalized.starts_with(&root) {
            return Err(ToolError::WorkspaceEscape(relative_path.into()));
        }

        if candidate.exists() {
            // Existing file: full canonicalize check
            let canon = candidate.canonicalize()?;
            if !canon.starts_with(&root) {
                return Err(ToolError::WorkspaceEscape(relative_path.into()));
            }
            Ok(canon)
        } else {
            // New file: canonicalize parent directory
            if let Some(parent) = candidate.parent() {
                if parent.as_os_str().is_empty() || parent == root {
                    // File is at the workspace root
                    return Ok(root.join(
                        candidate
                            .file_name()
                            .ok_or_else(|| ToolError::WorkspaceEscape(relative_path.into()))?,
                    ));
                }

                if parent.exists() {
                    let canon_parent = parent.canonicalize()?;
                    if !canon_parent.starts_with(&root) {
                        return Err(ToolError::WorkspaceEscape(relative_path.into()));
                    }
                    Ok(canon_parent.join(
                        candidate
                            .file_name()
                            .ok_or_else(|| ToolError::WorkspaceEscape(relative_path.into()))?,
                    ))
                } else {
                    Err(ToolError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("parent directory not found: {}", parent.display()),
                    )))
                }
            } else {
                Err(ToolError::WorkspaceEscape(relative_path.into()))
            }
        }
    }
}

#[async_trait]
impl Tool for PatchApplyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: PATCH_TOOL_ID.to_string(),
            description: "Apply a unified diff patch to workspace files".to_string(),
            required_capability: "patch.apply".to_string(),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        let patch_text = invocation
            .arguments
            .get("patch")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match parse_unified_diff(patch_text) {
            Ok(file_patches) => {
                let has_new_or_delete =
                    file_patches.iter().any(|fp| fp.is_new_file || fp.is_delete);
                if has_new_or_delete {
                    ToolRisk::destructive(PATCH_TOOL_ID)
                } else {
                    ToolRisk::write(PATCH_TOOL_ID)
                }
            }
            Err(_) => {
                // If we can't parse, assume write (least surprising)
                ToolRisk::write(PATCH_TOOL_ID)
            }
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let patch_text = invocation
            .arguments
            .get("patch")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Parse the diff
        let file_patches = parse_unified_diff(patch_text)
            .map_err(|e| ToolError::PatchParseFailed(e.to_string()))?;

        if file_patches.is_empty() {
            return Err(ToolError::PatchParseFailed(
                "no file patches found in diff".to_string(),
            ));
        }

        // Resolve paths
        struct ResolvedPatch {
            file_patch: FilePatch,
            path: PathBuf,
        }
        let mut resolved: Vec<ResolvedPatch> = Vec::new();

        for fp in &file_patches {
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

            let path = self.resolve_workspace_path(relative_path)?;
            resolved.push(ResolvedPatch {
                file_patch: fp.clone(),
                path,
            });
        }

        // ── Phase 1: Validate ──────────────────────────────────────────
        for rp in &resolved {
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

        // ── Phase 2: Apply ─────────────────────────────────────────────
        let mut affected_files: Vec<String> = Vec::new();

        for rp in &resolved {
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

        Ok(ToolOutput {
            text: format!(
                "Applied patch to {} file(s): {}",
                affected_files.len(),
                affected_files.join(", ")
            ),
            truncated: false,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::parse::{Hunk, PatchLine};
    use crate::registry::{Tool, ToolInvocation};

    fn make_hunk(old_start: usize, lines: Vec<PatchLine>) -> Hunk {
        let old_count = lines
            .iter()
            .filter(|l| matches!(l, PatchLine::Context(_) | PatchLine::Remove(_)))
            .count();
        let new_count = lines
            .iter()
            .filter(|l| matches!(l, PatchLine::Context(_) | PatchLine::Add(_)))
            .count();
        Hunk {
            old_start,
            old_count,
            new_start: old_start,
            new_count,
            lines,
        }
    }

    fn make_invocation(patch: &str) -> ToolInvocation {
        ToolInvocation {
            tool_id: PATCH_TOOL_ID.to_string(),
            arguments: serde_json::json!({"patch": patch}),
            workspace_id: "test".to_string(),
            preview: format!("patch apply: {} bytes", patch.len()),
            timeout_ms: 5000,
            output_limit_bytes: 10240,
        }
    }

    // ── apply_hunk_validate tests ─────────────────────────────────────

    #[test]
    fn validate_matching_context_succeeds() {
        let lines = vec![
            "fn main() {".to_string(),
            "    println!(\"hello\");".to_string(),
            "}".to_string(),
        ];
        let hunk = make_hunk(
            1,
            vec![
                PatchLine::Context("fn main() {".to_string()),
                PatchLine::Remove("    println!(\"hello\");".to_string()),
                PatchLine::Context("}".to_string()),
            ],
        );
        assert!(apply_hunk_validate(&lines, &hunk).is_ok());
    }

    #[test]
    fn validate_mismatched_context_fails() {
        let lines = vec![
            "fn main() {".to_string(),
            "    println!(\"world\");".to_string(),
            "}".to_string(),
        ];
        let hunk = make_hunk(
            1,
            vec![
                PatchLine::Context("fn main() {".to_string()),
                PatchLine::Remove("    println!(\"hello\");".to_string()),
                PatchLine::Context("}".to_string()),
            ],
        );
        let result = apply_hunk_validate(&lines, &hunk);
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ContextMismatch {
                line,
                expected,
                actual,
            } => {
                assert_eq!(line, 2);
                assert_eq!(expected, "    println!(\"hello\");");
                assert_eq!(actual, "    println!(\"world\");");
            }
            other => panic!("expected ContextMismatch, got {:?}", other),
        }
    }

    // ── apply_hunk tests ──────────────────────────────────────────────

    #[test]
    fn apply_hunk_replaces_lines() {
        let mut lines = vec![
            "fn main() {".to_string(),
            "    println!(\"hello\");".to_string(),
            "}".to_string(),
        ];
        let hunk = make_hunk(
            1,
            vec![
                PatchLine::Context("fn main() {".to_string()),
                PatchLine::Remove("    println!(\"hello\");".to_string()),
                PatchLine::Add("    println!(\"hello, world\");".to_string()),
                PatchLine::Add("    println!(\"new line\");".to_string()),
                PatchLine::Context("}".to_string()),
            ],
        );
        apply_hunk(&mut lines, &hunk);
        assert_eq!(
            lines,
            vec![
                "fn main() {",
                "    println!(\"hello, world\");",
                "    println!(\"new line\");",
                "}",
            ]
        );
    }

    #[test]
    fn apply_hunk_add_only() {
        let mut lines = vec!["line1".to_string(), "line3".to_string()];
        let hunk = make_hunk(
            1,
            vec![
                PatchLine::Context("line1".to_string()),
                PatchLine::Add("line2".to_string()),
                PatchLine::Context("line3".to_string()),
            ],
        );
        apply_hunk(&mut lines, &hunk);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn apply_hunk_remove_only() {
        let mut lines = vec![
            "line1".to_string(),
            "line2".to_string(),
            "line3".to_string(),
        ];
        let hunk = make_hunk(
            1,
            vec![
                PatchLine::Context("line1".to_string()),
                PatchLine::Remove("line2".to_string()),
                PatchLine::Context("line3".to_string()),
            ],
        );
        apply_hunk(&mut lines, &hunk);
        assert_eq!(lines, vec!["line1", "line3"]);
    }

    // ── PatchApplyTool integration tests ───────────────────────────────

    #[tokio::test]
    async fn patch_apply_tool_applies_single_hunk() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("src/main.rs");
        tokio::fs::create_dir_all(dir.path().join("src"))
            .await
            .unwrap();
        tokio::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n")
            .await
            .unwrap();

        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let diff = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
-    println!(\"hello\");
+    println!(\"hello, world\");
+    println!(\"new line\");
 }
";
        let result = tool.invoke(make_invocation(diff)).await.unwrap();
        assert!(result.text.contains("Applied patch to 1 file(s)"));
        assert!(result.text.contains("src/main.rs"));

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("hello, world"));
        assert!(content.contains("new line"));
        assert!(!content.contains("println!(\"hello\");"));
    }

    #[tokio::test]
    async fn patch_apply_tool_rejects_workspace_escape() {
        let dir = tempfile::tempdir().unwrap();
        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let diff = "\
--- a/../../etc/passwd
+++ b/../../etc/passwd
@@ -1,1 +1,1 @@
-root
+root2
";
        let result = tool.invoke(make_invocation(diff)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::WorkspaceEscape(path) => {
                assert!(
                    path.contains("../../etc/passwd"),
                    "expected escape path in error, got: {}",
                    path
                );
            }
            other => panic!("expected WorkspaceEscape, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn patch_apply_tool_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let diff = "\
--- /dev/null
+++ b/new_file.rs
@@ -0,0 +1,2 @@
+fn main() {
+}
";
        let result = tool.invoke(make_invocation(diff)).await.unwrap();
        assert!(result.text.contains("Applied patch to 1 file(s)"));

        let new_file = dir.path().join("new_file.rs");
        assert!(new_file.exists());
        let content = tokio::fs::read_to_string(&new_file).await.unwrap();
        assert_eq!(content, "fn main() {\n}");
    }

    #[tokio::test]
    async fn patch_apply_tool_context_mismatch_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "line1\nline2\nline3\n")
            .await
            .unwrap();

        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let diff = "\
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,3 @@
 line1
-wrong_line
+line2_new
 line3
";
        let result = tool.invoke(make_invocation(diff)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ContextMismatch { .. } => {}
            other => panic!("expected ContextMismatch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn patch_apply_tool_multi_hunk_applied_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("multi.txt");
        tokio::fs::write(&file_path, "aaa\nbbb\nccc\nddd\neee\nfff\nggg\n")
            .await
            .unwrap();

        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let diff = "\
--- a/multi.txt
+++ b/multi.txt
@@ -1,3 +1,3 @@
 aaa
-bbb
+BBB
 ccc
@@ -5,3 +5,3 @@
 eee
-fff
+FFF
 ggg
";
        let result = tool.invoke(make_invocation(diff)).await.unwrap();
        assert!(result.text.contains("Applied patch to 1 file(s)"));

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("BBB"));
        assert!(!content.contains("bbb"));
        assert!(content.contains("FFF"));
        assert!(!content.contains("fff"));
        // Untouched lines should remain
        assert!(content.contains("aaa"));
        assert!(content.contains("ccc"));
        assert!(content.contains("ddd"));
        assert!(content.contains("eee"));
        assert!(content.contains("ggg"));
    }

    #[test]
    fn patch_apply_tool_risk_write_for_modify() {
        let dir = tempfile::tempdir().unwrap();
        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let inv = make_invocation("--- a/foo.rs\n+++ b/foo.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n");
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::write(PATCH_TOOL_ID));
    }

    #[test]
    fn patch_apply_tool_risk_destructive_for_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let inv = make_invocation("--- /dev/null\n+++ b/new.rs\n@@ -0,0 +1,1 @@\n+content\n");
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::destructive(PATCH_TOOL_ID));
    }

    #[test]
    fn patch_apply_tool_risk_destructive_for_delete() {
        let dir = tempfile::tempdir().unwrap();
        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let inv = make_invocation("--- a/old.rs\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-content\n");
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::destructive(PATCH_TOOL_ID));
    }

    #[test]
    fn definition_returns_correct_id() {
        let dir = tempfile::tempdir().unwrap();
        let tool = PatchApplyTool::new(dir.path().to_path_buf());
        let def = tool.definition();
        assert_eq!(def.tool_id, PATCH_TOOL_ID);
        assert_eq!(def.required_capability, "patch.apply");
    }

    // ── normalize_path tests ──────────────────────────────────────────

    #[test]
    fn normalize_path_removes_dot_dot() {
        let base = std::path::Path::new("/tmp/workspace");
        let candidate = base.join("../../etc/passwd");
        let normalized = normalize_path(&candidate);
        assert!(
            !normalized.starts_with("/tmp/workspace"),
            "normalized should escape: {:?}",
            normalized
        );
    }

    #[test]
    fn normalize_path_stays_within_root() {
        let base = std::path::Path::new("/tmp/workspace");
        let candidate = base.join("src/main.rs");
        let normalized = normalize_path(&candidate);
        assert!(normalized.starts_with("/tmp/workspace"));
    }
}
