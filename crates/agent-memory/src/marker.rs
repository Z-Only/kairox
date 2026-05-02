use crate::memory::MemoryScope;
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMarker {
    pub scope: MemoryScope,
    pub key: Option<String>,
    pub content: String,
}

pub fn extract_memory_markers(text: &str) -> Vec<MemoryMarker> {
    let re =
        Regex::new(r#"<memory(?:\s+scope="([^"]*)")?(?:\s+key="([^"]*)")?\s*>([\s\S]*?)</memory>"#)
            .unwrap();

    re.captures_iter(text)
        .map(|cap| MemoryMarker {
            scope: match cap.get(1).map(|m| m.as_str()) {
                Some("user") => MemoryScope::User,
                Some("workspace") => MemoryScope::Workspace,
                _ => MemoryScope::Session,
            },
            key: cap.get(2).map(|m| m.as_str().to_string()),
            content: cap
                .get(3)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default(),
        })
        .filter(|m| !m.content.is_empty())
        .collect()
}

pub fn strip_memory_markers(text: &str) -> String {
    let re =
        Regex::new(r#"<memory(?:\s+scope="[^"]*")?(?:\s+key="[^"]*")?\s*>[\s\S]*?</memory>\n?"#)
            .unwrap();
    re.replace_all(text, "").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_single_marker_with_scope_and_key() {
        let text = r#"Some response <memory scope="workspace" key="test-runner">Use cargo nextest</memory> more text"#;
        let markers = extract_memory_markers(text);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].scope, MemoryScope::Workspace);
        assert_eq!(markers[0].key, Some("test-runner".to_string()));
        assert_eq!(markers[0].content, "Use cargo nextest");
    }

    #[test]
    fn extracts_marker_defaulting_to_session_scope() {
        let text = r#"<memory>Session note</memory>"#;
        let markers = extract_memory_markers(text);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].scope, MemoryScope::Session);
        assert_eq!(markers[0].key, None);
        assert_eq!(markers[0].content, "Session note");
    }

    #[test]
    fn extracts_multiple_markers() {
        let text = r#"<memory scope="user">User fact</memory><memory scope="workspace" key="build">Build info</memory>"#;
        let markers = extract_memory_markers(text);
        assert_eq!(markers.len(), 2);
    }

    #[test]
    fn skips_empty_markers() {
        let text = r#"<memory></memory><memory>   </memory><memory>Valid</memory>"#;
        let markers = extract_memory_markers(text);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].content, "Valid");
    }

    #[test]
    fn strip_removes_all_markers() {
        let text = r#"Hello <memory scope="workspace">Save this</memory> World"#;
        let stripped = strip_memory_markers(text);
        assert_eq!(stripped, "Hello  World");
        assert!(!stripped.contains("<memory"));
    }

    #[test]
    fn strip_multiline_marker() {
        let text = "Result:\n<memory scope=\"session\">\nMultiple\nlines\n</memory>\nDone";
        let stripped = strip_memory_markers(text);
        assert_eq!(stripped, "Result:\nDone");
    }

    #[test]
    fn no_markers_returns_empty_and_strip_is_noop() {
        let text = "No markers here";
        assert!(extract_memory_markers(text).is_empty());
        assert_eq!(strip_memory_markers(text), text);
    }
}
