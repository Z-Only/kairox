use crate::memory::MemoryScope;
use regex::Regex;
use std::sync::LazyLock;

static MEMORY_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<memory(?:\s+scope="([^"]*)")?(?:\s+key="([^"]*)")?\s*>([\s\S]*?)</memory>"#)
        .expect("memory tag regex must compile")
});

static MEMORY_STRIP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<memory(?:\s+scope="[^"]*")?(?:\s+key="[^"]*")?\s*>[\s\S]*?</memory>\n?"#)
        .expect("memory strip regex must compile")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMarker {
    pub scope: MemoryScope,
    pub key: Option<String>,
    pub content: String,
}

pub fn extract_memory_markers(text: &str) -> Vec<MemoryMarker> {
    MEMORY_TAG_RE
        .captures_iter(text)
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
    MEMORY_STRIP_RE.replace_all(text, "").trim_end().to_string()
}

#[cfg(test)]
#[path = "marker_tests.rs"]
mod tests;
