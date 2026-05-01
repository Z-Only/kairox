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
mod tests {
    use super::*;

    #[test]
    fn extracts_meaningful_keywords_and_filters_stop_words() {
        let keywords = extract_keywords("The project uses cargo nextest for testing");
        assert!(keywords.contains(&"project".to_string()));
        assert!(keywords.contains(&"cargo".to_string()));
        assert!(keywords.contains(&"nextest".to_string()));
        assert!(keywords.contains(&"testing".to_string()));
        assert!(!keywords.contains(&"the".to_string()));
        assert!(!keywords.contains(&"for".to_string()));
    }

    #[test]
    fn skips_short_tokens() {
        let keywords = extract_keywords("I am a go programmer");
        assert!(!keywords.iter().any(|k| k == "i" || k == "am" || k == "a"));
    }

    #[test]
    fn limits_to_20_keywords() {
        let long_text = (1..=50)
            .map(|i| format!("keyword{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let keywords = extract_keywords(&long_text);
        assert!(keywords.len() <= 20);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(extract_keywords("").is_empty());
    }
}
