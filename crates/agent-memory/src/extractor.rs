const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "can", "had", "her", "was", "one",
    "our", "out", "has", "this", "that", "from", "with", "have", "will", "been", "they", "what",
    "about", "which", "their", "would", "there", "its", "also", "just", "more", "some", "than",
    "into",
];

/// Extract meaningful keywords from text for storage and retrieval.
/// Splits on whitespace and punctuation, filters stop words and short tokens.
pub fn extract_keywords(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|s| s.len() > 2)
        .filter(|s| !STOP_WORDS.contains(s))
        .take(20)
        .map(String::from)
        .collect()
}

#[cfg(test)]
#[path = "extractor_tests.rs"]
mod tests;
