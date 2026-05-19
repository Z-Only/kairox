use super::{SearchEngine, SearchResults};

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

#[cfg(test)]
mod tests {
    use super::super::{SearchEngine, SearchResult, SearchResults};
    use super::*;

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
        let text = format_search_results(&results);
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
        let text = format_search_results(&results);
        assert!(text.contains("(max, truncated)"));
        assert!(text.contains("[rg]"));
    }

    #[test]
    fn format_search_results_zero_results() {
        let results = SearchResults {
            results: vec![],
            total_matches: 0,
            truncated: false,
            engine: SearchEngine::Fallback,
        };
        let text = format_search_results(&results);
        assert!(text.contains("[fallback] Found 0 matches in 0 files"));
    }

    #[test]
    fn format_search_results_multiple_files() {
        let results = SearchResults {
            results: vec![
                SearchResult {
                    file_path: "a.rs".into(),
                    line_number: 1,
                    line_content: "match a".into(),
                    match_start: 0,
                    match_end: 1,
                },
                SearchResult {
                    file_path: "b.rs".into(),
                    line_number: 2,
                    line_content: "match b".into(),
                    match_start: 0,
                    match_end: 1,
                },
                SearchResult {
                    file_path: "a.rs".into(),
                    line_number: 5,
                    line_content: "match c".into(),
                    match_start: 0,
                    match_end: 1,
                },
            ],
            total_matches: 3,
            truncated: false,
            engine: SearchEngine::Ripgrep,
        };
        let text = format_search_results(&results);
        // 3 matches in 2 distinct files
        assert!(text.contains("Found 3 matches in 2 files"));
        assert!(text.contains("[rg]"));
    }
}
