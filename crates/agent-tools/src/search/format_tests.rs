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
