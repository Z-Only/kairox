use super::hunk::{apply_hunk, apply_hunk_validate};
use super::path::normalize_path;
use super::PatchApplyTool;
use crate::patch::parse::{Hunk, PatchLine};
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolInvocation};
use crate::shell::PATCH_TOOL_ID;
use crate::ToolError;

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
        arguments: serde_json::json!({ "patch": patch }),
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

#[tokio::test]
async fn patch_apply_tool_relocates_hunk_after_line_drift() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("drift.txt");
    tokio::fs::write(&file_path, "intro\nline1\nline2\nline3\n")
        .await
        .unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    let diff = "\
--- a/drift.txt
+++ b/drift.txt
@@ -1,3 +1,3 @@
 line1
-line2
+line2-new
 line3
";
    let result = tool.invoke(make_invocation(diff)).await.unwrap();
    assert!(result.text.contains("Applied patch to 1 file(s)"));

    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, "intro\nline1\nline2-new\nline3\n");
}

#[tokio::test]
async fn patch_apply_tool_rejects_ambiguous_drift_context() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("ambiguous.txt");
    let original = "prefix\nline1\nline2\nline3\nmiddle\nline1\nline2\nline3\n";
    tokio::fs::write(&file_path, original).await.unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    let diff = "\
--- a/ambiguous.txt
+++ b/ambiguous.txt
@@ -20,3 +20,3 @@
 line1
-line2
+line2-new
 line3
";
    let result = tool.invoke(make_invocation(diff)).await;
    match result.unwrap_err() {
        ToolError::AmbiguousPatchContext { candidates, .. } => {
            assert_eq!(candidates, vec![2, 6]);
        }
        other => panic!("expected AmbiguousPatchContext, got {:?}", other),
    }

    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn patch_apply_tool_rejects_overlapping_same_file_edits() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("overlap.txt");
    let original = "line1\nline2\nline3\n";
    tokio::fs::write(&file_path, original).await.unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    let diff = "\
--- a/overlap.txt
+++ b/overlap.txt
@@ -1,3 +1,3 @@
 line1
-line2
+line2-a
 line3
--- a/overlap.txt
+++ b/overlap.txt
@@ -1,3 +1,3 @@
 line1
-line2
+line2-b
 line3
";
    let result = tool.invoke(make_invocation(diff)).await;
    assert!(result.is_err());

    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn patch_apply_tool_rejects_add_only_hunk_past_eof() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("short.txt");
    let original = "line1\n";
    tokio::fs::write(&file_path, original).await.unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    let diff = "\
--- a/short.txt
+++ b/short.txt
@@ -100,0 +100,1 @@
+line2
";
    let result = tool.invoke(make_invocation(diff)).await;
    assert!(result.is_err());

    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, original);
}

#[tokio::test]
async fn patch_apply_tool_rolls_back_when_later_file_write_fails() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("first.txt");
    let original = "line1\nline2\nline3\n";
    tokio::fs::write(&file_path, original).await.unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    let diff = "\
--- a/first.txt
+++ b/first.txt
@@ -1,3 +1,3 @@
 line1
-line2
+line2-updated
 line3
--- /dev/null
+++ b/bad\0file.txt
@@ -0,0 +1,1 @@
+new
";
    let result = tool.invoke(make_invocation(diff)).await;
    assert!(result.is_err());

    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, original);
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
