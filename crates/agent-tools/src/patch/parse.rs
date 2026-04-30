use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePatch {
    pub old_path: PathBuf,
    pub new_path: PathBuf,
    pub hunks: Vec<Hunk>,
    pub is_new_file: bool,
    pub is_delete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<PatchLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchLine {
    Context(String),
    Remove(String),
    Add(String),
}

#[derive(Debug, thiserror::Error)]
pub enum PatchParseError {
    #[error("invalid diff header: {0}")]
    InvalidHeader(String),
    #[error("invalid hunk header: {0}")]
    InvalidHunkHeader(String),
    #[error("unexpected line: {0}")]
    UnexpectedLine(String),
    #[error("missing new file path")]
    MissingNewPath,
}

/// Parse a unified diff string into a list of file patches.
pub fn parse_unified_diff(patch: &str) -> Result<Vec<FilePatch>, PatchParseError> {
    let mut results: Vec<FilePatch> = Vec::new();

    let mut current: Option<FilePatch> = None;
    let mut current_hunk: Option<Hunk> = None;

    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("--- ") {
            // Save any in-progress hunk/file before starting a new one
            if let Some(fp) = current.take() {
                push_hunk_and_file(fp, current_hunk.take(), &mut results);
            }
            current_hunk = None;

            let path_str = rest.trim();
            let (old_path, is_new_file) = if path_str == "/dev/null" {
                (PathBuf::new(), true)
            } else {
                (strip_prefix(path_str), false)
            };

            current = Some(FilePatch {
                old_path,
                new_path: PathBuf::new(),
                hunks: Vec::new(),
                is_new_file,
                is_delete: false,
            });
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            let fp = current.as_mut().ok_or(PatchParseError::MissingNewPath)?;
            let path_str = rest.trim();
            if path_str == "/dev/null" {
                fp.is_delete = true;
                fp.new_path = PathBuf::new();
            } else {
                fp.new_path = strip_prefix(path_str);
            }
        } else if let Some(rest) = line.strip_prefix("@@") {
            // Parse hunk header: @@ -x,y +a,b @@
            let end = rest
                .find("@@")
                .ok_or_else(|| PatchParseError::InvalidHunkHeader(line.to_string()))?;
            let header = &rest[..end].trim();

            // Parse the two ranges: -x,y +a,b
            let parts: Vec<&str> = header.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(PatchParseError::InvalidHunkHeader(line.to_string()));
            }

            let (old_start, old_count) = parse_range(parts[0])?;
            let (new_start, new_count) = parse_range(parts[1])?;

            // Save previous hunk if any
            if let Some(h) = current_hunk.take() {
                if let Some(fp) = current.as_mut() {
                    fp.hunks.push(h);
                }
            }

            current_hunk = Some(Hunk {
                old_start,
                old_count,
                new_start,
                new_count,
                lines: Vec::new(),
            });
        } else if let Some(h) = current_hunk.as_mut() {
            // Parse diff lines within a hunk
            if let Some(content) = line.strip_prefix(' ') {
                h.lines.push(PatchLine::Context(content.to_string()));
            } else if let Some(content) = line.strip_prefix('-') {
                h.lines.push(PatchLine::Remove(content.to_string()));
            } else if let Some(content) = line.strip_prefix('+') {
                h.lines.push(PatchLine::Add(content.to_string()));
            }
            // Lines with no recognized prefix or `\ ` are skipped
        }
        // Lines outside hunks with no recognized prefix are skipped
    }

    // Flush the last file
    if let Some(fp) = current.take() {
        push_hunk_and_file(fp, current_hunk, &mut results);
    }

    if results.is_empty() {
        return Err(PatchParseError::InvalidHeader(
            "no valid diff headers found".to_string(),
        ));
    }

    Ok(results)
}

/// Save a completed hunk into its FilePatch, then push the FilePatch into results.
fn push_hunk_and_file(fp: FilePatch, hunk: Option<Hunk>, results: &mut Vec<FilePatch>) {
    let mut fp = fp;
    if let Some(h) = hunk {
        fp.hunks.push(h);
    }
    results.push(fp);
}

/// Strip the `a/` or `b/` prefix from a diff path.
fn strip_prefix(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")) {
        PathBuf::from(stripped)
    } else {
        PathBuf::from(path)
    }
}

/// Parse a range like `-1,3` or `+1` into `(start, count)`.
/// When count is omitted it defaults to 1.
fn parse_range(s: &str) -> Result<(usize, usize), PatchParseError> {
    let s = s.trim_start_matches('-').trim_start_matches('+');
    if let Some((start_str, count_str)) = s.split_once(',') {
        let start = start_str.parse::<usize>().map_err(|_| {
            PatchParseError::InvalidHunkHeader(format!("invalid range start: {start_str}"))
        })?;
        let count = count_str.parse::<usize>().map_err(|_| {
            PatchParseError::InvalidHunkHeader(format!("invalid range count: {count_str}"))
        })?;
        Ok((start, count))
    } else {
        let start = s
            .parse::<usize>()
            .map_err(|_| PatchParseError::InvalidHunkHeader(format!("invalid range: {s}")))?;
        Ok((start, 1))
    }
}

#[cfg(test)]
mod tests {
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
}
