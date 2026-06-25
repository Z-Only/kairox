use super::*;

#[test]
fn parse_single_file_single_hunk() {
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
    let patches = parse_unified_diff(diff).unwrap();
    assert_eq!(patches.len(), 1);
    let fp = &patches[0];
    assert_eq!(fp.old_path, PathBuf::from("src/main.rs"));
    assert_eq!(fp.new_path, PathBuf::from("src/main.rs"));
    assert!(!fp.is_new_file);
    assert!(!fp.is_delete);
    assert_eq!(fp.hunks.len(), 1);
    let h = &fp.hunks[0];
    assert_eq!(h.old_start, 1);
    assert_eq!(h.old_count, 3);
    assert_eq!(h.new_start, 1);
    assert_eq!(h.new_count, 4);
    // 5 lines: context + remove + add + add + context
    assert_eq!(h.lines.len(), 5);
    assert!(matches!(&h.lines[0], PatchLine::Context(s) if s == "fn main() {"));
    assert!(matches!(&h.lines[1], PatchLine::Remove(s) if s == "    println!(\"hello\");"));
    assert!(matches!(&h.lines[2], PatchLine::Add(s) if s == "    println!(\"hello, world\");"));
    assert!(matches!(&h.lines[3], PatchLine::Add(s) if s == "    println!(\"new line\");"));
    assert!(matches!(&h.lines[4], PatchLine::Context(s) if s == "}"));
}

#[test]
fn parse_new_file() {
    let diff = "\
--- /dev/null
+++ b/new.rs
@@ -0,0 +1,2 @@
+fn main() {
+}
";
    let patches = parse_unified_diff(diff).unwrap();
    assert_eq!(patches.len(), 1);
    let fp = &patches[0];
    assert!(fp.is_new_file);
    assert!(!fp.is_delete);
    assert_eq!(fp.old_path, PathBuf::new());
    assert_eq!(fp.new_path, PathBuf::from("new.rs"));
    assert_eq!(fp.hunks.len(), 1);
    assert_eq!(fp.hunks[0].lines.len(), 2);
}

#[test]
fn parse_delete_file() {
    let diff = "\
--- a/old.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-fn main() {
-}
";
    let patches = parse_unified_diff(diff).unwrap();
    assert_eq!(patches.len(), 1);
    let fp = &patches[0];
    assert!(!fp.is_new_file);
    assert!(fp.is_delete);
    assert_eq!(fp.old_path, PathBuf::from("old.rs"));
    assert_eq!(fp.new_path, PathBuf::new());
}

#[test]
fn parse_multi_file() {
    let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,1 +1,2 @@
-old
+new
+extra
--- a/bar.rs
+++ b/bar.rs
@@ -1,1 +1,1 @@
-old
+new
";
    let patches = parse_unified_diff(diff).unwrap();
    assert_eq!(patches.len(), 2);
    assert_eq!(patches[0].old_path, PathBuf::from("foo.rs"));
    assert_eq!(patches[0].hunks[0].lines.len(), 3);
    assert_eq!(patches[1].old_path, PathBuf::from("bar.rs"));
    assert_eq!(patches[1].hunks[0].lines.len(), 2);
}

#[test]
fn parse_malformed_header_returns_error() {
    let diff = "this is not a diff at all\n";
    let result = parse_unified_diff(diff);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, PatchParseError::InvalidHeader(_)));
}

#[test]
fn malformed_hunk_header_reports_line_number_expected_format_and_context() {
    let diff = "\
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -10,2 +oops @@
 context after malformed header
-nearby removal
+too far addition
 trailing line outside context
";

    let err = parse_unified_diff(diff).unwrap_err().to_string();

    assert!(err.contains("line 3"), "{err}");
    assert!(err.contains("@@ -10,2 +oops @@"), "{err}");
    assert!(
        err.contains("expected format: @@ -old_start,old_count +new_start,new_count @@"),
        "{err}"
    );
    assert!(err.contains("nearby patch context:"), "{err}");
    assert!(err.contains("1 | --- a/src/lib.rs"), "{err}");
    assert!(err.contains("2 | +++ b/src/lib.rs"), "{err}");
    assert!(err.contains("3 | @@ -10,2 +oops @@"), "{err}");
    assert!(err.contains("4 |  context after malformed header"), "{err}");
    assert!(err.contains("5 | -nearby removal"), "{err}");
    assert!(!err.contains("too far addition"), "{err}");
    assert!(!err.contains("trailing line outside context"), "{err}");
}

#[test]
fn parse_strip_a_and_b_prefixes() {
    let diff = "\
--- a/src/deep/file.rs
+++ b/src/deep/file.rs
@@ -10,3 +10,3 @@
 context
-removed
+added
 context
";
    let patches = parse_unified_diff(diff).unwrap();
    let fp = &patches[0];
    assert_eq!(fp.old_path, PathBuf::from("src/deep/file.rs"));
    assert_eq!(fp.new_path, PathBuf::from("src/deep/file.rs"));
}

#[test]
fn parse_line_types_correct() {
    let diff = "\
--- a/test.txt
+++ b/test.txt
@@ -1,4 +1,4 @@
 unchanged context
-removed line
+added line
 more context
 final context
";
    let patches = parse_unified_diff(diff).unwrap();
    assert_eq!(patches.len(), 1);
    let h = &patches[0].hunks[0];
    assert_eq!(h.lines.len(), 5);
    assert!(matches!(&h.lines[0], PatchLine::Context(s) if s == "unchanged context"));
    assert!(matches!(&h.lines[1], PatchLine::Remove(s) if s == "removed line"));
    assert!(matches!(&h.lines[2], PatchLine::Add(s) if s == "added line"));
    assert!(matches!(&h.lines[3], PatchLine::Context(s) if s == "more context"));
    assert!(matches!(&h.lines[4], PatchLine::Context(s) if s == "final context"));
}

#[test]
fn roundtrip_parse_and_inspect() {
    let diff = "\
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -5,3 +5,4 @@
 use std::io;
-
+use std::fs;
+use std::path;
@@ -10,2 +11,3 @@
 fn main() {
-    old_function();
+    new_function();
+    println!(\"test\");
";
    let patches = parse_unified_diff(diff).unwrap();
    assert_eq!(patches.len(), 1);

    let fp = &patches[0];
    assert_eq!(fp.old_path, PathBuf::from("src/lib.rs"));
    assert_eq!(fp.new_path, PathBuf::from("src/lib.rs"));
    assert!(!fp.is_new_file);
    assert!(!fp.is_delete);

    assert_eq!(fp.hunks.len(), 2);

    // First hunk
    let h0 = &fp.hunks[0];
    assert_eq!(h0.old_start, 5);
    assert_eq!(h0.old_count, 3);
    assert_eq!(h0.new_start, 5);
    assert_eq!(h0.new_count, 4);

    // Second hunk
    let h1 = &fp.hunks[1];
    assert_eq!(h1.old_start, 10);
    assert_eq!(h1.old_count, 2);
    assert_eq!(h1.new_start, 11);
    assert_eq!(h1.new_count, 3);
}
