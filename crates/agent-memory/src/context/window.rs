use agent_core::ContextSource;

/// Find the index of the lowest-priority section that can be dropped.
///
/// Priority (highest first): System, Skill, ToolDefinitions,
/// ProjectInstruction, Memory, Git, KnowledgeBase, WorkspaceRetrieval,
/// History, ToolResult, SelectedFile, Image.
///
/// `System` and `Request` are never dropped; `Skill` is dropped only after
/// `ToolDefinitions`.
pub(super) fn find_lowest_priority_drop(
    sections: &[(ContextSource, String, u64)],
) -> Option<usize> {
    let drop_order = [
        ContextSource::Image,
        ContextSource::SelectedFile,
        ContextSource::ToolResult,
        ContextSource::History,
        ContextSource::WorkspaceRetrieval,
        ContextSource::KnowledgeBase,
        ContextSource::Git,
        ContextSource::Memory,
        ContextSource::ProjectInstruction,
        ContextSource::ToolDefinitions,
        ContextSource::Skill,
    ];
    for category in &drop_order {
        for (i, (src, _, _)) in sections.iter().enumerate() {
            if src == category {
                return Some(i);
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
