use super::super::{SearchContextLine, SearchEngine, SearchResult, SearchResults};
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
            context_before: vec![],
            context_after: vec![],
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
            context_before: vec![],
            context_after: vec![],
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
                context_before: vec![],
                context_after: vec![],
            },
            SearchResult {
                file_path: "b.rs".into(),
                line_number: 2,
                line_content: "match b".into(),
                match_start: 0,
                match_end: 1,
                context_before: vec![],
                context_after: vec![],
            },
            SearchResult {
                file_path: "a.rs".into(),
                line_number: 5,
                line_content: "match c".into(),
                match_start: 0,
                match_end: 1,
                context_before: vec![],
                context_after: vec![],
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

#[test]
fn format_search_results_with_context_lines() {
    let results = SearchResults {
        results: vec![SearchResult {
            file_path: "src/lib.rs".into(),
            line_number: 10,
            line_content: "needle".into(),
            match_start: 0,
            match_end: 6,
            context_before: vec![SearchContextLine {
                line_number: 9,
                line_content: "before".into(),
            }],
            context_after: vec![SearchContextLine {
                line_number: 11,
                line_content: "after".into(),
            }],
        }],
        total_matches: 1,
        truncated: false,
        engine: SearchEngine::Ripgrep,
    };

    let text = format_search_results(&results);
    assert!(text.contains("src/lib.rs-9-before"));
    assert!(text.contains("src/lib.rs:10:needle"));
    assert!(text.contains("src/lib.rs-11-after"));
}
