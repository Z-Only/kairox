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
        session_id: "ses_test".to_string(),
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

// ── Hunk drift search tests ──────────────────────────────────────

#[test]
fn locate_hunk_drift_finds_content_at_offset() {
    // File has 5 padding lines then the target content.
    // Hunk claims old_start=1 but content actually starts at line 6 (0-based offset 5).
    let mut lines: Vec<String> = (0..5).map(|i| format!("padding_{}", i)).collect();
    lines.push("target_a".to_string());
    lines.push("target_b".to_string());
    lines.push("target_c".to_string());

    let hunk = make_hunk(
        1, // declared at line 1 but really at offset 5
        vec![
            PatchLine::Context("target_a".to_string()),
            PatchLine::Remove("target_b".to_string()),
            PatchLine::Context("target_c".to_string()),
        ],
    );

    use super::hunk::locate_hunk;
    let offset = locate_hunk(&lines, &hunk).unwrap();
    assert_eq!(offset, 5);
}

#[test]
fn locate_hunk_drift_within_max_range() {
    // Content is 15 lines away from declared position (within MAX_HUNK_LINE_DRIFT=200).
    let mut lines: Vec<String> = (0..15).map(|i| format!("filler_{}", i)).collect();
    lines.push("alpha".to_string());
    lines.push("beta".to_string());

    let hunk = make_hunk(
        1, // declared at line 1
        vec![
            PatchLine::Context("alpha".to_string()),
            PatchLine::Remove("beta".to_string()),
        ],
    );

    use super::hunk::locate_hunk;
    let offset = locate_hunk(&lines, &hunk).unwrap();
    assert_eq!(offset, 15);
}

#[test]
fn locate_hunk_exact_match_preferred_over_drift() {
    // When content matches at declared position, drift search is skipped.
    let lines = vec![
        "line_a".to_string(),
        "line_b".to_string(),
        "line_c".to_string(),
    ];

    let hunk = make_hunk(
        1,
        vec![
            PatchLine::Context("line_a".to_string()),
            PatchLine::Remove("line_b".to_string()),
            PatchLine::Context("line_c".to_string()),
        ],
    );

    use super::hunk::locate_hunk;
    let offset = locate_hunk(&lines, &hunk).unwrap();
    assert_eq!(offset, 0); // exact match at declared position
}

// ── Multiple hunks offset accumulation ───────────────────────────

#[test]
fn multi_hunk_offset_accumulates_correctly() {
    use super::hunk::{apply_hunk_at, locate_hunk};
    // File: line1, line2, line3, line4, line5
    let mut lines: Vec<String> = (1..=5).map(|i| format!("line{}", i)).collect();

    // First hunk: remove line3 (between line2 and line4 context)
    let hunk1 = make_hunk(
        2,
        vec![
            PatchLine::Context("line2".to_string()),
            PatchLine::Remove("line3".to_string()),
            PatchLine::Context("line4".to_string()),
        ],
    );

    let offset1 = locate_hunk(&lines, &hunk1).unwrap();
    assert_eq!(offset1, 1);
    apply_hunk_at(&mut lines, &hunk1, offset1);
    // Now: line1, line2, line4, line5

    // Second hunk: replace line5
    let hunk2 = make_hunk(
        4, // declared at original line 4
        vec![
            PatchLine::Context("line4".to_string()),
            PatchLine::Remove("line5".to_string()),
            PatchLine::Add("LINE5".to_string()),
        ],
    );

    // After hunk1 removed a line, line4 is now at offset 2 (drift needed)
    let offset2 = locate_hunk(&lines, &hunk2).unwrap();
    assert_eq!(offset2, 2);
    apply_hunk_at(&mut lines, &hunk2, offset2);
    assert_eq!(lines, vec!["line1", "line2", "line4", "LINE5"]);
}

// ── Hunk at file boundary ────────────────────────────────────────

#[test]
fn locate_hunk_at_first_line() {
    let lines = vec!["first".to_string(), "second".to_string()];
    let hunk = make_hunk(
        1,
        vec![
            PatchLine::Remove("first".to_string()),
            PatchLine::Add("FIRST".to_string()),
            PatchLine::Context("second".to_string()),
        ],
    );

    use super::hunk::locate_hunk;
    let offset = locate_hunk(&lines, &hunk).unwrap();
    assert_eq!(offset, 0);
}

#[test]
fn locate_hunk_at_last_line() {
    let lines = vec![
        "first".to_string(),
        "second".to_string(),
        "last".to_string(),
    ];
    let hunk = make_hunk(
        3,
        vec![
            PatchLine::Context("last".to_string()),
            PatchLine::Add("appended".to_string()),
        ],
    );

    use super::hunk::locate_hunk;
    let offset = locate_hunk(&lines, &hunk).unwrap();
    assert_eq!(offset, 2);
}

#[test]
fn locate_hunk_empty_file_fails() {
    let lines: Vec<String> = vec![];
    let hunk = make_hunk(
        1,
        vec![
            PatchLine::Context("something".to_string()),
            PatchLine::Remove("else".to_string()),
        ],
    );

    use super::hunk::locate_hunk;
    let result = locate_hunk(&lines, &hunk);
    assert!(result.is_err());
}

// ── Context-only hunk ────────────────────────────────────────────

#[test]
fn context_only_hunk_validates_successfully() {
    let lines = vec!["aaa".to_string(), "bbb".to_string(), "ccc".to_string()];
    let hunk = make_hunk(
        1,
        vec![
            PatchLine::Context("aaa".to_string()),
            PatchLine::Context("bbb".to_string()),
            PatchLine::Context("ccc".to_string()),
        ],
    );
    assert!(apply_hunk_validate(&lines, &hunk).is_ok());
}

#[test]
fn context_only_hunk_apply_leaves_file_unchanged() {
    let mut lines = vec!["aaa".to_string(), "bbb".to_string(), "ccc".to_string()];
    let hunk = make_hunk(
        1,
        vec![
            PatchLine::Context("aaa".to_string()),
            PatchLine::Context("bbb".to_string()),
            PatchLine::Context("ccc".to_string()),
        ],
    );
    apply_hunk(&mut lines, &hunk);
    assert_eq!(lines, vec!["aaa", "bbb", "ccc"]);
}

// ── Path normalization tests (extended) ──────────────────────────

#[test]
fn normalize_path_strips_single_dot() {
    let p = std::path::Path::new("src/./main.rs");
    let normalized = normalize_path(p);
    assert_eq!(normalized, std::path::PathBuf::from("src/main.rs"));
}

#[test]
fn normalize_path_resolves_parent_and_dot_components() {
    // b/./src/../src/main.rs → b/src/main.rs (the .. cancels the first src)
    let p = std::path::Path::new("b/./src/../src/main.rs");
    let normalized = normalize_path(p);
    assert_eq!(normalized, std::path::PathBuf::from("b/src/main.rs"));
}

#[test]
fn normalize_path_multiple_parent_dirs() {
    let p = std::path::Path::new("a/b/c/../../d/e");
    let normalized = normalize_path(p);
    assert_eq!(normalized, std::path::PathBuf::from("a/d/e"));
}

#[test]
fn normalize_path_preserves_simple_path() {
    let p = std::path::Path::new("src/lib.rs");
    let normalized = normalize_path(p);
    assert_eq!(normalized, std::path::PathBuf::from("src/lib.rs"));
}

#[test]
fn normalize_path_handles_paths_with_spaces() {
    let p = std::path::Path::new("src/my module/file name.rs");
    let normalized = normalize_path(p);
    assert_eq!(
        normalized,
        std::path::PathBuf::from("src/my module/file name.rs")
    );
}

#[test]
fn normalize_path_handles_deeply_nested_dots() {
    // a/b/c/./d/../e → a/b/c/e
    let p = std::path::Path::new("a/b/c/./d/../e");
    let normalized = normalize_path(p);
    assert_eq!(normalized, std::path::PathBuf::from("a/b/c/e"));
}

// ── Integration: /dev/null paths (new/delete) ────────────────────

#[tokio::test]
async fn patch_apply_tool_deletes_file_with_dev_null() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("to_delete.rs");
    tokio::fs::write(&file_path, "fn old() {}\n").await.unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    let diff = "\
--- a/to_delete.rs
+++ /dev/null
@@ -1,1 +0,0 @@
-fn old() {}
";
    let result = tool.invoke(make_invocation(diff)).await.unwrap();
    assert!(result.text.contains("Applied patch to 1 file(s)"));
    assert!(!file_path.exists());
}

#[tokio::test]
async fn patch_apply_tool_creates_file_in_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::create_dir_all(dir.path().join("nested/dir"))
        .await
        .unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    let diff = "\
--- /dev/null
+++ b/nested/dir/new.rs
@@ -0,0 +1,3 @@
+fn new_func() {
+    // created
+}
";
    let result = tool.invoke(make_invocation(diff)).await.unwrap();
    assert!(result.text.contains("Applied patch to 1 file(s)"));

    let new_file = dir.path().join("nested/dir/new.rs");
    assert!(new_file.exists());
    let content = tokio::fs::read_to_string(&new_file).await.unwrap();
    assert!(content.contains("fn new_func()"));
    assert!(content.contains("// created"));
}

// ── Integration: hunk drift with larger offset ───────────────────

#[tokio::test]
async fn patch_apply_tool_drift_at_line_10_content_at_15() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("drift_big.txt");

    // Build file: 14 filler lines then target content at lines 15-17
    let mut content = String::new();
    for i in 0..14 {
        content.push_str(&format!("filler_{}\n", i));
    }
    content.push_str("target_x\n");
    content.push_str("target_y\n");
    content.push_str("target_z\n");

    tokio::fs::write(&file_path, &content).await.unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    // Declare hunk at line 10 but actual content is at line 15
    let diff = "\
--- a/drift_big.txt
+++ b/drift_big.txt
@@ -10,3 +10,3 @@
 target_x
-target_y
+TARGET_Y
 target_z
";
    let result = tool.invoke(make_invocation(diff)).await.unwrap();
    assert!(result.text.contains("Applied patch to 1 file(s)"));

    let final_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert!(final_content.contains("TARGET_Y"));
    assert!(!final_content.contains("target_y"));
}

// ── Integration: path with spaces ────────────────────────────────

#[tokio::test]
async fn patch_apply_tool_handles_path_with_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let space_dir = dir.path().join("my project");
    tokio::fs::create_dir_all(&space_dir).await.unwrap();
    let file_path = space_dir.join("file name.rs");
    tokio::fs::write(&file_path, "old_content\n").await.unwrap();

    let tool = PatchApplyTool::new(dir.path().to_path_buf());
    let diff = "\
--- a/my project/file name.rs
+++ b/my project/file name.rs
@@ -1,1 +1,1 @@
-old_content
+new_content
";
    let result = tool.invoke(make_invocation(diff)).await.unwrap();
    assert!(result.text.contains("Applied patch to 1 file(s)"));

    let final_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(final_content, "new_content\n");
}
