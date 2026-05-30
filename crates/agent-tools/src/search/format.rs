use super::{SearchEngine, SearchResult, SearchResults};

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

    let lines: Vec<String> = results.results.iter().flat_map(format_result).collect();

    format!("{}{}", header, lines.join("\n"))
}

fn format_result(result: &SearchResult) -> Vec<String> {
    let mut lines = Vec::new();
    for context in &result.context_before {
        lines.push(format!(
            "{}-{}-{}",
            result.file_path, context.line_number, context.line_content
        ));
    }
    lines.push(format!(
        "{}:{}:{}",
        result.file_path, result.line_number, result.line_content
    ));
    for context in &result.context_after {
        lines.push(format!(
            "{}-{}-{}",
            result.file_path, context.line_number, context.line_content
        ));
    }
    lines
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
