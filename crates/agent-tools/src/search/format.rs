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
#[path = "format_tests.rs"]
mod tests;
