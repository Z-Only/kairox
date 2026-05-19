use std::path::Path;

use regex::Regex;

use crate::Result;

use super::{glob_matches, SearchEngine, SearchResult, SearchResults};

/// Recursive async directory walk + grep.
#[allow(clippy::too_many_arguments)]
pub async fn walk_and_grep(
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
    if current_depth > max_depth || *files_visited >= max_files || results.len() >= max_results {
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
            Box::pin(walk_and_grep(
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

/// Run fallback search using pure Rust regex + directory walk.
pub async fn run_fallback(
    search_dir: &Path,
    workspace_root: &Path,
    pattern: &str,
    file_glob: Option<&str>,
    max_results: usize,
) -> Result<SearchResults> {
    let re = Regex::new(pattern)
        .map_err(|e| crate::ToolError::ExecutionFailed(format!("invalid regex pattern: {}", e)))?;

    let mut results = Vec::new();
    let mut files_visited = 0usize;
    const MAX_FILES: usize = 500;
    const MAX_DEPTH: usize = 10;

    walk_and_grep(
        search_dir,
        workspace_root,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;

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

        let results = run_fallback(dir.path(), dir.path(), "hello", None, 100)
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

        let results = run_fallback(dir.path(), dir.path(), "find", Some("*.rs"), 100)
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

        let results = run_fallback(dir.path(), dir.path(), "match", None, 1)
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

        let results = run_fallback(dir.path(), dir.path(), "findme", None, 100)
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

        let results = run_fallback(dir.path(), dir.path(), "findme", None, 100)
            .await
            .unwrap();

        assert_eq!(results.results.len(), 1);
        assert!(results.results[0].file_path.ends_with("text.txt"));
    }

    #[tokio::test]
    async fn fallback_respects_max_depth() {
        let dir = tempfile::tempdir().unwrap();
        // Create deep nested dirs
        let deep = dir.path().join("a").join("b").join("c").join("d").join("e");
        std::fs::create_dir_all(&deep).unwrap();
        tokio::fs::write(deep.join("deep.txt"), "findme\n")
            .await
            .unwrap();
        // Also write file at shallow level (should be found)
        tokio::fs::write(dir.path().join("shallow.txt"), "findme\n")
            .await
            .unwrap();

        let results = run_fallback(dir.path(), dir.path(), "findme", None, 100)
            .await
            .unwrap();

        // Both files should be found (depth 5 ≤ MAX_DEPTH 10)
        assert_eq!(results.results.len(), 2);
    }

    #[tokio::test]
    async fn fallback_limits_files_visited() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..10 {
            tokio::fs::write(dir.path().join(format!("f{}.txt", i)), "match\n")
                .await
                .unwrap();
        }

        let results = run_fallback(dir.path(), dir.path(), "match", None, 100)
            .await
            .unwrap();

        assert_eq!(results.results.len(), 10);
    }
}
